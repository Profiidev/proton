use std::collections::HashMap;

use reqwest::Client;
use tauri::{AppHandle, Result, State};

use super::{
  auth::{get_all_auth_info, ms_mc_login, refresh_all_auth_info, refresh_auth_info},
  info::{get_all_profile_info, refresh_all_profile_info, refresh_profile, ProfileInfo},
};

#[tauri::command]
pub fn account_list(handle: AppHandle) -> Result<HashMap<String, Option<ProfileInfo>>> {
  Ok(get_all_profile_info(&handle)?)
}

#[tauri::command]
pub async fn account_refresh(client: State<'_, Client>, handle: AppHandle) -> Result<()> {
  refresh_all_auth_info(client.inner(), &handle).await?;

  let auth_infos = get_all_auth_info(&handle)?;
  refresh_all_profile_info(client.inner(), &handle, &auth_infos).await?;
  Ok(())
}

#[tauri::command]
pub async fn account_refresh_one(
  client: State<'_, Client>,
  handle: AppHandle,
  id: &str,
) -> Result<()> {
  if let Some(auth_info) = refresh_auth_info(client.inner(), &handle, id).await? {
    refresh_profile(client.inner(), &handle, &auth_info.mc_token).await?;
  };
  Ok(())
}

#[tauri::command]
pub async fn account_login(client: State<'_, Client>, handle: AppHandle) -> Result<()> {
  ms_mc_login(client.inner(), &handle).await?;

  Ok(())
}
