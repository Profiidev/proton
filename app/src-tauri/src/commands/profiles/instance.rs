use log::trace;
use tauri::{Result, State};
use tokio::sync::Mutex;

use proton_profiles::{instance::InstanceInfo, store::ProfileStore};
use proton_utils::log::ResultLogExt;

#[tauri::command]
pub async fn instance_list(state: State<'_, Mutex<ProfileStore>>) -> Result<Vec<InstanceInfo>> {
  trace!("Command instance_list called");
  let store = state.lock().await;
  Ok(store.list_instances().await)
}

#[tauri::command]
pub async fn instance_logs(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  id: &str,
) -> Result<Vec<String>> {
  trace!("Command instance_logs called with profile {profile} id {id}");
  let store = state.lock().await;
  let lines = store.get_instance_logs(profile, id).await.log()?;
  Ok(lines)
}

#[tauri::command]
pub async fn instance_stop(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  id: &str,
) -> Result<()> {
  trace!("Command instance_stop called with profile {profile} id {id}");
  let store = state.lock().await;
  store.stop_instance(profile, id).await.log()?;
  Ok(())
}

#[tauri::command]
pub async fn instance_stop_all(
  state: State<'_, Mutex<ProfileStore>>,
  profile: Option<String>,
) -> Result<()> {
  trace!("Command instance_stop_all called with profile {profile:?}");
  let store = state.lock().await;
  store.stop_all_instances(profile.as_deref()).await;
  Ok(())
}
