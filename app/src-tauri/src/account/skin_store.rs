use std::io::Cursor;

use anyhow::Result;
use base64::prelude::*;
use image::ImageFormat;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Url};
use uuid::Uuid;

use crate::store::TauriAppStoreExt;

const SKIN_STORE_KEY_SKINS: &str = "skin_store.skins";
const SKIN_STORE_KEY_CAPES: &str = "skin_store.capes";
const SKIN_STORE_FOLDER: &str = "account_skins";

pub struct SkinStore {
  skins: Vec<SkinInfo>,
  capes: Vec<CapeInfo>,
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
    let mut data_dir = handle.path().app_data_dir()?;
    data_dir.push(SKIN_STORE_FOLDER);

    let mut data_path = data_dir.clone();
    data_path.push(format!("{}.png", &self.id));
    let data = std::fs::read(data_path)?;

    let mut head_path = data_dir.clone();
    head_path.push(format!("{}_head.png", &self.id));
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
    let mut data_path = handle.path().app_data_dir()?;
    data_path.push(SKIN_STORE_FOLDER);
    data_path.push(format!("{}.png", &self.id));
    let data = std::fs::read(data_path)?;

    Ok(Cape {
      id: self.id,
      url: self.url,
      data: BASE64_STANDARD.encode(data),
    })
  }
}

impl SkinStore {
  pub fn new(handle: &AppHandle) -> Result<SkinStore> {
    let store = handle.app_store()?;
    let skins: Vec<SkinInfo> = store.get_or_default(SKIN_STORE_KEY_SKINS)?;
    let capes: Vec<CapeInfo> = store.get_or_default(SKIN_STORE_KEY_CAPES)?;

    Ok(SkinStore { skins, capes })
  }

  pub fn add_skin(&mut self, handle: &AppHandle, url: Option<Url>, skin: &[u8]) -> Result<Skin> {
    let image = image::load_from_memory(skin)?;
    let head = image.crop_imm(8, 8, 8, 8);

    let mut cursor = Cursor::new(Vec::new());
    head.write_to(&mut cursor, ImageFormat::Png)?;
    let head = cursor.into_inner();

    let id = Uuid::new_v4().to_string();

    let mut data_dir = handle.path().app_data_dir()?;
    data_dir.push(SKIN_STORE_FOLDER);

    std::fs::create_dir_all(&data_dir)?;

    let mut data_path = data_dir.clone();
    data_path.push(format!("{}.png", id));
    std::fs::write(data_path, skin)?;

    let mut head_path = data_dir.clone();
    head_path.push(format!("{}_head.png", id));
    std::fs::write(head_path, &head)?;

    let skin_info = SkinInfo { url, id };

    self.skins.push(skin_info.clone());
    self.save(handle)?;

    Ok(Skin {
      id: skin_info.id,
      url: skin_info.url,
      data: BASE64_STANDARD.encode(skin),
      head: BASE64_STANDARD.encode(head),
    })
  }

  fn add_cape(&mut self, handle: &AppHandle, url: Url, cape: &[u8]) -> Result<Cape> {
    let cape_info = CapeInfo {
      url,
      id: Uuid::new_v4().to_string(),
    };
    self.capes.push(cape_info.clone());
    self.save(handle)?;

    Ok(Cape {
      id: cape_info.id,
      url: cape_info.url,
      data: BASE64_STANDARD.encode(cape),
    })
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
      skin.clone().load_skin(handle)
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
      skin.clone().load_cape(handle)
    } else {
      let cape = client.get(url.clone()).send().await?.bytes().await?;
      self.add_cape(handle, url, &cape)
    }
  }

  pub fn clear_skins(&mut self, handle: &AppHandle) -> Result<()> {
    self.skins.clear();
    self.save(handle)
  }

  pub fn list_skins(&self, handle: &AppHandle) -> Vec<Skin> {
    self
      .skins
      .iter()
      .flat_map(|skin| skin.clone().load_skin(handle))
      .collect()
  }

  pub fn remove_skin(&mut self, id: &str, handle: &AppHandle) -> Result<()> {
    self.skins.retain(|s| s.id != id);
    self.save(handle)
  }
}
