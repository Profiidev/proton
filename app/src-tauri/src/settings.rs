use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Result};

use crate::{
  store::TauriAppStoreExt,
  utils::updater::{update_data, UpdateType},
};

const SETTINGS_KEY: &str = "settings";

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
  sidebar_width: Option<f32>,
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
