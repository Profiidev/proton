use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;

use crate::{
  path,
  store::TauriAppStoreExt,
  utils::{
    file::{read_parse_file, write_file},
    updater::{update_data, UpdateType},
  },
};

pub struct ProfileStore {
  profiles: HashMap<String, PathBuf>,
  handle: AppHandle,
}

#[derive(Serialize, Deserialize)]
pub struct Profile {
  pub id: String,
  pub name: String,
  pub version: String,
  pub loader: LoaderType,
  pub loader_version: Option<String>,
  pub use_local_game: bool,
  pub game: Option<GameSettings>,
  pub use_local_jvm: bool,
  pub jvm: Option<JvmSettings>,
  pub use_local_dev: bool,
  pub dev: Option<DevSettings>,
}

#[derive(Serialize, Deserialize)]
pub struct GameSettings {
  pub width: usize,
  pub height: usize,
}

#[derive(Serialize, Deserialize)]
pub struct JvmSettings {
  pub args: Vec<String>,
  pub env_vars: HashMap<String, String>,
  pub mem_min: usize,
  pub mem_max: usize,
}

#[derive(Serialize, Deserialize)]
pub struct DevSettings {
  pub show_console: bool,
  pub keep_console_open: bool,
}

#[derive(Serialize, Deserialize)]
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

  pub fn new(handle: AppHandle) -> Result<ProfileStore> {
    let store = handle.app_store()?;
    let profiles = store.get_or_default(Self::PROFILE_KEY)?;

    Ok(ProfileStore { profiles, handle })
  }

  fn save(&self) -> Result<()> {
    let store = self.handle.app_store()?;
    store.set(Self::PROFILE_KEY, &self.profiles)
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
    let path = path!(self.handle.path().app_data_dir()?, Self::PROFILE_DIR, &id);

    if let Some(icon) = icon {
      if image::load_from_memory(icon).is_err() {
        return Err(ProfileError::InvalidImage.into());
      }
    }

    let profile = Profile {
      id: id.clone(),
      name,
      version,
      loader,
      loader_version,
      use_local_dev: false,
      use_local_game: false,
      use_local_jvm: false,
      game: None,
      jvm: None,
      dev: None,
    };

    fs::create_dir_all(&path)?;
    write_file(&path!(&path, Self::PROFILE_CONFIG), &profile)?;
    if let Some(icon) = icon {
      fs::write(&path!(&path, Self::PROFILE_IMAGE), icon)?;
    }

    self.profiles.insert(id, path);
    self.save()?;

    update_data(&self.handle, UpdateType::Profiles);
    Ok(())
  }

  pub fn update_profile(&mut self, profile: &Profile) -> Result<()> {
    let Some(path) = self.profiles.get(&profile.id) else {
      return Err(ProfileError::NotFound.into());
    };
    write_file(&path!(path, Self::PROFILE_CONFIG), profile)?;
    self.save()?;

    update_data(&self.handle, UpdateType::Profiles);

    Ok(())
  }

  pub fn update_profile_icon(&mut self, profile: &str, icon: &[u8]) -> Result<()> {
    if image::load_from_memory(icon).is_err() {
      return Err(ProfileError::InvalidImage.into());
    }

    let Some(path) = self.profiles.get(profile) else {
      return Err(ProfileError::NotFound.into());
    };
    fs::write(&path!(&path, Self::PROFILE_IMAGE), icon)?;
    self.save()?;

    update_data(&self.handle, UpdateType::Profiles);

    Ok(())
  }

  pub fn remove_profile(&mut self, id: &str) -> Result<()> {
    let Some(path) = self.profiles.remove(id) else {
      return Err(ProfileError::NotFound.into());
    };
    std::fs::remove_dir_all(path)?;
    self.save()?;

    update_data(&self.handle, UpdateType::Profiles);

    Ok(())
  }

  pub fn list_profiles(&self) -> Result<Vec<Profile>> {
    let mut profiles = Vec::new();
    for path in self.profiles.values() {
      profiles.push(read_parse_file(&path!(path, Self::PROFILE_CONFIG))?);
    }

    Ok(profiles)
  }

  pub fn get_profile(&self, profile: &str) -> Result<Profile> {
    let Some(path) = self.profiles.get(profile) else {
      return Err(ProfileError::NotFound.into());
    };
    read_parse_file(&path!(path, ProfileStore::PROFILE_CONFIG))
  }
}

impl Profile {
  pub fn relative_to_data(&self) -> PathBuf {
    path!(ProfileStore::PROFILE_DIR, &self.id)
  }
}
