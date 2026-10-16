use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};
use gpui_kit::{App, AppContext, Entity, EventEmitter, Global};
use tokio::sync::Mutex;

use crate::{
  config::{PlayHistoryFavoriteInfo, Profile, ProfileUpdate, QuickPlayInfo},
  instance::InstanceInfo,
  server::ServerStatus,
  store::LaunchInfo,
  store::{ProfileStore, ProfilesMsg, count_mod_jars},
};

use proton_config::{
  OfflineExt,
  offline::{OfflineHandle, OfflineResultExt},
};
use proton_utils::{log::ErrorLogExt, logs::LogBuffer, rt};
use proton_versions::{
  VersionsExt, event::DownloadsHandle, loader::LoaderType, store::McVersionStore,
};

pub mod config;
pub mod instance;
mod listener;
pub mod profile;
pub mod server;
pub mod store;
pub mod watcher;

const PROFILE_DIR: &str = "profiles";
const PROFILE_CONFIG: &str = "profile.json";
const PROFILE_IMAGE: &str = "image.png";
const PROFILE_LOGS: &str = "instance_logs";
const SAVES_DIR: &str = "saves";
/// Content manifest kept in each content folder
pub const MANIFEST: &str = "proton-content.json";
/// Downloads land here before they are verified and moved into place
pub const TMP_DIR: &str = ".proton-tmp";

#[derive(Debug, Clone, PartialEq)]
pub enum ProfilesEvent {
  /// A game exited with a non-zero status
  InstanceCrashed { profile_name: String },
  /// Content folder or manifest of this profile changed on disk, throttled to 500ms
  ContentChanged(String),
}

/// Only carries the [`ProfilesEvent`]s
pub struct ProfilesEvents;

impl EventEmitter<ProfilesEvent> for ProfilesEvents {}

/// Entities created on first use, keyed by profile id (instance id for `logs`)
#[derive(Default)]
pub struct LazyEntities {
  quick_play: HashMap<String, Entity<Vec<QuickPlayInfo>>>,
  runs: HashMap<String, Entity<Vec<DateTime<Utc>>>>,
  /// Live log of a running instance, appended in batches with one notify per batch
  logs: HashMap<String, Entity<LogBuffer>>,
}

/// Clone this into tokio code. The store lock is held only by tokio tasks.
#[derive(Clone)]
pub struct ProfilesHandle {
  pub store: Arc<Mutex<ProfileStore>>,
  tx: flume::Sender<ProfilesMsg>,
}

impl ProfilesHandle {
  /// Reloads the profile list and the home lists. Needs the tokio runtime.
  pub async fn refresh(&self) -> Result<()> {
    let mut store = self.store.lock().await;
    let msg = ProfilesMsg::Profiles {
      profiles: store.list_profiles().await?,
      favorites: store.list_favorites().await?,
      history: store.list_history().await?,
    };
    let _ = self.tx.send(msg);
    Ok(())
  }

  pub async fn refresh_quick_play(&self, profile: &str) -> Result<()> {
    let store = self.store.lock().await;
    let mut info = store.profile(profile).await?;
    let (list, pruned) = info.list_quick_play(store.data_dir()).await?;
    if pruned {
      info.update(store.data_dir()).await?;
    }
    let _ = self.tx.send(ProfilesMsg::QuickPlay(profile.into(), list));
    Ok(())
  }

  pub async fn refresh_runs(&self, profile: &str) -> Result<()> {
    let store = self.store.lock().await;
    let runs = store
      .profile_info(profile)?
      .list_runs(store.data_dir())
      .await?;
    let _ = self.tx.send(ProfilesMsg::Runs(profile.into(), runs));
    Ok(())
  }

  /// Runs `f` on the tokio runtime without awaiting it, errors are logged
  fn detach<F, Fut>(&self, f: F)
  where
    F: FnOnce(ProfilesHandle) -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
  {
    let fut = f(self.clone());
    rt::spawn(async move {
      let _ = fut.await.log_err();
    });
  }
}

#[derive(Clone)]
pub struct Profiles {
  /// All profiles, oldest first
  pub profiles: Entity<Vec<Profile>>,
  pub favorites: Entity<Vec<PlayHistoryFavoriteInfo>>,
  pub history: Entity<Vec<PlayHistoryFavoriteInfo>>,
  /// Running games, notifies only when one starts or exits
  pub instances: Entity<Vec<InstanceInfo>>,
  pub events: Entity<ProfilesEvents>,
  lazy: Entity<LazyEntities>,
  pub handle: ProfilesHandle,
}

impl Global for Profiles {}

pub trait ProfilesExt {
  fn profiles(&self) -> &Profiles;
}

impl ProfilesExt for App {
  fn profiles(&self) -> &Profiles {
    self.global::<Profiles>()
  }
}

impl Profiles {
  /// Runs `f` on the tokio runtime and awaits it from gpui code
  fn run<T, F, Fut>(&self, f: F) -> impl Future<Output = Result<T>> + use<T, F, Fut>
  where
    F: FnOnce(ProfilesHandle) -> Fut,
    Fut: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
  {
    let fut = f(self.handle.clone());
    async move { rt::spawn(fut).await? }
  }

  /// Quick play entries of a profile, loaded on first use
  pub fn quick_play(&self, profile: &str, cx: &mut App) -> Entity<Vec<QuickPlayInfo>> {
    if let Some(entity) = self.lazy.read(cx).quick_play.get(profile) {
      return entity.clone();
    }
    let entity = cx.new(|_| Vec::new());
    let id = profile.to_string();
    self.lazy.update(cx, |lazy, _| {
      lazy.quick_play.insert(id.clone(), entity.clone())
    });
    self
      .handle
      .detach(move |h| async move { h.refresh_quick_play(&id).await });
    entity
  }

  /// Start times of the stored runs of a profile, newest first, loaded on first use
  pub fn runs(&self, profile: &str, cx: &mut App) -> Entity<Vec<DateTime<Utc>>> {
    if let Some(entity) = self.lazy.read(cx).runs.get(profile) {
      return entity.clone();
    }
    let entity = cx.new(|_| Vec::new());
    let id = profile.to_string();
    self
      .lazy
      .update(cx, |lazy, _| lazy.runs.insert(id.clone(), entity.clone()));
    self
      .handle
      .detach(move |h| async move { h.refresh_runs(&id).await });
    entity
  }

  /// Live log of a running instance, None once it exited
  pub fn instance_logs(&self, instance: &str, cx: &App) -> Option<Entity<LogBuffer>> {
    self.lazy.read(cx).logs.get(instance).cloned()
  }

  pub fn create(
    &self,
    name: String,
    icon: Option<Vec<u8>>,
    version: String,
    loader: LoaderType,
    loader_version: Option<String>,
  ) -> impl Future<Output = Result<String>> + use<> {
    self.run(move |h| async move {
      let id = h
        .store
        .lock()
        .await
        .create_profile(name, icon.as_deref(), version, loader, loader_version)
        .await?;
      h.refresh().await?;
      Ok(id)
    })
  }

  pub fn update(&self, update: ProfileUpdate) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move {
      h.store.lock().await.update_profile(update).await?;
      h.refresh().await
    })
  }

  /// Raw image bytes, None when the profile uses the default icon
  pub fn icon(&self, profile: String) -> impl Future<Output = Result<Option<Vec<u8>>>> + use<> {
    self.run(move |h| async move {
      let store = h.store.lock().await;
      store
        .profile_info(&profile)?
        .get_icon(store.data_dir())
        .await
    })
  }

  /// Saves a new icon, the profiles entity notifies so views reload it
  pub fn update_icon(
    &self,
    profile: String,
    icon: Vec<u8>,
  ) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move {
      {
        let store = h.store.lock().await;
        store
          .profile_info(&profile)?
          .update_icon(&icon, store.data_dir())
          .await?;
      }
      h.refresh().await
    })
  }

  /// Opens the profile folder in the file manager
  pub fn open_path(&self, profile: String) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move {
      let path = h.store.lock().await.get_profile_path(&profile)?;
      Ok(open::that_detached(path)?)
    })
  }

  pub fn remove(&self, profile: String) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move {
      h.store.lock().await.remove_profile(&profile).await?;
      h.refresh().await
    })
  }

  /// Reloads all non-lazy entities, e.g. after another crate wrote a profile
  pub fn refresh(&self) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move { h.refresh().await })
  }

  /// Number of mod jars per profile id
  pub fn mods_count(&self) -> impl Future<Output = Result<HashMap<String, usize>>> + use<> {
    self.run(move |h| async move {
      let dirs = h.store.lock().await.mods_dirs();
      // blocking dir reads, done without holding the store lock
      Ok(
        rt::spawn_blocking(move || {
          dirs
            .into_iter()
            .map(|(id, dir)| (id, count_mod_jars(&dir)))
            .collect()
        })
        .await?,
      )
    })
  }

  /// Downloads or checks the game if needed (a row in the downloads panel), then starts it.
  /// `auth` comes from the account service (active account, token already refreshed). Ok
  /// without launching when the download was canceled.
  pub fn launch(
    &self,
    cx: &App,
    profile: String,
    quick_play: Option<QuickPlayInfo>,
    auth: LaunchInfo,
  ) -> impl Future<Output = Result<()>> + use<> {
    let files = GameFiles::new(cx);
    self.run(move |h| async move {
      let mut profile = h.store.lock().await.profile(&profile).await?;
      if !files.ensure(&profile, !profile.downloaded).await? {
        return Ok(());
      }
      profile.downloaded = true;
      mark_played(&mut profile, quick_play.as_ref(), Utc::now());

      let id = profile.id.clone();
      {
        let mut store = h.store.lock().await;
        profile.update(store.data_dir()).await?;
        store.launch_profile(auth, profile, quick_play).await?;
      }
      h.refresh().await?;
      h.refresh_quick_play(&id).await
    })
  }

  /// Checks all game files and downloads what is missing or broken
  pub fn repair(&self, cx: &App, profile: String) -> impl Future<Output = Result<()>> + use<> {
    let files = GameFiles::new(cx);
    self.run(move |h| async move {
      let mut profile = h.store.lock().await.profile(&profile).await?;
      if !files.ensure(&profile, true).await? {
        return Ok(());
      }
      profile.downloaded = true;
      profile.update(h.store.lock().await.data_dir()).await?;
      h.refresh().await
    })
  }

  pub fn clear_logs(&self, profile: String) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move {
      {
        let store = h.store.lock().await;
        store
          .profile_info(&profile)?
          .clear_logs(store.data_dir())
          .await?;
      }
      h.refresh_runs(&profile).await
    })
  }

  /// Stored log of the run started at `timestamp`
  pub fn logs(
    &self,
    profile: String,
    timestamp: DateTime<Utc>,
  ) -> impl Future<Output = Result<Vec<String>>> + use<> {
    self.run(move |h| async move {
      let store = h.store.lock().await;
      store
        .profile_info(&profile)?
        .logs(store.data_dir(), timestamp)
        .await
    })
  }

  pub fn remove_quick_play(
    &self,
    profile: String,
    quick_play: QuickPlayInfo,
  ) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move {
      {
        let store = h.store.lock().await;
        let mut info = store.profile(&profile).await?;
        info.remove_quick_play(quick_play).await?;
        info.update(store.data_dir()).await?;
      }
      h.refresh_quick_play(&profile).await?;
      h.refresh().await
    })
  }

  /// World or server icon as raw image bytes
  pub fn quick_play_icon(
    &self,
    cx: &App,
    profile: String,
    quick_play: QuickPlayInfo,
  ) -> impl Future<Output = Result<Option<Vec<u8>>>> + use<> {
    let offline = cx.offline().handle.clone();
    self.run(move |h| async move {
      let (info, data_dir) = {
        let store = h.store.lock().await;
        (store.profile(&profile).await?, store.data_dir().clone())
      };
      info
        .quick_play_icon(&data_dir, &quick_play)
        .await
        .check_online_state(&offline)
        .await
    })
  }

  pub async fn server_status(address: String) -> Option<ServerStatus> {
    rt::spawn(async move { server::server_status(&address).await })
      .await
      .ok()
      .flatten()
  }

  /// Favorites a profile, or one of its quick play entries
  pub fn set_favorite(
    &self,
    profile: String,
    quick_play: Option<QuickPlayInfo>,
    favorite: bool,
  ) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move {
      {
        let store = h.store.lock().await;
        let mut info = store.profile(&profile).await?;
        info.set_favorite(quick_play, favorite).await?;
        info.update(store.data_dir()).await?;
      }
      h.refresh().await?;
      h.refresh_quick_play(&profile).await
    })
  }

  pub fn stop_instance(
    &self,
    profile: String,
    id: String,
  ) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move { h.store.lock().await.stop_instance(&profile, &id).await })
  }

  /// Stops all instances, or only those of `profile`
  pub fn stop_all(&self, profile: Option<String>) -> impl Future<Output = Result<()>> + use<> {
    self.run(move |h| async move {
      h.store
        .lock()
        .await
        .stop_all_instances(profile.as_deref())
        .await;
      Ok(())
    })
  }

  /// Memory used per instance id, not part of `instances` so it does not cause re-renders
  pub fn instances_memory(&self) -> impl Future<Output = Result<HashMap<String, u64>>> + use<> {
    self.run(move |h| async move { Ok(h.store.lock().await.instances_memory().await) })
  }
}

/// What launch/repair need from the versions and offline services, taken from `cx` up front
struct GameFiles {
  store: Result<McVersionStore>,
  downloads: DownloadsHandle,
  offline: OfflineHandle,
}

impl GameFiles {
  fn new(cx: &App) -> Self {
    let versions = cx.versions();
    Self {
      store: versions.store(cx),
      downloads: versions.handle.clone(),
      offline: cx.offline().handle.clone(),
    }
  }

  /// True when the game files are ready, false when the download was canceled. Without
  /// `force` only the version meta is checked and the download runs only when it is broken.
  async fn ensure(self, profile: &Profile, force: bool) -> Result<bool> {
    let store = self.store?;
    let id = self.downloads.start(profile.name.clone());
    let result = async {
      if !force && store.check_meta(&profile.version, id).await? {
        return Ok(true);
      }
      store
        .check_or_download(
          &profile.version,
          id,
          profile.loader,
          profile.loader_version.clone(),
        )
        .await
        // a failed download can mean we went offline
        .check_online_state(&self.offline)
        .await
    }
    .await;
    self.downloads.finish(id, matches!(result, Ok(true)));
    result.log_err()
  }
}

fn mark_played(profile: &mut Profile, quick_play: Option<&QuickPlayInfo>, now: DateTime<Utc>) {
  profile.last_played = Some(now);
  match quick_play {
    Some(quick_play) => {
      if let Some(item) = profile.quick_play.iter_mut().find(|q| **q == *quick_play) {
        item.last_played_time = now;
        item.history = true;
      }
    }
    None => profile.last_played_non_quick_play = Some(now),
  }
}

/// Loads the profiles from the store and starts the listener. Needs `proton_config::init`.
pub fn init(cx: &mut App) {
  match proton_utils::dir::data_dir() {
    Ok(dir) => {
      let _ = init_at(cx, dir).log_err();
    }
    Err(err) => tracing::error!("No data dir, profiles disabled: {err:?}"),
  }
}

/// [`init`] with an explicit data dir, for tests
pub fn init_at(cx: &mut App, data_dir: PathBuf) -> Result<()> {
  let (tx, rx) = flume::unbounded();
  let mut store = ProfileStore::new(data_dir, tx.clone())?;
  let (profiles, favorites, history) = rt::block_on(async {
    anyhow::Ok((
      store.list_profiles().await?,
      store.list_favorites().await?,
      store.list_history().await?,
    ))
  })?;

  let service = Profiles {
    profiles: cx.new(|_| profiles),
    favorites: cx.new(|_| favorites),
    history: cx.new(|_| history),
    instances: cx.new(|_| Vec::new()),
    events: cx.new(|_| ProfilesEvents),
    lazy: cx.new(|_| LazyEntities::default()),
    handle: ProfilesHandle {
      store: Arc::new(Mutex::new(store)),
      tx,
    },
  };
  listener::listen(cx, rx, service.clone());
  cx.set_global(service);
  Ok(())
}

#[cfg(test)]
mod tests;
