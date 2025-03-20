use reqwest::Client;
use tauri::{AppHandle, Result, State};
use tokio::sync::Mutex;

use super::store::McVersionStore;

#[tauri::command]
pub async fn versions_download(
  state: State<'_, Mutex<McVersionStore>>,
  client: State<'_, Client>,
  handle: AppHandle,
  id: &str,
) -> Result<()> {
  let store = state.lock().await;

  store.check_or_download(id, client.inner(), &handle).await?;

  Ok(())
}
