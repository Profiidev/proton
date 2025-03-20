use std::{collections::HashMap, sync::Arc};

use reqwest::Client;
use tauri::{AppHandle, Result, State, Url};
use tokio::sync::Mutex;

use crate::account::skin_store::{Cape, Skin};

use super::{info::ProfileInfo, skin_store::SkinStore, store::AccountStore};

#[tauri::command]
pub async fn account_list(
  state: State<'_, Mutex<AccountStore>>,
) -> Result<HashMap<String, Option<ProfileInfo>>> {
  let store = state.lock().await;
  Ok(store.list_profiles())
}

#[tauri::command]
pub async fn account_refresh(
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  state: State<'_, Mutex<AccountStore>>,
) -> Result<()> {
  let mut store = state.lock().await;
  store.refresh_all(client.inner(), &handle).await?;

  Ok(())
}

#[tauri::command]
pub async fn account_refresh_one(
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  id: &str,
  state: State<'_, Mutex<AccountStore>>,
) -> Result<()> {
  let mut store = state.lock().await;
  store.refresh(id, client.inner(), &handle).await?;

  Ok(())
}

#[tauri::command]
pub async fn account_login(
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  state: State<'_, Mutex<AccountStore>>,
) -> Result<()> {
  let mut store = state.lock().await;
  store.login(client.inner(), &handle).await?;

  Ok(())
}

#[tauri::command]
pub async fn account_remove(
  state: State<'_, Mutex<AccountStore>>,
  id: &str,
  handle: AppHandle,
) -> Result<()> {
  let mut store = state.lock().await;
  store.remove_account(id, &handle)?;

  Ok(())
}

#[tauri::command]
pub async fn account_get_active(state: State<'_, Mutex<AccountStore>>) -> Result<String> {
  let store = state.lock().await;
  Ok(store.active().to_string())
}

#[tauri::command]
pub async fn account_set_active(
  state: State<'_, Mutex<AccountStore>>,
  id: &str,
  handle: AppHandle,
) -> Result<()> {
  let mut store = state.lock().await;
  store.set_active(id.to_string(), &handle)?;

  Ok(())
}

#[tauri::command]
pub async fn account_get_skin(
  state: State<'_, Mutex<SkinStore>>,
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  url: Url,
) -> Result<Skin> {
  let mut store = state.lock().await;
  Ok(store.get_skin_by_url(&handle, client.inner(), url).await?)
}

#[tauri::command]
pub async fn account_get_cape(
  state: State<'_, Mutex<SkinStore>>,
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  url: Url,
) -> Result<Cape> {
  let mut store = state.lock().await;
  Ok(store.get_cape_by_url(&handle, client.inner(), url).await?)
}

#[tauri::command]
pub async fn account_add_skin(
  state: State<'_, Mutex<SkinStore>>,
  handle: AppHandle,
  skin: Vec<u8>,
) -> Result<Skin> {
  let mut store = state.lock().await;
  Ok(store.add_skin(&handle, None, &skin)?)
}

#[tauri::command]
pub async fn account_remove_skin(
  state: State<'_, Mutex<SkinStore>>,
  handle: AppHandle,
  id: &str,
) -> Result<()> {
  let mut store = state.lock().await;
  store.remove_skin(id, &handle)?;
  Ok(())
}

#[tauri::command]
pub async fn account_list_skins(
  state: State<'_, Mutex<SkinStore>>,
  handle: AppHandle,
) -> Result<Vec<Skin>> {
  let store = state.lock().await;
  Ok(store.list_skins(&handle))
}

#[tauri::command]
pub async fn account_change_skin(
  state: State<'_, Mutex<SkinStore>>,
  accounts: State<'_, Mutex<AccountStore>>,
  handle: AppHandle,
  client: State<'_, Arc<Client>>,
  id: &str,
  account: &str,
) -> Result<()> {
  let mut store = state.lock().await;
  let mut accounts_store = accounts.lock().await;

  accounts_store
    .refresh_auth(account, client.inner(), &handle)
    .await?;

  if let Some(token) = accounts_store.mc_token(account) {
    let profile = store
      .select_skin(id, client.inner(), &handle, token)
      .await?;
    accounts_store.update_profile(profile, &handle)?;
  } else {
    //just any error
    return Err(tauri::Error::WindowNotFound);
  }

  Ok(())
}

#[tauri::command]
pub async fn account_change_cape(
  accounts: State<'_, Mutex<AccountStore>>,
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  account: &str,
  id: &str,
) -> Result<()> {
  let mut accounts_store = accounts.lock().await;

  accounts_store
    .select_cape_by_id(account, id, &handle, client.inner())
    .await?;

  Ok(())
}
