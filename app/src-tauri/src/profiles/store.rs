use std::{collections::HashMap, fs, io::Cursor, path::PathBuf, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};
use image::{imageops::FilterType, ImageFormat};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

use crate::{
  account::store::LaunchInfo,
  path,
  profiles::watcher::watch_profile,
  store::TauriAppStoreExt,
  utils::{
    file::{read_parse_file, write_file},
    updater::{update_data, UpdateType},
  },
  versions::launch::{launch_minecraft_version, LaunchArgs},
};

use super::instance::{Instance, InstanceError, InstanceInfo};

pub struct ProfileStore {
  profiles: HashMap<String, ProfileInfo>,
  instances: Arc<Mutex<HashMap<String, Vec<Instance>>>>,
  handle: AppHandle,
  data_dir: PathBuf,
}

struct ProfileInfo {
  path: PathBuf,
  watcher: Arc<Notify>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Profile {
  pub id: String,
  pub name: String,
  pub created_at: DateTime<Utc>,
  pub last_played: Option<DateTime<Utc>>,
  pub version: String,
  pub loader: LoaderType,
  pub loader_version: Option<String>,
  pub downloaded: bool,
  pub use_local_game: bool,
  pub game: Option<GameSettings>,
  pub use_local_jvm: bool,
  pub jvm: Option<JvmSettings>,
  pub use_local_dev: bool,
  pub dev: Option<DevSettings>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProfileUpdate {
  pub id: String,
  pub name: String,
  pub version: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GameSettings {
  pub width: usize,
  pub height: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JvmSettings {
  pub args: Vec<String>,
  pub env_vars: HashMap<String, String>,
  pub mem_min: usize,
  pub mem_max: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DevSettings {
  pub show_console: bool,
  pub keep_console_open: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum LoaderType {
  Vanilla,
}

#[derive(Error, Debug)]
enum ProfileError {
  #[error("NotFound")]
  NotFound,
  #[error("InvalidImage")]
  InvalidImage,
}

impl ProfileStore {
  const PROFILE_KEY: &str = "profiles";
  const PROFILE_DIR: &str = "profiles";
  const PROFILE_CONFIG: &str = "profile.json";
  const PROFILE_IMAGE: &str = "image.png";
  const PROFILE_LOGS: &str = "instance_logs";

  pub fn new(handle: AppHandle) -> Result<ProfileStore> {
    let store = handle.app_store()?;
    let profile_paths: HashMap<String, PathBuf> = store.get_or_default(Self::PROFILE_KEY)?;
    let data_dir = handle.path().app_data_dir()?;

    let mut profiles = HashMap::new();
    for (id, path) in profile_paths {
      let path = path!(&data_dir, &path);
      let Some(watcher) = watch_profile(path.clone(), handle.clone()).ok() else {
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
      Self::PROFILE_DIR,
      profile,
      Self::PROFILE_LOGS
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

  pub fn create_profile(
    &mut self,
    name: String,
    icon: Option<&[u8]>,
    version: String,
    loader: LoaderType,
    loader_version: Option<String>,
  ) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let relative_path = path!(Self::PROFILE_DIR, &id);
    let path = path!(&self.data_dir, &relative_path);

    let icon = match icon {
      Some(icon) => {
        let Some(icon) = image::load_from_memory(icon).ok() else {
          return Err(ProfileError::InvalidImage.into());
        };

        let scaled = icon.resize_to_fill(256, 256, FilterType::Lanczos3);
        let mut cursor = Cursor::new(Vec::new());
        scaled.write_to(&mut cursor, ImageFormat::Png)?;
        Some(cursor.into_inner())
      }
      None => None,
    };

    let profile = Profile {
      id: id.clone(),
      name,
      created_at: Utc::now(),
      last_played: None,
      version,
      loader,
      loader_version,
      downloaded: false,
      use_local_dev: false,
      use_local_game: false,
      use_local_jvm: false,
      game: None,
      jvm: None,
      dev: None,
    };

    fs::create_dir_all(&path)?;

    let stop = watch_profile(path.clone(), self.handle.clone())?;

    write_file(&path!(&path, Self::PROFILE_CONFIG), &profile)?;
    if let Some(icon) = icon {
      fs::write(&path!(&path, Self::PROFILE_IMAGE), icon)?;
    }

    self.profiles.insert(
      id,
      ProfileInfo {
        path: relative_path,
        watcher: stop,
      },
    );
    self.save()?;

    update_data(&self.handle, UpdateType::Profiles);
    Ok(())
  }

  pub fn update_profile(&mut self, profile: &Profile) -> Result<()> {
    let Some(info) = self.profiles.get(&profile.id) else {
      return Err(ProfileError::NotFound.into());
    };
    write_file(
      &path!(&self.data_dir, &info.path, Self::PROFILE_CONFIG),
      profile,
    )?;
    self.save()?;

    update_data(&self.handle, UpdateType::Profiles);

    Ok(())
  }

  pub fn get_profile_icon(&self, profile: &str) -> Result<Option<Vec<u8>>> {
    let Some(info) = self.profiles.get(profile) else {
      return Err(ProfileError::NotFound.into());
    };
    let icon_path = path!(&self.data_dir, &info.path, Self::PROFILE_IMAGE);
    if !icon_path.exists() {
      return Ok(None);
    }
    let icon = fs::read(icon_path)?;
    Ok(Some(icon))
  }

  pub fn get_profile_path(&self, profile: &str) -> Result<PathBuf> {
    let Some(info) = self.profiles.get(profile) else {
      return Err(ProfileError::NotFound.into());
    };
    Ok(path!(&self.data_dir, &info.path))
  }

  pub fn update_profile_icon(&mut self, profile: &str, icon: &[u8]) -> Result<()> {
    if image::load_from_memory(icon).is_err() {
      return Err(ProfileError::InvalidImage.into());
    }

    let Some(info) = self.profiles.get(profile) else {
      return Err(ProfileError::NotFound.into());
    };
    fs::write(
      &path!(&self.data_dir, &info.path, Self::PROFILE_IMAGE),
      icon,
    )?;
    self.save()?;

    update_data(&self.handle, UpdateType::Profiles);

    Ok(())
  }

  pub fn remove_profile(&mut self, id: &str) -> Result<()> {
    let Some(info) = self.profiles.remove(id) else {
      return Err(ProfileError::NotFound.into());
    };

    info.watcher.notify_waiters();

    std::fs::remove_dir_all(path!(&self.data_dir, &info.path))?;
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
        Self::PROFILE_CONFIG
      ))?);
    }

    Ok(profiles)
  }

  pub fn get_profile(&self, profile: &str) -> Result<Profile> {
    let Some(info) = self.profiles.get(profile) else {
      return Err(ProfileError::NotFound.into());
    };
    read_parse_file(&path!(
      &self.data_dir,
      &info.path,
      ProfileStore::PROFILE_CONFIG
    ))
  }

  pub async fn launch_profile(&mut self, info: LaunchInfo, profile: &Profile) -> Result<()> {
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
      quick_play: None,
    })?;

    Instance::create(child, &self.handle, profile, &self.instances).await?;

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
    for entry in fs::read_dir(log_dir)? {
      let entry = entry?;
      if entry.file_type()?.is_file() {
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

    let content = fs::read_to_string(log_file)?;
    Ok(content.lines().map(String::from).collect())
  }
}

impl Profile {
  pub fn relative_to_data(&self) -> PathBuf {
    path!(ProfileStore::PROFILE_DIR, &self.id)
  }
}
