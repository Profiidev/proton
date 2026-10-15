use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::Result;
use chrono::Utc;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::{
  account::store::LaunchInfo,
  path,
  profiles::{
    PROFILE_CONFIG, PROFILE_DIR,
    config::{
      PlayHistoryFavoriteInfo, Profile, ProfileError, ProfileInfo, QuickPlayInfo, QuickPlayType,
    },
    watcher::watch_profile,
  },
  settings::SettingsExt,
  store::TauriAppStoreExt,
  utils::{
    file::read_parse_file,
    updater::{UpdateType, update_data},
  },
  versions::{
    launch::{LaunchArgs, launch_minecraft_version},
    loader::LoaderType,
  },
};

use super::instance::{Instance, InstanceError, InstanceInfo};

pub struct ProfileStore {
  profiles: HashMap<String, ProfileInfo>,
  instances: Arc<Mutex<HashMap<String, Vec<Instance>>>>,
  handle: AppHandle,
  data_dir: PathBuf,
}

impl ProfileStore {
  const PROFILE_KEY: &str = "profiles";

  pub fn new(handle: AppHandle) -> Result<ProfileStore> {
    let store = handle.app_store()?;
    let data_dir = handle.path().app_data_dir()?;
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
      let Some(watcher) = watch_profile(path!(&data_dir, &path), id.clone(), handle.clone()).ok()
      else {
        continue;
      };
      profiles.insert(id, ProfileInfo { path, watcher });
    }

    let store = ProfileStore {
      profiles,
      handle,
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
  ) -> Result<()> {
    let (id, info) = Profile::create(
      &self.data_dir,
      &self.handle,
      name,
      icon,
      version,
      loader,
      loader_version,
    )
    .await?;
    self.profiles.insert(id, info);
    self.save()?;

    Ok(())
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

  pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
    let mut profiles = Vec::new();
    for info in self.profiles.values() {
      profiles.push(read_parse_file(&path!(&self.data_dir, &info.path, PROFILE_CONFIG)).await?);
    }

    Ok(profiles)
  }

  pub async fn launch_profile(
    &mut self,
    info: LaunchInfo,
    profile: Profile,
    quick_play: Option<QuickPlayInfo>,
  ) -> Result<()> {
    let data_dir = self.data_dir.clone();
    let settings = self.handle.app_settings()?.minecraft;

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
      launcher_name: self.handle.package_info().name.clone(),
      launcher_version: self.handle.package_info().version.to_string(),
      player_name: info.name.clone(),
      player_uuid: info.id.clone(),
      user_type: "msa".into(),
      data_dir,
      version: profile.version.clone(),
      working_sub_dir: profile.relative_to_data().display().to_string(),
      quick_play: quick_play.clone().map(|q| q.into()),
      loader,
      game_settings,
      jvm_settings,
    })
    .await?;

    Instance::create(
      child,
      &self.handle,
      &profile,
      info.name,
      info.id,
      &self.instances,
    )
    .await?;

    Ok(())
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

      let saves = profile_info.list_saves(&self.data_dir).await?;
      let mut updated = false;

      for (i, quick_play) in profile_info.quick_play.clone().into_iter().enumerate() {
        if quick_play.r#type == QuickPlayType::Singleplayer && !saves.contains(&quick_play.id) {
          profile_info.quick_play.remove(i);
          updated = true;
          continue;
        }

        if quick_play_access(&quick_play) {
          entries.push(PlayHistoryFavoriteInfo {
            profile: profile_info.clone(),
            quick_play: Some(quick_play.clone()),
          });
        }
      }

      if updated {
        profile_info.update(&self.data_dir).await?;
        self.update_data(UpdateType::ProfileQuickPlay);
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
    let mut res = Vec::new();

    let pids: Vec<Pid> = instances
      .values()
      .flatten()
      .map(|i| Pid::from_u32(i.pid()))
      .collect();
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&pids), true);

    for (profile, instances) in instances.iter() {
      let profile_name = self.profile(profile).await.ok().map(|p| p.name);

      let instances: Vec<InstanceInfo> = instances
        .iter()
        .map(|i| InstanceInfo {
          id: i.id().to_string(),
          profile_name: profile_name.clone().unwrap_or(i.profile_name().to_string()),
          profile_id: i.profile_id().to_string(),
          version: i.version().to_string(),
          loader: i.loader(),
          loader_version: i.loader_version().cloned(),
          launched_at: i.launched_at(),
          account_name: Some(i.account_name().to_string()),
          account_id: Some(i.account_id().to_string()),
          memory_used: system.process(Pid::from_u32(i.pid())).map(|p| p.memory()),
        })
        .collect();
      if instances.is_empty() {
        continue;
      }

      res.extend(instances);
    }

    res
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

  pub async fn get_instance_logs(&self, profile: &str, id: &str) -> Result<Vec<String>> {
    let instances = self.instances.lock().await;
    let instances = instances.get(profile).ok_or(InstanceError::NotFound)?;
    let instance = instances
      .iter()
      .find(|i| i.id() == id)
      .ok_or(InstanceError::NotFound)?;
    Ok(instance.lines().await)
  }

  pub async fn stop_instance(&self, profile: &str, id: &str) -> Result<()> {
    let mut instances = self.instances.lock().await;
    let entry = instances.get_mut(profile).ok_or(InstanceError::NotFound)?;
    let instance = entry
      .iter()
      .find(|i| i.id() == id)
      .ok_or(InstanceError::NotFound)?;
    instance.stop();
    Ok(())
  }

  pub fn data_dir(&self) -> &PathBuf {
    &self.data_dir
  }

  pub fn handle(&self) -> &AppHandle {
    &self.handle
  }

  fn save(&self) -> Result<()> {
    let mut profiles = HashMap::new();
    for (id, info) in &self.profiles {
      profiles.insert(id.clone(), info.path.clone());
    }

    let store = self.handle.app_store()?;
    store.set(Self::PROFILE_KEY, &profiles)
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

  pub fn update_data(&self, r#type: UpdateType) {
    update_data(&self.handle, r#type);
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
