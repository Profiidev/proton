#[cfg(target_family = "unix")]
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use log::debug;
use reqwest::Client;
use tauri::AppHandle;
use tokio::fs;

use crate::{
  path,
  utils::{download::download_file, file::file_hash},
  versions::{
    download::{check_pool, download_pool},
    event::DownloadCheckStatus,
    meta::java::{self, Files},
    paths::JavaVersionPath,
  },
};

pub async fn check_download_java_files(
  files: &Files,
  client: &Client,
  java_path: &JavaVersionPath,
  handle: &AppHandle,
  update_id: usize,
) -> Result<()> {
  debug!("Collecting checks for java");
  let mut futures = Vec::new();

  for (path, file) in &files.files {
    let path = path!(java_path.base_path(), path);
    match file {
      java::File::Directory => fs::create_dir_all(path).await?,
      java::File::Link { .. } => {}
      java::File::File {
        downloads,
        #[cfg(target_family = "unix")]
        executable,
        ..
      } => {
        let download = &downloads.raw;
        let client = client.clone();
        let url = download.url.clone();
        let hash = download.sha1.clone();
        #[cfg(target_family = "unix")]
        let executable = *executable;
        let size = download.size;

        futures.push(async move {
          debug!("Checking java file {}", path.display());
          if !file_hash(&hash, &path).await? {
            return Ok(Some((
              async move |cb| {
                debug!("Downloading java file {}", path.display());
                download_file(&client, &path, url, &hash, cb).await?;

                #[cfg(target_family = "unix")]
                set_permissions(&path, executable).await?;
                anyhow::Ok(())
              },
              size,
            )));
          }

          #[cfg(target_family = "unix")]
          set_permissions(&path, executable).await?;

          Ok(None)
        });
      }
    }
  }

  debug!("Got {} checks for java", futures.len());
  let futures = check_pool(futures, handle, update_id, DownloadCheckStatus::JavaCheck).await?;
  debug!("Completed all checks for java in");

  debug!("Downloading {} java files", futures.len());
  let now = Instant::now();
  download_pool(
    futures,
    handle.clone(),
    update_id,
    DownloadCheckStatus::JavaDownload,
  )
  .await?;
  debug!("Completed all downloads for java in {:?}", now.elapsed());

  Ok(())
}

#[cfg(target_family = "unix")]
async fn set_permissions(path: &PathBuf, executable: bool) -> Result<()> {
  if executable {
    use std::os::unix::fs::PermissionsExt;

    let file = tokio::fs::File::open(path).await?;
    file
      .set_permissions(std::fs::Permissions::from_mode(0o755))
      .await?;
  }
  Ok(())
}
