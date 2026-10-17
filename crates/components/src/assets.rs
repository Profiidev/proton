use std::borrow::Cow;

use anyhow::{Context, Result};
use gpui_kit::{
  AnyWindowHandle, App, AssetSource, Entity, Render, SharedString, Styled, Window, WindowOptions,
  assets::AllAssets as KitAssets,
  base::Root,
  component::{Theme, ThemeRegistry},
  relative,
};
use include_dir::{Dir, include_dir};

/// The repo `assets/` dir: fonts, themes, icons
static ASSETS: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../assets");

/// The web app is forced dark
pub const THEME: &str = "shadcn Zinc Blue Dark";

/// Window-wide line height. gpui defaults to 1.618 (phi), Tailwind's text-sm/xl use 1.4286/1.4
/// (14/20, 20/28): one ratio is ±1px off for xs (16) and base (24), pages pin those exactly.
pub const LINE_HEIGHT: f32 = 1.4;

/// `gpui_kit::open_window` with [`LINE_HEIGHT`] set on the kit root, so dialogs and popovers
/// (rendered in root overlays, outside the view) inherit it too
pub fn open_window<V: Render>(
  options: WindowOptions,
  cx: &mut App,
  build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> Result<(AnyWindowHandle, Entity<V>)> {
  let (handle, view) = gpui_kit::open_window(options, cx, build)?;
  handle.update(cx, |_, window, cx| {
    if let Some(Some(root)) = window.root::<Root>() {
      root.update(cx, |root, _| {
        root.text_style().line_height = Some(relative(LINE_HEIGHT));
      });
    }
  })?;
  Ok((handle, view))
}

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

  // Tailwind's `font-mono` stack (ui-monospace/SFMono are not real font names outside Apple)
  let installed = cx.text_system().all_font_names();
  if let Some(mono) = ["Menlo", "Monaco", "Consolas", "Liberation Mono"]
    .into_iter()
    .find(|f| installed.iter().any(|i| i == f))
  {
    Theme::update(cx, |theme| theme.mono_font_family = mono.into());
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use gpui_kit::{IntoElement, component::ActiveTheme};

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

  #[gpui_kit::test]
  fn theme_and_font_load(cx: &mut gpui_kit::TestAppContext) {
    let (handle, _) = crate::test_view::open(cx, |_, _| gpui_kit::div().into_any_element());
    crate::test_view::draw(handle, cx);
    cx.update(|cx| {
      assert!(cx.theme().is_dark());
      assert_eq!(cx.theme().theme_name(), THEME);
      assert_eq!(cx.theme().font_family.as_ref(), "Geist");
    });
  }
}
