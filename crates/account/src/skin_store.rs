use std::{io::Cursor, path::PathBuf};

use anyhow::{Result, bail};
use image::ImageFormat;
use reqwest::{Client, multipart::Form};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::debug;
use url::Url;

use proton_utils::{dir::data_dir, download::default_client, file::bytes_hash, log::ErrorLogExt};

use super::info::{ProfileInfo, SkinVariant, State};

const SKIN_CHANGE_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";

pub struct SkinStore {
  skins: Vec<SkinInfo>,
  capes: Vec<CapeInfo>,
  client: Client,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SkinInfo {
  id: String,
  url: Option<Url>,
}

/// A cached skin. The paths work with gpui `img(path)`; for `SkinTextures::from_png` read the
/// bytes with `std::fs::read(&skin.data)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Skin {
  pub id: String,
  pub url: Option<Url>,
  /// full skin png
  pub data: PathBuf,
  /// 8x8 face png
  pub head: PathBuf,
}

impl SkinInfo {
  fn load_skin(&self) -> Result<Skin> {
    let dir = skin_dir()?;
    let skin = Skin {
      id: self.id.clone(),
      url: self.url.clone(),
      data: dir.join(format!("{}.png", self.id)),
      head: dir.join(format!("{}_head.png", self.id)),
    };
    anyhow::ensure!(
      skin.data.exists() && skin.head.exists(),
      "Skin files missing: {}",
      self.id
    );
    Ok(skin)
  }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CapeInfo {
  id: String,
  url: Url,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cape {
  pub id: String,
  pub url: Url,
  /// cape png
  pub data: PathBuf,
}

impl CapeInfo {
  fn load_cape(&self) -> Result<Cape> {
    let data = skin_dir()?.join(format!("{}.png", self.id));
    anyhow::ensure!(data.exists(), "Cape file missing: {}", self.id);
    Ok(Cape {
      id: self.id.clone(),
      url: self.url.clone(),
      data,
    })
  }
}

fn skin_dir() -> Result<PathBuf> {
  Ok(data_dir()?.join(SkinStore::SKIN_FOLDER))
}

impl SkinStore {
  const SKIN_KEY: &str = "skins";
  const CAPE_KEY: &str = "capes";
  const SKIN_FOLDER: &str = "skins";

  pub fn new() -> Result<SkinStore> {
    let store = proton_store::store();
    let skins: Vec<SkinInfo> = store.get_or_default(Self::SKIN_KEY)?;
    let capes: Vec<CapeInfo> = store.get_or_default(Self::CAPE_KEY)?;

    Ok(SkinStore {
      skins,
      capes,
      client: default_client(),
    })
  }

  pub async fn add_skin(&mut self, url: Option<Url>, skin: &[u8]) -> Result<Skin> {
    let image = image::load_from_memory(skin)?;
    let head = image.crop_imm(8, 8, 8, 8);

    let mut cursor = Cursor::new(Vec::new());
    head.write_to(&mut cursor, ImageFormat::Png)?;
    let head = cursor.into_inner();

    let id = bytes_hash(skin);
    debug!("Saving skin with id: {}", id);

    let dir = skin_dir()?;
    fs::create_dir_all(&dir).await?;
    fs::write(dir.join(format!("{id}.png")), skin).await?;
    fs::write(dir.join(format!("{id}_head.png")), &head).await?;

    let skin_info = SkinInfo { url, id };
    self.skins.push(skin_info.clone());
    self.save()?;

    skin_info.load_skin()
  }

  async fn add_cape(&mut self, url: Url, cape: &[u8]) -> Result<Cape> {
    let id = bytes_hash(cape);
    debug!("Saving cape with id: {}", id);

    let dir = skin_dir()?;
    fs::create_dir_all(&dir).await?;
    fs::write(dir.join(format!("{id}.png")), cape).await?;

    let cape_info = CapeInfo { url, id };
    self.capes.push(cape_info.clone());
    self.save()?;

    cape_info.load_cape()
  }

  fn save(&self) -> Result<()> {
    let store = proton_store::store();
    store.set(Self::CAPE_KEY, &self.capes)?;
    store.set(Self::SKIN_KEY, &self.skins)
  }

  async fn download(&self, url: &Url) -> Result<Vec<u8>> {
    debug!("Downloading texture {url}");
    let res = self.client.get(url.clone()).send().await?;
    Ok(res.error_for_status()?.bytes().await?.to_vec())
  }

  pub async fn get_skin_by_url(&mut self, url: Url) -> Result<Skin> {
    if let Some(skin) = self.skins.iter().find(|s| s.url.as_ref() == Some(&url))
      && let Ok(skin) = skin.load_skin()
    {
      return Ok(skin);
    }
    let skin = self.download(&url).await?;
    self.add_skin(Some(url), &skin).await
  }

  pub async fn get_cape_by_url(&mut self, url: Url) -> Result<Cape> {
    if let Some(cape) = self.capes.iter().find(|c| c.url == url)
      && let Ok(cape) = cape.load_cape()
    {
      return Ok(cape);
    }
    let cape = self.download(&url).await?;
    self.add_cape(url, &cape).await
  }

  pub fn list_skins(&self) -> Vec<Skin> {
    self
      .skins
      .iter()
      .flat_map(|skin| skin.load_skin().log_err())
      .collect()
  }

  pub fn remove_skin(&mut self, id: &str) -> Result<()> {
    let dir = skin_dir()?;
    debug!("Deleting skin with id: {}", id);

    //ignore result to prevent inconsistent saved data
    let _ = std::fs::remove_file(dir.join(format!("{id}.png"))).log_err();
    let _ = std::fs::remove_file(dir.join(format!("{id}_head.png"))).log_err();

    self.skins.retain(|s| s.id != id);
    self.save()
  }

  pub async fn select_skin(&mut self, id: &str, mc_token: &str) -> Result<ProfileInfo> {
    debug!("Selecting skin with id: {id}");
    let Some(skin) = self.skins.iter_mut().find(|s| s.id == id) else {
      bail!("Skin {id} not found");
    };

    let profile = if let Some(url) = &skin.url {
      let res = self
        .client
        .post(SKIN_CHANGE_URL)
        .bearer_auth(mc_token)
        .json(&SkinChangeReq {
          variant: SkinVariant::Classic,
          url: url.clone(),
        })
        .send()
        .await?
        .error_for_status()?;
      debug!("Got response with code: {}", res.status());

      res.json().await?
    } else {
      debug!("Skin with id {id} has no url. uploading");
      let data_path = skin_dir()?.join(format!("{}.png", skin.id));

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
        .await?
        .error_for_status()?;
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
  url: Url,
}
