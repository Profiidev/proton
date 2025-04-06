use log::trace;
use tauri::{Result, State};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
  account::store::AccountStore, utils::log::ResultLogExt, versions::store::McVersionStore,
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
  trace!("Command profile_create called");
  let mut store = state.lock().await;
  store.create_profile(name, icon.as_deref(), version, loader, loader_version)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_update(state: State<'_, Mutex<ProfileStore>>, profile: Profile) -> Result<()> {
  trace!("Command profile_update called");
  let mut store = state.lock().await;
  store.update_profile(&profile)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_update_icon(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  icon: Vec<u8>,
) -> Result<()> {
  trace!("Command profile_update_icon called");
  let mut store = state.lock().await;
  store.update_profile_icon(profile, &icon)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_remove(state: State<'_, Mutex<ProfileStore>>, profile: &str) -> Result<()> {
  trace!("Command profile_remove called");
  let mut store = state.lock().await;
  store.remove_profile(profile)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_list(state: State<'_, Mutex<ProfileStore>>) -> Result<Vec<Profile>> {
  trace!("Command profile_list called");
  let store = state.lock().await;
  Ok(store.list_profiles()?)
}

#[tauri::command]
pub async fn profile_launch(
  state: State<'_, Mutex<ProfileStore>>,
  versions: State<'_, Mutex<McVersionStore>>,
  auth: State<'_, Mutex<AccountStore>>,
  profile: &str,
) -> Result<()> {
  trace!("Command profile_launch called");
  let mut store = state.lock().await;
  let mc_store = versions.lock().await;
  let auth_store = auth.lock().await;

  let Some(info) = auth_store.launch_info(auth_store.active()) else {
    let err: anyhow::Result<()> = Err(LaunchError::NoAccountFound.into()).log();
    return Ok(err?);
  };

  let mut profile = store.get_profile(profile)?;
  if !profile.downloaded {
    mc_store.check_or_download(&profile.version).await?;
    profile.downloaded = true;
    store.update_profile(&profile)?;
  } else if !mc_store.check_meta(&profile.version)? {
    mc_store.check_or_download(&profile.version).await?;
  }

  mc_store.launch_version(info, &profile)?;

  Ok(())
}
