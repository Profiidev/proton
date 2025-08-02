use std::{path::PathBuf, time::Instant};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use tauri::AppHandle;

use crate::{
  path,
  utils::{file::{download_file, file_hash}, future::FuturePool},
  versions::{
    event::{emit_download_check_status, DownloadCheckStatus},
    meta::minecraft::Assets,
    ASSETS_DIR, MC_DIR,
  },
};

pub const MC_RESOURCES_URL: &str = "https://resources.download.minecraft.net";

pub async fn check_download_version_assets(
  assets: &Assets,
  data_dir: &PathBuf,
  client: &Client,
  handle: &AppHandle,
  update_id: usize,
) -> Result<()> {
  debug!("Collecting checks for assets");
  let mut futures = Vec::new();

  for asset in assets.objects.values() {
    let prefix_hash = &asset.hash[0..2];
    let hash = asset.hash.clone();
    let path = path!(data_dir, MC_DIR, ASSETS_DIR, "objects", prefix_hash, &hash);
    let url = format!("{MC_RESOURCES_URL}/{prefix_hash}/{hash}").parse()?;

    let client = client.clone();
    //futures.push(async move { download_file(&client, &path, url, &hash).await });
    futures.push(async move {
      debug!("Checking asset file {}", path.display());
      if !file_hash(&hash, &path).await? {
        return Ok(Some(async move {
          debug!("Downloading asset file {}", path.display());
          download_file(&client, &path, url, &hash).await?;
          anyhow::Ok(())
        }));
      }
      anyhow::Ok(None)
    });
  }

  debug!("Got {} checks for assets", futures.len());
  let pool = FuturePool::new(futures);

  let res = pool
    .run(None, |done, total| {
      emit_download_check_status(
        handle,
        DownloadCheckStatus::AssetsCheck(done, total),
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
  debug!("Completed all checks for assets");

  debug!("Downloading {} assets", futures.len());
  let now = Instant::now();

  let pool = FuturePool::new(futures);
  let res = pool
    .run(None, |done, total| {
      emit_download_check_status(
        handle,
        DownloadCheckStatus::AssetsDownload(done, total),
        update_id,
      )
    })
    .await;
  for result in res {
    result??;
  }
  debug!("Completed all downloads for assets in {:?}", now.elapsed());

  Ok(())
}
