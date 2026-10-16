use serde::{Deserialize, Serialize};

pub mod java;
pub mod minecraft;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
  pub action: Action,
  pub features: Option<Features>,
  pub os: Option<Os>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Features {
  pub is_demo_user: Option<bool>,
  pub has_custom_resolution: Option<bool>,
  pub is_quick_play_singleplayer: Option<bool>,
  pub is_quick_play_multiplayer: Option<bool>,
  pub has_quick_plays_support: Option<bool>,
  pub is_quick_play_realms: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Os {
  pub name: Option<OsName>,
  pub arch: Option<Arch>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Arch {
  X86,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OsName {
  Windows,
  Osx,
  Linux,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
  Allow,
  Disallow,
}

impl Features {
  pub fn is_superset_of(&self, other: &Features) -> bool {
    (other.is_demo_user.is_none() || self.is_demo_user == other.is_demo_user)
      && (other.has_custom_resolution.is_none()
        || self.has_custom_resolution == other.has_custom_resolution)
      && (other.is_quick_play_singleplayer.is_none()
        || self.is_quick_play_singleplayer == other.is_quick_play_singleplayer)
      && (other.is_quick_play_multiplayer.is_none()
        || self.is_quick_play_multiplayer == other.is_quick_play_multiplayer)
      && (other.has_quick_plays_support.is_none()
        || self.has_quick_plays_support == other.has_quick_plays_support)
      && (other.is_quick_play_realms.is_none()
        || self.is_quick_play_realms == other.is_quick_play_realms)
  }
}
