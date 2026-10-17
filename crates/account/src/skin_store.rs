use std::{io::Cursor, path::PathBuf};

use anyhow::{Result, bail};
use image::ImageFormat;
use reqwest::{Client, multipart::Form};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::debug;
use url::Url;

use proton_utils::{
  dir::data_dir, download::default_client, file::bytes_hash, http, log::ErrorLogExt,
};

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

    // the id is the content hash, the same png twice is one skin
    if let Some(i) = self.skins.iter().position(|s| s.id == id) {
      if self.skins[i].url.is_none() && url.is_some() {
        self.skins[i].url = url;
        self.save()?;
      }
      return self.skins[i].load_skin();
    }

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

    if let Some(known) = self.capes.iter().find(|c| c.id == id) {
      return known.load_cape();
    }

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
        .post(&*http::url(SKIN_CHANGE_URL))
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
        .post(&*http::url(SKIN_CHANGE_URL))
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

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  };

  use image::{Rgba, RgbaImage};
  use proton_utils::{
    rt,
    test_http::{FakeServer, Response},
  };

  use super::*;
  use crate::{info::tests::profile_json, tests::init_dirs};

  /// A 64x64 skin png, `seed` changes the face pixels and so the id
  fn png(seed: u8) -> Vec<u8> {
    let img = RgbaImage::from_fn(64, 64, |x, y| Rgba([x as u8, y as u8, seed, 255]));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
  }

  fn tex_url(server: &FakeServer, name: &str) -> Url {
    format!("{}/tex/{name}", server.origin()).parse().unwrap()
  }

  #[test]
  fn adding_the_same_skin_twice_keeps_one_entry() {
    init_dirs();
    let mut store = SkinStore::new().unwrap();
    let bytes = png(7);
    let first = rt::block_on(store.add_skin(None, &bytes)).unwrap();
    let url: Url = "https://textures.minecraft.net/texture/x".parse().unwrap();
    // seen again as a worn skin: the known entry learns its url
    let second = rt::block_on(store.add_skin(Some(url.clone()), &bytes)).unwrap();
    assert_eq!(second.id, first.id);
    assert_eq!(second.url, Some(url.clone()));
    // an upload of the same png later does not forget it
    rt::block_on(store.add_skin(None, &bytes)).unwrap();
    assert_eq!(SkinStore::new().unwrap().list_skins(), [second]);
  }

  #[test]
  fn add_skin_writes_the_skin_and_its_face() {
    let data = init_dirs();
    let mut store = SkinStore::new().unwrap();
    let bytes = png(7);
    let skin = rt::block_on(store.add_skin(None, &bytes)).unwrap();

    assert_eq!(skin.id, bytes_hash(&bytes));
    assert_eq!(
      skin.data,
      data.join("skins").join(format!("{}.png", skin.id))
    );
    assert_eq!(std::fs::read(&skin.data).unwrap(), bytes);
    let head = image::open(&skin.head).unwrap().to_rgba8();
    assert_eq!(head.dimensions(), (8, 8));
    // the face starts at 8,8 of the skin
    assert_eq!(head.get_pixel(0, 0), &Rgba([8, 8, 7, 255]));

    assert_eq!(SkinStore::new().unwrap().list_skins(), [skin]);
  }

  #[test]
  fn add_skin_rejects_non_images_and_tiny_ones() {
    init_dirs();
    let mut store = SkinStore::new().unwrap();
    assert!(rt::block_on(store.add_skin(None, b"not a png")).is_err());
    assert!(rt::block_on(store.add_skin(None, &[])).is_err());

    // smaller than the face area: the crop is empty and a 0x0 png cannot be written
    let mut tiny = Cursor::new(Vec::new());
    RgbaImage::new(4, 4)
      .write_to(&mut tiny, ImageFormat::Png)
      .unwrap();
    assert!(rt::block_on(store.add_skin(None, &tiny.into_inner())).is_err());
    assert!(SkinStore::new().unwrap().skins.is_empty());
  }

  #[test]
  fn list_skips_skins_with_missing_files_and_remove_cleans_up() {
    init_dirs();
    let mut store = SkinStore::new().unwrap();
    let a = rt::block_on(store.add_skin(None, &png(1))).unwrap();
    let b = rt::block_on(store.add_skin(None, &png(2))).unwrap();
    std::fs::remove_file(&a.head).unwrap();
    assert_eq!(store.list_skins(), std::slice::from_ref(&b));

    store.remove_skin(&b.id).unwrap();
    assert!(!b.data.exists() && !b.head.exists());
    // half deleted and unknown ids still drop the entry without an error
    store.remove_skin(&a.id).unwrap();
    store.remove_skin("ghost").unwrap();
    assert!(SkinStore::new().unwrap().skins.is_empty());
  }

  #[test]
  fn skin_by_url_downloads_once_until_the_files_go_missing() {
    init_dirs();
    let server = FakeServer::start();
    server.route("GET", "/tex/s.png", 200, png(3));
    let mut store = SkinStore::new().unwrap();
    let url = tex_url(&server, "s.png");

    let first = rt::block_on(store.get_skin_by_url(url.clone())).unwrap();
    let again = rt::block_on(store.get_skin_by_url(url.clone())).unwrap();
    assert_eq!(first, again);
    assert_eq!(first.url.as_ref(), Some(&url));
    assert_eq!(server.calls().len(), 1);

    std::fs::remove_file(&first.data).unwrap();
    rt::block_on(store.get_skin_by_url(url)).unwrap();
    assert_eq!(server.calls().len(), 2);
  }

  #[test]
  fn cape_by_url_downloads_once_and_persists() {
    let data = init_dirs();
    let server = FakeServer::start();
    server.route("GET", "/tex/c.png", 200, b"cape bytes".to_vec());
    let mut store = SkinStore::new().unwrap();
    let url = tex_url(&server, "c.png");

    let cape = rt::block_on(store.get_cape_by_url(url.clone())).unwrap();
    // capes are not decoded, any bytes are stored
    assert_eq!(std::fs::read(&cape.data).unwrap(), b"cape bytes");
    assert_eq!(cape.data.parent().unwrap(), data.join("skins"));
    let mut reloaded = SkinStore::new().unwrap();
    assert_eq!(rt::block_on(reloaded.get_cape_by_url(url)).unwrap(), cape);
    assert_eq!(server.calls().len(), 1);
  }

  #[test]
  fn texture_download_errors() {
    init_dirs();
    let server = FakeServer::start();
    server.route("GET", "/tex/bad.png", 200, b"garbage".to_vec());
    server.route("GET", "/tex/gone.png", 500, png(1));
    let mut store = SkinStore::new().unwrap();
    for name in ["missing.png", "gone.png", "bad.png"] {
      assert!(
        rt::block_on(store.get_skin_by_url(tex_url(&server, name))).is_err(),
        "{name}"
      );
    }
    assert!(rt::block_on(store.get_cape_by_url(tex_url(&server, "missing.png"))).is_err());
    assert!(store.skins.is_empty() && store.capes.is_empty());
  }

  #[test]
  fn select_unknown_skin_fails_without_a_request() {
    init_dirs();
    let server = FakeServer::start();
    let mut store = SkinStore::new().unwrap();
    let err = rt::block_on(store.select_skin("ghost", "t")).unwrap_err();
    assert_eq!(err.to_string(), "Skin ghost not found");
    assert!(server.calls().is_empty());
  }

  #[test]
  fn select_skin_with_url_posts_the_url() {
    init_dirs();
    let server = FakeServer::start();
    server.route("GET", "/tex/s.png", 200, png(4));
    server.route(
      "POST",
      &FakeServer::path(SKIN_CHANGE_URL),
      200,
      profile_json("A"),
    );
    let mut store = SkinStore::new().unwrap();
    let url = tex_url(&server, "s.png");
    let skin = rt::block_on(store.get_skin_by_url(url.clone())).unwrap();

    let profile = rt::block_on(store.select_skin(&skin.id, "tok")).unwrap();
    assert_eq!(profile.name, "A");
    let call = &server.calls()[1];
    assert_eq!(call.header("authorization"), Some("Bearer tok"));
    let body: serde_json::Value = serde_json::from_slice(&call.body).unwrap();
    assert_eq!(
      body,
      serde_json::json!({"variant": "CLASSIC", "url": url.as_str()})
    );
  }

  #[test]
  fn select_local_skin_uploads_it_and_keeps_the_new_url() {
    init_dirs();
    let server = FakeServer::start();
    server.route(
      "POST",
      &FakeServer::path(SKIN_CHANGE_URL),
      200,
      profile_json("A"),
    );
    let mut store = SkinStore::new().unwrap();
    let bytes = png(5);
    let skin = rt::block_on(store.add_skin(None, &bytes)).unwrap();

    rt::block_on(store.select_skin(&skin.id, "tok")).unwrap();
    let call = &server.calls()[0];
    let ty = call.header("content-type").unwrap();
    assert!(ty.starts_with("multipart/form-data; boundary="), "{ty}");
    let body = String::from_utf8_lossy(&call.body);
    assert!(
      body.contains("name=\"variant\"\r\n\r\nclassic\r\n"),
      "{body}"
    );
    assert!(body.contains(&format!("name=\"file\"; filename=\"{}.png\"", skin.id)));
    assert!(call.body.windows(bytes.len()).any(|w| w == bytes));

    // the active skin's url from the answer is remembered, the next select posts json
    let saved = SkinStore::new().unwrap();
    let url = saved.skins[0].url.as_ref().unwrap();
    assert_eq!(url.as_str(), "http://textures.minecraft.net/texture/s1");
  }

  #[test]
  fn upload_without_an_active_skin_in_the_answer_keeps_no_url() {
    init_dirs();
    let server = FakeServer::start();
    let inactive = profile_json("A").replace("\"ACTIVE\"", "\"INACTIVE\"");
    server.route("POST", &FakeServer::path(SKIN_CHANGE_URL), 200, inactive);
    let mut store = SkinStore::new().unwrap();
    let skin = rt::block_on(store.add_skin(None, &png(6))).unwrap();
    rt::block_on(store.select_skin(&skin.id, "tok")).unwrap();
    assert!(store.skins[0].url.is_none());
  }

  #[test]
  fn select_skin_errors() {
    let cases = [
      (401, profile_json("A")),
      (500, String::new()),
      (200, "{".into()),
    ];
    for (status, body) in cases {
      init_dirs();
      let server = FakeServer::start();
      server.route("POST", &FakeServer::path(SKIN_CHANGE_URL), status, body);
      let mut store = SkinStore::new().unwrap();
      let skin = rt::block_on(store.add_skin(None, &png(1))).unwrap();
      assert!(
        rt::block_on(store.select_skin(&skin.id, "t")).is_err(),
        "{status}"
      );
      assert!(store.skins[0].url.is_none());

      // an uploaded skin whose file vanished cannot be read for the form
      std::fs::remove_file(&skin.data).unwrap();
      let calls = Arc::new(AtomicUsize::new(0));
      let seen = calls.clone();
      server.route_fn("POST", &FakeServer::path(SKIN_CHANGE_URL), move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        Response::new(200, profile_json("A"))
      });
      assert!(rt::block_on(store.select_skin(&skin.id, "t")).is_err());
      assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
  }
}
