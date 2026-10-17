use std::{
  future::Future,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use anyhow::Result;
use reqwest::Client;
use thiserror::Error;
use tracing::debug;

use crate::{
  download::{
    assets::check_download_version_assets,
    java::check_download_java_files,
    libraries::check_download_version_java_libraries,
    manifest::{check_assets_manifest, check_client, check_java_manifest, check_version_manifest},
  },
  event::{DownloadCheckStatus, DownloadsHandle},
  loader::LoaderVersion,
  meta::{java::PlatformVersion, minecraft::ManifestVersion},
  paths::{JavaVersionPath, MCPath, MCVersionPath},
};
use proton_utils::future::run_pool;

mod assets;
mod java;
mod libraries;
mod manifest;

#[derive(Error, Debug)]
pub enum DownloadError {
  #[error("Version not found")]
  NotFound,
  #[error("Not supported on this system")]
  NotSupported,
}

pub async fn check_download_version(
  mc: &ManifestVersion,
  java: &PlatformVersion,
  data_dir: &PathBuf,
  client: &Client,
  handle: &DownloadsHandle,
  update_id: usize,
  loader_version: Option<Box<dyn LoaderVersion>>,
) -> Result<()> {
  let mc_path = MCPath::new(data_dir);
  let version_path = MCVersionPath::new(data_dir, &mc.id);

  let version = check_version_manifest(mc, &version_path, client, handle, update_id).await?;
  let java_path = JavaVersionPath::new(data_dir, version.java_version.component, mc.id.clone());
  let assets = check_assets_manifest(&version, &mc_path, client, handle, update_id).await?;
  let files = check_java_manifest(&version, java, &java_path, client, handle, update_id).await?;

  check_client(&version, &version_path, client, handle, update_id).await?;

  check_download_version_assets(&assets, &mc_path, client, handle, update_id).await?;
  check_download_java_files(&files, client, &java_path, handle, update_id).await?;
  let libs = check_download_version_java_libraries(
    &version, client, &java_path, &mc_path, handle, update_id,
  )
  .await?;

  if let Some(loader) = loader_version {
    handle.status(update_id, DownloadCheckStatus::ModLoaderMeta);

    debug!("Collecting checks for mod loader files");
    let futures = loader
      .download(client, &version_path, &mc_path, &libs)
      .await?;
    debug!("Collected {} mod loader file checks", futures.len());

    let futures = check_pool(
      futures,
      handle,
      update_id,
      DownloadCheckStatus::ModLoaderFilesCheck,
    )
    .await?;
    debug!("Completed all checks for mod loader files");

    handle.status(update_id, DownloadCheckStatus::ModLoaderFilesDownloadInfo);
    let mut downloads = Vec::with_capacity(futures.len());
    for fut in futures {
      downloads.push(fut.await?);
    }

    download_pool(
      downloads,
      handle.clone(),
      update_id,
      DownloadCheckStatus::ModLoaderFilesDownload,
    )
    .await?;
    debug!("Completed all downloads for mod loader files");

    debug!("Running mod loader preprocess");
    handle.status(update_id, DownloadCheckStatus::ModLoaderPreprocess);

    loader
      .preprocess(&version_path, &mc_path, java_path.bin_path())
      .await?;

    handle.status(update_id, DownloadCheckStatus::ModLoaderPreprocessDone);
    debug!("Completed mod loader preprocess");
  }

  handle.status(update_id, DownloadCheckStatus::Done);

  Ok(())
}

async fn check_pool<S, F, O>(
  futures: Vec<F>,
  handle: &DownloadsHandle,
  update_id: usize,
  status: S,
) -> Result<Vec<O>>
where
  S: Fn(usize, usize) -> DownloadCheckStatus,
  F: Future<Output = Result<Option<O>>> + Send + 'static,
  O: Send + 'static,
{
  let res = run_pool(futures, |done, total| {
    handle.status(update_id, status(done, total))
  })
  .await;

  let mut futures = Vec::new();
  for result in res {
    if let Some(fut) = result?? {
      futures.push(fut);
    }
  }

  Ok(futures)
}

async fn download_pool<S, F, Fut>(
  funcs: Vec<(F, usize)>,
  handle: DownloadsHandle,
  update_id: usize,
  status: S,
) -> Result<()>
where
  S: Fn(usize, usize) -> DownloadCheckStatus + Clone + Send + 'static,
  F: FnOnce(Box<dyn Fn(usize) + Send + 'static>) -> Fut,
  Fut: Future<Output = Result<()>> + Send + 'static,
{
  let done = Arc::new(AtomicUsize::new(0));
  let total_size: usize = funcs.iter().map(|(_, size)| *size).sum();

  let cb = {
    let status = status.clone();
    let handle = handle.clone();

    Box::new(move |chunk| {
      done.fetch_add(chunk, Ordering::SeqCst);
      let done = done.load(Ordering::SeqCst);
      handle.status(update_id, status(done, total_size))
    })
  };

  let mut futures = Vec::with_capacity(funcs.len());
  for (func, _) in funcs {
    futures.push(func(cb.clone()));
  }

  handle.status(update_id, status(0, total_size));

  for result in run_pool(futures, |_, _| {}).await {
    result??;
  }

  Ok(())
}
