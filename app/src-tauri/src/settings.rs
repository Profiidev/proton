use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Result, Url};

use crate::{
  store::TauriAppStoreExt,
  utils::updater::{UpdateType, update_data},
};

const SETTINGS_KEY: &str = "settings";

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
  sidebar_width: Option<f32>,
  url: Option<Url>,
  #[serde(default)]
  pub minecraft: MinecraftSettings,
}

#[derive(Serialize, Deserialize, Default)]
pub struct MinecraftSettings {
  pub show_snapshots: bool,
  #[serde(default)]
  pub custom_window_size: bool,
  #[serde(default = "default_custom_window_width")]
  pub custom_window_width: u32,
  #[serde(default = "default_custom_window_height")]
  pub custom_window_height: u32,
}

pub fn default_custom_window_width() -> u32 {
  854
}

pub fn default_custom_window_height() -> u32 {
  480
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
pub async fn settings_get(app_handle: AppHandle) -> Result<Settings> {
  let store = app_handle.app_store()?;
  Ok(store.get_or_default(SETTINGS_KEY)?)
}

#[tauri::command]
pub async fn settings_set(app_handle: AppHandle, settings: Settings) -> Result<()> {
  let store = app_handle.app_store()?;
  update_data(&app_handle, UpdateType::Settings);
  Ok(store.set(SETTINGS_KEY, &settings)?)
}
