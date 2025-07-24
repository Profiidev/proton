use base64::prelude::*;
use chrono::{DateTime, Utc};
use log::trace;
use tauri::{AppHandle, Result, State};
use tauri_plugin_opener::OpenerExt;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
  account::store::AccountStore,
  profiles::{instance::InstanceInfo, store::ProfileUpdate},
  utils::log::ResultLogExt,
  versions::store::McVersionStore,
};

use super::store::{LoaderType, Profile, ProfileStore};

#[derive(Error, Debug)]
enum LaunchError {
  #[error("No Account found")]
  NoAccountFound,
}

#[tauri::command]
pub async fn profile_create(
  state: State<'_, Mutex<ProfileStore>>,
  name: String,
  icon: Option<Vec<u8>>,
  version: String,
  loader: LoaderType,
  loader_version: Option<String>,
) -> Result<()> {
  trace!(
    "Command profile_create called with name {} version {} loader {:?} loader_version {:?}",
    &name,
    &version,
    &loader,
    &loader_version
  );
  let mut store = state.lock().await;
  store
    .create_profile(name, icon.as_deref(), version, loader, loader_version)
    .log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_update(
  state: State<'_, Mutex<ProfileStore>>,
  profile: ProfileUpdate,
) -> Result<()> {
  trace!("Command profile_update called with profile {:?}", &profile);
  let mut store = state.lock().await;

  let mut current_profile = store.get_profile(&profile.id).log()?;

  current_profile.name = profile.name;
  current_profile.version = profile.version;

  store.update_profile(&current_profile).log()?;

  Ok(())
}

#[tauri::command]
pub async fn profile_get_icon(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<Option<String>> {
  trace!("Command profile_get_icon called with profile {profile}");
  let store = state.lock().await;
  Ok(
    store
      .get_profile_icon(profile)?
      .map(|data| BASE64_STANDARD.encode(data)),
  )
}

#[tauri::command]
pub async fn profile_open_path(
  handle: AppHandle,
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<()> {
  trace!("Command profile_get_path called with profile {profile}");
  let store = state.lock().await;
  let path = store
    .get_profile_path(profile)?
    .to_string_lossy()
    .to_string();
  handle
    .opener()
    .open_path(path, None::<&str>)
    .map_err(anyhow::Error::from)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_update_icon(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  icon: Vec<u8>,
) -> Result<()> {
  trace!("Command profile_update_icon called with profile {profile}");
  let mut store = state.lock().await;
  store.update_profile_icon(profile, &icon).log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_remove(state: State<'_, Mutex<ProfileStore>>, profile: &str) -> Result<()> {
  trace!("Command profile_remove called with profile {profile}");
  let mut store = state.lock().await;
  store.remove_profile(profile).log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_list(state: State<'_, Mutex<ProfileStore>>) -> Result<Vec<Profile>> {
  trace!("Command profile_list called");
  let store = state.lock().await;
  Ok(store.list_profiles().log()?)
}

#[tauri::command]
pub async fn profile_launch(
  state: State<'_, Mutex<ProfileStore>>,
  versions: State<'_, Mutex<McVersionStore>>,
  auth: State<'_, Mutex<AccountStore>>,
  profile: &str,
  id: usize,
) -> Result<()> {
  trace!("Command profile_launch called with profile {profile} id {id}");
  let mut store = state.lock().await;
  let mc_store = versions.lock().await;
  let auth_store = auth.lock().await;

  let Some(info) = auth_store.launch_info(auth_store.active()) else {
    let err: anyhow::Result<()> = Err(LaunchError::NoAccountFound.into()).log();
    return Ok(err?);
  };

  let mut profile = store.get_profile(profile).log()?;
  if !profile.downloaded {
    mc_store
      .check_or_download(&profile.version, id)
      .await
      .log()?;
    profile.downloaded = true;
    store.update_profile(&profile).log()?;
  } else if !mc_store.check_meta(&profile.version, id).log()? {
    mc_store.check_or_download(&profile.version, id).await?;
  }

  store.launch_profile(info, &profile).await.log()?;

  profile.last_played = Some(Utc::now());
  store.update_profile(&profile).log()?;

  Ok(())
}

#[tauri::command]
pub async fn profile_repair(
  state: State<'_, Mutex<ProfileStore>>,
  versions: State<'_, Mutex<McVersionStore>>,
  profile: &str,
  id: usize,
) -> Result<()> {
  trace!("Command profile_repair called with profile {profile} id {id}");
  let store = state.lock().await;
  let mc_store = versions.lock().await;

  let profile = store.get_profile(profile).log()?;
  mc_store
    .check_or_download(&profile.version, id)
    .await
    .log()?;

  Ok(())
}

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
pub async fn profile_runs_list(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<Vec<DateTime<Utc>>> {
  trace!("Command profile_logs called with profile {profile}");
  let store = state.lock().await;
  Ok(store.list_profile_runs(profile).await.log()?)
}

#[tauri::command]
pub async fn profile_logs(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  timestamp: DateTime<Utc>,
) -> Result<Vec<String>> {
  trace!("Command profile_logs_run called with profile {profile} timestamp {timestamp}");
  let store = state.lock().await;
  Ok(store.profile_logs(profile, timestamp).await.log()?)
}
