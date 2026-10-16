use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use sysinfo::System;
use tauri::{AppHandle, Manager, Url};

use proton_store::TauriAppStoreExt;

pub const SETTINGS_KEY: &str = "settings";

pub struct MaxMem {
  pub max_mem: u64,
}

#[allow(clippy::new_without_default)]
impl MaxMem {
  /// Leaves 2 GB for the OS, the same rule the memory sliders explain
  pub fn fits(&self, mb: u64) -> bool {
    mb + 2048 <= self.max_mem
  }

  pub fn new() -> Self {
    let system = System::new_all();
    let bytes = system.total_memory();
    let mb = bytes / 1024 / 1024;
    MaxMem { max_mem: mb }
  }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
  #[serde(default)]
  pub system_max_mem: u64,
  sidebar_width: Option<f32>,
  pub url: Option<Url>,
  #[serde(default)]
  pub auto_update: bool,
  #[serde(default)]
  pub hide_window_controls: bool,
  #[serde(default)]
  pub minecraft: MinecraftSettings,
  /// Folders checked for manual downloads, `None` means the system Downloads folder
  #[serde(default)]
  pub download_dirs: Option<Vec<PathBuf>>,
}

impl Settings {
  pub fn download_dirs(&self, handle: &AppHandle) -> Vec<PathBuf> {
    self
      .download_dirs
      .clone()
      .unwrap_or_else(|| handle.path().download_dir().into_iter().collect())
  }
}

#[derive(Serialize, Deserialize, Default)]
pub struct MinecraftSettings {
  pub show_snapshots: bool,
  #[serde(default)]
  pub game_settings: GameSettings,
  #[serde(default)]
  pub jvm_settings: JvmSettings,
}

pub trait SettingsExt {
  fn app_settings(&self) -> anyhow::Result<Settings>;
}

impl SettingsExt for AppHandle {
  fn app_settings(&self) -> anyhow::Result<Settings> {
    self.app_store()?.get_or_default(SETTINGS_KEY)
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameSettings {
  pub use_custom: bool,
  pub width: usize,
  pub height: usize,
}

impl Default for GameSettings {
  fn default() -> Self {
    Self {
      use_custom: false,
      width: 854,
      height: 480,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JvmSettings {
  pub args: Vec<String>,
  pub env_vars: HashMap<String, String>,
  // in megabytes
  pub mem_max: usize,
}

impl Default for JvmSettings {
  fn default() -> Self {
    Self {
      args: Vec::new(),
      env_vars: HashMap::new(),
      mem_max: 2 * 1024, // 2 GB
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn settings_without_auto_update_default_false() {
    let settings: Settings =
      serde_json::from_str(r#"{"system_max_mem":0,"minecraft":{"show_snapshots":true}}"#).unwrap();
    assert!(!settings.auto_update);
    assert!(!settings.hide_window_controls);
    assert!(settings.minecraft.show_snapshots);

    let settings: Settings = serde_json::from_str(r#"{"auto_update":true}"#).unwrap();
    assert!(settings.auto_update);
  }
}
