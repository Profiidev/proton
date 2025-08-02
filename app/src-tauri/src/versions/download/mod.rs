use std::path::PathBuf;

use anyhow::Result;
use reqwest::Client;
use tauri::AppHandle;
use thiserror::Error;

use crate::versions::{
  download::{
    assets::check_download_version_assets,
    java::check_download_java_files,
    libraries::check_download_version_java_libraries,
    manifest::{check_assets_manifest, check_client, check_java_manifest, check_version_manifest},
  },
  event::{emit_download_check_status, DownloadCheckStatus},
  meta::{java::PlatformVersion, minecraft::ManifestVersion},
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
) -> Result<()> {
  emit_download_check_status(handle, DownloadCheckStatus::VersionManifestCheck, update_id);
  let version_fut = check_version_manifest(mc, data_dir, client).await?;
  if !version_fut.is_data() {
    emit_download_check_status(
      handle,
      DownloadCheckStatus::VersionManifestDownload,
      update_id,
    );
  }
  let version = version_fut.resolve().await?;

  emit_download_check_status(handle, DownloadCheckStatus::AssetsManifestCheck, update_id);
  let assets_fut = check_assets_manifest(&version, data_dir, client).await?;
  if !assets_fut.is_data() {
    emit_download_check_status(
      handle,
      DownloadCheckStatus::AssetsManifestDownload,
      update_id,
    );
  }
  let assets = assets_fut.resolve().await?;

  emit_download_check_status(handle, DownloadCheckStatus::JavaManifestCheck, update_id);
  let java_fut = check_java_manifest(&version, java, data_dir, client).await?;
  if !java_fut.is_data() {
    emit_download_check_status(handle, DownloadCheckStatus::JavaManifestDownload, update_id);
  }
  let (files, java_component) = java_fut.resolve().await?;

  emit_download_check_status(handle, DownloadCheckStatus::ClientCheck, update_id);
  let client_fut = check_client(&version, data_dir, client).await?;
  if let Some(client_fut) = client_fut {
    emit_download_check_status(handle, DownloadCheckStatus::ClientDownload, update_id);
    client_fut.await?;
  }

  check_download_version_assets(&assets, data_dir, client, handle, update_id).await?;
  check_download_java_files(&files, java_component, client, data_dir, handle, update_id).await?;
  check_download_version_java_libraries(
    &version,
    java_component,
    client,
    data_dir,
    handle,
    update_id,
  )
  .await?;

  emit_download_check_status(handle, DownloadCheckStatus::Done, update_id);

  Ok(())
}
