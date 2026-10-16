//! Skin heads and 3D textures from the account skin cache, cached in gpui's asset cache

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use gpui_kit::{App, Asset, ImageCacheError, ImageSource, RenderImage};
use image::imageops::{self, FilterType};
use proton_account::{AccountsExt, AccountsHandle, info::ProfileInfo, info::State};
use proton_skin_render::{ModelType, SkinTextures, to_render_image};
use proton_utils::{log::ErrorLogExt, rt};
use url::Url;

/// Where the skin png comes from
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum SkinSrc {
  /// An account skin, downloaded once into the skin cache
  Url(Url),
  /// A skin of the local library
  File(PathBuf),
}

pub fn active_skin(info: &ProfileInfo) -> Option<&proton_account::info::Skin> {
  info.skins.iter().find(|s| s.state == State::Active)
}

pub fn active_cape(info: &ProfileInfo) -> Option<&proton_account::info::Cape> {
  info.capes.iter().find(|c| c.state == State::Active)
}

pub fn model(info: &ProfileInfo) -> ModelType {
  match active_skin(info).map(|s| &s.variant) {
    Some(proton_account::info::SkinVariant::Slim) => ModelType::Slim,
    _ => ModelType::Classic,
  }
}

/// Reads the skin (and cape) png and decodes them. Await it on the background executor.
pub async fn load_textures(
  handle: AccountsHandle,
  skin: SkinSrc,
  cape: Option<Url>,
) -> Result<Arc<SkinTextures>> {
  let (skin, cape) = rt::spawn(async move {
    let skin = match skin {
      SkinSrc::Url(url) => handle.get_skin(url).await?.data,
      SkinSrc::File(path) => path,
    };
    let skin = std::fs::read(skin)?;
    let cape = match cape {
      Some(url) => Some(std::fs::read(handle.get_cape(url).await?.data)?),
      None => None,
    };
    anyhow::Ok((skin, cape))
  })
  .await??;
  Ok(Arc::new(SkinTextures::from_png(&skin, cape.as_deref())?))
}

/// [`load_textures`] as an asset: `window.use_asset::<Textures>(..)` is `None` while loading
/// and `Some(None)` when it failed
pub enum Textures {}

impl Asset for Textures {
  type Source = (SkinSrc, Option<Url>);
  type Output = Option<Arc<SkinTextures>>;

  #[allow(clippy::manual_async_fn)]
  fn load(
    (skin, cape): Self::Source,
    cx: &mut App,
  ) -> impl Future<Output = Self::Output> + Send + 'static {
    let handle = cx.accounts().handle.clone();
    async move { load_textures(handle, skin, cape).await.log_err().ok() }
  }
}

/// Face of an account skin, scaled up without smoothing
pub enum Head {}

/// 8x8 face → 64x64, sharp at every display scale up to 2x (gpui samples linearly)
const HEAD_SIZE: u32 = 64;

impl Asset for Head {
  type Source = Url;
  type Output = Result<Arc<RenderImage>, ImageCacheError>;

  #[allow(clippy::manual_async_fn)]
  fn load(url: Url, cx: &mut App) -> impl Future<Output = Self::Output> + Send + 'static {
    let handle = cx.accounts().handle.clone();
    async move {
      let head: Result<_> = async {
        let path = rt::spawn(async move { handle.get_skin(url).await }).await??.head;
        let face = image::load_from_memory(&std::fs::read(path)?)?.into_rgba8();
        Ok(imageops::resize(&face, HEAD_SIZE, HEAD_SIZE, FilterType::Nearest))
      }
      .await;
      head
        .map(|img| Arc::new(to_render_image(img)))
        .map_err(|e| ImageCacheError::Other(Arc::new(e)))
    }
  }
}

/// Image source of the head of `skin_url`, for `AccountImage`
pub fn head(skin_url: Url) -> ImageSource {
  ImageSource::Custom(Arc::new(move |window, cx| {
    window.use_asset::<Head>(&skin_url, cx)
  }))
}
