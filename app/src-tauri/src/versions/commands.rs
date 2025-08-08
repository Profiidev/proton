use log::trace;
use tauri::{Result, State};
use tokio::sync::Mutex;

use crate::versions::loader::LoaderType;

use super::store::McVersionStore;

#[tauri::command]
pub async fn version_list(
  state: State<'_, Mutex<McVersionStore>>,
  loader: LoaderType,
) -> Result<Vec<String>> {
  trace!("Command version_list called");
  let store = state.lock().await;
  Ok(store.list_versions(&loader).await?)
}
