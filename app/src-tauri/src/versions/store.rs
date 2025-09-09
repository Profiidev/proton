use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::Result;
use log::info;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Url};
use tokio::{
  join, select,
  sync::{Mutex, Notify},
};

use crate::{
  settings::SettingsExt,
  utils::{
    download::{download_and_parse_file_no_hash, download_and_parse_file_no_hash_force},
    file::file_hash,
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
  cancel_notify: Arc<Mutex<HashMap<usize, Arc<Notify>>>>,
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
      cancel_notify: Arc::new(Mutex::new(HashMap::new())),
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
  ) -> Result<bool> {
    let notify = Arc::new(Notify::new());
    let mut notifies = self.cancel_notify.lock().await;
    notifies.insert(id, notify.clone());
    drop(notifies);

    let start = Instant::now();
    info!("Checking/Downloading minecraft version {version} with download id {id}");
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

    let mut download_finished = false;
    select! {
      result = check_download_version(
        mc,
        java,
        &data_dir,
        &self.client,
        &self.handle,
        id,
        loader_version,
      ) => {
        result?;
        info!(
          "Finished checking/downloading minecraft version {version} with download id {id} in {:?}",
          start.elapsed()
        );
        download_finished = true;
      },
      _ = notify.notified() => {
        info!("Check/Download for minecraft version {version} with id {id} was canceled");
      }
    };

    let mut notifies = self.cancel_notify.lock().await;
    notifies.remove(&id);
    drop(notifies);

    Ok(download_finished)
  }

  pub async fn cancel_check_or_download(&self, id: usize) {
    let notifies = self.cancel_notify.lock().await;
    if let Some(notify) = notifies.get(&id) {
      notify.notify_waiters();
    }
    drop(notifies);
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
    let stable = !self.handle.app_settings()?.minecraft.show_snapshots;

    if let Some(loader) = loader.loader() {
      let mc_versions = self
        .mc_manifest
        .versions
        .iter()
        .map(|v| v.id.clone())
        .collect::<Vec<_>>();

      let data_dir = self.handle.path().app_data_dir().unwrap();
      let version_path = MCVersionPath::new(&data_dir, "");
      let mut supported_versions = loader.supported_versions(&version_path, stable).await?;

      // Sort the supported versions based on their position in the mc_versions list which is the order of release time
      supported_versions.sort_by_cached_key(|v| {
        mc_versions
          .iter()
          .position(|mc_v| mc_v == v)
          .unwrap_or(usize::MAX)
      });

      Ok(supported_versions)
    } else {
      Ok(
        self
          .mc_manifest
          .versions
          .iter()
          .filter(|v| v.r#type == VersionType::Release || !stable)
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
    let stable = !self.handle.app_settings()?.minecraft.show_snapshots;

    if let Some(loader) = loader.loader() {
      let data_dir = self.handle.path().app_data_dir().unwrap();
      let version_path = MCVersionPath::new(&data_dir, mc_version);
      loader
        .loader_versions_for_mc_version(mc_version, &version_path, stable)
        .await
    } else {
      Ok(vec![])
    }
  }

  pub fn handle(&self) -> &AppHandle {
    &self.handle
  }
}
