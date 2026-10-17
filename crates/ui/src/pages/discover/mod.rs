//! Discover (mods, modpacks, resource packs, shaders) and the project page

mod project;
mod search;

use gpui_kit::{App, AppContext};

pub use project::ProjectPage;
pub use search::{DiscoverPage, SearchFn};

use crate::{router::Route, shell::PageFactory};

pub fn register(cx: &mut App) {
  PageFactory::register(
    cx,
    |r| matches!(r, Route::Discover(_)),
    |route, window, cx| {
      let Route::Discover(kind) = route else {
        unreachable!()
      };
      let kind = *kind;
      cx.new(|cx| DiscoverPage::new(kind, window, cx)).into()
    },
  );
  PageFactory::register(
    cx,
    |r| matches!(r, Route::Project { .. }),
    |route, window, cx| {
      let Route::Project { id, kind, source } = route.clone() else {
        unreachable!()
      };
      cx.new(|cx| ProjectPage::new(id, kind, source, window, cx))
        .into()
    },
  );
}
