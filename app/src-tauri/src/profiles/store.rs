use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::store::TauriAppStoreExt;

const PROFILE_KEY: &str = "profiles";

pub struct ProfileStore {
  profiles: Vec<String>,
  handle: AppHandle,
}

#[derive(Serialize, Deserialize)]
struct Profile {
  name: String,
  icon: String,
  version: String,
  loader: LoaderType,
  loader_version: Option<String>,
  use_local_game: bool,
  game: Option<GameSettings>,
  use_local_jvm: bool,
  jvm: Option<JvmSettings>,
  use_local_dev: bool,
  dev: Option<DevSettings>,
}

#[derive(Serialize, Deserialize)]
struct GameSettings {
  width: usize,
  height: usize,
}

#[derive(Serialize, Deserialize)]
struct JvmSettings {
  args: Vec<String>,
  env_vars: HashMap<String, String>,
  mem_min: usize,
  mem_max: usize,
}

#[derive(Serialize, Deserialize)]
struct DevSettings {
  show_console: bool,
  keep_console_open: bool,
}

#[derive(Serialize, Deserialize)]
enum LoaderType {
  Vanilla,
}

impl ProfileStore {
  pub fn new(handle: AppHandle) -> Result<ProfileStore> {
    let store = handle.app_store()?;
    let profiles = store.get_or_default(PROFILE_KEY)?;

    Ok(ProfileStore { profiles, handle })
  }

  fn save(&self) -> Result<()> {
    let store = self.handle.app_store()?;
    store.set(PROFILE_KEY, &self.profiles)
  }
}
