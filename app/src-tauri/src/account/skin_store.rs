use std::{collections::HashMap, io::Cursor};

use anyhow::Result;
use base64::prelude::*;
use image::ImageFormat;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Url};

use crate::store::TauriAppStoreExt;

const SKIN_STORE_KEY: &str = "skin_store";

pub struct SkinStore {
  skins: HashMap<Url, Skin>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Skin {
  data: String,
  head: Option<String>,
}

impl SkinStore {
  pub fn new(handle: &AppHandle) -> Result<SkinStore> {
    let store = handle.app_store()?;
    let skins: HashMap<Url, Skin> = store.get_or_default(SKIN_STORE_KEY)?;

    Ok(SkinStore { skins })
  }

  fn add_skin(
    &mut self,
    handle: &AppHandle,
    url: Url,
    skin: &[u8],
    head: bool,
  ) -> Result<Skin> {
    let head = if head {
      let image = image::load_from_memory(skin)?;
      let head = image.crop_imm(8, 8, 8, 8);

      let mut cursor = Cursor::new(Vec::new());
      head.write_to(&mut cursor, ImageFormat::Png)?;

      Some(BASE64_STANDARD.encode(cursor.into_inner()))
    } else {
      None
    };

    let data = BASE64_STANDARD.encode(skin);
    let skin = Skin { head, data };

    self.skins.insert(url, skin.clone());
    self.save(handle)?;

    Ok(skin)
  }

  fn save(&self, handle: &AppHandle) -> Result<()> {
    let store = handle.app_store()?;
    store.set(SKIN_STORE_KEY, &self.skins)
  }

  pub async fn get(
    &mut self,
    handle: &AppHandle,
    client: &Client,
    url: Url,
    head: bool,
  ) -> Result<Skin> {
    if let Some(skin) = self.skins.get(&url) {
      Ok(skin.clone())
    } else {
      let skin = client.get(url.clone()).send().await?.bytes().await?;
      self.add_skin(handle, url, &skin, head)
    }
  }

  pub fn clear(&mut self, handle: &AppHandle) -> Result<()> {
    self.skins.clear();
    self.save(handle)
  }
}
