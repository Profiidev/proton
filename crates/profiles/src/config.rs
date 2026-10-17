use std::{path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Notify;

pub use proton_config::settings::{GameSettings, JvmSettings};
use proton_versions::{launch::QuickPlay, loader::LoaderType};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Source {
  Modrinth,
  CurseForge,
}

impl Source {
  pub fn name(self) -> &'static str {
    match self {
      Source::Modrinth => "Modrinth",
      Source::CurseForge => "CurseForge",
    }
  }
}

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
  #[serde(default)]
  pub modpack: Option<ModpackRef>,
}

/// The modpack version a profile was installed from
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModpackRef {
  pub source: Source,
  pub project_id: String,
  pub version_id: String,
}

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub struct ProfileUpdate {
  pub id: String,
  pub name: String,
  pub version: String,
  pub loader_version: Option<String>,
  pub use_local_game: bool,
  pub game: Option<GameSettings>,
  pub use_local_jvm: bool,
  pub jvm: Option<JvmSettings>,
}

#[derive(Error, Debug)]
pub enum ProfileError {
  #[error("Profile not found")]
  NotFound,
  #[error("Invalid image")]
  InvalidImage,
}
