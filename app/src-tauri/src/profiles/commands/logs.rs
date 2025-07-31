use chrono::{DateTime, Utc};
use log::trace;
use tauri::{Result, State};
use tokio::sync::Mutex;

use crate::{
  profiles::store::ProfileStore,
  utils::{log::ResultLogExt, updater::UpdateType},
};

#[tauri::command]
pub async fn profile_runs_list(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<Vec<DateTime<Utc>>> {
  trace!("Command profile_logs called with profile {profile}");
  let store = state.lock().await;

  let info = store.profile_info(profile).log()?;
  Ok(info.list_runs(store.data_dir()).await.log()?)
}

#[tauri::command]
pub async fn profile_clear_logs(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<()> {
  trace!("Command profile_clear_logs called with profile {profile}");
  let store = state.lock().await;

  let info = store.profile_info(profile).log()?;
  info.clear_logs(store.data_dir()).await.log()?;
  store.update_data(UpdateType::ProfileLogs);

  Ok(())
}

#[tauri::command]
pub async fn profile_logs(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  timestamp: DateTime<Utc>,
) -> Result<Vec<String>> {
  trace!("Command profile_logs_run called with profile {profile} timestamp {timestamp}");
  let store = state.lock().await;

  let info = store.profile_info(profile).log()?;
  Ok(info.logs(store.data_dir(), timestamp).await.log()?)
}
