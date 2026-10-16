use std::borrow::Cow;

use anyhow::{Context, Result};
use gpui_kit::{
  App, AssetSource, SharedString,
  assets::AllAssets as KitAssets,
  component::{Theme, ThemeRegistry},
};
use include_dir::{Dir, include_dir};

/// The repo `assets/` dir: fonts, themes, icons
static ASSETS: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../assets");

/// The web app is forced dark
pub const THEME: &str = "shadcn Zinc Blue Dark";

/// Own assets first, then gpui-kit's (lucide icons etc.)
pub struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
    match ASSETS.get_file(path) {
      Some(file) => Ok(Some(Cow::Borrowed(file.contents()))),
      None => KitAssets.load(path),
    }
  }

  fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
    let mut assets = KitAssets.list(path)?;
    if let Some(dir) = ASSETS.get_dir(path.trim_end_matches('/')) {
      assets.extend(
        dir
          .files()
          .map(|f| f.path().to_string_lossy().into_owned().into()),
      );
    }
    Ok(assets)
  }
}

/// Registers the bundled fonts, loads every bundled theme and applies [`THEME`]
pub fn load(cx: &mut App) -> Result<()> {
  let fonts = ASSETS.get_dir("fonts").context("no fonts dir")?;
  let fonts = fonts
    .files()
    .filter(|f| f.path().extension().is_some_and(|e| e == "ttf"))
    .map(|f| Cow::Borrowed(f.contents()))
    .collect();
  cx.text_system().add_fonts(fonts)?;

  let registry = ThemeRegistry::global_mut(cx);
  for file in ASSETS.get_dir("themes").context("no themes dir")?.files() {
    let content = file.contents_utf8().context("theme is not utf8")?;
    registry.load_themes_from_str(content)?;
  }
  let config = ThemeRegistry::global(cx)
    .themes()
    .get(THEME)
    .with_context(|| format!("unknown theme: {THEME}"))?
    .clone();
  // apply_config also switches to the theme's mode
  Theme::update(cx, |theme| theme.apply_config(&config));
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn own_assets_win_and_kit_is_the_fallback() {
    let own = Assets.load("icons/proton.svg").unwrap().unwrap();
    assert_eq!(
      &*own,
      ASSETS.get_file("icons/proton.svg").unwrap().contents()
    );
    assert!(Assets.load("no/such/file").ok().flatten().is_none());
    assert!(
      Assets
        .list("fonts/")
        .unwrap()
        .iter()
        .any(|p| p == "fonts/Geist-Regular.ttf")
    );
  }
}
