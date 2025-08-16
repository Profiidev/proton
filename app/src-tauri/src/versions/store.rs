use std::{sync::Arc, time::Instant};

use anyhow::Result;
use log::info;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Url};
use tokio::join;

use crate::{
  utils::{
    file::{download_and_parse_file_no_hash, download_and_parse_file_no_hash_force, file_hash},
    updater::{UpdateType, default_client, update_data},
  },
  versions::{
    download::check_download_version,
    event::{DownloadCheckStatus, emit_download_check_status},
    loader::LoaderType,
    meta::java::Component,
    paths::{JavaVersionPath, MCPath, MCVersionPath},
  },
};

use super::{
  download::DownloadError,
  meta::{
    java::JavaVersions,
    minecraft::{Manifest, VersionType},
  },
};

const MC_VERSION_MANIFEST_URL: &str =
  "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const JAVA_VERSION_MANIFEST_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Clone)]
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
    let client = default_client();
    let data_dir = handle.path().app_data_dir()?;
    let mc_manifest_path = MCPath::new(&data_dir).mc_manifest();
    let java_manifest_path =
      JavaVersionPath::new(&data_dir, Component::Unknown, String::new()).java_manifest();

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
    let mc_manifest_path = MCPath::new(&data_dir).mc_manifest();
    let java_manifest_path =
      JavaVersionPath::new(&data_dir, Component::Unknown, String::new()).java_manifest();

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

  pub async fn check_or_download(
    &self,
    version: &str,
    id: usize,
    loader: LoaderType,
    loader_version: Option<String>,
  ) -> Result<()> {
    let start = Instant::now();
    info!("Checking minecraft version {version}");
    let data_dir = self.handle.path().app_data_dir()?;

    let mc = self
      .mc_manifest
      .versions
      .iter()
      .find(|v| v.id == version)
      .ok_or(DownloadError::NotFound)?;

    #[cfg(target_os = "linux")]
    let java = &self.java_manifest.linux;
    #[cfg(target_os = "windows")]
    let java = &self.java_manifest.windows_x64;
    #[cfg(target_os = "macos")]
    let java = &self.java_manifest.mac_os;

    let loader_version = loader_version.and_then(|v| loader.loader_version(version.to_string(), v));

    check_download_version(
      mc,
      java,
      &data_dir,
      &self.client,
      &self.handle,
      id,
      loader_version,
    )
    .await?;

    info!(
      "Finished checking minecraft version {} in {:?}",
      id,
      start.elapsed()
    );

    Ok(())
  }

  pub async fn check_meta(&self, version: &str, id: usize) -> Result<bool> {
    let data_dir = self.handle.path().app_data_dir()?;
    let manifest_version = self
      .mc_manifest
      .versions
      .iter()
      .find(|v| v.id == version)
      .ok_or(DownloadError::NotFound)?;

    let path = MCVersionPath::new(&data_dir, &manifest_version.id).version_manifest();
    let ok = file_hash(&manifest_version.sha1, &path).await?;
    if ok {
      emit_download_check_status(&self.handle, DownloadCheckStatus::Done, id);
    }

    Ok(ok)
  }

  pub async fn list_versions(&self, loader: &LoaderType) -> Result<Vec<String>> {
    if let Some(loader) = loader.loader() {
      let data_dir = self.handle.path().app_data_dir().unwrap();
      let version_path = MCVersionPath::new(&data_dir, "");
      loader.supported_versions(&version_path, true).await
    } else {
      Ok(
        self
          .mc_manifest
          .versions
          .iter()
          .filter(|v| v.r#type == VersionType::Release)
          .map(|v| &v.id)
          .cloned()
          .collect(),
      )
    }
  }

  pub async fn list_loader_versions(
    &self,
    loader: &LoaderType,
    mc_version: &str,
  ) -> Result<Vec<String>> {
    if let Some(loader) = loader.loader() {
      let data_dir = self.handle.path().app_data_dir().unwrap();
      let version_path = MCVersionPath::new(&data_dir, mc_version);
      loader
        .loader_versions_for_mc_version(mc_version, &version_path, true)
        .await
    } else {
      Ok(vec![])
    }
  }

  pub fn handle(&self) -> &AppHandle {
    &self.handle
  }
}
