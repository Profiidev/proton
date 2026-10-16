use std::{cell::Cell, rc::Rc};

use gpui_kit::{
  self as gpui, AnyWindowHandle, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions,
  px, size, test::TestWindowExt,
};

use super::*;
use crate::router::{Kind, ProfileTab, QuickPlayTab, RouterExt};

/// Opens the shell; counts page builds
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Rc<Cell<usize>>) {
  let builds = Rc::new(Cell::new(0));
  let counter = builds.clone();
  let dir = tempfile::tempdir().unwrap().keep();
  proton_store::init_at(dir.join("store.json"));
  let handle = cx.update(|cx| {
    gpui_kit::init(cx);
    proton_components::assets::load(cx).unwrap();
    proton_config::init(cx);
    proton_versions::init_state(cx);
    crate::init(cx);
    PageFactory::register(
      cx,
      |_| true,
      move |route, _, cx| {
        counter.set(counter.get() + 1);
        cx.new(|_| Placeholder(route.to_url().into())).into()
      },
    );
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(1300.), px(800.)),
        cx,
      ))),
      ..Default::default()
    };
    gpui_kit::open_window(options, cx, |window, cx| {
      cx.new(|cx| Shell::new(window, cx))
    })
    .unwrap()
    .0
  });
  (handle, builds)
}

fn click(handle: AnyWindowHandle, id: &str, cx: &mut TestAppContext) {
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.click(id.to_string(), cx);
  })
  .unwrap();
  cx.run_until_parked();
}

fn route(cx: &mut TestAppContext) -> Route {
  cx.update(|cx| cx.route())
}

#[gpui::test]
fn sidebar_navigates_and_pages_are_cached(cx: &mut TestAppContext) {
  let (handle, builds) = open(cx);
  click(handle, "nav-Profiles", cx);
  assert_eq!(route(cx), Route::Profiles);
  click(handle, "nav-Shaders", cx);
  assert_eq!(route(cx), Route::Discover(Kind::Shader));
  click(handle, "nav-Settings", cx);
  assert_eq!(route(cx), Route::Settings(SettingsTab::Accounts));
  // the nested settings nav
  click(handle, "tab-Java", cx);
  assert_eq!(route(cx), Route::Settings(SettingsTab::Java));
  // home, profiles, shaders, accounts, java
  assert_eq!(builds.get(), 5);

  let router = cx.update(|cx| cx.router());
  router.update(cx, |r, cx| r.back(cx));
  assert_eq!(route(cx), Route::Settings(SettingsTab::Accounts));
  router.update(cx, |r, cx| r.back(cx));
  assert_eq!(route(cx), Route::Discover(Kind::Shader));
  router.update(cx, |r, cx| r.forward(cx));
  assert_eq!(route(cx), Route::Settings(SettingsTab::Accounts));
  cx.update_window(handle, |_, window, cx| window.render_frame(cx))
    .unwrap();
  // revisited pages come from the cache
  assert_eq!(builds.get(), 5);
}

#[gpui::test]
fn profile_tabs_and_breadcrumbs(cx: &mut TestAppContext) {
  let (handle, _) = open(cx);
  cx.update(|cx| {
    cx.shell_data().update(cx, |d, _| {
      d.names.profiles.insert("p".into(), "Survival".into());
    });
    cx.navigate(Route::Profile {
      id: "p".into(),
      tab: ProfileTab::Logs,
    });
  });
  click(handle, "tab-Quick play", cx);
  assert_eq!(
    route(cx),
    Route::Profile {
      id: "p".into(),
      tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
    }
  );
  // crumbs: Profiles > Survival > Quick Play > Singleplayer, ids are `crumb-<index>`
  click(handle, "crumb-0", cx);
  assert_eq!(route(cx), Route::Profiles);
}

#[gpui::test]
fn offline_badge_asks_to_reconnect(cx: &mut TestAppContext) {
  let (handle, _) = open(cx);
  let asked = Rc::new(Cell::new(false));
  let flag = asked.clone();
  let data = cx.update(|cx| cx.shell_data());
  let _sub = cx.update(|cx| cx.subscribe(&data, move |_, ShellEvent::Reconnect, _| flag.set(true)));
  assert!(cx.update(|cx| {
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.try_find("offline").is_none()
    })
    .unwrap()
  }));
  data.update(cx, |d, cx| {
    d.offline = true;
    cx.notify();
  });
  cx.run_until_parked();
  click(handle, "offline", cx);
  assert!(asked.get());
}
