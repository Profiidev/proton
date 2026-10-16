use std::time::Instant;

use anyhow::Result;
use reqwest::Client;
use tracing::debug;

use crate::{
  download::{check_pool, download_pool},
  event::{DownloadCheckStatus, DownloadsHandle},
  meta::minecraft::Assets,
  paths::MCPath,
};
use proton_utils::path;
use proton_utils::{download::download_file, file::file_hash};

pub const MC_RESOURCES_URL: &str = "https://resources.download.minecraft.net";

pub async fn check_download_version_assets(
  assets: &Assets,
  mc_path: &MCPath,
  client: &Client,
  handle: &DownloadsHandle,
  update_id: usize,
) -> Result<()> {
  debug!("Collecting checks for assets");
  let mut futures = Vec::new();

  for asset in assets.objects.values() {
    let prefix_hash = &asset.hash[0..2];
    let hash = asset.hash.clone();
    let path = path!(mc_path.assets_objects_path(), prefix_hash, &hash);
    let url = format!("{MC_RESOURCES_URL}/{prefix_hash}/{hash}").parse()?;
    let size = asset.size;

    let client = client.clone();
    //futures.push(async move { download_file(&client, &path, url, &hash).await });
    futures.push(async move {
      debug!("Checking asset file {}", path.display());
      if !file_hash(&hash, &path).await? {
        return Ok(Some((
          async move |cb| {
            debug!("Downloading asset file {}", path.display());
            download_file(&client, &path, url, &hash, cb).await?;
            anyhow::Ok(())
          },
          size,
        )));
      }
      anyhow::Ok(None)
    });
  }

  debug!("Got {} checks for assets", futures.len());
  let futures = check_pool(futures, handle, update_id, DownloadCheckStatus::AssetsCheck).await?;
  debug!("Completed all checks for assets");

  debug!("Downloading {} assets", futures.len());
  let now = Instant::now();
  download_pool(
    futures,
    handle.clone(),
    update_id,
    DownloadCheckStatus::AssetsDownload,
  )
  .await?;
  debug!("Completed all downloads for assets in {:?}", now.elapsed());

  Ok(())
}
