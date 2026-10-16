use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use anyhow::Result;
use proton_utils::dir;
use serde::{Deserialize, Serialize};
use sysinfo::System;

pub const SETTINGS_KEY: &str = "settings";

/// Total system memory in MB
pub fn system_max_mem() -> u64 {
  static MAX_MEM: LazyLock<u64> = LazyLock::new(|| {
    let mut system = System::new();
    system.refresh_memory();
    system.total_memory() / 1024 / 1024
  });
  *MAX_MEM
}

/// Leaves 2 GB for the OS, the same rule the memory sliders explain
pub fn mem_fits(mb: u64) -> bool {
  mb + 2048 <= system_max_mem()
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Settings {
  #[serde(default)]
  pub system_max_mem: u64,
  pub sidebar_width: Option<f32>,
  /// The last route, the UI owns the format. The tauri app stored a full URL here.
  pub url: Option<String>,
  #[serde(default)]
  pub auto_update: bool,
  #[serde(default)]
  pub hide_window_controls: bool,
  #[serde(default)]
  pub minecraft: MinecraftSettings,
  /// Folders checked for manual downloads, `None` means the system Downloads folder
  #[serde(default)]
  pub download_dirs: Option<Vec<PathBuf>>,
  /// Main window bounds, `None` until the window was moved or resized once
  #[serde(default)]
  pub window: Option<WindowState>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct WindowState {
  pub x: f32,
  pub y: f32,
  pub width: f32,
  pub height: f32,
  #[serde(default)]
  pub maximized: bool,
}

impl Settings {
  /// Reads the stored settings. gpui code reads `cx.settings()` instead, this is for code
  /// without a gpui context.
  pub fn load() -> Result<Self> {
    proton_store::store().get_or_default(SETTINGS_KEY)
  }

  pub fn save(&self) -> Result<()> {
    proton_store::store().set(SETTINGS_KEY, self)
  }

  pub fn download_dirs(&self) -> Vec<PathBuf> {
    self
      .download_dirs
      .clone()
      .unwrap_or_else(|| dir::download_dir().into_iter().collect())
  }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct MinecraftSettings {
  pub show_snapshots: bool,
  #[serde(default)]
  pub game_settings: GameSettings,
  #[serde(default)]
  pub jvm_settings: JvmSettings,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

  #[test]
  fn reads_the_tauri_url() {
    let settings: Settings =
      serde_json::from_str(r#"{"url":"tauri://localhost/profiles?id=1"}"#).unwrap();
    assert_eq!(
      settings.url.as_deref(),
      Some("tauri://localhost/profiles?id=1")
    );
  }
}
