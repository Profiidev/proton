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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
