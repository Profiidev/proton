use std::fmt::Display;

use anyhow::Result;
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::Url;

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

#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
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

impl Display for SkinVariant {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SkinVariant::Classic => write!(f, "classic"),
      SkinVariant::Slim => write!(f, "slim"),
    }
  }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Cape {
  pub id: String,
  pub state: State,
  pub url: Url,
  pub alias: String,
}

pub async fn get_profile_info(client: &Client, mc_token: &str) -> Result<ProfileInfo> {
  debug!("Retrieving player profile");
  let res = client
    .get(MC_PROFILE_URL)
    .bearer_auth(mc_token)
    .send()
    .await?;
  debug!("Got response with code: {}", res.status());

  Ok(res.json().await?)
}
