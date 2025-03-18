use std::io::Cursor;

use anyhow::Result;
use base64::prelude::*;
use image::ImageFormat;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Url};
use uuid::Uuid;

use crate::store::TauriAppStoreExt;

const SKIN_STORE_KEY_SKINS: &str = "skin_store.skins";
const SKIN_STORE_KEY_CAPES: &str = "skin_store.capes";

pub struct SkinStore {
  skins: Vec<Skin>,
  capes: Vec<Cape>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Skin {
  id: String,
  url: Option<Url>,
  data: String,
  head: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Cape {
  id: String,
  url: Url,
  data: String,
}

impl SkinStore {
  pub fn new(handle: &AppHandle) -> Result<SkinStore> {
    let store = handle.app_store()?;
    let skins: Vec<Skin> = store.get_or_default(SKIN_STORE_KEY_SKINS)?;
    let capes: Vec<Cape> = store.get_or_default(SKIN_STORE_KEY_CAPES)?;

    Ok(SkinStore { skins, capes })
  }

  pub fn add_skin(&mut self, handle: &AppHandle, url: Option<Url>, skin: &[u8]) -> Result<Skin> {
    let image = image::load_from_memory(skin)?;
    let head = image.crop_imm(8, 8, 8, 8);

    let mut cursor = Cursor::new(Vec::new());
    head.write_to(&mut cursor, ImageFormat::Png)?;

    let head = BASE64_STANDARD.encode(cursor.into_inner());

    let data = BASE64_STANDARD.encode(skin);
    let skin = Skin {
      head,
      data,
      url,
      id: Uuid::new_v4().to_string(),
    };

    self.skins.push(skin.clone());
    self.save(handle)?;

    Ok(skin)
  }

  fn add_cape(&mut self, handle: &AppHandle, url: Url, cape: &[u8]) -> Result<Cape> {
    let data = BASE64_STANDARD.encode(cape);

    let cape = Cape {
      data,
      url,
      id: Uuid::new_v4().to_string(),
    };
    self.capes.push(cape.clone());
    self.save(handle)?;

    Ok(cape)
  }

  fn save(&self, handle: &AppHandle) -> Result<()> {
    let store = handle.app_store()?;
    store.set(SKIN_STORE_KEY_CAPES, &self.capes)?;
    store.set(SKIN_STORE_KEY_SKINS, &self.skins)
  }

  pub async fn get_skin_by_url(
    &mut self,
    handle: &AppHandle,
    client: &Client,
    url: Url,
  ) -> Result<Skin> {
    if let Some(skin) = self.skins.iter().find(|s| s.url.as_ref() == Some(&url)) {
      Ok(skin.clone())
    } else {
      let skin = client.get(url.clone()).send().await?.bytes().await?;
      self.add_skin(handle, Some(url), &skin)
    }
  }

  pub async fn get_cape_by_url(
    &mut self,
    handle: &AppHandle,
    client: &Client,
    url: Url,
  ) -> Result<Cape> {
    if let Some(skin) = self.capes.iter().find(|c| c.url == url) {
      Ok(skin.clone())
    } else {
      let cape = client.get(url.clone()).send().await?.bytes().await?;
      self.add_cape(handle, url, &cape)
    }
  }

  pub fn clear_skins(&mut self, handle: &AppHandle) -> Result<()> {
    self.skins.clear();
    self.save(handle)
  }

  pub fn list_skins(&self) -> &[Skin] {
    &self.skins
  }

  pub fn remove_skin(&mut self, id: &str, handle: &AppHandle) -> Result<()> {
    self.skins.retain(|s| s.id != id);
    self.save(handle)
  }
}
