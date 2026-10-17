//! gpui half of the versions service: `Versions` global, `cx.versions()`, `init`.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use gpui_kit::{App, AppContext, Entity, Global, Task};
use proton_config::SettingsExt;
use proton_utils::{dir::data_dir, download::default_client, log::ErrorLogExt, rt};

use crate::{
  downloads::DownloadProgress, event::DownloadsHandle, listener, loader::LoaderType,
  paths::MCVersionPath, store, store::McVersionStore,
};

/// The manifests plus the version lists fetched so far. Notifies when the manifests change.
#[derive(Default)]
pub struct VersionLists {
  store: Option<McVersionStore>,
  /// by (loader, stable)
  versions: HashMap<(LoaderType, bool), Vec<String>>,
  /// by (loader, mc version, stable)
  loader_versions: HashMap<(LoaderType, String, bool), Vec<String>>,
}

/// The downloads panel list. Notifies only on add/remove, each row is its own entity.
#[derive(Default)]
pub struct Downloads {
  /// (download id, row) in start order
  pub(crate) rows: Vec<(usize, Entity<DownloadProgress>)>,
}

impl Downloads {
  pub fn rows(&self) -> impl Iterator<Item = &Entity<DownloadProgress>> {
    self.rows.iter().map(|(_, row)| row)
  }

  pub fn get(&self, id: usize) -> Option<Entity<DownloadProgress>> {
    self
      .rows
      .iter()
      .find(|(row_id, _)| *row_id == id)
      .map(|(_, row)| row.clone())
  }
}

#[derive(Clone)]
pub struct Versions {
  pub versions: Entity<VersionLists>,
  pub downloads: Entity<Downloads>,
  /// Clone this into tokio code (`start`, `status`, `finish`)
  pub handle: DownloadsHandle,
}

impl Global for Versions {}

fn stable(cx: &App) -> bool {
  !cx.settings().get(cx).minecraft.show_snapshots
}

impl Versions {
  /// A clone of the store for `check_or_download`/`check_meta` on the tokio runtime
  pub fn store(&self, cx: &App) -> Result<McVersionStore> {
    self
      .versions
      .read(cx)
      .store
      .clone()
      .ok_or_else(|| anyhow!("Version manifests are not loaded"))
  }

  pub fn version_list(&self, loader: LoaderType, cx: &mut App) -> Task<Result<Vec<String>>> {
    let stable = stable(cx);
    let key = (loader, stable);
    if let Some(list) = self.versions.read(cx).versions.get(&key) {
      return Task::ready(Ok(list.clone()));
    }
    let store = match self.store(cx) {
      Ok(store) => store,
      Err(e) => return Task::ready(Err(e)),
    };
    let entity = self.versions.clone();
    cx.spawn(async move |cx| {
      let list = rt::spawn(async move { store.list_versions(&loader, stable).await }).await??;
      entity.update(cx, |lists, _| lists.versions.insert(key, list.clone()));
      Ok(list)
    })
  }

  pub fn loader_version_list(
    &self,
    loader: LoaderType,
    mc_version: String,
    cx: &mut App,
  ) -> Task<Result<Vec<String>>> {
    let stable = stable(cx);
    let key = (loader, mc_version.clone(), stable);
    if let Some(list) = self.versions.read(cx).loader_versions.get(&key) {
      return Task::ready(Ok(list.clone()));
    }
    let store = match self.store(cx) {
      Ok(store) => store,
      Err(e) => return Task::ready(Err(e)),
    };
    let entity = self.versions.clone();
    cx.spawn(async move |cx| {
      let list = rt::spawn(async move {
        store
          .list_loader_versions(&loader, &mc_version, stable)
          .await
      })
      .await??;
      entity.update(cx, |lists, _| {
        lists.loader_versions.insert(key, list.clone())
      });
      Ok(list)
    })
  }

  /// Force-downloads the mc/java manifests and the mod loader metadata, then drops the cached
  /// lists and notifies. The startup sequence and the offline reconnect call this.
  pub fn refresh_manifests(&self, cx: &mut App) -> Task<Result<()>> {
    let entity = self.versions.clone();
    let handle = self.handle.clone();
    cx.spawn(async move |cx| {
      let client = default_client();
      let loader_client = client.clone();
      let (mc, java) =
        rt::spawn(async move { McVersionStore::load_manifests(&client, true).await }).await??;
      entity.update(cx, |lists, _| match &mut lists.store {
        Some(store) => store.update_manifests(mc, java),
        None => lists.store = Some(McVersionStore::from_manifests(mc, java, handle)),
      });

      rt::spawn(async move {
        let version_path = MCVersionPath::new(&data_dir()?, "");
        for loader in LoaderType::mod_loaders() {
          loader
            .download_metadata(&loader_client, &version_path)
            .await?;
        }
        anyhow::Ok(())
      })
      .await??;

      entity.update(cx, |lists, cx| {
        lists.versions.clear();
        lists.loader_versions.clear();
        cx.notify();
      });
      Ok(())
    })
  }

  /// Marks the row (canceled wins on finish) and wakes the job registered under `id`
  pub fn cancel_download(&self, id: usize, cx: &mut App) {
    if let Some(row) = self.downloads.read(cx).get(id) {
      row.update(cx, |d, cx| {
        if !d.cancel_requested {
          d.cancel_requested = true;
          cx.notify();
        }
      });
    }
    store::cancel(id);
  }

  /// Drops every row that is not running
  pub fn clear_finished(&self, cx: &mut App) {
    let rows = self.downloads.read(cx).rows.clone();
    let running: Vec<_> = rows
      .into_iter()
      .filter(|(_, row)| row.read(cx).is_running())
      .collect();
    self.downloads.update(cx, |list, cx| {
      if list.rows.len() != running.len() {
        list.rows = running;
        cx.notify();
      }
    });
  }
}

pub trait VersionsExt {
  fn versions(&self) -> &Versions;
}

impl VersionsExt for App {
  fn versions(&self) -> &Versions {
    self.global::<Versions>()
  }
}

/// Sets the global and starts the listener, without reading the manifests (tests use this).
/// Needs `proton_config::init` first.
pub fn init_state(cx: &mut App) -> Versions {
  let (handle, rx) = DownloadsHandle::new();
  let versions = Versions {
    versions: cx.new(|_| VersionLists::default()),
    downloads: cx.new(|_| Downloads::default()),
    handle,
  };
  listener::listen(cx, rx, versions.downloads.clone());
  cx.set_global(versions.clone());
  versions
}

/// Also reads the cached manifests in the background (downloads them when missing). The
/// startup sequence calls `refresh_manifests` once online.
pub fn init(cx: &mut App) {
  let versions = init_state(cx);
  let entity = versions.versions;
  let handle = versions.handle;
  cx.spawn(async move |cx| {
    let Ok(Ok(store)) = rt::spawn(McVersionStore::new(handle))
      .await
      .map(|r| r.log_err())
    else {
      return;
    };
    entity.update(cx, |lists, cx| {
      // a refresh may have been faster
      if lists.store.is_none() {
        lists.store = Some(store);
        cx.notify();
      }
    });
  })
  .detach();
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use gpui_kit::{self as gpui, TestAppContext};

  use super::*;
  use crate::{
    downloads::DownloadStatus,
    event::{DownloadCheckStatus, DownloadMsg},
    meta::{java::JavaVersions, minecraft::Manifest},
  };

  fn init_test(cx: &mut TestAppContext) -> Versions {
    let dir = tempfile::tempdir().unwrap().keep();
    proton_store::init_at(dir.join("store.json"));
    cx.update(|cx| {
      proton_config::init(cx);
      init_state(cx)
    })
  }

  fn count<T: 'static>(cx: &mut TestAppContext, entity: &Entity<T>) -> Rc<Cell<usize>> {
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    cx.update(|cx| {
      cx.observe(entity, move |_, _| seen.set(seen.get() + 1))
        .detach()
    });
    count
  }

  fn status(id: usize, status: DownloadCheckStatus) -> DownloadMsg {
    DownloadMsg::Status { id, status }
  }

  #[gpui::test]
  fn progress_reaches_only_its_row(cx: &mut TestAppContext) {
    let versions = init_test(cx);
    let list_count = count(cx, &versions.downloads);
    let a = versions.handle.start("A");
    let b = versions.handle.start("B");
    cx.run_until_parked();
    assert_eq!(list_count.get(), 2);

    let row = |id| cx.read(|cx| versions.downloads.read(cx).get(id).unwrap());
    let (row_a, row_b) = (row(a), row(b));
    let (a_count, b_count) = (count(cx, &row_a), count(cx, &row_b));

    for i in 1..=5 {
      let tick = status(a, DownloadCheckStatus::AssetsCheck(i, 5));
      versions.handle.send(tick);
    }
    // no change, no notify
    let tick = status(a, DownloadCheckStatus::AssetsCheck(5, 5));
    versions.handle.send(tick);
    versions.handle.send(status(99, DownloadCheckStatus::Done));
    cx.run_until_parked();

    assert_eq!((a_count.get(), b_count.get(), list_count.get()), (5, 0, 2));
    cx.read(|cx| {
      assert_eq!(row_a.read(cx).value, Some(5));
      assert_eq!(row_b.read(cx).value, None);
    });
  }

  #[gpui::test]
  fn cancel_finish_and_clear(cx: &mut TestAppContext) {
    let versions = init_test(cx);
    let a = versions.handle.start("A");
    let b = versions.handle.start("B");
    let c = versions.handle.start("C");
    cx.run_until_parked();
    let guard = store::register_cancel(a);

    cx.update(|cx| versions.cancel_download(a, cx));
    versions.handle.finish(a, true);
    versions.handle.finish(b, false);
    cx.run_until_parked();
    // the job was woken (stored permit)
    rt::block_on(guard.notify.notified());

    let state = |cx: &mut TestAppContext, id| {
      cx.read(|cx| {
        let row = versions.downloads.read(cx).get(id);
        row.map(|r| r.read(cx).status)
      })
    };
    assert_eq!(state(cx, a), Some(DownloadStatus::Canceled));
    assert_eq!(state(cx, b), Some(DownloadStatus::Error));

    let list_count = count(cx, &versions.downloads);
    cx.update(|cx| versions.clear_finished(cx));
    cx.update(|cx| versions.clear_finished(cx));
    cx.run_until_parked();
    assert_eq!(list_count.get(), 1);
    assert_eq!(state(cx, a), None);
    assert_eq!(state(cx, c), Some(DownloadStatus::Running));
  }

  #[gpui::test]
  async fn version_list_is_cached_per_snapshot_setting(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let versions = init_test(cx);
    let manifest: Manifest = serde_json::from_value(serde_json::json!({
      "latest": { "release": "1.21", "snapshot": "24w01a" },
      "versions": [
        { "id": "24w01a", "type": "snapshot", "url": "https://x.invalid/a.json",
          "time": "2024-01-01T00:00:00Z", "releaseTime": "2024-01-01T00:00:00Z", "sha1": "a" },
        { "id": "1.21", "type": "release", "url": "https://x.invalid/b.json",
          "time": "2024-01-01T00:00:00Z", "releaseTime": "2024-01-01T00:00:00Z", "sha1": "b" }
      ]
    }))
    .unwrap();
    let handle = versions.handle.clone();
    let store = McVersionStore::from_manifests(manifest, JavaVersions::default(), handle);
    cx.update(|cx| {
      versions
        .versions
        .update(cx, |lists, _| lists.store = Some(store))
    });

    let list =
      |cx: &mut TestAppContext| cx.update(|cx| versions.version_list(LoaderType::Vanilla, cx));
    assert_eq!(list(cx).await.unwrap(), ["1.21"]);
    cx.read(|cx| {
      let cached = &versions.versions.read(cx).versions;
      assert_eq!(cached[&(LoaderType::Vanilla, true)], ["1.21"]);
    });

    cx.update(|cx| proton_config::update(cx, |s| s.minecraft.show_snapshots = true));
    assert_eq!(list(cx).await.unwrap(), ["24w01a", "1.21"]);
  }

  fn serve_manifests(server: &proton_utils::test_http::FakeServer, release: &str) {
    let mc = serde_json::json!({ "versions": [
      { "id": release, "type": "release", "url": "https://x.invalid/v.json", "sha1": "s" }
    ] });
    let empty = serde_json::json!({
      "java-runtime-alpha": [], "java-runtime-beta": [], "java-runtime-delta": [],
      "java-runtime-epsilon": [], "java-runtime-gamma": [],
      "java-runtime-gamma-snapshot": [], "jre-legacy": []
    });
    let java = serde_json::json!({
      "linux": empty, "linux-i386": empty, "mac-os": empty, "mac-os-arm64": empty,
      "windows-arm64": empty, "windows-x64": empty, "windows-x86": empty
    });
    let game = r#"[{ "version": "1.21", "stable": true }]"#;
    server
      .route("GET", "/launchermeta.mojang.com/mc/game/version_manifest_v2.json", 200, mc.to_string())
      .route(
        "GET",
        "/launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
        200,
        java.to_string(),
      )
      .route("GET", "/meta.fabricmc.net/v2/versions/loader", 200, game)
      .route("GET", "/meta.fabricmc.net/v2/versions/game", 200, game)
      .route("GET", "/meta.quiltmc.org/v3/versions/loader", 200, r#"[{ "version": "0.25.0" }]"#)
      .route("GET", "/meta.quiltmc.org/v3/versions/game", 200, game)
      .route(
        "GET",
        "/files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json",
        200,
        r#"{ "1.21": ["1.21-51.0.0"] }"#,
      )
      .route(
        "GET",
        "/maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml",
        200,
        "<metadata><versioning><versions><version>21.0.1</version></versions></versioning></metadata>",
      );
  }

  #[gpui::test]
  async fn lists_need_loaded_manifests(cx: &mut TestAppContext) {
    let versions = init_test(cx);
    assert!(cx.update(|cx| versions.store(cx)).is_err());
    let list = cx.update(|cx| versions.version_list(LoaderType::Vanilla, cx));
    assert!(list.await.is_err());
    let list = cx.update(|cx| versions.loader_version_list(LoaderType::Fabric, "1.21".into(), cx));
    assert!(list.await.is_err());
  }

  #[gpui::test]
  async fn refresh_loads_everything_and_drops_cached_lists(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let server = proton_utils::test_http::FakeServer::start();
    crate::testing::data_dir();
    let versions = init_test(cx);
    serve_manifests(&server, "1.21");
    let notified = count(cx, &versions.versions);

    cx.update(|cx| versions.refresh_manifests(cx))
      .await
      .unwrap();
    assert_eq!(notified.get(), 1);
    let neo = cx.update(|cx| versions.version_list(LoaderType::NeoForge, cx));
    assert_eq!(neo.await.unwrap(), ["1.21"]);
    let forge = cx.update(|cx| versions.loader_version_list(LoaderType::Forge, "1.21".into(), cx));
    assert_eq!(forge.await.unwrap(), ["51.0.0"]);
    // cached: a second ask makes no request and needs no store
    let calls = server.calls().len();
    let forge = cx.update(|cx| versions.loader_version_list(LoaderType::Forge, "1.21".into(), cx));
    assert_eq!(forge.await.unwrap(), ["51.0.0"]);
    assert_eq!(server.calls().len(), calls);

    // a second refresh swaps the manifests and forgets the lists
    serve_manifests(&server, "1.22");
    cx.update(|cx| versions.refresh_manifests(cx))
      .await
      .unwrap();
    cx.read(|cx| {
      let lists = versions.versions.read(cx);
      assert!(lists.versions.is_empty() && lists.loader_versions.is_empty());
    });
    let vanilla = cx.update(|cx| versions.version_list(LoaderType::Vanilla, cx));
    assert_eq!(vanilla.await.unwrap(), ["1.22"]);

    // a failing loader endpoint fails the refresh
    server.route("GET", "/meta.quiltmc.org/v3/versions/game", 500, "");
    assert!(
      cx.update(|cx| versions.refresh_manifests(cx))
        .await
        .is_err()
    );
  }

  #[gpui::test]
  async fn init_reads_cached_manifests(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let server = proton_utils::test_http::FakeServer::start();
    crate::testing::data_dir();
    serve_manifests(&server, "1.21");
    let dir = tempfile::tempdir().unwrap().keep();
    proton_store::init_at(dir.join("store.json"));
    cx.update(|cx| {
      proton_config::init(cx);
      init(cx);
    });
    let versions = cx.update(|cx| cx.versions().clone());
    // the store arrives from the tokio side
    for _ in 0..200 {
      if cx.update(|cx| versions.store(cx)).is_ok() {
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(10));
      cx.run_until_parked();
    }
    let list = cx.update(|cx| versions.version_list(LoaderType::Vanilla, cx));
    assert_eq!(list.await.unwrap(), ["1.21"]);
  }

  #[gpui::test]
  fn finish_for_a_cleared_row_is_ignored(cx: &mut TestAppContext) {
    let versions = init_test(cx);
    let a = versions.handle.start("A");
    versions.handle.finish(a, true);
    cx.run_until_parked();
    cx.update(|cx| versions.clear_finished(cx));
    // late messages for the dropped row do not panic or bring it back
    versions.handle.finish(a, false);
    versions.handle.send(status(a, DownloadCheckStatus::Done));
    // cancel without a row still reaches the job
    let guard = store::register_cancel(a);
    cx.update(|cx| versions.cancel_download(a, cx));
    cx.run_until_parked();
    rt::block_on(guard.notify.notified());
    cx.read(|cx| assert_eq!(versions.downloads.read(cx).rows().count(), 0));
  }
}
