use std::collections::HashMap;

use gpui_kit::{App, AppContext, Entity, Global};
use proton_config::OfflineExt;
use proton_profiles::{Profiles, ProfilesEvent, ProfilesExt};
use proton_versions::VersionsExt;
use thiserror::Error;

use crate::{
  model::{Category, ContentKind, Installed, PackUpdate, UpdateInfo},
  updates::ContentHandle,
};

pub mod curseforge;
pub mod http;
pub mod install;
mod listener;
pub mod model;
pub mod modpack;
pub mod modrinth;
pub mod profiles;
pub mod provider;
pub mod render;
mod service;
pub mod store;
pub mod updates;

pub const CF_KEY: &str = match option_env!("CURSEFORGE_API_KEY") {
  Some(k) => k,
  None => "$2a$10$LcjEjkIPGcXllU7ufwNLX.tSj3/Zp6h/aCJmnBNJueYp/sPKwo4OW",
};
pub const USER_AGENT: &str = concat!(
  "ProfiiDev/proton/",
  env!("CARGO_PKG_VERSION"),
  " (github.com/ProfiiDev/proton)"
);
pub use proton_profiles::{MANIFEST, TMP_DIR};

#[derive(Error, Debug)]
pub enum ContentError {
  #[error("Incompatible")]
  Incompatible,
  #[error("NeedsManualDownload")]
  NeedsManualDownload,
  #[error("Network")]
  Network,
  #[error("RateLimited")]
  RateLimited,
  #[error("HashMismatch")]
  HashMismatch,
  #[error("Unsupported")]
  Unsupported,
  #[error("Running")]
  Running,
  #[error("NotFound")]
  NotFound,
  #[error("InvalidPack")]
  InvalidPack,
}

pub type Result<T> = std::result::Result<T, ContentError>;

/// Installed content of one profile, empty until the first scan finished
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstalledContent {
  pub loaded: bool,
  pub by_kind: HashMap<ContentKind, Vec<Installed>>,
}

impl InstalledContent {
  pub fn get(&self, kind: ContentKind) -> &[Installed] {
    self
      .by_kind
      .get(&kind)
      .map(Vec::as_slice)
      .unwrap_or_default()
  }
}

/// Last update check result per profile id
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContentUpdates(pub HashMap<String, Vec<UpdateInfo>>);

/// Pack updates per modpack profile id, only profiles with an update are listed
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModpackUpdates(pub HashMap<String, PackUpdate>);

#[derive(Clone)]
pub struct Content {
  /// Categories per kind, filled by `load_categories`
  pub categories: Entity<HashMap<ContentKind, Vec<Category>>>,
  /// Per profile id, created by `installed`
  installed: Entity<HashMap<String, Entity<InstalledContent>>>,
  pub updates: Entity<ContentUpdates>,
  pub pack_updates: Entity<ModpackUpdates>,
  /// Clone this into tokio code
  pub handle: ContentHandle,
}

impl Global for Content {}

pub trait ContentExt {
  fn content(&self) -> &Content;
}

impl ContentExt for App {
  fn content(&self) -> &Content {
    self.global::<Content>()
  }
}

/// Needs `proton_config`, `proton_versions` and `proton_profiles` initialized first. Does not
/// start the updates loop, the startup sequence calls `start_updates_loop`.
pub fn init(cx: &mut App) {
  if !cx.has_global::<Profiles>() {
    tracing::error!("Profiles service missing, content disabled");
    return;
  }
  // the file watcher reports changed content folders
  let events = cx.profiles().events.clone();
  cx.subscribe(&events, |_, event: &ProfilesEvent, cx| {
    if let ProfilesEvent::ContentChanged(profile) = event {
      cx.content().clone().refresh_installed(profile, cx).detach();
    }
  })
  .detach();

  let (handle, rx) = ContentHandle::new(
    cx.offline().handle.clone(),
    cx.versions().handle.clone(),
    cx.profiles().handle.clone(),
  );
  let content = Content {
    categories: cx.new(|_| HashMap::new()),
    installed: cx.new(|_| HashMap::new()),
    updates: cx.new(|_| ContentUpdates::default()),
    pack_updates: cx.new(|_| ModpackUpdates::default()),
    handle,
  };
  listener::listen(
    cx,
    rx,
    content.updates.clone(),
    content.pack_updates.clone(),
  );
  cx.set_global(content);
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, path::PathBuf, rc::Rc, sync::LazyLock};

  use gpui_kit::{self as gpui, TestAppContext};
  use proton_utils::rt;
  use proton_versions::loader::LoaderType;

  use super::*;
  use crate::{
    model::Source,
    store::{Entry, Manifest},
  };

  /// One temp store per process, the store can only be pointed somewhere once
  static DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = tempfile::tempdir().unwrap().keep();
    proton_store::init_at(dir.join("store.json"));
    dir
  });

  fn init_test(cx: &mut TestAppContext) {
    LazyLock::force(&DIR);
    cx.update(|cx| {
      proton_config::init(cx);
      // the versions global without its manifest loading
      let (handle, _) = proton_versions::event::DownloadsHandle::new();
      let versions = proton_versions::Versions {
        versions: cx.new(|_| Default::default()),
        downloads: cx.new(|_| Default::default()),
        handle,
      };
      cx.set_global(versions);
      proton_profiles::init_at(cx, tempfile::tempdir().unwrap().keep()).unwrap();
      init(cx);
    });
  }

  fn counted<T: 'static>(cx: &mut TestAppContext, entity: &Entity<T>) -> Rc<Cell<usize>> {
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    cx.update(|cx| {
      cx.observe(entity, move |_, _| seen.set(seen.get() + 1))
        .detach()
    });
    count
  }

  /// Writes one identified mod into the profile, so scans need no network
  async fn add_mod(dir: PathBuf) {
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(dir.join("mods/a.jar"), b"jar").unwrap();
    let entry = Entry {
      kind: ContentKind::Mod,
      file_name: "a.jar".into(),
      sha1: "x".into(),
      fingerprint: 0,
      manual: false,
      from_pack: false,
      installed_at: None,
      source: Some(Source::Modrinth),
      project_id: Some("a".into()),
      version_id: Some("1".into()),
      name: Some("A".into()),
      icon_url: None,
      version_number: Some("1".into()),
      channel: None,
      dependency_of: vec![],
      pending: false,
    };
    let manifest = Manifest {
      entries: vec![entry],
      ..Default::default()
    };
    store::save(&dir, &manifest).await.unwrap();
  }

  #[gpui::test]
  async fn installed_refreshes_once_after_a_mutation(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    init_test(cx);
    let create = cx.update(|cx| {
      cx.profiles()
        .create("P".into(), None, "1.21.1".into(), LoaderType::Vanilla, None)
    });
    let id = create.await.unwrap();
    let profiles = cx.update(|cx| cx.profiles().handle.clone());
    let lookup = id.clone();
    rt::spawn(async move {
      let dir = profiles
        .store
        .lock()
        .await
        .get_profile_path(&lookup)
        .unwrap();
      add_mod(dir).await
    })
    .await
    .unwrap();

    let installed = cx.update(|cx| cx.content().clone().installed(&id, cx));
    // the first scan runs on tokio, wait for it
    while !installed.read_with(cx, |i, _| i.loaded) {
      cx.run_until_parked();
      std::thread::yield_now();
    }
    let mods = installed.read_with(cx, |i, _| i.get(ContentKind::Mod).to_vec());
    assert_eq!((mods.len(), mods[0].enabled), (1, true));

    let count = counted(cx, &installed);
    let task = cx.update(|cx| {
      cx.content()
        .clone()
        .set_enabled(&id, ContentKind::Mod, "a.jar".into(), false, cx)
    });
    task.await.unwrap();
    cx.run_until_parked();
    assert_eq!(count.get(), 1);
    assert!(!installed.read_with(cx, |i, _| i.get(ContentKind::Mod)[0].enabled));

    // nothing changed on disk, a refresh does not notify
    let task = cx.update(|cx| cx.content().clone().refresh_installed(&id, cx));
    task.await.unwrap();
    cx.run_until_parked();
    assert_eq!(count.get(), 1);
  }

  #[gpui::test]
  fn updates_entity_follows_the_handle(cx: &mut TestAppContext) {
    init_test(cx);
    let (handle, updates) = cx.update(|cx| {
      let content = cx.content();
      (content.handle.clone(), content.updates.clone())
    });
    let count = counted(cx, &updates);

    // from another thread, like the updates loop
    let tokio_side = handle.clone();
    std::thread::spawn(move || tokio_side.seed("p", &["a.jar", "b.jar"]))
      .join()
      .unwrap();
    cx.run_until_parked();
    assert_eq!(count.get(), 1);
    assert_eq!(updates.read_with(cx, |u, _| u.0["p"].len()), 2);

    handle.forget("p", &["a.jar".into()]);
    handle.forget("p", &["a.jar".into()]);
    cx.run_until_parked();
    assert_eq!(count.get(), 2);
    let left = updates.read_with(cx, |u, _| u.0["p"][0].file_name.clone());
    assert_eq!(left, "b.jar");
  }
}
