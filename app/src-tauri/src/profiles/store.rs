use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;
use chrono::Utc;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::{
  account::store::LaunchInfo,
  path,
  profiles::{
    config::{
      PlayHistoryFavoriteInfo, Profile, ProfileError, ProfileInfo, QuickPlayInfo, QuickPlayType,
    },
    watcher::watch_profile,
    PROFILE_CONFIG,
  },
  store::TauriAppStoreExt,
  utils::{
    file::read_parse_file,
    updater::{update_data, UpdateType},
  },
  versions::{
    launch::{launch_minecraft_version, LaunchArgs},
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
      let path = path!(&data_dir, &path);
      let Some(watcher) = watch_profile(path.clone(), id.clone(), handle.clone()).ok() else {
        continue;
      };
      profiles.insert(id, ProfileInfo { path, watcher });
    }

    Ok(ProfileStore {
      profiles,
      handle,
      instances: Default::default(),
      data_dir,
    })
  }

  pub async fn create_profile(
    &mut self,
    name: String,
    icon: Option<&[u8]>,
    version: String,
    loader: LoaderType,
  ) -> Result<()> {
    let (id, info) =
      Profile::create(&self.data_dir, &self.handle, name, icon, version, loader).await?;
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
    profile: &Profile,
    quick_play: Option<QuickPlayInfo>,
  ) -> Result<()> {
    let data_dir = self.data_dir.clone();

    let child = launch_minecraft_version(&LaunchArgs {
      access_token: info.access_token,
      launcher_name: self.handle.package_info().name.clone(),
      launcher_version: self.handle.package_info().version.to_string(),
      player_name: info.name,
      player_uuid: info.id,
      user_type: "msa".into(),
      data_dir,
      version: profile.version.clone(),
      working_sub_dir: profile.relative_to_data().display().to_string(),
      quick_play: quick_play.clone().map(|q| q.into()),
      loader: None,
    })
    .await?;

    Instance::create(child, &self.handle, profile, &self.instances).await?;

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
        })
        .collect();
      if instances.is_empty() {
        continue;
      }

      res.extend(instances);
    }

    res
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
