use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use tauri::AppHandle;

use crate::{
  path,
  utils::{
    download::{download_and_parse_file, download_file},
    file::{file_hash, read_parse_file},
  },
  versions::{
    download::DownloadError,
    event::{DownloadCheckStatus, emit_download_check_status},
    meta::{
      java::{Component, Files, PlatformVersion},
      minecraft::{Assets, ManifestVersion, Version},
    },
    paths::{JavaVersionPath, MCPath, MCVersionPath},
  },
};

pub async fn check_version_manifest(
  info: &ManifestVersion,
  version_path: &MCVersionPath,
  client: &Client,
  handle: &AppHandle,
  update_id: usize,
) -> Result<Version> {
  emit_download_check_status(handle, DownloadCheckStatus::VersionManifestCheck, update_id);
  let path = version_path.version_manifest();

  debug!("Checking minecraft manifest for version {}", info.id);
  if !file_hash(&info.sha1, &path).await? {
    emit_download_check_status(
      handle,
      DownloadCheckStatus::VersionManifestDownload,
      update_id,
    );
    debug!("Downloading minecraft manifest for version {}", info.id);
    return download_and_parse_file(client, &path, info.url.clone(), &info.sha1).await;
  }

  read_parse_file(&path).await
}

pub async fn check_assets_manifest(
  info: &Version,
  mc_path: &MCPath,
  client: &Client,
  handle: &AppHandle,
  update_id: usize,
) -> Result<Assets> {
  emit_download_check_status(handle, DownloadCheckStatus::AssetsManifestCheck, update_id);
  let assets_index = &info.asset_index;
  let path = path!(
    mc_path.assets_index_path(),
    format!("{}.json", assets_index.id)
  );

  debug!("Checking assets manifest {}", assets_index.id);
  if !file_hash(&assets_index.sha1, &path).await? {
    emit_download_check_status(
      handle,
      DownloadCheckStatus::AssetsManifestDownload,
      update_id,
    );
    debug!("Downloading assets manifest {}", assets_index.id);
    return download_and_parse_file(client, &path, assets_index.url.clone(), &assets_index.sha1)
      .await;
  }

  read_parse_file(&path).await
}

pub async fn check_java_manifest(
  info: &Version,
  version: &PlatformVersion,
  java_path: &JavaVersionPath,
  client: &Client,
  handle: &AppHandle,
  update_id: usize,
) -> Result<Files> {
  emit_download_check_status(handle, DownloadCheckStatus::JavaManifestCheck, update_id);
  let java_version = &info.java_version;
  let java_component = &java_version.component;

  let list = match java_component {
    Component::JavaRuntimeAlpha => &version.java_runtime_alpha,
    Component::JavaRuntimeBeta => &version.java_runtime_beta,
    Component::JavaRuntimeDelta => &version.java_runtime_delta,
    Component::JavaRuntimeGamma => &version.java_runtime_gamma,
    Component::JavaRuntimeGammaSnapshot => &version.java_runtime_gamma_snapshot,
    Component::JreLegacy => &version.jre_legacy,
    _ => return Err(DownloadError::NotSupported.into()),
  };
  let Some(version) = list.first() else {
    return Err(DownloadError::NotSupported.into());
  };

  let id = java_component.to_string();
  let path = path!(java_path.base_path(), format!("{}.json", &id));

  let download = &version.manifest;
  debug!("Checking java manifest for {id}");
  if !file_hash(&download.sha1, &path).await? {
    emit_download_check_status(handle, DownloadCheckStatus::JavaManifestDownload, update_id);
    debug!("Downloading java manifest for {id}");
    return download_and_parse_file(client, &path, download.url.clone(), &download.sha1).await;
  }

  read_parse_file(&path).await
}

pub async fn check_client(
  version: &Version,
  version_path: &MCVersionPath,
  client: &Client,
  handle: &AppHandle,
  update_id: usize,
) -> Result<()> {
  emit_download_check_status(handle, DownloadCheckStatus::ClientCheck, update_id);
  let download = &version.downloads.client;
  let path = version_path.client_jar();

  debug!("Checking client jar for version {}", version.id);
  if !file_hash(&download.sha1, &path).await? {
    emit_download_check_status(
      handle,
      DownloadCheckStatus::ClientDownload(0, download.size),
      update_id,
    );
    let done = AtomicUsize::new(0);
    let total = download.size;
    let handle = handle.clone();

    debug!("Downloading client jar for version {}", version.id);
    download_file(
      client,
      &path,
      download.url.clone(),
      &download.sha1,
      Box::new(move |chunk| {
        done.fetch_add(chunk, Ordering::SeqCst);
        let done = done.load(Ordering::SeqCst);
        emit_download_check_status(
          &handle,
          DownloadCheckStatus::ClientDownload(done, total),
          update_id,
        );
      }),
    )
    .await?;
  }

  Ok(())
}
