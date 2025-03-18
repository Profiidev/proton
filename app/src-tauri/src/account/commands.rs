use std::collections::HashMap;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Result, State};

use crate::store::TauriAppStoreExt;

use super::{
  auth::{ms_mc_login, refresh_mc_token, AuthInfo},
  info::{get_profile_info, ProfileInfo},
};

const ACCOUNT_KEY: &str = "account_info";

#[derive(Serialize, Deserialize)]
struct AccountInfo {
  auth: AuthInfo,
  profile: ProfileInfo,
}

#[tauri::command]
pub fn account_list(handle: AppHandle) -> Result<HashMap<String, Option<ProfileInfo>>> {
  let accounts = load_accounts(&handle)?;

  Ok(
    accounts
      .into_iter()
      .map(|(id, info)| (id, info.map(|info| info.profile)))
      .collect(),
  )
}

#[tauri::command]
pub async fn account_refresh(client: State<'_, Client>, handle: AppHandle) -> Result<()> {
  let accounts = load_accounts(&handle)?;
  let mut refreshed_accounts = HashMap::new();

  for (id, account) in accounts {
    let info = if let Some(account) = account {
      if let Some(auth) = refresh_mc_token(client.inner(), account.auth).await? {
        let profile = get_profile_info(client.inner(), &auth.mc_token).await?;
        Some(AccountInfo { auth, profile })
      } else {
        None
      }
    } else {
      None
    };

    refreshed_accounts.insert(id, info);
  }

  save_accounts(&handle, &refreshed_accounts)?;

  Ok(())
}

#[tauri::command]
pub async fn account_refresh_one(
  client: State<'_, Client>,
  handle: AppHandle,
  id: &str,
) -> Result<()> {
  let mut accounts = load_accounts(&handle)?;

  if let Some(Some(account)) = accounts.get(id) {
    let info = if let Some(auth) = refresh_mc_token(client.inner(), account.auth.clone()).await? {
      let profile = get_profile_info(client.inner(), &auth.mc_token).await?;
      Some(AccountInfo { auth, profile })
    } else {
      None
    };

    accounts.insert(id.to_string(), info);
  }

  save_accounts(&handle, &accounts)?;

  Ok(())
}

#[tauri::command]
pub async fn account_login(client: State<'_, Client>, handle: AppHandle) -> Result<()> {
  let auth = ms_mc_login(client.inner(), &handle).await?;
  let profile = get_profile_info(client.inner(), &auth.mc_token).await?;

  let mut accounts = load_accounts(&handle)?;
  accounts.insert(profile.id.clone(), Some(AccountInfo { auth, profile }));
  save_accounts(&handle, &accounts)?;

  Ok(())
}

fn save_accounts(
  handle: &AppHandle,
  accounts: &HashMap<String, Option<AccountInfo>>,
) -> anyhow::Result<()> {
  let store = handle.app_store()?;
  store.set(ACCOUNT_KEY, &accounts)?;
  store.store.save()?;

  Ok(())
}

fn load_accounts(handle: &AppHandle) -> anyhow::Result<HashMap<String, Option<AccountInfo>>> {
  let store = handle.app_store()?;
  store.get_or_default(ACCOUNT_KEY)
}
