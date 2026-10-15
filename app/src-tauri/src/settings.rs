use serde::{Deserialize, Serialize};
use sysinfo::System;
use tauri::{AppHandle, Result, State, Url};

use crate::{
  profiles::config::{GameSettings, JvmSettings},
  store::TauriAppStoreExt,
  utils::updater::{UpdateType, update_data},
};

pub const SETTINGS_KEY: &str = "settings";

pub struct MaxMem {
  max_mem: u64,
}

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
  system_max_mem: u64,
  sidebar_width: Option<f32>,
  pub url: Option<Url>,
  #[serde(default)]
  pub auto_update: bool,
  #[serde(default)]
  pub hide_window_controls: bool,
  #[serde(default)]
  pub minecraft: MinecraftSettings,
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

#[tauri::command]
pub async fn settings_get(app_handle: AppHandle, state: State<'_, MaxMem>) -> Result<Settings> {
  let store = app_handle.app_store()?;
  let mut settings: Settings = store.get_or_default(SETTINGS_KEY)?;
  settings.system_max_mem = state.max_mem;
  Ok(settings)
}

#[tauri::command]
pub async fn settings_set(app_handle: AppHandle, settings: Settings) -> Result<()> {
  let store = app_handle.app_store()?;
  update_data(&app_handle, UpdateType::Settings);
  Ok(store.set(SETTINGS_KEY, &settings)?)
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
