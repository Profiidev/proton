use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::store::TauriAppStoreExt;

use super::{
  info::refresh_profile,
  mc_auth::{
    get_minecraft_token, get_ms_token, get_xbox_security_token, get_xbox_token, refresh_ms_token,
  },
};

const AUTH_KEY: &str = "mc_auth_info";

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct AuthInfo {
  pub mc_token: String,
  mc_token_expire: DateTime<Utc>,
  user_hash: String,
  xbox_security_token: String,
  xbox_security_token_expire: DateTime<Utc>,
  xbox_token: String,
  xbox_token_expire: DateTime<Utc>,
  ms_access_token: String,
  ms_access_token_expire: DateTime<Utc>,
  ms_refresh_token: String,
}

pub async fn ms_mc_login(client: &Client, handle: &AppHandle) -> Result<()> {
  let ms_token = get_ms_token(client, handle).await?;
  let xbox_token = get_xbox_token(client, &ms_token.access_token).await?;
  let xbox_security_token = get_xbox_security_token(client, &xbox_token.token).await?;
  let mc_token =
    get_minecraft_token(client, &xbox_token.user_hash, &xbox_security_token.token).await?;

  let profile = refresh_profile(client, handle, &mc_token.token).await?;

  let info = AuthInfo {
    mc_token: mc_token.token,
    mc_token_expire: mc_token.expires,
    user_hash: xbox_token.user_hash,
    xbox_security_token: xbox_security_token.token,
    xbox_security_token_expire: xbox_security_token.expires,
    xbox_token: xbox_token.token,
    xbox_token_expire: xbox_token.expires,
    ms_access_token: ms_token.access_token,
    ms_access_token_expire: ms_token.access_token_expires,
    ms_refresh_token: ms_token.refresh_token,
  };

  let store = handle.app_store()?;
  let mut accounts: HashMap<String, Option<AuthInfo>> = store.get_or_default(AUTH_KEY)?;

  accounts.insert(profile.id, Some(info));

  store.set(AUTH_KEY, &accounts)?;
  store.store.save()?;

  Ok(())
}

async fn refresh_mc_token(client: &Client, mut info: AuthInfo) -> Result<Option<AuthInfo>> {
  if Utc::now() < info.mc_token_expire {
    return Ok(Some(info));
  }

  'mc_token: {
    if Utc::now() < info.xbox_security_token_expire {
      break 'mc_token;
    }

    'xbox_security_token: {
      if Utc::now() < info.xbox_token_expire {
        break 'xbox_security_token;
      }

      'xbox_token: {
        if Utc::now() < info.ms_access_token_expire {
          break 'xbox_token;
        }

        let Some(res) = refresh_ms_token(client, &info.ms_refresh_token).await? else {
          return Ok(None);
        };

        info.ms_access_token = res.access_token;
        info.ms_access_token_expire = res.access_token_expires;
        info.ms_refresh_token = res.refresh_token;
      };

      let res = get_xbox_token(client, &info.ms_access_token).await?;

      info.xbox_token = res.token;
      info.xbox_token_expire = res.expires;
      info.user_hash = res.user_hash;
    };

    let res = get_xbox_security_token(client, &info.xbox_token).await?;
    info.xbox_security_token = res.token;
    info.xbox_security_token_expire = res.expires;
  };

  let res = get_minecraft_token(client, &info.user_hash, &info.xbox_security_token).await?;
  info.mc_token = res.token;
  info.mc_token_expire = res.expires;

  Ok(Some(info))
}

pub async fn refresh_all_auth_info(client: &Client, handle: &AppHandle) -> Result<()> {
  let store = handle.app_store()?;
  let accounts: HashMap<String, Option<AuthInfo>> = store.get_or_default(AUTH_KEY)?;
  let mut refreshed_accounts = HashMap::new();

  for (id, account) in accounts {
    let info = if let Some(account) = account {
      refresh_mc_token(client, account).await?
    } else {
      None
    };

    refreshed_accounts.insert(id, info);
  }

  store.set(AUTH_KEY, &refreshed_accounts)?;
  store.store.save()?;

  Ok(())
}

pub fn get_all_auth_info(handle: &AppHandle) -> Result<HashMap<String, Option<AuthInfo>>> {
  let store = handle.app_store()?;
  store.get_or_default(AUTH_KEY)
}

pub async fn refresh_auth_info(
  client: &Client,
  handle: &AppHandle,
  id: &str,
) -> Result<Option<AuthInfo>> {
  let store = handle.app_store()?;
  let mut accounts: HashMap<String, Option<AuthInfo>> = store.get_or_default(AUTH_KEY)?;

  let account = if let Some(Some(account)) = accounts.get(id) {
    refresh_mc_token(client, account.clone()).await?
  } else {
    None
  };

  accounts.insert(id.to_string(), account.clone());

  store.set(AUTH_KEY, &accounts)?;
  store.store.save()?;

  Ok(account)
}
