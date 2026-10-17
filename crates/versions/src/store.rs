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
  dir::data_dir, download::default_client, download::download_and_parse_file_no_hash,
  file::file_hash, http,
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
    let (mc, java) = Self::load_manifests(&default_client(), false).await?;
    Ok(Self::from_manifests(mc, java, downloads))
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

  /// The mc/java manifests, `force` downloads them even when cached
  pub async fn load_manifests(client: &Client, force: bool) -> Result<(Manifest, JavaVersions)> {
    let data_dir = data_dir()?;
    let mc_manifest_path = MCPath::new(&data_dir).mc_manifest();
    let java_manifest_path =
      JavaVersionPath::new(&data_dir, Component::Unknown, String::new()).java_manifest();

    let (mc_manifest, java_manifest) = join!(
      download_and_parse_file_no_hash(
        client,
        &mc_manifest_path,
        http::url(MC_VERSION_MANIFEST_URL)
          .parse()
          .expect("Failed to parse mc version url"),
        force
      ),
      download_and_parse_file_no_hash(
        client,
        &java_manifest_path,
        http::url(JAVA_VERSION_MANIFEST_URL)
          .parse()
          .expect("Failed to parse java version url"),
        force
      )
    );
    Ok((mc_manifest?, java_manifest?))
  }

  pub fn update_manifests(&mut self, mc_manifest: Manifest, java_manifest: JavaVersions) {
    self.mc_manifest = Arc::new(mc_manifest);
    self.java_manifest = Arc::new(java_manifest);
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
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use proton_utils::{rt, test_http::FakeServer};
  use tokio::fs;

  use super::*;
  use crate::testing::{data_dir, serve_version};

  const MC_PATH: &str = "/launchermeta.mojang.com/mc/game/version_manifest_v2.json";
  const JAVA_PATH: &str = "/launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

  fn manifest(ids: &[(&str, &str)]) -> String {
    let versions: Vec<_> = ids
      .iter()
      .map(|(id, ty)| {
        serde_json::json!({ "id": id, "type": ty, "url": "https://x.invalid/v.json", "sha1": "s" })
      })
      .collect();
    serde_json::json!({ "versions": versions }).to_string()
  }

  fn store(ids: &[(&str, &str)]) -> McVersionStore {
    let manifest = serde_json::from_str(&manifest(ids)).unwrap();
    McVersionStore::from_manifests(manifest, JavaVersions::default(), DownloadsHandle::new().0)
  }

  fn java_manifest() -> String {
    let empty = serde_json::json!({
      "java-runtime-alpha": [], "java-runtime-beta": [], "java-runtime-delta": [],
      "java-runtime-epsilon": [], "java-runtime-gamma": [],
      "java-runtime-gamma-snapshot": [], "jre-legacy": []
    });
    let platforms = [
      "linux",
      "linux-i386",
      "mac-os",
      "mac-os-arm64",
      "windows-arm64",
      "windows-x64",
      "windows-x86",
    ];
    let all: serde_json::Map<_, _> = platforms
      .into_iter()
      .map(|p| (p.to_string(), empty.clone()))
      .collect();
    serde_json::Value::Object(all).to_string()
  }

  #[test]
  fn cancel_permits_survive_until_the_job_waits() {
    assert!(!is_busy());
    // canceling a job nobody runs is a no-op
    cancel(7);
    let guard = register_cancel(7);
    assert!(is_busy());
    cancel(7);
    rt::block_on(async {
      tokio::time::timeout(Duration::from_secs(1), guard.notify.notified())
        .await
        .expect("the stored permit wakes the job");
    });
    drop(guard);
    // the guard unregisters on drop, even on early returns
    assert!(!is_busy());
  }

  #[test]
  fn manifests_are_cached_until_forced() {
    let server = FakeServer::start();
    let data = data_dir();
    server
      .route("GET", MC_PATH, 200, manifest(&[("1.21", "release")]))
      .route("GET", JAVA_PATH, 200, java_manifest());
    let client = default_client();
    rt::block_on(async {
      let (mc, _) = McVersionStore::load_manifests(&client, false)
        .await
        .unwrap();
      assert_eq!(mc.versions[0].id, "1.21");
      assert!(MCPath::new(&data).mc_manifest().exists());

      // offline: the cache still answers
      server
        .route("GET", MC_PATH, 503, "")
        .route("GET", JAVA_PATH, 503, "");
      assert!(McVersionStore::load_manifests(&client, false).await.is_ok());
      assert!(McVersionStore::load_manifests(&client, true).await.is_err());

      server.route("GET", MC_PATH, 200, manifest(&[("1.22", "release")]));
      server.route("GET", JAVA_PATH, 200, java_manifest());
      let (mc, _) = McVersionStore::load_manifests(&client, true).await.unwrap();
      assert_eq!(mc.versions[0].id, "1.22");

      // a corrupt cache is an error, not an empty list
      fs::write(MCPath::new(&data).mc_manifest(), "{")
        .await
        .unwrap();
      assert!(McVersionStore::new(DownloadsHandle::new().0).await.is_err());
    });
    assert_eq!(server.calls().len(), 6);
  }

  #[test]
  fn vanilla_lists_hide_snapshots_when_stable() {
    data_dir();
    let store = store(&[
      ("24w01a", "snapshot"),
      ("1.21", "release"),
      ("b1.7.3", "old_beta"),
    ]);
    rt::block_on(async {
      let list = |stable| store.list_versions(&LoaderType::Vanilla, stable);
      assert_eq!(list(true).await.unwrap(), ["1.21"]);
      assert_eq!(list(false).await.unwrap(), ["24w01a", "1.21", "b1.7.3"]);
      let loaders = store.list_loader_versions(&LoaderType::Vanilla, "1.21", true);
      assert!(loaders.await.unwrap().is_empty());
    });
  }

  #[test]
  fn loader_lists_follow_the_release_order() {
    let data = data_dir();
    let store = store(&[
      ("1.21", "release"),
      ("1.20.1", "release"),
      ("1.19", "release"),
    ]);
    let root = MCVersionPath::new(&data, "").version_root().clone();
    rt::block_on(async {
      // no metadata yet
      assert!(
        store
          .list_versions(&LoaderType::Fabric, true)
          .await
          .is_err()
      );

      fs::create_dir_all(&root).await.unwrap();
      let game = r#"[
        { "version": "1.19", "stable": true },
        { "version": "23w01a", "stable": true },
        { "version": "1.21", "stable": true }
      ]"#;
      fs::write(root.join("fabric-game.json"), game)
        .await
        .unwrap();
      let loader = r#"[{ "version": "0.15.0", "stable": true }]"#;
      fs::write(root.join("fabric-loader.json"), loader)
        .await
        .unwrap();
      // versions missing from the manifest go last
      assert_eq!(
        store
          .list_versions(&LoaderType::Fabric, true)
          .await
          .unwrap(),
        ["1.21", "1.19", "23w01a"]
      );
      let loaders = store.list_loader_versions(&LoaderType::Fabric, "1.21", true);
      assert_eq!(loaders.await.unwrap(), ["0.15.0"]);
    });
  }

  #[test]
  fn meta_check_compares_the_manifest_hash() {
    let data = data_dir();
    let manifest = format!(
      r#"{{ "versions": [{{ "id": "1.21", "type": "release", "url": "https://x.invalid", "sha1": "{}" }}] }}"#,
      proton_utils::file::bytes_hash(b"meta")
    );
    let (handle, rx) = DownloadsHandle::new();
    let store = McVersionStore::from_manifests(
      serde_json::from_str(&manifest).unwrap(),
      JavaVersions::default(),
      handle,
    );
    let path = MCVersionPath::new(&data, "1.21").version_manifest();
    rt::block_on(async {
      let err = store.check_meta("9.9", 1).await.unwrap_err();
      assert!(matches!(err.downcast_ref(), Some(DownloadError::NotFound)));
      assert!(!store.check_meta("1.21", 1).await.unwrap());

      fs::create_dir_all(path.parent().unwrap()).await.unwrap();
      fs::write(&path, "other").await.unwrap();
      assert!(!store.check_meta("1.21", 1).await.unwrap());
      fs::write(&path, "meta").await.unwrap();
      assert!(store.check_meta("1.21", 1).await.unwrap());
      tokio::time::sleep(Duration::from_millis(100)).await;
    });
    // only the good check reports done
    let done: Vec<_> = rx.try_iter().collect();
    assert_eq!(done.len(), 1, "{done:?}");
  }

  #[cfg(target_os = "linux")]
  #[test]
  fn check_or_download_finishes_or_is_canceled() {
    let server = FakeServer::start();
    let data = data_dir();
    let fixture = rt::block_on(serve_version(&server));
    let store =
      McVersionStore::from_manifests(fixture.manifest, fixture.java, DownloadsHandle::new().0);
    rt::block_on(async {
      let run = |v: &'static str, id| store.check_or_download(v, id, LoaderType::Vanilla, None);
      let err = run("9.9", 1).await.unwrap_err();
      assert!(matches!(err.downcast_ref(), Some(DownloadError::NotFound)));
      assert!(run("1.20.1", 2).await.unwrap());
      assert!(MCVersionPath::new(&data, "1.20.1").client_jar().exists());
      assert!(!is_busy());
    });
  }

  #[test]
  fn a_hanging_download_can_be_canceled() {
    // accepts connections but never answers
    let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v.json", silent.local_addr().unwrap());
    data_dir();
    let manifest = format!(
      r#"{{ "versions": [{{ "id": "1.21", "type": "release", "url": "{url}", "sha1": "s" }}] }}"#
    );
    let store = McVersionStore::from_manifests(
      serde_json::from_str(&manifest).unwrap(),
      JavaVersions::default(),
      DownloadsHandle::new().0,
    );
    let job = rt::spawn(async move {
      store
        .check_or_download("1.21", 3, LoaderType::Vanilla, None)
        .await
    });
    rt::block_on(async {
      while !is_busy() {
        tokio::time::sleep(Duration::from_millis(5)).await;
      }
      cancel(3);
      let finished = tokio::time::timeout(Duration::from_secs(5), job)
        .await
        .unwrap()
        .unwrap();
      assert!(!finished.unwrap());
    });
    assert!(!is_busy());
  }
}
