use proton_config::settings::{MaxMem, SETTINGS_KEY, Settings};
use proton_store::TauriAppStoreExt;
use proton_utils::updater::{UpdateType, update_data};
use tauri::{AppHandle, Result, State};

#[tauri::command]
pub async fn settings_get(app_handle: AppHandle, state: State<'_, MaxMem>) -> Result<Settings> {
  let store = app_handle.app_store()?;
  let mut settings: Settings = store.get_or_default(SETTINGS_KEY)?;
  settings.system_max_mem = state.max_mem;
  settings.download_dirs = Some(settings.download_dirs(&app_handle));
  Ok(settings)
}

#[tauri::command]
pub async fn settings_set(app_handle: AppHandle, settings: Settings) -> Result<()> {
  let store = app_handle.app_store()?;
  update_data(&app_handle, UpdateType::Settings);
  Ok(store.set(SETTINGS_KEY, &settings)?)
}
