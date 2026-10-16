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

/// Wires the content dialogs into the create-profile dialog tabs and the profile menu
pub fn register_hooks(cx: &mut App) {
  use gpui_kit::component::WindowExt;

  use crate::{
    content::{
      FromModpackTab, ImportFileTab, ModpackInstallDialog, ProjectRef, export_mrpack,
      open_duplicate_dialog,
    },
    pages::{
      profile::{MenuAction, set_menu_action},
      profiles::{CreateTab, set_create_tab},
    },
  };

  set_create_tab(cx, CreateTab::FromModpack, |window, cx| {
    cx.new(|cx| {
      FromModpackTab::new(
        |project, window, cx| {
          window.close_dialog(cx);
          ModpackInstallDialog::open(ProjectRef::from(&project), window, cx);
        },
        window,
        cx,
      )
    })
    .into()
  });
  set_create_tab(cx, CreateTab::ImportFile, |_, cx| {
    cx.new(|_| {
      ImportFileTab::new(|plan, window, cx| {
        window.close_dialog(cx);
        ModpackInstallDialog::open_plan(plan, window, cx);
      })
    })
    .into()
  });
  set_menu_action(cx, MenuAction::Duplicate, open_duplicate_dialog);
  set_menu_action(cx, MenuAction::Export, export_mrpack);
  set_menu_action(cx, MenuAction::ChangeVersion, |id, window, cx| {
    ModpackInstallDialog::open_change(&id, window, cx)
  });
}
