use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Url};

use crate::store::TauriAppStoreExt;

use super::auth::AuthInfo;

const PROFILE_KEY: &str = "mc_profile_info";

const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Deserialize, Serialize, Clone)]
pub struct ProfileInfo {
  pub id: String,
  pub name: String,
  pub skins: Vec<Skin>,
  pub capes: Vec<Cape>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
  pub id: String,
  pub state: State,
  pub url: Url,
  pub texture_key: String,
  pub variant: SkinVariant,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum State {
  Active,
  Inactive,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum SkinVariant {
  Classic,
  Slim,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Cape {
  pub id: String,
  pub state: State,
  pub url: Url,
  pub alias: String,
}

async fn get_profile_info(client: &Client, mc_token: &str) -> Result<ProfileInfo> {
  Ok(
    client
      .get(MC_PROFILE_URL)
      .bearer_auth(mc_token)
      .send()
      .await?
      .json()
      .await?,
  )
}

pub async fn refresh_profile(
  client: &Client,
  handle: &AppHandle,
  mc_token: &str,
) -> Result<ProfileInfo> {
  let profile = get_profile_info(client, mc_token).await?;

  let store = handle.app_store()?;
  let mut profiles: HashMap<String, Option<ProfileInfo>> = store.get_or_default(PROFILE_KEY)?;

  profiles.insert(profile.id.clone(), Some(profile.clone()));

  store.set(PROFILE_KEY, &profiles)?;
  store.store.save()?;

  Ok(profile)
}

pub fn get_all_profile_info(handle: &AppHandle) -> Result<HashMap<String, Option<ProfileInfo>>> {
  let store = handle.app_store()?;
  store.get_or_default(PROFILE_KEY)
}

pub async fn refresh_all_profile_info(
  client: &Client,
  handle: &AppHandle,
  auth_infos: &HashMap<String, Option<AuthInfo>>,
) -> Result<()> {
  let store = handle.app_store()?;
  let mut profiles: HashMap<String, Option<ProfileInfo>> = store.get_or_default(PROFILE_KEY)?;

  for (id, auth) in auth_infos {
    let profile = if let Some(auth) = auth {
      Some(get_profile_info(client, &auth.mc_token).await?)
    } else {
      None
    };

    profiles.insert(id.clone(), profile);
  }

  store.set(PROFILE_KEY, &profiles)?;
  store.store.save()?;

  Ok(())
}
