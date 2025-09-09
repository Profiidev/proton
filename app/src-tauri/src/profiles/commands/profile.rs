use base64::prelude::*;
use chrono::Utc;
use log::trace;
use tauri::{AppHandle, Result, State};
use tauri_plugin_opener::OpenerExt;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
  account::store::AccountStore,
  offline::OfflineResultExt,
  profiles::{
    config::{Profile, ProfileUpdate, QuickPlayInfo},
    store::ProfileStore,
  },
  utils::{log::ResultLogExt, updater::UpdateType},
  versions::{loader::LoaderType, store::McVersionStore},
};

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
) -> Result<()> {
  trace!(
    "Command profile_create called with name {} version {} loader {:?}",
    &name, &version, &loader,
  );
  let mut store = state.lock().await;

  store
    .create_profile(name, icon.as_deref(), version, loader)
    .await
    .log()?;
  store.update_data(UpdateType::Profiles);

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

  if profile.version != current_profile.version
    || profile.loader_version != current_profile.loader_version
  {
    current_profile.downloaded = false;
  }
  current_profile.name = profile.name;
  current_profile.version = profile.version;
  current_profile.loader_version = profile.loader_version;
  current_profile.use_local_game = profile.use_local_game;
  current_profile.game = profile.game;
  current_profile.use_local_jvm = profile.use_local_jvm;
  current_profile.jvm = profile.jvm;

  current_profile.update(store.data_dir()).await.log()?;
  store.update_data(UpdateType::Profiles);

  Ok(())
}

#[tauri::command]
pub async fn profile_get_icon(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<Option<String>> {
  trace!("Command profile_get_icon called with profile {profile}");
  let store = state.lock().await;

  let info = store.profile_info(profile).log()?;
  let icon = info.get_icon(store.data_dir()).await.log()?;
  Ok(icon.map(|data| BASE64_STANDARD.encode(data)))
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

  let info = store.profile_info(profile).log()?;
  info.update_icon(&icon, store.data_dir()).await.log()?;
  store.update_data(UpdateType::Profiles);

  Ok(())
}

#[tauri::command]
pub async fn profile_remove(state: State<'_, Mutex<ProfileStore>>, profile: &str) -> Result<()> {
  trace!("Command profile_remove called with profile {profile}");
  let mut store = state.lock().await;

  store.remove_profile(profile).await.log()?;
  store.update_data(UpdateType::Profiles);

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
  let store = state.lock().await;
  // clone so the lock is dropped before the download
  let mc_store = versions.lock().await.clone();
  let auth_store = auth.lock().await;

  let Some(info) = auth_store.launch_info(auth_store.active()) else {
    let err: anyhow::Result<()> = Err(LaunchError::NoAccountFound.into()).log();
    return Ok(err?);
  };
  drop(auth_store);

  let mut profile = store.profile(profile).await.log()?;
  drop(store);

  if !profile.downloaded {
    mc_store
      .check_or_download(
        &profile.version,
        id,
        profile.loader,
        profile.loader_version.clone(),
      )
      .await
      .check_online_state(mc_store.handle())
      .await?;
    profile.downloaded = true;
  } else if !mc_store.check_meta(&profile.version, id).await.log()? {
    mc_store
      .check_or_download(
        &profile.version,
        id,
        profile.loader,
        profile.loader_version.clone(),
      )
      .await
      .check_online_state(mc_store.handle())
      .await?;
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
    profile.last_played_non_quick_play = Some(Utc::now());
  }

  let mut store = state.lock().await;
  profile.update(store.data_dir()).await.log()?;
  store.update_data(UpdateType::Profiles);

  store
    .launch_profile(info, profile, quick_play)
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
  // clone so the lock is dropped before the download
  let mc_store = versions.lock().await.clone();

  let mut profile = store.profile(profile).await.log()?;
  let data_dir = store.data_dir().clone();
  let handle = store.handle().clone();
  drop(store);

  // check online state if err because this requires internet and can indicate offline state
  mc_store
    .check_or_download(
      &profile.version,
      id,
      profile.loader,
      profile.loader_version.clone(),
    )
    .await
    .check_online_state(&handle)
    .await?;

  profile.downloaded = true;
  profile.update(&data_dir).await.log()?;

  Ok(())
}

#[tauri::command]
pub async fn profile_cancel_download(
  state: State<'_, Mutex<McVersionStore>>,
  id: usize,
) -> Result<()> {
  let store = state.lock().await;
  store.cancel_check_or_download(id).await;
  Ok(())
}
