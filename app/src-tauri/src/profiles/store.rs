use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};
use tauri::{AppHandle, Manager};
use tokio::{fs, sync::Mutex};

use crate::{
  account::store::LaunchInfo,
  path,
  profiles::{
    config::{LoaderType, Profile, ProfileError, ProfileInfo, QuickPlayInfo},
    profile::create_profile,
    watcher::watch_profile,
    PROFILE_CONFIG, PROFILE_DIR, PROFILE_IMAGE, PROFILE_LOGS, SAVES_DIR,
  },
  store::TauriAppStoreExt,
  utils::{
    dir::list_dirs_in_dir,
    file::{read_parse_file, write_file},
    updater::{update_data, UpdateType},
  },
  versions::{
    launch::{launch_minecraft_version, LaunchArgs},
    QUICK_PLAY,
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
    let profile_paths: HashMap<String, PathBuf> = store.get_or_default(Self::PROFILE_KEY)?;
    let data_dir = handle.path().app_data_dir()?;

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

  pub fn log_dir(handle: &AppHandle, profile: &str) -> Result<PathBuf> {
    Ok(path!(
      handle.path().app_data_dir()?,
      PROFILE_DIR,
      profile,
      PROFILE_LOGS
    ))
  }

  fn save(&self) -> Result<()> {
    let mut profiles = HashMap::new();
    for (id, info) in &self.profiles {
      profiles.insert(id.clone(), info.path.clone());
    }

    let store = self.handle.app_store()?;
    store.set(Self::PROFILE_KEY, &profiles)
  }

  fn profile_info(&self, profile: &str) -> Result<&ProfileInfo> {
    self
      .profiles
      .get(profile)
      .ok_or(ProfileError::NotFound.into())
  }

  pub async fn create_profile(
    &mut self,
    name: String,
    icon: Option<&[u8]>,
    version: String,
    loader: LoaderType,
    loader_version: Option<String>,
  ) -> Result<()> {
    let (id, info) = create_profile(
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

    update_data(&self.handle, UpdateType::Profiles);
    Ok(())
  }

  pub async fn update_profile(&mut self, profile: &Profile) -> Result<()> {
    let info = self.profile_info(&profile.id)?;
    write_file(&path!(&self.data_dir, &info.path, PROFILE_CONFIG), profile).await?;

    self.save()?;
    update_data(&self.handle, UpdateType::Profiles);

    Ok(())
  }

  pub async fn get_profile_icon(&self, profile: &str) -> Result<Option<Vec<u8>>> {
    let info = self.profile_info(profile)?;
    let icon_path = path!(&self.data_dir, &info.path, PROFILE_IMAGE);
    if !icon_path.exists() {
      return Ok(None);
    }
    let icon = fs::read(icon_path).await?;
    Ok(Some(icon))
  }

  pub fn get_profile_path(&self, profile: &str) -> Result<PathBuf> {
    let info = self.profile_info(profile)?;
    Ok(path!(&self.data_dir, &info.path))
  }

  pub async fn update_profile_icon(&mut self, profile: &str, icon: &[u8]) -> Result<()> {
    if image::load_from_memory(icon).is_err() {
      return Err(ProfileError::InvalidImage.into());
    }

    let info = self.profile_info(profile)?;
    fs::write(&path!(&self.data_dir, &info.path, PROFILE_IMAGE), icon).await?;

    update_data(&self.handle, UpdateType::Profiles);

    Ok(())
  }

  pub async fn remove_profile(&mut self, id: &str) -> Result<()> {
    let info = self.profile_info(id)?;

    info.watcher.notify_waiters();

    fs::remove_dir_all(path!(&self.data_dir, &info.path)).await?;
    self.save()?;

    update_data(&self.handle, UpdateType::Profiles);

    Ok(())
  }

  pub fn list_profiles(&self) -> Result<Vec<Profile>> {
    let mut profiles = Vec::new();
    for info in self.profiles.values() {
      profiles.push(read_parse_file(&path!(
        &self.data_dir,
        &info.path,
        PROFILE_CONFIG
      ))?);
    }

    Ok(profiles)
  }

  pub fn get_profile(&self, profile: &str) -> Result<Profile> {
    let info = self.profile_info(profile)?;
    read_parse_file(&path!(&self.data_dir, &info.path, PROFILE_CONFIG))
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
      quick_play: quick_play.map(|q| q.into()),
    })?;

    Instance::create(child, &self.handle, profile, &self.instances).await?;

    Ok(())
  }

  pub async fn update_quick_play(&mut self, profile: &str) -> Result<()> {
    let mut profile = self.get_profile(profile)?;
    let quick_play_path = path!(&self.data_dir, &profile.relative_to_data(), QUICK_PLAY);

    let quick_plays: Vec<QuickPlayInfo> = read_parse_file(&quick_play_path)?;

    for quick_play in quick_plays {
      let index = profile
        .quick_play
        .iter()
        .position(|q| q.id() == quick_play.id());

      if let Some(index) = index {
        profile.quick_play[index] = quick_play;
      } else {
        profile.quick_play.push(quick_play);
      }
    }

    self.update_profile(&profile).await?;
    update_data(&self.handle, UpdateType::ProfileQuickPlay);

    Ok(())
  }

  pub async fn list_quick_play(&mut self, profile: &str) -> Result<Vec<QuickPlayInfo>> {
    let mut profile = self.get_profile(profile)?;
    let saves_path = path!(&self.data_dir, &profile.relative_to_data(), SAVES_DIR);
    if !saves_path.exists() {
      return Ok(profile.quick_play.clone());
    }

    let saves = list_dirs_in_dir(saves_path).await?;
    let prev_len = profile.quick_play.len();
    profile
      .quick_play
      .retain(|q| saves.contains(&q.id()) || !q.is_singleplayer());

    if profile.quick_play.len() < prev_len {
      self.update_profile(&profile).await?;
      update_data(&self.handle, UpdateType::ProfileQuickPlay);
    }

    Ok(profile.quick_play.clone())
  }

  pub async fn remove_quick_play(&mut self, profile: &str, id: &str) -> Result<()> {
    let mut profile = self.get_profile(profile)?;
    let index = profile.quick_play.iter().position(|q| q.id() == id);

    if let Some(index) = index {
      let _ = profile.quick_play.remove(index);
      self.update_profile(&profile).await?;
      update_data(&self.handle, UpdateType::ProfileQuickPlay);
    }

    Ok(())
  }

  pub async fn list_instances(&self) -> Vec<InstanceInfo> {
    let instances = self.instances.lock().await;
    let mut res = Vec::new();

    for (profile, instances) in instances.iter() {
      let profile_name = self.get_profile(profile).ok().map(|p| p.name);

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

  pub async fn list_profile_runs(&self, profile: &str) -> Result<Vec<DateTime<Utc>>> {
    let log_dir = Self::log_dir(&self.handle, profile)?;
    if !log_dir.exists() {
      return Ok(Vec::new());
    }

    let mut res = Vec::new();
    let mut stream = fs::read_dir(log_dir).await?;
    while let Some(entry) = stream.next_entry().await? {
      if entry.file_type().await?.is_file() {
        if let Some(name) = entry.file_name().to_str() {
          // replace the last 3 dashes with colons but leave the rest of the name intact
          let name = name.trim_end_matches(".log").replace("-", ":");
          if let Ok(date) = DateTime::parse_from_str(&name, "%Y:%m:%dT%H:%M:%S.%f%:z") {
            res.push(date.to_utc());
          }
        }
      }
    }

    Ok(res)
  }

  pub async fn profile_logs(&self, profile: &str, timestamp: DateTime<Utc>) -> Result<Vec<String>> {
    let log_dir = Self::log_dir(&self.handle, profile)?;
    if !log_dir.exists() {
      return Ok(Vec::new());
    }

    let log_file = log_dir.join(format!("{}.log", timestamp.to_rfc3339().replace(":", "-")));
    println!("Log file path: {:?}", log_file.to_str());
    if !log_file.exists() {
      return Ok(Vec::new());
    }

    let content = fs::read_to_string(log_file).await?;
    Ok(content.lines().map(String::from).collect())
  }
}
