use std::{collections::HashMap, path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Notify;

use crate::{path, profiles::PROFILE_DIR, versions::launch::QuickPlay};

pub struct ProfileInfo {
  pub path: PathBuf,
  pub watcher: Arc<Notify>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Profile {
  pub id: String,
  pub name: String,
  pub created_at: DateTime<Utc>,
  pub last_played: Option<DateTime<Utc>>,
  #[serde(default)]
  pub quick_play: Vec<QuickPlayInfo>,
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

impl Profile {
  pub fn relative_to_data(&self) -> PathBuf {
    path!(PROFILE_DIR, &self.id)
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QuickPlayInfo {
  Singleplayer {
    id: String,
    name: String,
    #[serde(rename = "lastPlayedTime")]
    last_played_time: DateTime<Utc>,
  },
  Multiplayer {
    id: String,
    name: String,
    #[serde(rename = "lastPlayedTime")]
    last_played_time: DateTime<Utc>,
  },
  Realms {
    id: String,
    name: String,
    #[serde(rename = "lastPlayedTime")]
    last_played_time: DateTime<Utc>,
  },
}

impl QuickPlayInfo {
  pub fn id(&self) -> String {
    match self {
      QuickPlayInfo::Singleplayer { id, .. } => id.clone(),
      QuickPlayInfo::Multiplayer { id, .. } => id.clone(),
      QuickPlayInfo::Realms { id, .. } => id.clone(),
    }
  }

  pub fn is_singleplayer(&self) -> bool {
    matches!(self, QuickPlayInfo::Singleplayer { .. })
  }
}

impl From<QuickPlayInfo> for QuickPlay {
  fn from(info: QuickPlayInfo) -> Self {
    match info {
      QuickPlayInfo::Singleplayer { id, .. } => QuickPlay::Singleplayer { world_name: id },
      QuickPlayInfo::Multiplayer { id, .. } => QuickPlay::Multiplayer { uri: id },
      QuickPlayInfo::Realms { id, .. } => QuickPlay::Realms { realm_id: id },
    }
  }
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
pub enum ProfileError {
  #[error("NotFound")]
  NotFound,
  #[error("InvalidImage")]
  InvalidImage,
}
