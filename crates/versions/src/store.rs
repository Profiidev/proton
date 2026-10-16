use std::{
  collections::HashMap,
  sync::{Arc, LazyLock},
  time::Instant,
};

use anyhow::Result;
use reqwest::Client;
use tokio::{join, select, sync::Notify};
use tracing::info;

use crate::{
  download::check_download_version,
  event::{DownloadCheckStatus, DownloadsHandle},
  loader::LoaderType,
  meta::java::Component,
  paths::{JavaVersionPath, MCPath, MCVersionPath},
};
use proton_utils::{
  dir::data_dir,
  download::default_client,
  download::{download_and_parse_file_no_hash, download_and_parse_file_no_hash_force},
  file::file_hash,
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
  mc_manifest: Arc<Manifest>,
  java_manifest: Arc<JavaVersions>,
  downloads: DownloadsHandle,
  client: Arc<Client>,
}

/// Cancel handles of running jobs (downloads, installs, updates) by job id
static CANCEL: LazyLock<std::sync::Mutex<HashMap<usize, Arc<Notify>>>> =
  LazyLock::new(Default::default);

/// Registered job, unregistered on drop so early returns cannot leak it
pub struct CancelGuard {
  id: usize,
  pub notify: Arc<Notify>,
}

impl Drop for CancelGuard {
  fn drop(&mut self) {
    CANCEL.lock().unwrap().remove(&self.id);
  }
}

/// Registers a job, `cancel(id)` wakes `notify`
pub fn register_cancel(id: usize) -> CancelGuard {
  let notify = Arc::new(Notify::new());
  CANCEL.lock().unwrap().insert(id, notify.clone());
  CancelGuard { id, notify }
}

/// Whether any job is running
pub fn is_busy() -> bool {
  !CANCEL.lock().unwrap().is_empty()
}

pub fn cancel(id: usize) {
  if let Some(notify) = CANCEL.lock().unwrap().get(&id) {
    // stores a permit, so a cancel sent before the job reaches its select! is not lost
    notify.notify_one();
  }
}

impl McVersionStore {
  /// Reads the cached manifests, downloads them when missing
  pub async fn new(downloads: DownloadsHandle) -> Result<McVersionStore> {
    let client = default_client();
    let data_dir = data_dir()?;
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

    Ok(Self::from_manifests(
      mc_manifest?,
      java_manifest?,
      downloads,
    ))
  }

  pub fn from_manifests(
    mc_manifest: Manifest,
    java_manifest: JavaVersions,
    downloads: DownloadsHandle,
  ) -> Self {
    McVersionStore {
      mc_manifest: Arc::new(mc_manifest),
      java_manifest: Arc::new(java_manifest),
      downloads,
      client: Arc::new(default_client()),
    }
  }

  /// Downloads the manifests even when cached
  pub async fn download_manifests(client: &Client) -> Result<(Manifest, JavaVersions)> {
    let data_dir = data_dir()?;
    let mc_manifest_path = MCPath::new(&data_dir).mc_manifest();
    let java_manifest_path =
      JavaVersionPath::new(&data_dir, Component::Unknown, String::new()).java_manifest();

    let (mc_manifest, java_manifest) = join!(
      download_and_parse_file_no_hash_force(
        client,
        &mc_manifest_path,
        MC_VERSION_MANIFEST_URL
          .parse()
          .expect("Failed to parse mc version url")
      ),
      download_and_parse_file_no_hash_force(
        client,
        &java_manifest_path,
        JAVA_VERSION_MANIFEST_URL
          .parse()
          .expect("Failed to parse java version url")
      )
    );
    Ok((mc_manifest?, java_manifest?))
  }

  /// Returns whether something changed
  pub fn update_manifests(&mut self, mc_manifest: Manifest, java_manifest: JavaVersions) -> bool {
    let update = *self.mc_manifest != mc_manifest || *self.java_manifest != java_manifest;
    self.mc_manifest = Arc::new(mc_manifest);
    self.java_manifest = Arc::new(java_manifest);
    update
  }

  pub async fn check_or_download(
    &self,
    version: &str,
    id: usize,
    loader: LoaderType,
    loader_version: Option<String>,
  ) -> Result<bool> {
    let guard = register_cancel(id);

    let start = Instant::now();
    info!("Checking/Downloading minecraft version {version} with download id {id}");
    let data_dir = data_dir()?;

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
        &self.downloads,
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
      _ = guard.notify.notified() => {
        info!("Check/Download for minecraft version {version} with id {id} was canceled");
      }
    };

    Ok(download_finished)
  }

  pub async fn check_meta(&self, version: &str, id: usize) -> Result<bool> {
    let data_dir = data_dir()?;
    let manifest_version = self
      .mc_manifest
      .versions
      .iter()
      .find(|v| v.id == version)
      .ok_or(DownloadError::NotFound)?;

    let path = MCVersionPath::new(&data_dir, &manifest_version.id).version_manifest();
    let ok = file_hash(&manifest_version.sha1, &path).await?;
    if ok {
      self.downloads.status(id, DownloadCheckStatus::Done);
    }

    Ok(ok)
  }

  /// `stable` hides snapshots (`!settings.minecraft.show_snapshots`)
  pub async fn list_versions(&self, loader: &LoaderType, stable: bool) -> Result<Vec<String>> {
    if let Some(loader) = loader.loader() {
      let mc_versions = self
        .mc_manifest
        .versions
        .iter()
        .map(|v| v.id.clone())
        .collect::<Vec<_>>();

      let data_dir = data_dir()?;
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
    stable: bool,
  ) -> Result<Vec<String>> {
    if let Some(loader) = loader.loader() {
      let data_dir = data_dir()?;
      let version_path = MCVersionPath::new(&data_dir, mc_version);
      loader
        .loader_versions_for_mc_version(mc_version, &version_path, stable)
        .await
    } else {
      Ok(vec![])
    }
  }

  pub fn downloads(&self) -> &DownloadsHandle {
    &self.downloads
  }
}
