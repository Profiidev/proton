use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use anyhow::Result;
use proton_utils::dir;
use serde::{Deserialize, Serialize};
use sysinfo::System;

pub const SETTINGS_KEY: &str = "settings";

/// Total system memory in MB, 8 GB when it cannot be read
pub fn system_max_mem() -> u64 {
  static MAX_MEM: LazyLock<u64> = LazyLock::new(|| {
    let mut system = System::new();
    system.refresh_memory();
    match system.total_memory() / 1024 / 1024 {
      0 => 8192,
      mb => mb,
    }
  });
  *MAX_MEM
}

/// Leaves 2 GB for the OS, the same rule the memory sliders explain
pub fn mem_fits(mb: u64) -> bool {
  mb + 2048 <= system_max_mem()
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Settings {
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
    let settings: Settings = serde_json::from_str(
      r#"{"system_max_mem":0,"sidebar_width":null,"minecraft":{"show_snapshots":true}}"#,
    )
    .unwrap();
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

  const TAURI_SETTINGS: &str = r#"{
    "system_max_mem": 16384,
    "sidebar_width": null,
    "url": "tauri://localhost/home",
    "auto_update": false,
    "hide_window_controls": false,
    "download_dirs": [],
    "minecraft": {
      "show_snapshots": false,
      "game_settings": { "use_custom": false, "width": 854, "height": 480 },
      "jvm_settings": { "args": [], "env_vars": {}, "mem_max": 4096 }
    }
  }"#;

  #[test]
  fn empty_object_is_the_default() {
    let settings: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(settings, Settings::default());
    assert_eq!(settings.minecraft.game_settings.width, 854);
    assert_eq!(settings.minecraft.game_settings.height, 480);
    assert_eq!(settings.minecraft.jvm_settings.mem_max, 2048);
    assert_eq!(settings.window, None);
  }

  #[test]
  fn reads_the_tauri_settings_and_round_trips() {
    let settings: Settings = serde_json::from_str(TAURI_SETTINGS).unwrap();
    assert_eq!(settings.minecraft.jvm_settings.mem_max, 4096);
    // an empty list means "no folders", not "the system Downloads folder"
    assert_eq!(settings.download_dirs, Some(Vec::new()));
    assert!(settings.download_dirs().is_empty());

    let json = serde_json::to_string(&settings).unwrap();
    assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
  }

  #[test]
  fn minecraft_without_sub_settings_uses_their_defaults() {
    let settings: Settings =
      serde_json::from_str(r#"{"minecraft":{"show_snapshots":true}}"#).unwrap();
    assert_eq!(settings.minecraft.game_settings, GameSettings::default());
    assert_eq!(settings.minecraft.jvm_settings, JvmSettings::default());
  }

  #[test]
  fn window_state_without_maximized_is_not_maximized() {
    let settings: Settings =
      serde_json::from_str(r#"{"window":{"x":1,"y":-2.5,"width":800,"height":600}}"#).unwrap();
    let window = settings.window.unwrap();
    assert_eq!((window.x, window.y), (1.0, -2.5));
    assert!(!window.maximized);
  }

  #[test]
  fn malformed_settings_are_an_error() {
    for json in [
      r#"{"auto_update":"yes"}"#,
      r#"{"minecraft":{"jvm_settings":{"mem_max":-1}}}"#,
      r#"{"window":{"x":1}}"#,
      "[]",
    ] {
      assert!(serde_json::from_str::<Settings>(json).is_err(), "{json}");
    }
  }

  #[test]
  fn custom_download_dirs_replace_the_system_one() {
    let dirs = vec![PathBuf::from("/a"), PathBuf::from("/ü/b")];
    let settings = Settings {
      download_dirs: Some(dirs.clone()),
      ..Default::default()
    };
    assert_eq!(settings.download_dirs(), dirs);
    // none falls back to the system folder, if there is one
    assert_eq!(
      Settings::default().download_dirs(),
      dir::download_dir().into_iter().collect::<Vec<_>>()
    );
  }

  #[test]
  fn mem_fits_leaves_two_gb_for_the_os() {
    let max = system_max_mem();
    assert!(max > 0);
    assert_eq!(max, system_max_mem(), "cached");
    if max >= 2048 {
      assert!(mem_fits(max - 2048));
      assert!(!mem_fits(max - 2047));
    }
    assert!(!mem_fits(max));
  }
}
