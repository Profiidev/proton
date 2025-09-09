use std::{
  future::Future,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use tauri::AppHandle;
use thiserror::Error;

use crate::{
  utils::future::FuturePool,
  versions::{
    download::{
      assets::check_download_version_assets,
      java::check_download_java_files,
      libraries::check_download_version_java_libraries,
      manifest::{
        check_assets_manifest, check_client, check_java_manifest, check_version_manifest,
      },
    },
    event::{DownloadCheckStatus, emit_download_check_status},
    loader::LoaderVersion,
    meta::{java::PlatformVersion, minecraft::ManifestVersion},
    paths::{JavaVersionPath, MCPath, MCVersionPath},
  },
};

mod assets;
mod java;
mod libraries;
mod manifest;

#[derive(Error, Debug)]
pub enum DownloadError {
  #[error("NotFound")]
  NotFound,
  #[error("NotSupported")]
  NotSupported,
}

pub async fn check_download_version(
  mc: &ManifestVersion,
  java: &PlatformVersion,
  data_dir: &PathBuf,
  client: &Client,
  handle: &AppHandle,
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
    emit_download_check_status(handle, DownloadCheckStatus::ModLoaderMeta, update_id);

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

    emit_download_check_status(
      handle,
      DownloadCheckStatus::ModLoaderFilesDownloadInfo,
      update_id,
    );
    let mut total = 0;
    let mut downloads = Vec::with_capacity(futures.len());
    for fut in futures {
      let (size, fut) = fut.await?;
      total += size;
      downloads.push(fut);
    }

    download_pool(
      downloads,
      handle.clone(),
      update_id,
      DownloadCheckStatus::ModLoaderFilesDownload,
      total,
    )
    .await?;
    debug!("Completed all downloads for mod loader files");

    debug!("Running mod loader preprocess");
    emit_download_check_status(handle, DownloadCheckStatus::ModLoaderPreprocess, update_id);

    loader
      .preprocess(&version_path, &mc_path, java_path.bin_path())
      .await?;

    emit_download_check_status(
      handle,
      DownloadCheckStatus::ModLoaderPreprocessDone,
      update_id,
    );
    debug!("Completed mod loader preprocess");
  }

  emit_download_check_status(handle, DownloadCheckStatus::Done, update_id);

  Ok(())
}

async fn check_pool<S, F, O>(
  futures: Vec<F>,
  handle: &AppHandle,
  update_id: usize,
  status: S,
) -> Result<Vec<O>>
where
  S: Fn(usize, usize) -> DownloadCheckStatus,
  F: Future<Output = Result<Option<O>>> + Send + 'static,
  O: Send + 'static,
{
  let pool = FuturePool::new(futures);

  let res = pool
    .run_cb(None, |done, total| {
      emit_download_check_status(handle, status(done, total), update_id)
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
  funcs: Vec<F>,
  handle: AppHandle,
  update_id: usize,
  status: S,
  total_size: usize,
) -> Result<()>
where
  S: Fn(usize, usize) -> DownloadCheckStatus + Clone + Send + 'static,
  F: FnOnce(Box<dyn Fn(usize) + Send + 'static>) -> Fut,
  Fut: Future<Output = Result<()>> + Send + 'static,
{
  emit_download_check_status(&handle, status(0, total_size), update_id);
  let done = Arc::new(AtomicUsize::new(0));

  let cb = {
    let status = status.clone();
    let handle = handle.clone();

    Box::new(move |chunk| {
      done.fetch_add(chunk, Ordering::SeqCst);
      let done = done.load(Ordering::SeqCst);
      emit_download_check_status(&handle, status(done, total_size), update_id)
    })
  };

  let mut futures = Vec::with_capacity(funcs.len());
  for func in funcs {
    futures.push(func(cb.clone()));
  }

  let pool = FuturePool::new(futures);

  let results = pool.run(None).await;
  for result in results {
    result??;
  }

  Ok(())
}
