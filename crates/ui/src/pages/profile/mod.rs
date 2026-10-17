//! Profile info: header above the tabs, quick play tabs and the settings tabs

mod header;
mod quick_play;
mod settings;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use gpui_kit::{App, AppContext, Image, ImageFormat, ImageSource};
use proton_profiles::{
  ProfilesExt,
  config::{Profile, ProfileUpdate},
};

pub use header::ProfileHeader;
pub use quick_play::QuickPlayPage;
pub use settings::SettingsPage;

use crate::{
  router::{ProfileTab, Route},
  shell::PageFactory,
};

/// Registers the header, the quick play and settings tabs. Needs `proton_profiles::init`.
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

/// World icons are stored as png
fn png(bytes: Vec<u8>) -> ImageSource {
  ImageSource::Image(Arc::new(Image::from_bytes(ImageFormat::Png, bytes)))
}
