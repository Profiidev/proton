use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use thiserror::Error;

use crate::{
  store::TauriAppStoreExt,
  updater::{update_data, UpdateType},
};

use super::{
  auth::{ms_mc_login, refresh_mc_token, AuthInfo},
  info::{get_profile_info, ProfileInfo},
};

const ACCOUNT_KEY: &str = "account_info";
const ACTIVE_ACCOUNT_KEY: &str = "active_account";

const CAPE_CHANGE_URL: &str = "https://api.minecraftservices.com/minecraft/profile/capes/active";

pub struct AccountStore {
  accounts: HashMap<String, Option<AccountInfo>>,
  active: String,
}

#[derive(Serialize, Deserialize)]
struct AccountInfo {
  auth: AuthInfo,
  profile: ProfileInfo,
}

pub struct LaunchInfo {
  pub id: String,
  pub name: String,
  pub access_token: String,
}

impl AccountStore {
  pub fn new(handle: &AppHandle) -> Result<AccountStore> {
    let store = handle.app_store()?;
    let accounts: HashMap<String, Option<AccountInfo>> = store.get_or_default(ACCOUNT_KEY)?;
    let active: String = store.get_or_default(ACTIVE_ACCOUNT_KEY)?;

    Ok(AccountStore { accounts, active })
  }

  pub fn list_profiles(&self) -> HashMap<String, Option<ProfileInfo>> {
    self
      .accounts
      .iter()
      .map(|(id, info)| (id.clone(), info.as_ref().map(|info| info.profile.clone())))
      .collect()
  }

  async fn refresh_token(&mut self, id: &str, client: &Client) -> Result<()> {
    if let Some(Some(account)) = self.accounts.get_mut(id) {
      if let Some(auth) = refresh_mc_token(client, account.auth.clone()).await? {
        account.auth = auth;
      } else {
        self.accounts.insert(id.to_string(), None);
      };
    }

    Ok(())
  }

  async fn refresh_profile(&mut self, id: &str, client: &Client) -> Result<()> {
    if let Some(Some(account)) = self.accounts.get_mut(id) {
      let profile = get_profile_info(client, &account.auth.mc_token).await?;
      account.profile = profile;
    }

    Ok(())
  }

  fn save(&self, handle: &AppHandle) -> Result<()> {
    let store = handle.app_store()?;
    store.set(ACCOUNT_KEY, &self.accounts)?;
    store.set(ACTIVE_ACCOUNT_KEY, &self.active)
  }

  pub async fn refresh(&mut self, id: &str, client: &Client, handle: &AppHandle) -> Result<()> {
    //ignore result to prevent inconsistent saved data
    let _ = self.refresh_token(id, client).await;
    let _ = self.refresh_profile(id, client).await;
    self.save(handle)?;

    update_data(handle, UpdateType::Accounts);
    Ok(())
  }

  pub async fn refresh_all(&mut self, client: &Client, handle: &AppHandle) -> Result<()> {
    let keys: Vec<String> = self.accounts.keys().cloned().collect();

    for id in &keys {
      //ignore result to prevent inconsistent saved data
      let _ = self.refresh_token(id, client).await;
      let _ = self.refresh_profile(id, client).await;
    }

    self.save(handle)?;

    update_data(handle, UpdateType::Accounts);
    Ok(())
  }

  pub fn active(&self) -> &str {
    &self.active
  }

  pub fn mc_token(&self, id: &str) -> Option<&String> {
    self
      .accounts
      .get(id)
      .and_then(|a| a.as_ref().map(|a| &a.auth.mc_token))
  }

  pub fn launch_info(&self, id: &str) -> Option<LaunchInfo> {
    self.accounts.get(id).and_then(|a| {
      a.as_ref().map(|a| LaunchInfo {
        id: a.profile.id.clone(),
        name: a.profile.name.clone(),
        access_token: a.auth.mc_token.clone(),
      })
    })
  }

  pub fn set_active(&mut self, id: String, handle: &AppHandle) -> Result<()> {
    self.active = id;
    self.save(handle)?;

    update_data(handle, UpdateType::AccountActive);
    Ok(())
  }

  pub fn remove_account(&mut self, id: &str, handle: &AppHandle) -> Result<()> {
    self.accounts.remove(id);
    self.save(handle)?;

    update_data(handle, UpdateType::Accounts);
    Ok(())
  }

  pub async fn login(&mut self, client: &Client, handle: &AppHandle) -> Result<()> {
    let auth = ms_mc_login(client, handle).await?;
    let profile = get_profile_info(client, &auth.mc_token).await?;

    self
      .accounts
      .insert(profile.id.clone(), Some(AccountInfo { auth, profile }));

    self.save(handle)?;

    update_data(handle, UpdateType::Accounts);
    Ok(())
  }

  pub async fn refresh_auth(
    &mut self,
    id: &str,
    client: &Client,
    handle: &AppHandle,
  ) -> Result<()> {
    self.refresh_token(id, client).await?;
    self.save(handle)
  }

  pub fn update_profile(&mut self, profile: ProfileInfo, handle: &AppHandle) -> Result<()> {
    if let Some(Some(account)) = self.accounts.get_mut(&profile.id) {
      account.profile = profile;
    }
    self.save(handle)?;

    update_data(handle, UpdateType::Accounts);
    Ok(())
  }

  pub async fn select_cape_by_id(
    &mut self,
    account: &str,
    id: &str,
    handle: &AppHandle,
    client: &Client,
  ) -> Result<()> {
    self.refresh_auth(account, client, handle).await?;

    if let Some(Some(account)) = self.accounts.get_mut(account) {
      let profile: ProfileInfo = client
        .put(CAPE_CHANGE_URL)
        .bearer_auth(&account.auth.mc_token)
        .json(&CapeChangeReq {
          cape_id: id.to_string(),
        })
        .send()
        .await?
        .json()
        .await?;

      account.profile = profile;
      self.save(handle)?;

      update_data(handle, UpdateType::Accounts);
      return Ok(());
    }

    Err(CapeChangeError::Other.into())
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapeChangeReq {
  cape_id: String,
}

#[derive(Error, Debug)]
enum CapeChangeError {
  #[error("Cape change error")]
  Other,
}
