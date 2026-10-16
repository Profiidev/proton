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
pub fn register_hooks(_cx: &mut App) {
  // ===== integrator: these hooks live on other branches (2a profiles, 2b profile header) =====
  // use crate::content::{
  //   FromModpackTab, ImportFileTab, ModpackInstallDialog, ProjectRef, export_mrpack,
  //   open_duplicate_dialog,
  // };
  // use gpui_kit::component::WindowExt;
  // crate::pages::profiles::set_create_tab(_cx, crate::pages::profiles::CreateTab::FromModpack, |window, cx| {
  //   cx.new(|cx| {
  //     FromModpackTab::new(
  //       |project, window, cx| {
  //         window.close_dialog(cx);
  //         ModpackInstallDialog::open(ProjectRef::from(&project), window, cx);
  //       },
  //       window,
  //       cx,
  //     )
  //   })
  //   .into()
  // });
  // crate::pages::profiles::set_create_tab(_cx, crate::pages::profiles::CreateTab::ImportFile, |_, cx| {
  //   cx.new(|_| {
  //     ImportFileTab::new(|plan, window, cx| {
  //       window.close_dialog(cx);
  //       ModpackInstallDialog::open_plan(plan, window, cx);
  //     })
  //   })
  //   .into()
  // });
  // crate::pages::profile::set_menu_action(_cx, crate::pages::profile::MenuAction::Duplicate, open_duplicate_dialog);
  // crate::pages::profile::set_menu_action(_cx, crate::pages::profile::MenuAction::Export, export_mrpack);
  // "Change pack version" (2b): ModpackInstallDialog::open_change(&profile_id, window, cx)
  // ===== end integrator =====
}
