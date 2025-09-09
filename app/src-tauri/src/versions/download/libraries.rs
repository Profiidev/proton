use std::{path::PathBuf, time::Instant};

use anyhow::Result;
use async_zip::tokio::read::fs::ZipFileReader;
use log::debug;
use reqwest::Client;
use tauri::AppHandle;
use tokio::io;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use crate::{
  path,
  utils::{
    download::download_file,
    file::{create_or_open_file, file_hash},
  },
  versions::{
    check_rule,
    download::{check_pool, download_pool},
    event::DownloadCheckStatus,
    meta::minecraft::Version,
    paths::{JavaVersionPath, MCPath},
  },
};

pub async fn check_download_version_java_libraries(
  version: &Version,
  client: &Client,
  java_path: &JavaVersionPath,
  mc_path: &MCPath,
  handle: &AppHandle,
  update_id: usize,
) -> Result<Vec<String>> {
  debug!("Collecting checks for java libraries");
  let mut futures_1 = Vec::new();
  let mut total_1 = 0;
  let mut futures_2 = Vec::new();
  let mut total_2 = 0;
  let mut libs = Vec::new();

  'l: for library in &version.libraries {
    let Some(downloads) = &library.downloads else {
      continue;
    };

    if let Some(classifier) = &downloads.classifiers {
      // add library before checks so it does not need to be checked again by the loader
      libs.push(library.name.clone());

      #[cfg(target_os = "linux")]
      let Some(library_download) = &classifier.natives_linux else {
        continue;
      };

      #[cfg(target_os = "windows")]
      let Some(library_download) = &classifier.natives_windows else {
        continue;
      };

      #[cfg(target_os = "macos")]
      let Some(library_download) = &classifier.natives_osx else {
        continue;
      };

      let path = path!(java_path.native_path(), &library_download.path);
      let java_lib_path = java_path.native_path();
      let client = client.clone();
      let url = library_download.url.clone();
      let hash = library_download.sha1.clone();
      total_1 += library_download.size;

      futures_1.push(async move {
        debug!("Checking native java library {}", path.display());
        if !file_hash(&hash, &path).await? {
          return Ok(Some(async move |cb| {
            debug!("Downloading native java library {}", path.display());
            download_file(&client, &path, url, &hash, cb).await?;

            unzip_native_library(java_lib_path, path).await?;
            anyhow::Ok(())
          }));
        }

        unzip_native_library(java_lib_path, path).await?;
        Ok(None)
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
      let path = path!(mc_path.library_path(), &library_download.path);
      let client = client.clone();
      let url = library_download.url.clone();
      let hash = library_download.sha1.clone();
      total_2 += library_download.size;

      futures_2.push(async move {
        debug!("Checking java library {}", path.display());
        if !file_hash(&hash, &path).await? {
          return Ok(Some(async move |cb| {
            debug!("Downloading java library {}", path.display());
            download_file(&client, &path, url, &hash, cb).await?;
            anyhow::Ok(())
          }));
        }
        anyhow::Ok(None)
      });
      libs.push(library.name.clone());
    }
  }

  debug!(
    "Got {} checks for java libraries",
    futures_1.len() + futures_2.len()
  );
  let futures = check_pool(
    futures_1,
    handle,
    update_id,
    DownloadCheckStatus::NativeLibraryCheck,
  )
  .await?;
  debug!("Completed all checks for native java libraries");

  debug!("Downloading {} native java libraries", futures.len());
  let now = Instant::now();
  download_pool(
    futures,
    handle.clone(),
    update_id,
    DownloadCheckStatus::NativeLibraryDownload,
    total_1,
  )
  .await?;
  debug!(
    "Completed all downloads for native java libraries in {:?}",
    now.elapsed()
  );

  debug!("Checking java libraries");
  let futures = check_pool(
    futures_2,
    handle,
    update_id,
    DownloadCheckStatus::LibraryCheck,
  )
  .await?;
  debug!("Completed all checks for java libraries");

  debug!("Downloading {} java libraries", futures.len());
  let now = Instant::now();
  download_pool(
    futures,
    handle.clone(),
    update_id,
    DownloadCheckStatus::LibraryDownload,
    total_2,
  )
  .await?;
  debug!(
    "Completed all downloads for java libraries in {:?}",
    now.elapsed()
  );

  Ok(libs)
}

async fn unzip_native_library(java_lib_path: PathBuf, path: PathBuf) -> Result<()> {
  let zip = ZipFileReader::new(path).await?;
  for i in 0..zip.file().entries().len() {
    let reader = zip.reader_with_entry(i).await?;
    let entry = reader.entry();

    let name = entry.filename().as_str().unwrap_or_default();
    if !(name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib")) {
      continue;
    }
    let path = path!(&java_lib_path, name);
    debug!("Extracting file {}", path.display());
    let mut file = create_or_open_file(&path).await?;
    io::copy(&mut reader.compat(), &mut file).await?;
  }

  Ok(())
}
