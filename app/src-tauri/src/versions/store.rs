use std::{path::PathBuf, sync::Arc, time::Instant};

use anyhow::Result;
use log::{debug, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Url};
use tokio::join;

use crate::{
  path,
  utils::{
    file::{
      download_and_parse_file, download_and_parse_file_no_hash,
      download_and_parse_file_no_hash_force, file_hash,
    },
    updater::{update_data, UpdateType},
  },
  versions::event::CheckStatus,
};

use super::{
  download::{
    download_assets_manifest, download_client, download_java_files, download_version_assets,
    download_version_java_libraries, DownloadError,
  },
  event::emit_check_status,
  meta::{
    java::{Component, Files, JavaVersions},
    minecraft::{Manifest, Version, VersionType},
  },
  JAVA_DIR, MC_DIR, VERSION_DIR,
};

const MC_VERSION_MANIFEST_URL: &str =
  "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const JAVA_VERSION_MANIFEST_URL: &str =
  "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

const MANIFEST_NAME: &str = "manifest.json";

pub struct McVersionStore {
  mc_manifest: Manifest,
  java_manifest: JavaVersions,
  handle: AppHandle,
  client: Arc<Client>,
}

#[derive(Serialize, Deserialize)]
struct IndexInfo {
  id: String,
  url: Url,
}

impl McVersionStore {
  pub async fn new(handle: AppHandle) -> Result<McVersionStore> {
    let client = Client::new();
    let data_dir = handle.path().app_data_dir()?;
    let mc_manifest_path = path!(&data_dir, MC_DIR, MANIFEST_NAME);
    let java_manifest_path = path!(&data_dir, JAVA_DIR, MANIFEST_NAME);

    let (mc_manifest, java_manifest) = join!(
      download_and_parse_file_no_hash(
        &client,
        &mc_manifest_path,
        MC_VERSION_MANIFEST_URL
          .parse()
          .expect("Failed to parse mc version url")
      ),
      download_and_parse_file_no_hash(
        &client,
        &java_manifest_path,
        JAVA_VERSION_MANIFEST_URL
          .parse()
          .expect("Failed to parse java version url")
      ),
    );

    Ok(McVersionStore {
      mc_manifest: mc_manifest?,
      java_manifest: java_manifest?,
      handle,
      client: Arc::new(client),
    })
  }

  pub async fn refresh_manifests(&mut self) -> Result<()> {
    let data_dir = self.handle.path().app_data_dir()?;
    let mc_manifest_path = path!(&data_dir, MC_DIR, MANIFEST_NAME);
    let java_manifest_path = path!(&data_dir, JAVA_DIR, MANIFEST_NAME);

    let (mc_manifest, java_manifest) = join!(
      download_and_parse_file_no_hash_force(
        &self.client,
        &mc_manifest_path,
        MC_VERSION_MANIFEST_URL
          .parse()
          .expect("Failed to parse mc version url")
      ),
      download_and_parse_file_no_hash_force(
        &self.client,
        &java_manifest_path,
        JAVA_VERSION_MANIFEST_URL
          .parse()
          .expect("Failed to parse java version url")
      )
    );
    let (mc_manifest, java_manifest) = (mc_manifest?, java_manifest?);

    let update = self.mc_manifest != mc_manifest || self.java_manifest != java_manifest;

    self.mc_manifest = mc_manifest;
    self.java_manifest = java_manifest;

    if update {
      update_data(&self.handle, UpdateType::Versions);
    }

    Ok(())
  }

  pub async fn check_or_download(&self, version: &str, id: usize) -> Result<()> {
    let start = Instant::now();
    info!("Checking minecraft version {}", version);
    let data_dir = self.handle.path().app_data_dir()?;
    let version = self.get_version_manifest(version, &data_dir).await?;
    emit_check_status(&self.handle, CheckStatus::Manifest(1), id);
    let assets = download_assets_manifest(&data_dir, &self.client, &version).await?;
    emit_check_status(&self.handle, CheckStatus::Manifest(2), id);
    let (java, java_component) = self.get_java_manifest(&data_dir, &version).await?;
    emit_check_status(&self.handle, CheckStatus::Manifest(3), id);

    download_client(&data_dir, &self.client, &version).await?;
    emit_check_status(&self.handle, CheckStatus::Client, id);
    download_version_assets(self.client.clone(), &data_dir, &assets, &self.handle, id).await?;
    download_java_files(
      self.client.clone(),
      &data_dir,
      &java,
      java_component,
      &self.handle,
      id,
    )
    .await?;
    download_version_java_libraries(self.client.clone(), &data_dir, &version, &self.handle, id)
      .await?;

    emit_check_status(&self.handle, CheckStatus::Done, id);
    info!(
      "Finished checking minecraft version {} in {:?}",
      id,
      start.elapsed()
    );

    Ok(())
  }

  pub fn check_meta(&self, version: &str, id: usize) -> Result<bool> {
    let data_dir = self.handle.path().app_data_dir()?;
    let manifest_version = self
      .mc_manifest
      .versions
      .iter()
      .find(|v| v.id == version)
      .ok_or(DownloadError::NotFound)?;
    let path = path!(
      &data_dir,
      MC_DIR,
      VERSION_DIR,
      version,
      format!("{}.json", version)
    );
    let ok = file_hash(&manifest_version.sha1, &path)?;
    if ok {
      emit_check_status(&self.handle, CheckStatus::Done, id);
    }

    Ok(ok)
  }

  async fn get_version_manifest(&self, id: &str, data_dir: &PathBuf) -> Result<Version> {
    let manifest_version = self
      .mc_manifest
      .versions
      .iter()
      .find(|v| v.id == id)
      .ok_or(DownloadError::NotFound)?;

    let path = path!(data_dir, MC_DIR, VERSION_DIR, id, format!("{}.json", id));
    debug!("Checking minecraft manifest for version {}", id);
    download_and_parse_file(
      &self.client,
      &path,
      manifest_version.url.clone(),
      &manifest_version.sha1,
    )
    .await
  }

  async fn get_java_manifest(
    &self,
    data_dir: &PathBuf,
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
      download_and_parse_file(&self.client, &path, download.url.clone(), &download.sha1).await?;

    Ok((files, *java_component))
  }

  pub fn list_versions(&self) -> Vec<String> {
    self
      .mc_manifest
      .versions
      .iter()
      .filter(|v| v.r#type == VersionType::Release)
      .map(|v| &v.id)
      .cloned()
      .collect()
  }
}
