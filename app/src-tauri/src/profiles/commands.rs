use base64::prelude::*;
use chrono::{DateTime, Utc};
use log::trace;
use tauri::{AppHandle, Result, State};
use tauri_plugin_opener::OpenerExt;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
  account::store::AccountStore,
  profiles::{
    config::{LoaderType, PlayHistoryFavoriteInfo, Profile, ProfileUpdate, QuickPlayInfo},
    instance::InstanceInfo,
  },
  utils::log::ResultLogExt,
  versions::store::McVersionStore,
};

use super::store::ProfileStore;

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
    .await
    .log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_update(
  state: State<'_, Mutex<ProfileStore>>,
  profile: ProfileUpdate,
) -> Result<()> {
  trace!("Command profile_update called with profile {:?}", &profile);
  let store = state.lock().await;

  let mut current_profile = store.profile(&profile.id).await.log()?;

  current_profile.name = profile.name;
  current_profile.version = profile.version;

  current_profile
    .update(store.data_dir(), store.app())
    .await
    .log()?;

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
      .profile_info(profile)
      .log()?
      .get_icon(store.data_dir())
      .await
      .log()?
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
  let store = state.lock().await;
  store
    .profile_info(profile)
    .log()?
    .update_icon(&icon, store.data_dir(), store.app())
    .await
    .log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_remove(state: State<'_, Mutex<ProfileStore>>, profile: &str) -> Result<()> {
  trace!("Command profile_remove called with profile {profile}");
  let mut store = state.lock().await;
  store.remove_profile(profile).await.log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_list(state: State<'_, Mutex<ProfileStore>>) -> Result<Vec<Profile>> {
  trace!("Command profile_list called");
  let store = state.lock().await;
  Ok(store.list_profiles().await.log()?)
}

#[tauri::command]
pub async fn profile_launch(
  state: State<'_, Mutex<ProfileStore>>,
  versions: State<'_, Mutex<McVersionStore>>,
  auth: State<'_, Mutex<AccountStore>>,
  profile: &str,
  id: usize,
  quick_play: Option<QuickPlayInfo>,
) -> Result<()> {
  trace!("Command profile_launch called with profile {profile} id {id}");
  let mut store = state.lock().await;
  let mc_store = versions.lock().await;
  let auth_store = auth.lock().await;

  let Some(info) = auth_store.launch_info(auth_store.active()) else {
    let err: anyhow::Result<()> = Err(LaunchError::NoAccountFound.into()).log();
    return Ok(err?);
  };

  let mut profile = store.profile(profile).await.log()?;
  if !profile.downloaded {
    mc_store
      .check_or_download(&profile.version, id)
      .await
      .log()?;
    profile.downloaded = true;
  } else if !mc_store.check_meta(&profile.version, id).await.log()? {
    mc_store.check_or_download(&profile.version, id).await?;
  }

  profile.last_played = Some(Utc::now());
  if let Some(quick_play) = &quick_play {
    if let Some(item) = profile
      .quick_play
      .iter_mut()
      .find(|q| q.id == quick_play.id && q.r#type == quick_play.r#type)
    {
      item.last_played_time = Utc::now();
      item.history = true;
    }
  } else {
    profile.history = true;
  }
  profile.update(store.data_dir(), store.app()).await.log()?;

  store
    .launch_profile(info, &profile, quick_play)
    .await
    .log()?;

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

  let profile = store.profile(profile).await.log()?;
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
  Ok(
    store
      .profile_info(profile)
      .log()?
      .list_runs(store.data_dir())
      .await
      .log()?,
  )
}

#[tauri::command]
pub async fn profile_clear_logs(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<()> {
  trace!("Command profile_clear_logs called with profile {profile}");
  let store = state.lock().await;
  store
    .profile_info(profile)
    .log()?
    .clear_logs(store.data_dir())
    .await
    .log()?;
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
  Ok(
    store
      .profile_info(profile)
      .log()?
      .logs(store.data_dir(), timestamp)
      .await
      .log()?,
  )
}

#[tauri::command]
pub async fn profile_quick_play_list(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<Vec<QuickPlayInfo>> {
  trace!("Command profile_quick_play_list called with profile {profile}");
  let store = state.lock().await;
  Ok(
    store
      .profile(profile)
      .await
      .log()?
      .list_quick_play(store.data_dir(), store.app())
      .await
      .log()?,
  )
}

#[tauri::command]
pub async fn profile_quick_play_remove(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  quick_play: QuickPlayInfo,
) -> Result<()> {
  trace!(
    "Command profile_quick_play_remove called with profile {profile} quick_play {quick_play:?}"
  );
  let store = state.lock().await;
  store
    .profile(profile)
    .await
    .log()?
    .remove_quick_play(quick_play, store.data_dir(), store.app())
    .await
    .log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_favorites_list(
  state: State<'_, Mutex<ProfileStore>>,
) -> Result<Vec<PlayHistoryFavoriteInfo>> {
  trace!("Command profile_favorites_list called");
  let mut store = state.lock().await;
  Ok(store.list_favorites().await.log()?)
}

#[tauri::command]
pub async fn profile_favorites_add(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  quick_play: Option<QuickPlayInfo>,
) -> Result<()> {
  trace!("Command profile_favorites_add called with profile {profile} quick_play {quick_play:?}");
  let store = state.lock().await;
  store
    .profile(profile)
    .await
    .log()?
    .set_favorite(quick_play, true, store.data_dir(), store.app())
    .await
    .log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_favorites_remove(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  quick_play: Option<QuickPlayInfo>,
) -> Result<()> {
  trace!(
    "Command profile_favorites_remove called with profile {profile} quick_play {quick_play:?}"
  );
  let store = state.lock().await;
  store
    .profile(profile)
    .await
    .log()?
    .set_favorite(quick_play, false, store.data_dir(), store.app())
    .await
    .log()?;
  Ok(())
}

#[tauri::command]
pub async fn profile_history_list(
  state: State<'_, Mutex<ProfileStore>>,
) -> Result<Vec<PlayHistoryFavoriteInfo>> {
  trace!("Command profile_history_list called");
  let mut store = state.lock().await;
  Ok(store.list_history().await.log()?)
}
