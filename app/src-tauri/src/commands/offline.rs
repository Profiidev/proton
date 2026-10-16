use proton_config::offline::OfflineState;
use tauri::{Result, State};

#[tauri::command]
pub async fn is_offline(state: State<'_, OfflineState>) -> Result<bool> {
  Ok(state.is_offline())
}

#[tauri::command]
pub async fn try_reconnect(state: State<'_, OfflineState>) -> Result<bool> {
  Ok(state.check_online_state().await)
}
