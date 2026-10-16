//! Profile info: header above the tabs, quick play tabs and the settings tabs

mod header;
mod quick_play;
mod settings;
#[cfg(test)]
mod tests;

use std::{collections::HashMap, sync::Arc};

use gpui_kit::{
  App, AppContext, Global, Image, ImageFormat, ImageSource, Window,
  component::notification::Notification,
};
use proton_profiles::{
  ProfilesExt,
  config::{Profile, ProfileUpdate},
};

pub use header::ProfileHeader;
pub use quick_play::QuickPlayPage;
pub use settings::SettingsPage;

use crate::{
  actions::toast,
  router::{ProfileTab, Route},
  shell::{PageFactory, ShellDataExt},
};

/// Header menu entries whose dialogs live in other page modules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuAction {
  Duplicate,
  Export,
  /// Only shown for modpack profiles
  ChangeVersion,
}

type MenuFn = fn(String, &mut Window, &mut App);

#[derive(Default)]
struct MenuActions(HashMap<MenuAction, MenuFn>);

impl Global for MenuActions {}

/// Hooks a header menu entry, called with the profile id. Unset entries toast "not available yet".
pub fn set_menu_action(cx: &mut App, action: MenuAction, f: MenuFn) {
  cx.default_global::<MenuActions>().0.insert(action, f);
}

fn run_menu_action(action: MenuAction, id: String, window: &mut Window, cx: &mut App) {
  match cx
    .try_global::<MenuActions>()
    .and_then(|a| a.0.get(&action))
  {
    Some(f) => f(id, window, cx),
    None => toast(Notification::info("Not available yet"), cx),
  }
}

/// Registers the header, the quick play and settings tabs and keeps the breadcrumb names
/// in sync with the profiles. Needs `proton_profiles::init`.
pub fn register(cx: &mut App) {
  PageFactory::set_profile_header(cx, |id, window, cx| {
    let id = id.to_string();
    cx.new(|cx| ProfileHeader::new(id, window, cx)).into()
  });
  PageFactory::register(
    cx,
    |r| {
      matches!(
        r,
        Route::Profile {
          tab: ProfileTab::QuickPlay(_),
          ..
        }
      )
    },
    |route, window, cx| {
      let Route::Profile {
        id,
        tab: ProfileTab::QuickPlay(tab),
      } = route
      else {
        unreachable!()
      };
      let (id, tab) = (id.clone(), *tab);
      cx.new(|cx| QuickPlayPage::new(id, tab, window, cx)).into()
    },
  );
  PageFactory::register(
    cx,
    |r| {
      matches!(
        r,
        Route::Profile {
          tab: ProfileTab::Settings(_),
          ..
        }
      )
    },
    |route, window, cx| {
      let Route::Profile {
        id,
        tab: ProfileTab::Settings(tab),
      } = route
      else {
        unreachable!()
      };
      let (id, tab) = (id.clone(), *tab);
      cx.new(|cx| SettingsPage::new(id, tab, window, cx)).into()
    },
  );

  sync_names(cx);
  let profiles = cx.profiles().profiles.clone();
  cx.observe(&profiles, |_, cx| sync_names(cx)).detach();
}

/// Profile names for the breadcrumbs
fn sync_names(cx: &mut App) {
  let names: HashMap<String, String> = cx
    .profiles()
    .profiles
    .read(cx)
    .iter()
    .map(|p| (p.id.clone(), p.name.clone()))
    .collect();
  cx.shell_data().update(cx, |data, cx| {
    if data.names.profiles != names {
      data.names.profiles = names;
      cx.notify();
    }
  });
}

fn find_profile(id: &str, cx: &App) -> Option<Profile> {
  cx.profiles()
    .profiles
    .read(cx)
    .iter()
    .find(|p| p.id == id)
    .cloned()
}

/// The editable fields of `profile`, to change one and send it back
fn update_of(p: &Profile) -> ProfileUpdate {
  ProfileUpdate {
    id: p.id.clone(),
    name: p.name.clone(),
    version: p.version.clone(),
    loader_version: p.loader_version.clone(),
    use_local_game: p.use_local_game,
    game: p.game.clone(),
    use_local_jvm: p.use_local_jvm,
    jvm: p.jvm.clone(),
  }
}

/// Profile icons and world icons are stored as png
fn png(bytes: Vec<u8>) -> ImageSource {
  ImageSource::Image(Arc::new(Image::from_bytes(ImageFormat::Png, bytes)))
}

/// Loads the icon of profile `id`
fn load_icon(id: String, cx: &App) -> impl Future<Output = Option<ImageSource>> + use<> {
  let task = cx.profiles().icon(id);
  async move { task.await.ok().flatten().map(png) }
}
