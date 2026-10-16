use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use reqwest::Client;
use tracing::debug;

use crate::{
  download::DownloadError,
  event::{DownloadCheckStatus, DownloadsHandle},
  meta::{
    java::{Component, Files, PlatformVersion},
    minecraft::{Assets, ManifestVersion, Version},
  },
  paths::{JavaVersionPath, MCPath, MCVersionPath},
};
use proton_utils::path;
use proton_utils::{
  download::{download_and_parse_file, download_file},
  file::{file_hash, read_parse_file},
};

pub async fn check_version_manifest(
  info: &ManifestVersion,
  version_path: &MCVersionPath,
  client: &Client,
  handle: &DownloadsHandle,
  update_id: usize,
) -> Result<Version> {
  handle.status(update_id, DownloadCheckStatus::VersionManifestCheck);
  let path = version_path.version_manifest();

  debug!("Checking minecraft manifest for version {}", info.id);
  if !file_hash(&info.sha1, &path).await? {
    handle.status(update_id, DownloadCheckStatus::VersionManifestDownload);
    debug!("Downloading minecraft manifest for version {}", info.id);
    return download_and_parse_file(client, &path, info.url.clone(), &info.sha1).await;
  }

  read_parse_file(&path).await
}

pub async fn check_assets_manifest(
  info: &Version,
  mc_path: &MCPath,
  client: &Client,
  handle: &DownloadsHandle,
  update_id: usize,
) -> Result<Assets> {
  handle.status(update_id, DownloadCheckStatus::AssetsManifestCheck);
  let assets_index = &info.asset_index;
  let path = path!(
    mc_path.assets_index_path(),
    format!("{}.json", assets_index.id)
  );

  debug!("Checking assets manifest {}", assets_index.id);
  if !file_hash(&assets_index.sha1, &path).await? {
    handle.status(update_id, DownloadCheckStatus::AssetsManifestDownload);
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
  handle: &DownloadsHandle,
  update_id: usize,
) -> Result<Files> {
  handle.status(update_id, DownloadCheckStatus::JavaManifestCheck);
  let java_version = &info.java_version;
  let java_component = &java_version.component;

  let list = match java_component {
    Component::JavaRuntimeAlpha => &version.java_runtime_alpha,
    Component::JavaRuntimeBeta => &version.java_runtime_beta,
    Component::JavaRuntimeDelta => &version.java_runtime_delta,
    Component::JavaRuntimeEpsilon => &version.java_runtime_epsilon,
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
    handle.status(update_id, DownloadCheckStatus::JavaManifestDownload);
    debug!("Downloading java manifest for {id}");
    return download_and_parse_file(client, &path, download.url.clone(), &download.sha1).await;
  }

  read_parse_file(&path).await
}

pub async fn check_client(
  version: &Version,
  version_path: &MCVersionPath,
  client: &Client,
  handle: &DownloadsHandle,
  update_id: usize,
) -> Result<()> {
  handle.status(update_id, DownloadCheckStatus::ClientCheck);
  let download = &version.downloads.client;
  let path = version_path.client_jar();

  debug!("Checking client jar for version {}", version.id);
  if !file_hash(&download.sha1, &path).await? {
    handle.status(
      update_id,
      DownloadCheckStatus::ClientDownload(0, download.size),
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
        handle.status(update_id, DownloadCheckStatus::ClientDownload(done, total));
      }),
    )
    .await?;
  }

  Ok(())
}
