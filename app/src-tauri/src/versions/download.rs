use std::{
  fs::{self, File},
  path::PathBuf,
  sync::Arc,
};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use tauri::Url;
use thiserror::Error;

use crate::{
  path,
  versions::{
    meta::{Action, Os, Rule},
    LIBRARY_DIR,
  },
};

use super::{
  meta::{
    java::{self, Component, Files},
    minecraft::{Assets, Version},
    Arch, OsName,
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
  let mut handles = Vec::new();
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

        handles.push(tauri::async_runtime::spawn(async move {
          download_java_file(&client, path, url, hash, executable).await
        }));
      }
    }
  }

  let mut res = Ok(());
  for handle in handles {
    if let Err(error) = handle.await? {
      res = Err(error);
    }
  }

  res
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
  let mut handles = Vec::new();

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
      handles.push(tauri::async_runtime::spawn(async move {
        download_file(&client, &path, url, &hash).await
      }));
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
      handles.push(tauri::async_runtime::spawn(async move {
        download_file(&client, &path, url, &hash).await
      }));
    }
  }

  let mut res = Ok(());
  for handle in handles {
    if let Err(error) = handle.await? {
      res = Err(error);
    }
  }

  res
}

pub async fn download_version_assets(
  client: Arc<Client>,
  data_dir: &PathBuf,
  assets: &Assets,
) -> Result<()> {
  let mut handles = Vec::new();
  for asset in assets.objects.values() {
    let prefix_hash = &asset.hash[0..2];
    let hash = asset.hash.clone();
    let path = path!(data_dir, MC_DIR, ASSETS_DIR, "objects", prefix_hash, &hash);
    let url = format!("{}/{}/{}", MC_RESOURCES_URL, prefix_hash, hash).parse()?;

    let client = client.clone();
    debug!("Checking asset file {}", path.display());
    handles.push(tauri::async_runtime::spawn(async move {
      download_file(&client, &path, url, &hash).await
    }));
  }

  let mut res = Ok(());
  for handle in handles {
    if let Err(error) = handle.await? {
      res = Err(error);
    }
  }

  res
}

fn check_rule(rule: &Rule) -> bool {
  #[cfg(target_os = "linux")]
  const OS_NAME: Option<OsName> = Some(OsName::Linux);
  #[cfg(target_os = "windows")]
  const OS_NAME: Option<OsName> = Some(OsName::Windows);
  #[cfg(target_os = "macos")]
  const OS_NAME: Option<OsName> = Some(OsName::Osx);
  #[cfg(target_arch = "x86")]
  const ARCH: Option<Arch> = Some(Arch::X86);
  #[cfg(not(target_arch = "x86"))]
  const ARCH: Option<Arch> = None;

  let Rule { action, os, .. } = rule;

  match (os, action) {
    (
      Some(Os {
        name: OS_NAME,
        arch: ARCH,
      }),
      Action::Allow,
    ) => true,
    (
      Some(Os {
        name: OS_NAME,
        arch: ARCH,
      }),
      Action::Disallow,
    ) => false,
    (None, Action::Allow) => true,
    (None, Action::Disallow) => false,
    (_, Action::Disallow) => true,
    (_, Action::Allow) => false,
  }
}
