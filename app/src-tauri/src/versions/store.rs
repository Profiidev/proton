use std::{path::PathBuf, sync::Arc, time::Instant};

use anyhow::Result;
use log::{debug, info};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Url};
use tokio::join;

use crate::{
  path,
  versions::event::{CheckStatus, VERSION_CHECK_STATUS_EVENT},
};

use super::{
  download::{
    download_assets_manifest, download_client, download_java_files, download_version_assets,
    download_version_java_libraries, DownloadError,
  },
  meta::{
    java::{Component, Files, JavaVersions},
    minecraft::{Manifest, Version},
  },
  util::download_and_parse_file,
  JAVA_DIR, MC_DIR, VERSION_DIR,
};

const MC_VERSION_MANIFEST_URL: &str =
  "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const JAVA_VERSION_MANIFEST_URL: &str =
  "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

pub struct McVersionStore {
  mc_manifest: Manifest,
  java_manifest: JavaVersions,
}

#[derive(Serialize, Deserialize)]
struct IndexInfo {
  id: String,
  url: Url,
}

impl McVersionStore {
  pub async fn new(client: &Client) -> Result<McVersionStore> {
    let (mc_manifest, java_manifest) = join!(
      download_parse(client, MC_VERSION_MANIFEST_URL),
      download_parse(client, JAVA_VERSION_MANIFEST_URL)
    );

    Ok(McVersionStore {
      mc_manifest: mc_manifest?,
      java_manifest: java_manifest?,
    })
  }

  pub async fn check_or_download(
    &self,
    id: &str,
    client: Arc<Client>,
    handle: &AppHandle,
  ) -> Result<()> {
    let start = Instant::now();
    info!("Checking minecraft version {}", id);
    let data_dir = handle.path().app_data_dir()?;
    let version = self.get_version_manifest(id, &data_dir, &client).await?;
    let assets = download_assets_manifest(&data_dir, &client, &version).await?;
    let (java, java_component) = self.get_java_manifest(&data_dir, &client, &version).await?;
    handle.emit(VERSION_CHECK_STATUS_EVENT, CheckStatus::Manifest)?;

    download_client(&data_dir, &client, &version).await?;
    download_version_assets(client.clone(), &data_dir, &assets).await?;
    handle.emit(VERSION_CHECK_STATUS_EVENT, CheckStatus::Assets)?;

    download_java_files(client.clone(), &data_dir, &java, java_component).await?;
    download_version_java_libraries(client, &data_dir, &version).await?;
    handle.emit(VERSION_CHECK_STATUS_EVENT, CheckStatus::Java)?;
    info!(
      "Finished checking minecraft version {} in {:?}",
      id,
      start.elapsed()
    );

    Ok(())
  }

  async fn get_version_manifest(
    &self,
    id: &str,
    data_dir: &PathBuf,
    client: &Client,
  ) -> Result<Version> {
    let manifest_version = self
      .mc_manifest
      .versions
      .iter()
      .find(|v| v.id == id)
      .ok_or(DownloadError::NotFound)?;

    let path = path!(data_dir, MC_DIR, VERSION_DIR, id, format!("{}.json", id));
    debug!("Checking minecraft manifest for version {}", id);
    download_and_parse_file(
      client,
      &path,
      manifest_version.url.clone(),
      &manifest_version.sha1,
    )
    .await
  }

  async fn get_java_manifest(
    &self,
    data_dir: &PathBuf,
    client: &Client,
    version: &Version,
  ) -> Result<(Files, Component)> {
    let java_version = &version.java_version;
    #[cfg(target_os = "linux")]
    let version = &self.java_manifest.linux;
    #[cfg(target_os = "windows")]
    let version = &self.java_manifest.windows_x64;
    #[cfg(target_os = "macos")]
    let version = &self.java_manifest.mac_os;
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
    let path = path!(data_dir, JAVA_DIR, &id, format!("{}.json", &id));

    let download = &version.manifest;
    debug!("Checking java manifest for version {}", id);
    let files =
      download_and_parse_file(client, &path, download.url.clone(), &download.sha1).await?;

    Ok((files, *java_component))
  }
}

async fn download_parse<R: DeserializeOwned>(client: &Client, url: &str) -> Result<R> {
  Ok(client.get(url).send().await?.json().await?)
}
