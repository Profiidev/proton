use std::{collections::HashMap, sync::Arc};

use log::{trace, warn};
use reqwest::Client;
use tauri::{AppHandle, Result, State, Url};
use tokio::sync::Mutex;

use crate::{
  account::skin_store::{Cape, Skin},
  log::ResultLogExt,
};

use super::{info::ProfileInfo, skin_store::SkinStore, store::AccountStore};

#[tauri::command]
pub async fn account_list(
  state: State<'_, Mutex<AccountStore>>,
) -> Result<HashMap<String, Option<ProfileInfo>>> {
  trace!("Command account_list called");
  let store = state.lock().await;
  Ok(store.list_profiles())
}

#[tauri::command]
pub async fn account_refresh(
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  state: State<'_, Mutex<AccountStore>>,
) -> Result<()> {
  trace!("Command account_refresh called");
  let mut store = state.lock().await;
  store.refresh_all(client.inner(), &handle).await.log()?;

  Ok(())
}

#[tauri::command]
pub async fn account_refresh_one(
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  id: &str,
  state: State<'_, Mutex<AccountStore>>,
) -> Result<()> {
  trace!("Command account_refresh_one called");
  let mut store = state.lock().await;
  store.refresh(id, client.inner(), &handle).await.log()?;

  Ok(())
}

#[tauri::command]
pub async fn account_login(
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  state: State<'_, Mutex<AccountStore>>,
) -> Result<()> {
  trace!("Command account_login called");
  let mut store = state.lock().await;
  store.login(client.inner(), &handle).await.log()?;

  Ok(())
}

#[tauri::command]
pub async fn account_remove(
  state: State<'_, Mutex<AccountStore>>,
  id: &str,
  handle: AppHandle,
) -> Result<()> {
  trace!("Command account_remove called");
  let mut store = state.lock().await;
  store.remove_account(id, &handle).log()?;

  Ok(())
}

#[tauri::command]
pub async fn account_get_active(state: State<'_, Mutex<AccountStore>>) -> Result<String> {
  trace!("Command account_get_active called");
  let store = state.lock().await;
  Ok(store.active().to_string())
}

#[tauri::command]
pub async fn account_set_active(
  state: State<'_, Mutex<AccountStore>>,
  id: &str,
  handle: AppHandle,
) -> Result<()> {
  trace!("Command account_set_active called");
  let mut store = state.lock().await;
  store.set_active(id.to_string(), &handle).log()?;

  Ok(())
}

#[tauri::command]
pub async fn account_get_skin(
  state: State<'_, Mutex<SkinStore>>,
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  url: Url,
) -> Result<Skin> {
  trace!("Command account_get_skin called");
  let mut store = state.lock().await;
  Ok(
    store
      .get_skin_by_url(&handle, client.inner(), url)
      .await
      .log()?,
  )
}

#[tauri::command]
pub async fn account_get_cape(
  state: State<'_, Mutex<SkinStore>>,
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  url: Url,
) -> Result<Cape> {
  trace!("Command account_get_cape called");
  let mut store = state.lock().await;
  Ok(
    store
      .get_cape_by_url(&handle, client.inner(), url)
      .await
      .log()?,
  )
}

#[tauri::command]
pub async fn account_add_skin(
  state: State<'_, Mutex<SkinStore>>,
  handle: AppHandle,
  skin: Vec<u8>,
) -> Result<Skin> {
  trace!("Command account_add_skin called");
  let mut store = state.lock().await;
  Ok(store.add_skin(&handle, None, &skin).log()?)
}

#[tauri::command]
pub async fn account_remove_skin(
  state: State<'_, Mutex<SkinStore>>,
  handle: AppHandle,
  id: &str,
) -> Result<()> {
  trace!("Command account_remove_skin called");
  let mut store = state.lock().await;
  store.remove_skin(id, &handle).log()?;
  Ok(())
}

#[tauri::command]
pub async fn account_list_skins(
  state: State<'_, Mutex<SkinStore>>,
  handle: AppHandle,
) -> Result<Vec<Skin>> {
  trace!("Command account_list_skins called");
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
  trace!("Command account_change_skin called");
  let mut store = state.lock().await;
  let mut accounts_store = accounts.lock().await;

  accounts_store
    .refresh_auth(account, client.inner(), &handle)
    .await
    .log()?;

  if let Some(token) = accounts_store.mc_token(account) {
    let profile = store
      .select_skin(id, client.inner(), &handle, token)
      .await
      .log()?;
    accounts_store.update_profile(profile, &handle)?;
  } else {
    warn!("Account {} not found", account);
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
  trace!("Command account_change_cape called");
  let mut accounts_store = accounts.lock().await;

  accounts_store
    .select_cape_by_id(account, id, &handle, client.inner())
    .await
    .log()?;

  Ok(())
}
