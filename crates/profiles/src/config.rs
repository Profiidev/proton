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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::{OLD_PROFILE, quick_play};

  #[test]
  fn old_profiles_load_with_defaults_and_roundtrip() {
    let profile: Profile = serde_json::from_str(OLD_PROFILE).unwrap();
    assert_eq!(profile.last_played_non_quick_play, None);
    assert!(!profile.favorite);
    assert!(profile.modpack.is_none());
    let q = &profile.quick_play[0];
    assert!(!q.favorite && !q.history);
    assert_eq!(q.r#type, QuickPlayType::Singleplayer);

    let json = serde_json::to_value(&profile).unwrap();
    // the game's quick play file uses camelCase for this one key
    assert!(json["quick_play"][0].get("lastPlayedTime").is_some());
    let back: Profile = serde_json::from_value(json).unwrap();
    assert_eq!(back.created_at, profile.created_at);
  }

  #[test]
  fn modpack_ref_and_source_use_stable_names() {
    let json = r#"{"source":"curseforge","projectId":"1","versionId":"2"}"#;
    let pack: ModpackRef = serde_json::from_str(json).unwrap();
    assert_eq!(pack.source, Source::CurseForge);
    assert_eq!(serde_json::to_string(&pack).unwrap(), json);
    assert_eq!(Source::Modrinth.name(), "Modrinth");
    assert_eq!(Source::CurseForge.name(), "CurseForge");
    assert!(serde_json::from_str::<Source>(r#""Modrinth""#).is_err());
  }

  #[test]
  fn quick_play_types_parse_and_unknown_ones_fail() {
    for (raw, expected) in [
      ("singleplayer", QuickPlayType::Singleplayer),
      ("multiplayer", QuickPlayType::Multiplayer),
      ("realms", QuickPlayType::Realms),
    ] {
      let parsed: QuickPlayType = serde_json::from_str(&format!("\"{raw}\"")).unwrap();
      assert_eq!(parsed, expected);
    }
    assert!(serde_json::from_str::<QuickPlayType>(r#""lan""#).is_err());
  }

  #[test]
  fn quick_play_entries_are_equal_by_id_and_type_only() {
    let a = quick_play("x", QuickPlayType::Multiplayer);
    let mut renamed = a.clone();
    renamed.name = "other".into();
    renamed.favorite = true;
    assert_eq!(a, renamed);
    assert_ne!(a, quick_play("x", QuickPlayType::Singleplayer));
    assert_ne!(a, quick_play("y", QuickPlayType::Multiplayer));
  }

  #[test]
  fn quick_play_converts_to_the_launch_target() {
    let target = |t| QuickPlay::from(quick_play("id", t));
    assert!(matches!(
      target(QuickPlayType::Singleplayer),
      QuickPlay::Singleplayer { world_name } if world_name == "id"
    ));
    assert!(matches!(
      target(QuickPlayType::Multiplayer),
      QuickPlay::Multiplayer { uri } if uri == "id"
    ));
    assert!(matches!(
      target(QuickPlayType::Realms),
      QuickPlay::Realms { realm_id } if realm_id == "id"
    ));
  }
}
