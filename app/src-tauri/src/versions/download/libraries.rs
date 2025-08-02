use std::{future::Future, path::PathBuf, time::Instant};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use tauri::{async_runtime::spawn_blocking, AppHandle, Url};
use zip::ZipArchive;

use crate::{
  path,
  utils::{
    file::{create_or_open_file_std, download_file, file_hash},
    future::FuturePool,
  },
  versions::{
    check_rule,
    event::{emit_download_check_status, DownloadCheckStatus},
    meta::{java::Component, minecraft::Version},
    JAVA_DIR, LIBRARY_DIR, MC_DIR,
  },
};

pub async fn check_download_version_java_libraries(
  version: &Version,
  component: Component,
  client: &Client,
  data_dir: &PathBuf,
  handle: &AppHandle,
  update_id: usize,
) -> Result<()> {
  debug!("Collecting checks for java libraries");
  let mut futures_1 = Vec::new();
  let mut futures_2 = Vec::new();
  let component = component.to_string();

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

      let path = path!(
        data_dir,
        JAVA_DIR,
        component.clone(),
        LIBRARY_DIR,
        &library_download.path
      );
      let client = client.clone();
      let url = library_download.url.clone();
      let hash = library_download.sha1.clone();
      let data_dir = data_dir.clone();
      let component = component.clone();

      futures_1.push(async move {
        check_download_native_library(data_dir, component, client, path, url, hash).await
      });
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
      let client = client.clone();
      let url = library_download.url.clone();
      let hash = library_download.sha1.clone();
      futures_2.push(async move {
        debug!("Checking java library {}", path.display());
        if !file_hash(&hash, &path).await? {
          return Ok(Some(async move {
            debug!("Downloading java library {}", path.display());
            download_file(&client, &path, url, &hash).await?;
            anyhow::Ok(())
          }));
        }
        anyhow::Ok(None)
      });
    }
  }

  debug!(
    "Got {} checks for java libraries",
    futures_1.len() + futures_2.len()
  );
  let pool_1 = FuturePool::new(futures_1);
  let pool_2 = FuturePool::new(futures_2);

  let res = pool_1
    .run(None, |done, total| {
      emit_download_check_status(
        handle,
        DownloadCheckStatus::NativeLibraryCheck(done, total),
        update_id,
      )
    })
    .await;

  let mut futures = Vec::new();
  for result in res {
    if let Some(fut) = result?? {
      futures.push(fut);
    }
  }
  debug!("Completed all checks for native java libraries");

  debug!("Downloading {} native java libraries", futures.len());
  let now = Instant::now();
  let pool = FuturePool::new(futures);

  let res = pool
    .run(None, |done, total| {
      emit_download_check_status(
        handle,
        DownloadCheckStatus::NativeLibraryDownload(done, total),
        update_id,
      )
    })
    .await;

  for result in res {
    result??;
  }
  debug!(
    "Completed all downloads for native java libraries in {:?}",
    now.elapsed()
  );

  debug!("Checking java libraries");
  let res = pool_2
    .run(None, |done, total| {
      emit_download_check_status(
        handle,
        DownloadCheckStatus::LibraryCheck(done, total),
        update_id,
      )
    })
    .await;

  let mut futures = Vec::new();
  for result in res {
    if let Some(fut) = result?? {
      futures.push(fut);
    }
  }
  debug!("Completed all checks for java libraries");

  debug!("Downloading {} java libraries", futures.len());
  let now = Instant::now();
  let pool = FuturePool::new(futures);

  let res = pool
    .run(None, |done, total| {
      emit_download_check_status(
        handle,
        DownloadCheckStatus::LibraryDownload(done, total),
        update_id,
      )
    })
    .await;
  for result in res {
    result??;
  }
  debug!(
    "Completed all downloads for java libraries in {:?}",
    now.elapsed()
  );

  Ok(())
}

async fn check_download_native_library(
  data_dir: PathBuf,
  component: String,
  client: Client,
  path: PathBuf,
  url: Url,
  hash: String,
) -> Result<Option<impl Future<Output = Result<()>> + Send>> {
  debug!("Checking native java library {}", path.display());
  if !file_hash(&hash, &path).await? {
    return Ok(Some(async move {
      debug!("Downloading native java library {}", path.display());
      download_file(&client, &path, url, &hash).await?;

      unzip_native_library(data_dir, component, path).await?;
      anyhow::Ok(())
    }));
  }

  unzip_native_library(data_dir, component, path).await?;
  Ok(None)
}

async fn unzip_native_library(data_dir: PathBuf, component: String, path: PathBuf) -> Result<()> {
  spawn_blocking(move || {
    let file = std::fs::File::open(&path)?;
    let mut zip = ZipArchive::new(&file)?;
    for i in 0..zip.len() {
      let mut zip_file = zip.by_index(i)?;
      let name = zip_file.name();
      if !(name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib")) {
        continue;
      }
      let path = path!(&data_dir, JAVA_DIR, component.clone(), LIBRARY_DIR, name);
      debug!("Extracting file {}", path.display());
      let mut file = create_or_open_file_std(&path)?;
      std::io::copy(&mut zip_file, &mut file)?;
    }

    anyhow::Ok(())
  })
  .await??;

  Ok(())
}
