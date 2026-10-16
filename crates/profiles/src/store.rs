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
    QuickPlayType,
  },
  instance::{Instance, InstanceError, InstanceInfo, Instances},
  watcher::watch_profile,
};
use proton_account::store::LaunchInfo;
use proton_config::settings::Settings;
use proton_utils::{file::read_parse_file, path};
use proton_versions::{
  launch::{LaunchArgs, launch_minecraft_version},
  loader::LoaderType,
};

pub const LAUNCHER_NAME: &str = "Proton";

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

    self.add_instance(child, &profile, info.name, info.id).await
  }

  /// Tracks a started game process as an instance of `profile`
  pub async fn add_instance(
    &self,
    child: tokio::process::Child,
    profile: &Profile,
    account_name: String,
    account_id: String,
  ) -> Result<()> {
    Instance::create(
      child,
      &self.data_dir,
      profile,
      account_name,
      account_id,
      &self.instances,
      &self.tx,
    )
    .await
  }

  pub async fn list_history(&mut self) -> Result<Vec<PlayHistoryFavoriteInfo>> {
    self
      .list_home_entries(
        |profile| profile.last_played_non_quick_play.is_some(),
        |quick_play| quick_play.history,
      )
      .await
  }

  pub async fn list_favorites(&mut self) -> Result<Vec<PlayHistoryFavoriteInfo>> {
    self
      .list_home_entries(|profile| profile.favorite, |quick_play| quick_play.favorite)
      .await
  }

  async fn list_home_entries(
    &mut self,
    profile_access: impl Fn(&Profile) -> bool,
    quick_play_access: impl Fn(&QuickPlayInfo) -> bool,
  ) -> Result<Vec<PlayHistoryFavoriteInfo>> {
    let mut entries = Vec::new();
    let keys = self.profiles.keys().cloned().collect::<Vec<_>>();
    for profile in keys {
      let mut profile_info = self.profile(&profile).await?;
      if profile_access(&profile_info) {
        entries.push(PlayHistoryFavoriteInfo {
          profile: profile_info.clone(),
          quick_play: None,
        });
      }

      // drop singleplayer entries whose world was deleted
      let saves = profile_info.list_saves(&self.data_dir).await?;
      let before = profile_info.quick_play.len();
      profile_info
        .quick_play
        .retain(|q| q.r#type != QuickPlayType::Singleplayer || saves.contains(&q.id));
      if profile_info.quick_play.len() < before {
        profile_info.update(&self.data_dir).await?;
      }

      for quick_play in &profile_info.quick_play {
        if quick_play_access(quick_play) {
          entries.push(PlayHistoryFavoriteInfo {
            profile: profile_info.clone(),
            quick_play: Some(quick_play.clone()),
          });
        }
      }
    }

    entries.sort_unstable_by_key(|i| {
      if let Some(quick_play) = &i.quick_play {
        quick_play.last_played_time
      } else {
        i.profile.last_played.unwrap_or(Utc::now())
      }
    });
    entries.reverse();
    entries.truncate(12);

    Ok(entries)
  }

  pub async fn list_instances(&self) -> Vec<InstanceInfo> {
    let instances = self.instances.lock().await;
    instances
      .values()
      .flatten()
      .map(|i| i.info().clone())
      .collect()
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

#[cfg(test)]
mod tests {
  use super::*;

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
