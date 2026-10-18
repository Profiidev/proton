use std::{
  collections::HashMap,
  path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::{
  PROFILE_CONFIG, PROFILE_DIR,
  config::{
    PlayHistoryFavoriteInfo, Profile, ProfileError, ProfileInfo, ProfileUpdate, QuickPlayInfo,
  },
  instance::{Instance, InstanceError, InstanceInfo, Instances},
  watcher::watch_profile,
};

use proton_config::settings::Settings;
use proton_utils::{file::read_parse_file, path};
use proton_versions::{
  launch::{LaunchArgs, launch_minecraft_version},
  loader::LoaderType,
};

pub const LAUNCHER_NAME: &str = "Proton";

/// The account a game is started with, from the account service
#[derive(Debug, Clone)]
pub struct LaunchInfo {
  pub id: String,
  pub name: String,
  pub access_token: String,
}

/// What the tokio side reports to the gpui side, drained by `listener::listen`
#[derive(Debug)]
pub enum ProfilesMsg {
  Profiles {
    profiles: Vec<Profile>,
    favorites: Vec<PlayHistoryFavoriteInfo>,
    history: Vec<PlayHistoryFavoriteInfo>,
  },
  QuickPlay(String, Vec<QuickPlayInfo>),
  Runs(String, Vec<DateTime<Utc>>),
  /// The watcher merged the game's quick play file into this profile
  QuickPlayFileChanged(String),
  /// Content folder or manifest of this profile changed on disk, throttled
  ContentChanged(String),
  InstanceAdded(InstanceInfo),
  InstanceRemoved(String),
  /// A batch of new log lines of the instance with this id
  InstanceLines(String, Vec<String>),
  InstanceCrashed {
    profile_name: String,
  },
}

pub struct ProfileStore {
  profiles: HashMap<String, ProfileInfo>,
  instances: Instances,
  tx: flume::Sender<ProfilesMsg>,
  data_dir: PathBuf,
}

impl ProfileStore {
  const PROFILE_KEY: &str = "profiles";

  /// No profiles and no disk access, stands in until [`Self::new`] is done
  pub(crate) fn empty(data_dir: PathBuf, tx: flume::Sender<ProfilesMsg>) -> ProfileStore {
    ProfileStore {
      profiles: HashMap::new(),
      tx,
      instances: Default::default(),
      data_dir,
    }
  }

  pub(crate) fn sender(&self) -> flume::Sender<ProfilesMsg> {
    self.tx.clone()
  }

  pub fn new(data_dir: PathBuf, tx: flume::Sender<ProfilesMsg>) -> Result<ProfileStore> {
    let store = proton_store::store();
    let profile_paths: HashMap<String, PathBuf> = store.get_or_default(Self::PROFILE_KEY)?;

    let mut profiles = HashMap::new();
    for (id, path) in profile_paths {
      // older versions stored absolute paths, which break when the data dir moves
      let path = if path.is_absolute() {
        match path.strip_prefix(&data_dir) {
          Ok(relative) => relative.to_path_buf(),
          Err(_) if !path.exists() => path!(PROFILE_DIR, &id),
          Err(_) => path,
        }
      } else {
        path
      };
      let Some(watcher) = watch_profile(path!(&data_dir, &path), id.clone(), tx.clone()).ok()
      else {
        continue;
      };
      profiles.insert(id, ProfileInfo { path, watcher });
    }

    let store = ProfileStore {
      profiles,
      tx,
      instances: Default::default(),
      data_dir,
    };
    store.save()?;

    Ok(store)
  }

  pub async fn create_profile(
    &mut self,
    name: String,
    icon: Option<&[u8]>,
    version: String,
    loader: LoaderType,
    loader_version: Option<String>,
  ) -> Result<String> {
    let (id, info) = Profile::create(
      &self.data_dir,
      &self.tx,
      name,
      icon,
      version,
      loader,
      loader_version,
    )
    .await?;
    self.profiles.insert(id.clone(), info);
    self.save()?;

    Ok(id)
  }

  /// Registers a profile whose directory was written at `profile.relative_to_data()`
  pub fn insert_profile(&mut self, profile: &Profile) -> Result<()> {
    let path = profile.relative_to_data();
    let watcher = watch_profile(
      path!(&self.data_dir, &path),
      profile.id.clone(),
      self.tx.clone(),
    )?;
    self
      .profiles
      .insert(profile.id.clone(), ProfileInfo { path, watcher });
    self.save()
  }

  /// Applies the editable fields, a changed version or loader needs a new download
  pub async fn update_profile(&self, update: ProfileUpdate) -> Result<()> {
    let mut profile = self.profile(&update.id).await?;
    if update.version != profile.version || update.loader_version != profile.loader_version {
      profile.downloaded = false;
    }
    profile.name = update.name;
    profile.version = update.version;
    profile.loader_version = update.loader_version;
    profile.use_local_game = update.use_local_game;
    profile.game = update.game;
    profile.use_local_jvm = update.use_local_jvm;
    profile.jvm = update.jvm;
    profile.update(&self.data_dir).await
  }

  pub fn get_profile_path(&self, profile: &str) -> Result<PathBuf> {
    let info = self.profile_info(profile)?;
    Ok(path!(&self.data_dir, &info.path))
  }

  pub async fn remove_profile(&mut self, id: &str) -> Result<()> {
    let info = self.profile_info(id)?.clone();

    self.profiles.remove(id);
    self.save()?;

    info.remove_profile(&self.data_dir).await?;
    Ok(())
  }

  /// All profiles, oldest first
  pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
    let mut profiles: Vec<Profile> = Vec::new();
    for info in self.profiles.values() {
      profiles.push(read_parse_file(&path!(&self.data_dir, &info.path, PROFILE_CONFIG)).await?);
    }
    profiles.sort_by_key(|p| p.created_at);

    Ok(profiles)
  }

  pub async fn launch_profile(
    &mut self,
    info: LaunchInfo,
    profile: Profile,
    quick_play: Option<QuickPlayInfo>,
  ) -> Result<()> {
    let settings = Settings::load()?.minecraft;

    let jvm_settings = if profile.use_local_jvm {
      profile.jvm.clone().unwrap_or_default()
    } else {
      settings.jvm_settings
    };

    let game_settings = if profile.use_local_game {
      profile.game.clone().unwrap_or_default()
    } else {
      settings.game_settings
    };

    let loader = profile
      .loader_version
      .clone()
      .and_then(|v| profile.loader.loader_version(profile.version.clone(), v));

    let mem_max = jvm_settings.mem_max as u64;
    let child = launch_minecraft_version(&LaunchArgs {
      access_token: info.access_token,
      launcher_name: LAUNCHER_NAME.into(),
      launcher_version: env!("CARGO_PKG_VERSION").into(),
      player_name: info.name.clone(),
      player_uuid: info.id.clone(),
      user_type: "msa".into(),
      data_dir: self.data_dir.clone(),
      version: profile.version.clone(),
      working_sub_dir: profile.relative_to_data().display().to_string(),
      quick_play: quick_play.map(|q| q.into()),
      loader,
      game_settings,
      jvm_settings,
    })
    .await?;

    self.add_instance(child, &profile, info.name, mem_max).await
  }

  /// Tracks a started game process as an instance of `profile`
  pub async fn add_instance(
    &self,
    child: tokio::process::Child,
    profile: &Profile,
    account_name: String,
    mem_max: u64,
  ) -> Result<()> {
    Instance::create(
      child,
      &self.data_dir,
      profile,
      account_name,
      mem_max,
      &self.instances,
      &self.tx,
    )
    .await
  }

  /// All profiles (oldest first), the home favorites and the home history. Drops singleplayer
  /// quick play entries whose world was deleted.
  pub async fn list_home(
    &self,
  ) -> Result<(
    Vec<Profile>,
    Vec<PlayHistoryFavoriteInfo>,
    Vec<PlayHistoryFavoriteInfo>,
  )> {
    let mut profiles = self.list_profiles().await?;
    for profile in &mut profiles {
      if profile.list_quick_play(&self.data_dir).await?.1 {
        profile.update(&self.data_dir).await?;
      }
    }
    let favorites = home_entries(&profiles, |p| p.favorite, |q| q.favorite);
    let history = home_entries(
      &profiles,
      |p| p.last_played_non_quick_play.is_some(),
      |q| q.history,
    );
    Ok((profiles, favorites, history))
  }

  /// Memory used per instance id, polled by the UI
  pub async fn instances_memory(&self) -> HashMap<String, u64> {
    let instances = self.instances.lock().await;
    let pids: Vec<Pid> = instances
      .values()
      .flatten()
      .map(|i| Pid::from_u32(i.pid()))
      .collect();
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&pids), true);

    instances
      .values()
      .flatten()
      .filter_map(|i| {
        let memory = system.process(Pid::from_u32(i.pid()))?.memory();
        Some((i.info().id.clone(), memory))
      })
      .collect()
  }

  pub async fn is_running(&self, profile: &str) -> bool {
    let instances = self.instances.lock().await;
    instances.get(profile).is_some_and(|i| !i.is_empty())
  }

  pub fn mods_dirs(&self) -> Vec<(String, PathBuf)> {
    self
      .profiles
      .iter()
      .map(|(id, info)| (id.clone(), path!(&self.data_dir, &info.path, "mods")))
      .collect()
  }

  pub async fn stop_all_instances(&self, profile: Option<&str>) {
    let instances = self.instances.lock().await;
    instances
      .iter()
      .filter(|(id, _)| profile.is_none_or(|p| p == id.as_str()))
      .flat_map(|(_, entry)| entry)
      .for_each(Instance::stop);
  }

  pub async fn stop_instance(&self, profile: &str, id: &str) -> Result<()> {
    let instances = self.instances.lock().await;
    let instance = instances
      .get(profile)
      .and_then(|entry| entry.iter().find(|i| i.info().id == id))
      .ok_or(InstanceError::NotFound)?;
    instance.stop();
    Ok(())
  }

  pub fn data_dir(&self) -> &PathBuf {
    &self.data_dir
  }

  fn save(&self) -> Result<()> {
    let profiles: HashMap<&String, &PathBuf> = self
      .profiles
      .iter()
      .map(|(id, info)| (id, &info.path))
      .collect();
    proton_store::store().set(Self::PROFILE_KEY, &profiles)
  }

  pub fn profile_info(&self, profile: &str) -> Result<&ProfileInfo> {
    self
      .profiles
      .get(profile)
      .ok_or(ProfileError::NotFound.into())
  }

  pub async fn profile(&self, profile: &str) -> Result<Profile> {
    self.profile_info(profile)?.profile(&self.data_dir).await
  }
}

/// Number of `*.jar` files directly inside `dir`, 0 if it does not exist
pub fn count_mod_jars(dir: &Path) -> usize {
  let Ok(entries) = std::fs::read_dir(dir) else {
    return 0;
  };
  entries
    .flatten()
    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
    .filter(|e| {
      e.path()
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
    })
    .count()
}

/// The 12 latest profiles and quick play entries matching `profile_access` / `quick_play_access`
fn home_entries(
  profiles: &[Profile],
  profile_access: impl Fn(&Profile) -> bool,
  quick_play_access: impl Fn(&QuickPlayInfo) -> bool,
) -> Vec<PlayHistoryFavoriteInfo> {
  let mut entries = Vec::new();
  for profile in profiles {
    if profile_access(profile) {
      entries.push(PlayHistoryFavoriteInfo {
        profile: profile.clone(),
        quick_play: None,
      });
    }
    for quick_play in profile.quick_play.iter().filter(|q| quick_play_access(q)) {
      entries.push(PlayHistoryFavoriteInfo {
        profile: profile.clone(),
        quick_play: Some(quick_play.clone()),
      });
    }
  }

  entries.sort_unstable_by_key(|i| match &i.quick_play {
    Some(quick_play) => quick_play.last_played_time,
    None => i.profile.last_played.unwrap_or(Utc::now()),
  });
  entries.reverse();
  entries.truncate(12);
  entries
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use chrono::TimeDelta;
  use proton_utils::rt;

  use super::*;
  use crate::{
    config::QuickPlayType,
    tests::{profile, quick_play, sh},
  };

  /// Store file and data dir in a temp dir, the store file holds `profiles` when given
  fn setup(
    profiles: Option<serde_json::Value>,
  ) -> (PathBuf, flume::Receiver<ProfilesMsg>, ProfileStore) {
    let dir = tempfile::tempdir().unwrap().keep();
    if let Some(profiles) = profiles {
      let json = serde_json::json!({ "profiles": profiles });
      std::fs::write(dir.join("store.json"), json.to_string()).unwrap();
    }
    proton_store::init_at(dir.join("store.json"));
    let data_dir = dir.join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let (tx, rx) = flume::unbounded();
    let store = ProfileStore::new(data_dir.clone(), tx).unwrap();
    (data_dir, rx, store)
  }

  fn saved_paths() -> HashMap<String, PathBuf> {
    proton_store::store().get_or_default("profiles").unwrap()
  }

  async fn create(store: &mut ProfileStore, name: &str) -> String {
    store
      .create_profile(name.into(), None, "1.21".into(), LoaderType::Vanilla, None)
      .await
      .unwrap()
  }

  fn update(id: &str, version: &str, loader_version: Option<&str>) -> ProfileUpdate {
    ProfileUpdate {
      id: id.into(),
      name: "Renamed".into(),
      version: version.into(),
      loader_version: loader_version.map(Into::into),
      use_local_game: true,
      game: None,
      use_local_jvm: false,
      jvm: None,
    }
  }

  #[test]
  fn new_migrates_old_absolute_paths() {
    let dir = tempfile::tempdir().unwrap().keep();
    let data_dir = dir.join("data");
    let outside = dir.join("outside");
    for d in [
      data_dir.join("profiles/inside"),
      data_dir.join("profiles/moved"),
      outside.join("kept"),
      data_dir.join("profiles/relative"),
    ] {
      std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(
      dir.join("store.json"),
      serde_json::json!({ "profiles": {
        "inside": data_dir.join("profiles/inside"),
        // the old data dir is gone, the profile was moved along with the new one
        "moved": "/gone/old-data/profiles/moved",
        // a profile outside the data dir that still exists stays where it is
        "kept": outside.join("kept"),
        "relative": "profiles/relative",
        // nothing on disk, the watcher fails and the entry is dropped
        "missing": "profiles/missing",
      }})
      .to_string(),
    )
    .unwrap();
    proton_store::init_at(dir.join("store.json"));
    let (tx, _rx) = flume::unbounded();
    let store = ProfileStore::new(data_dir.clone(), tx).unwrap();

    let expected = HashMap::from([
      ("inside".to_string(), PathBuf::from("profiles/inside")),
      ("moved".into(), PathBuf::from("profiles/moved")),
      ("kept".into(), outside.join("kept")),
      ("relative".into(), PathBuf::from("profiles/relative")),
    ]);
    assert_eq!(saved_paths(), expected);
    for (id, path) in &expected {
      assert_eq!(&store.profile_info(id).unwrap().path, path);
    }
    assert!(store.profile_info("missing").is_err());
  }

  #[test]
  fn new_fails_on_a_garbled_profiles_key() {
    let dir = tempfile::tempdir().unwrap().keep();
    std::fs::write(dir.join("store.json"), r#"{"profiles":5}"#).unwrap();
    proton_store::init_at(dir.join("store.json"));
    let (tx, _rx) = flume::unbounded();
    assert!(ProfileStore::new(dir.join("data"), tx).is_err());
  }

  #[test]
  fn profiles_are_created_listed_oldest_first_and_removed() {
    let (data_dir, _rx, mut store) = setup(None);
    rt::block_on(async {
      let first = create(&mut store, "First").await;
      let second = create(&mut store, "Second").await;
      let names: Vec<_> = store
        .list_profiles()
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
      assert_eq!(names, ["First", "Second"]);
      assert_eq!(saved_paths().len(), 2);
      assert_eq!(
        store.get_profile_path(&first).unwrap(),
        data_dir.join(PROFILE_DIR).join(&first)
      );

      store.remove_profile(&first).await.unwrap();
      assert!(!data_dir.join(PROFILE_DIR).join(&first).exists());
      assert_eq!(saved_paths().into_keys().collect::<Vec<_>>(), [second]);
      for err in [
        store.remove_profile(&first).await.unwrap_err(),
        store.profile(&first).await.unwrap_err(),
        store.get_profile_path(&first).unwrap_err(),
      ] {
        assert!(matches!(err.downcast_ref(), Some(ProfileError::NotFound)));
      }
    });
  }

  #[test]
  fn update_needs_a_new_download_only_when_the_game_changes() {
    let (_, _rx, mut store) = setup(None);
    rt::block_on(async {
      let id = create(&mut store, "P").await;
      let cases = [
        // same version and loader: the files stay valid
        ("1.21", None, false),
        ("1.20.1", None, true),
        ("1.20.1", Some("0.15"), true),
      ];
      for (version, loader_version, reset) in cases {
        let mut p = store.profile(&id).await.unwrap();
        p.downloaded = true;
        p.update(store.data_dir()).await.unwrap();
        store
          .update_profile(update(&id, version, loader_version))
          .await
          .unwrap();
        let p = store.profile(&id).await.unwrap();
        assert_eq!(p.downloaded, !reset, "{version} {loader_version:?}");
        assert_eq!((p.name.as_str(), p.version.as_str()), ("Renamed", version));
        assert!(p.use_local_game);
      }
      let err = store
        .update_profile(update("nope", "1.21", None))
        .await
        .unwrap_err();
      assert!(matches!(err.downcast_ref(), Some(ProfileError::NotFound)));
    });
  }

  #[test]
  fn insert_profile_registers_a_dir_written_elsewhere() {
    let (data_dir, _rx, mut store) = setup(None);
    let p = profile("imported");
    // the watcher needs the dir
    assert!(store.insert_profile(&p).is_err());
    std::fs::create_dir_all(data_dir.join(p.relative_to_data())).unwrap();
    rt::block_on(p.update(&data_dir)).unwrap();
    store.insert_profile(&p).unwrap();
    assert_eq!(
      rt::block_on(store.profile("imported")).unwrap().id,
      "imported"
    );
    assert_eq!(saved_paths()["imported"], p.relative_to_data());
    assert_eq!(
      store.mods_dirs(),
      [("imported".into(), data_dir.join("profiles/imported/mods"))]
    );
  }

  #[test]
  fn list_home_prunes_deleted_worlds_on_disk() {
    let (_, _rx, mut store) = setup(None);
    rt::block_on(async {
      let id = create(&mut store, "P").await;
      let mut p = store.profile(&id).await.unwrap();
      p.quick_play = vec![
        quick_play("deleted", QuickPlayType::Singleplayer),
        quick_play("server", QuickPlayType::Multiplayer),
      ];
      p.update(store.data_dir()).await.unwrap();

      let (profiles, _, _) = store.list_home().await.unwrap();
      assert_eq!(profiles[0].quick_play.len(), 1);
      // written back, not only filtered
      assert_eq!(store.profile(&id).await.unwrap().quick_play.len(), 1);
    });
  }

  #[test]
  fn home_entries_are_newest_first_and_capped_at_twelve() {
    let base = Utc::now() - TimeDelta::days(1);
    let mut profiles = Vec::new();
    for i in 0..10 {
      let mut p = profile(&format!("p{i}"));
      p.favorite = true;
      p.last_played = Some(base + TimeDelta::minutes(i));
      let mut q = quick_play(&format!("q{i}"), QuickPlayType::Multiplayer);
      q.last_played_time = base + TimeDelta::minutes(i) + TimeDelta::seconds(30);
      q.favorite = i % 2 == 0;
      p.quick_play = vec![q];
      profiles.push(p);
    }
    // never played sorts as now, so it comes first
    let mut fresh = profile("fresh");
    fresh.favorite = true;
    profiles.push(fresh);

    let entries = home_entries(&profiles, |p| p.favorite, |q| q.favorite);
    let ids: Vec<_> = entries
      .iter()
      .map(|e| match &e.quick_play {
        Some(q) => q.id.clone(),
        None => e.profile.id.clone(),
      })
      .collect();
    assert_eq!(
      ids,
      [
        "fresh", "p9", "q8", "p8", "p7", "q6", "p6", "p5", "q4", "p4", "p3", "q2"
      ]
    );
    assert!(home_entries(&profiles, |_| false, |_| false).is_empty());
  }

  #[test]
  fn instances_are_tracked_until_they_stop() {
    let (_, rx, mut store) = setup(None);
    rt::block_on(async {
      let id = create(&mut store, "P").await;
      let p = store.profile(&id).await.unwrap();
      assert!(!store.is_running(&id).await);
      store
        .add_instance(sh("sleep 30"), &p, "acc".into(), 2048)
        .await
        .unwrap();
      store
        .add_instance(sh("sleep 30"), &p, "acc".into(), 2048)
        .await
        .unwrap();
      assert!(store.is_running(&id).await);
      let memory = store.instances_memory().await;
      assert_eq!(memory.len(), 2);
      assert!(memory.values().all(|m| *m > 0));

      let err = store.stop_instance(&id, "nope").await.unwrap_err();
      assert!(matches!(err.downcast_ref(), Some(InstanceError::NotFound)));
      assert!(store.stop_instance("other", "nope").await.is_err());
      // another profile's stop leaves these running
      store.stop_all_instances(Some("other")).await;
      let first = memory.keys().next().unwrap().clone();
      store.stop_instance(&id, &first).await.unwrap();
      let removed = loop {
        if let ProfilesMsg::InstanceRemoved(i) = rx.recv_async().await.unwrap() {
          break i;
        }
      };
      assert_eq!(removed, first);
      assert!(store.is_running(&id).await);

      store.stop_all_instances(None).await;
      tokio::time::timeout(Duration::from_secs(5), async {
        while store.is_running(&id).await {
          tokio::time::sleep(Duration::from_millis(20)).await;
        }
      })
      .await
      .unwrap();
      assert!(store.instances_memory().await.is_empty());
    });
  }

  #[test]
  fn count_mod_jars_counts_only_jar_files() {
    let dir = std::env::temp_dir().join(format!("proton-mods-test-{}", std::process::id()));
    assert_eq!(count_mod_jars(&dir), 0);

    std::fs::create_dir_all(dir.join("sub.jar")).unwrap();
    for name in ["a.jar", "b.JAR", "c.jar.disabled", "d.txt", "jar"] {
      std::fs::write(dir.join(name), b"").unwrap();
    }
    std::fs::write(dir.join("sub.jar").join("e.jar"), b"").unwrap();

    assert_eq!(count_mod_jars(&dir), 2);

    std::fs::remove_dir_all(&dir).unwrap();
  }
}
