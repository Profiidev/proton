use std::{collections::HashMap, path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Notify;

use crate::versions::{launch::QuickPlay, loader::LoaderType};

#[derive(Clone)]
pub struct ProfileInfo {
  pub path: PathBuf,
  pub watcher: Arc<Notify>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Profile {
  pub id: String,
  pub name: String,
  pub created_at: DateTime<Utc>,
  pub last_played: Option<DateTime<Utc>>,
  #[serde(default)]
  pub last_played_non_quick_play: Option<DateTime<Utc>>,
  #[serde(default)]
  pub favorite: bool,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayHistoryFavoriteInfo {
  pub profile: Profile,
  pub quick_play: Option<QuickPlayInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuickPlayInfo {
  pub id: String,
  pub name: String,
  #[serde(rename = "lastPlayedTime")]
  pub last_played_time: DateTime<Utc>,
  #[serde(default)]
  pub favorite: bool,
  #[serde(default)]
  pub history: bool,
  pub r#type: QuickPlayType,
}

impl PartialEq for QuickPlayInfo {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.r#type == other.r#type
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuickPlayType {
  Singleplayer,
  Multiplayer,
  Realms,
}

impl From<QuickPlayInfo> for QuickPlay {
  fn from(info: QuickPlayInfo) -> Self {
    let id = info.id;
    match info.r#type {
      QuickPlayType::Singleplayer => QuickPlay::Singleplayer { world_name: id },
      QuickPlayType::Multiplayer => QuickPlay::Multiplayer { uri: id },
      QuickPlayType::Realms => QuickPlay::Realms { realm_id: id },
    }
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProfileUpdate {
  pub id: String,
  pub name: String,
  pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameSettings {
  pub width: usize,
  pub height: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JvmSettings {
  pub args: Vec<String>,
  pub env_vars: HashMap<String, String>,
  pub mem_min: usize,
  pub mem_max: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DevSettings {
  pub show_console: bool,
  pub keep_console_open: bool,
}

#[derive(Error, Debug)]
pub enum ProfileError {
  #[error("NotFound")]
  NotFound,
  #[error("InvalidImage")]
  InvalidImage,
}
