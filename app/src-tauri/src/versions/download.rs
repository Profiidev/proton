use std::{
  fs::{self, File},
  io,
  path::PathBuf,
  sync::Arc,
  time::Instant,
};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use tauri::Url;
use thiserror::Error;
use zip::ZipArchive;

use crate::{
  path,
  utils::future::FuturePool,
  versions::{util::create_or_open_file, LIBRARY_DIR, NATIVE_DIR},
};

use super::{
  check_rule,
  meta::{
    java::{self, Component, Files},
    minecraft::{Assets, Version},
  },
  util::{download_and_parse_file, download_file},
  ASSETS_DIR, ASSETS_INDEX_DIR, JAVA_DIR, MC_DIR, VERSION_DIR,
};

pub const MC_RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Error, Debug)]
pub enum DownloadError {
  #[error("NotFound")]
  NotFound,
  #[error("NotSupported")]
  NotSupported,
}

pub async fn download_client(data_dir: &PathBuf, client: &Client, version: &Version) -> Result<()> {
  let download = &version.downloads.client;

  let path = path!(
    data_dir,
    MC_DIR,
    VERSION_DIR,
    &version.id,
    format!("{}.jar", version.id)
  );

  debug!("Checking client jar for version {}", version.id);
  download_file(client, &path, download.url.clone(), &download.sha1).await?;

  Ok(())
}

pub async fn download_assets_manifest(
  data_dir: &PathBuf,
  client: &Client,
  version: &Version,
) -> Result<Assets> {
  let assets_index = &version.asset_index;
  let id = &assets_index.id;

  let path = path!(
    data_dir,
    MC_DIR,
    ASSETS_DIR,
    ASSETS_INDEX_DIR,
    format!("{}.json", id)
  );

  debug!("Checking assets manifest for id {}", version.id);
  download_and_parse_file(client, &path, assets_index.url.clone(), &assets_index.sha1).await
}

pub async fn download_java_files(
  client: Arc<Client>,
  data_dir: &PathBuf,
  files: &Files,
  component: Component,
) -> Result<()> {
  debug!("Collecting checks/downloads for java");
  let start = Instant::now();
  let mut futures = Vec::new();
  let id = component.to_string();

  for (path, file) in &files.files {
    let path = path!(data_dir, JAVA_DIR, &id, path);
    match file {
      java::File::Directory => fs::create_dir_all(path)?,
      java::File::Link { .. } => {}
      java::File::File {
        downloads,
        #[cfg(target_family = "unix")]
        executable,
        ..
      } => {
        let download = &downloads.raw;
        debug!("Checking java file {}", path.display());
        let client = client.clone();
        let url = download.url.clone();
        let hash = download.sha1.clone();
        #[cfg(target_family = "unix")]
        let executable = *executable;

        futures.push(async move {
          download_java_file(
            &client,
            path,
            url,
            hash,
            #[cfg(target_family = "unix")]
            executable,
          )
          .await
        });
      }
    }
  }

  debug!("Got {} checks/downloads for java", futures.len());
  let pool = FuturePool::new(futures);

  let res = pool.run(None).await;
  for result in res {
    result??;
  }
  debug!(
    "Completed all checks/downloads for java in {:?}",
    start.elapsed()
  );

  Ok(())
}

async fn download_java_file(
  client: &Client,
  path: PathBuf,
  url: Url,
  hash: String,
  #[cfg(target_family = "unix")] executable: bool,
) -> Result<()> {
  download_file(client, &path, url, &hash).await?;

  #[cfg(target_family = "unix")]
  if executable {
    use std::os::unix::fs::PermissionsExt;
    let file = File::open(&path)?;
    file.set_permissions(fs::Permissions::from_mode(0o755))?;
  }

  Ok(())
}

pub async fn download_version_java_libraries(
  client: Arc<Client>,
  data_dir: &PathBuf,
  version: &Version,
) -> Result<()> {
  debug!("Collecting checks/downloads for java libraries");
  let start = Instant::now();
  let mut futures_1 = Vec::new();
  let mut futures_2 = Vec::new();

  'l: for library in &version.libraries {
    let Some(downloads) = &library.downloads else {
      continue;
    };

    if let Some(classifier) = &downloads.classifiers {
      #[cfg(target_os = "linux")]
      let Some(library_download) = &classifier.natives_linux
      else {
        continue;
      };

      #[cfg(target_os = "windows")]
      let Some(library_download) = &classifier.natives_windows
      else {
        continue;
      };

      #[cfg(target_os = "macos")]
      let Some(library_download) = &classifier.natives_osx
      else {
        continue;
      };

      let path = path!(data_dir, MC_DIR, LIBRARY_DIR, &library_download.path);
      debug!("Checking native java library {}", path.display());
      let client = client.clone();
      let url = library_download.url.clone();
      let hash = library_download.sha1.clone();
      let data_dir = data_dir.clone();

      futures_1
        .push(async move { download_native_library(&data_dir, &client, path, url, hash).await });
    }

    if let Some(rules) = &library.rules {
      for rule in rules {
        if !check_rule(rule) {
          continue 'l;
        }
      }
    }

    if let Some(library_download) = &downloads.artifact {
      let path = path!(data_dir, MC_DIR, LIBRARY_DIR, &library_download.path);
      debug!("Checking java library {}", path.display());
      let client = client.clone();
      let url = library_download.url.clone();
      let hash = library_download.sha1.clone();
      futures_2.push(async move { download_file(&client, &path, url, &hash).await });
    }
  }

  debug!(
    "Got {} checks/downloads for java libraries",
    futures_1.len() + futures_2.len()
  );
  let pool_1 = FuturePool::new(futures_1);
  let pool_2 = FuturePool::new(futures_2);

  let res = pool_1.run(None).await;
  for result in res {
    result??;
  }
  let res = pool_2.run(None).await;
  for result in res {
    result??;
  }
  debug!(
    "Completed all checks/downloads for java libraries in {:?}",
    start.elapsed()
  );

  Ok(())
}

async fn download_native_library(
  data_dir: &PathBuf,
  client: &Client,
  path: PathBuf,
  url: Url,
  hash: String,
) -> Result<()> {
  download_file(client, &path, url, &hash).await?;

  let file = File::open(&path)?;
  let mut zip = ZipArchive::new(&file)?;
  for i in 0..zip.len() {
    let mut zip_file = zip.by_index(i)?;
    let name = zip_file.name();
    if !(name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib")) {
      continue;
    }
    let path = path!(data_dir, MC_DIR, NATIVE_DIR, name);
    debug!("Extracting file {}", path.display());
    let mut file = create_or_open_file(&path)?;
    io::copy(&mut zip_file, &mut file)?;
  }

  Ok(())
}

pub async fn download_version_assets(
  client: Arc<Client>,
  data_dir: &PathBuf,
  assets: &Assets,
) -> Result<()> {
  debug!("Collecting checks/downloads for assets");
  let start = Instant::now();
  let mut futures = Vec::new();

  for asset in assets.objects.values() {
    let prefix_hash = &asset.hash[0..2];
    let hash = asset.hash.clone();
    let path = path!(data_dir, MC_DIR, ASSETS_DIR, "objects", prefix_hash, &hash);
    let url = format!("{}/{}/{}", MC_RESOURCES_URL, prefix_hash, hash).parse()?;

    let client = client.clone();
    debug!("Checking asset file {}", path.display());
    futures.push(async move { download_file(&client, &path, url, &hash).await });
  }

  debug!("Got {} checks/downloads for assets", futures.len());
  let pool = FuturePool::new(futures);

  let res = pool.run(None).await;
  for result in res {
    result??;
  }
  debug!(
    "Completed all checks/downloads for assets in {:?}",
    start.elapsed()
  );

  Ok(())
}
