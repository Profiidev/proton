use std::io::Cursor;

use anyhow::Result;
use base64::prelude::*;
use image::ImageFormat;
use log::debug;
use reqwest::{Client, multipart::Form};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Url};
use thiserror::Error;
use tokio::fs;

use crate::{
  path,
  store::TauriAppStoreExt,
  utils::{
    file::bytes_hash,
    log::ResultLogExt,
    updater::{UpdateType, update_data},
  },
};

use super::info::{ProfileInfo, SkinVariant, State};

const SKIN_CHANGE_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";

pub struct SkinStore {
  skins: Vec<SkinInfo>,
  capes: Vec<CapeInfo>,
  handle: AppHandle,
  client: Client,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SkinInfo {
  id: String,
  url: Option<Url>,
}

#[derive(Serialize, Clone)]
pub struct Skin {
  id: String,
  url: Option<Url>,
  data: String,
  head: String,
}

impl SkinInfo {
  fn load_skin(self, handle: &AppHandle) -> Result<Skin> {
    debug!("Loading skin data: {}", &self.id);
    let data_dir = path!(handle.path().app_data_dir()?, SkinStore::SKIN_FOLDER);

    let data_path = path!(&data_dir, format!("{}.png", &self.id));
    let data = std::fs::read(data_path)?;

    let head_path = path!(&data_dir, format!("{}_head.png", &self.id));
    let head = std::fs::read(head_path)?;

    Ok(Skin {
      id: self.id,
      url: self.url,
      data: BASE64_STANDARD.encode(data),
      head: BASE64_STANDARD.encode(head),
    })
  }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CapeInfo {
  id: String,
  url: Url,
}

#[derive(Serialize, Clone)]
pub struct Cape {
  id: String,
  url: Url,
  data: String,
}

impl CapeInfo {
  fn load_cape(self, handle: &AppHandle) -> Result<Cape> {
    debug!("Loading cape data: {}", &self.id);
    let data_path = path!(
      handle.path().app_data_dir()?,
      SkinStore::SKIN_FOLDER,
      format!("{}.png", &self.id)
    );
    let data = std::fs::read(data_path)?;

    Ok(Cape {
      id: self.id,
      url: self.url,
      data: BASE64_STANDARD.encode(data),
    })
  }
}

impl SkinStore {
  const SKIN_KEY: &str = "skins";
  const CAPE_KEY: &str = "capes";
  const SKIN_FOLDER: &str = "skins";

  pub fn new(handle: AppHandle) -> Result<SkinStore> {
    let store = handle.app_store()?;
    let skins: Vec<SkinInfo> = store.get_or_default(Self::SKIN_KEY)?;
    let capes: Vec<CapeInfo> = store.get_or_default(Self::CAPE_KEY)?;

    Ok(SkinStore {
      skins,
      capes,
      handle,
      client: Client::new(),
    })
  }

  pub async fn add_skin(&mut self, url: Option<Url>, skin: &[u8]) -> Result<Skin> {
    let image = image::load_from_memory(skin)?;
    let head = image.crop_imm(8, 8, 8, 8);

    let mut cursor = Cursor::new(Vec::new());
    head.write_to(&mut cursor, ImageFormat::Png)?;
    let head = cursor.into_inner();

    let id = bytes_hash(skin)?;
    debug!("Saving skin with id: {}", &id);

    let data_dir = path!(self.handle.path().app_data_dir()?, Self::SKIN_FOLDER);
    fs::create_dir_all(&data_dir).await?;

    let data_path = path!(&data_dir, format!("{}.png", id));
    fs::write(data_path, skin).await?;

    let head_path = path!(&data_dir, format!("{}_head.png", id));
    fs::write(head_path, &head).await?;

    let skin_info = SkinInfo { url, id };

    self.skins.push(skin_info.clone());
    self.save()?;

    update_data(&self.handle, UpdateType::AccountSkins);

    Ok(Skin {
      id: skin_info.id,
      url: skin_info.url,
      data: BASE64_STANDARD.encode(skin),
      head: BASE64_STANDARD.encode(head),
    })
  }

  fn add_cape(&mut self, url: Url, cape: &[u8]) -> Result<Cape> {
    let id = bytes_hash(cape)?;
    debug!("Saving cape with id: {}", &id);

    let mut data_path = path!(&self.handle.path().app_data_dir()?, Self::SKIN_FOLDER);
    std::fs::create_dir_all(&data_path)?;

    data_path.push(format!("{id}.png"));
    std::fs::write(data_path, cape)?;

    let cape_info = CapeInfo { url, id };
    self.capes.push(cape_info.clone());
    self.save()?;

    Ok(Cape {
      id: cape_info.id,
      url: cape_info.url,
      data: BASE64_STANDARD.encode(cape),
    })
  }

  fn save(&self) -> Result<()> {
    let store = self.handle.app_store()?;
    store.set(Self::CAPE_KEY, &self.capes)?;
    store.set(Self::SKIN_KEY, &self.skins)
  }

  pub async fn get_skin_by_url(&mut self, url: Url) -> Result<Skin> {
    if let Some(skin) = self.skins.iter().find(|s| s.url.as_ref() == Some(&url)) {
      skin.clone().load_skin(&self.handle)
    } else {
      debug!("Skin with url {} not found. downloading", &url);
      let skin = self.client.get(url.clone()).send().await?.bytes().await?;
      self.add_skin(Some(url), &skin).await
    }
  }

  pub async fn get_cape_by_url(&mut self, url: Url) -> Result<Cape> {
    if let Some(skin) = self.capes.iter().find(|c| c.url == url) {
      skin.clone().load_cape(&self.handle)
    } else {
      debug!("Cape with url {} not found. downloading", &url);
      let cape = self.client.get(url.clone()).send().await?.bytes().await?;
      self.add_cape(url, &cape)
    }
  }

  pub fn list_skins(&self) -> Vec<Skin> {
    self
      .skins
      .iter()
      .flat_map(|skin| skin.clone().load_skin(&self.handle))
      .collect()
  }

  pub fn remove_skin(&mut self, id: &str) -> Result<()> {
    let data_dir = path!(&self.handle.path().app_data_dir()?, Self::SKIN_FOLDER);
    debug!("Deleting skin with id: {}", &id);

    let data_path = path!(&data_dir, format!("{}.png", id));
    //ignore result to prevent inconsistent saved data
    let _ = std::fs::remove_file(data_path).log();

    let head_path = path!(&data_dir, format!("{}_head.png", id));
    //ignore result to prevent inconsistent saved data
    let _ = std::fs::remove_file(head_path).log();

    self.skins.retain(|s| s.id != id);
    self.save()?;

    update_data(&self.handle, UpdateType::AccountSkins);
    Ok(())
  }

  pub async fn select_skin(&mut self, id: &str, mc_token: &str) -> Result<ProfileInfo> {
    debug!("Selecting skin with id: {id}");
    let Some(skin) = self.skins.iter_mut().find(|s| s.id == id) else {
      return Err(SkinChangeError::NotFound.into());
    };

    let profile = if let Some(url) = &skin.url {
      let res = self
        .client
        .post(SKIN_CHANGE_URL)
        .bearer_auth(mc_token)
        .json(&SkinChangeReq {
          variant: SkinVariant::Classic,
          url: Some(url.clone()),
          file: None,
        })
        .send()
        .await?;
      debug!("Got response with code: {}", res.status());

      res.json().await?
    } else {
      debug!("Skin with id {id} has no url. uploading");
      let data_path = path!(
        self.handle.path().app_data_dir()?,
        Self::SKIN_FOLDER,
        format!("{}.png", &skin.id)
      );

      let form = Form::new()
        .text("variant", SkinVariant::Classic.to_string())
        .file("file", data_path)
        .await?;

      let res = self
        .client
        .post(SKIN_CHANGE_URL)
        .bearer_auth(mc_token)
        .multipart(form)
        .send()
        .await?;
      debug!("Got response with code: {}", res.status());

      let profile: ProfileInfo = res.json().await?;

      if let Some(new_skin) = profile.skins.iter().find(|s| s.state == State::Active) {
        skin.url = Some(new_skin.url.clone());
        self.save()?;
      }

      profile
    };

    Ok(profile)
  }
}

#[derive(Serialize)]
struct SkinChangeReq {
  variant: SkinVariant,
  #[serde(skip_serializing_if = "Option::is_none")]
  url: Option<Url>,
  #[serde(skip_serializing_if = "Option::is_none")]
  file: Option<Vec<u8>>,
}

#[derive(Error, Debug)]
enum SkinChangeError {
  #[error("Not Found")]
  NotFound,
}
