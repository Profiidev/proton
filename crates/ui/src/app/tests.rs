use std::{cell::Cell, rc::Rc};

use gpui_kit::{self as gpui, TestAppContext, component::WindowExt};

use super::*;
use crate::{router::SettingsTab, shell::Placeholder};

/// Temp store + data dir, config and the router
fn init_test(cx: &mut TestAppContext) -> std::path::PathBuf {
  let dir = tempfile::tempdir().unwrap().keep();
  proton_store::init_at(dir.join("store.json"));
  cx.update(|cx| {
    proton_config::init(cx);
    crate::router::init(cx, Route::Home);
  });
  dir
}

#[gpui::test]
fn route_is_persisted_once_after_the_debounce(cx: &mut TestAppContext) {
  init_test(cx);
  cx.update(persist_route);
  let writes = Rc::new(Cell::new(0));
  let seen = writes.clone();
  cx.update(|cx| {
    cx.observe(&cx.settings().settings.clone(), move |_, _| {
      seen.set(seen.get() + 1)
    })
    .detach();
    cx.navigate(Route::Profiles);
    cx.navigate(Route::Instances);
    cx.navigate(Route::Settings(SettingsTab::Java));
  });
  cx.executor().advance_clock(PERSIST_DELAY / 2);
  cx.run_until_parked();
  assert_eq!(writes.get(), 0);
  cx.executor().advance_clock(PERSIST_DELAY);
  cx.run_until_parked();
  assert_eq!(writes.get(), 1);
  let url = proton_config::settings::Settings::load().unwrap().url;
  assert_eq!(url.as_deref(), Some("/settings/java"));
}

#[gpui::test]
fn start_route_comes_from_the_settings(cx: &mut TestAppContext) {
  init_test(cx);
  assert_eq!(cx.update(|cx| initial_route(cx)), Route::Home);
  cx.update(|cx| proton_config::update(cx, |s| s.url = Some("tauri://localhost/instances".into())));
  assert_eq!(cx.update(|cx| initial_route(cx)), Route::Instances);
}

#[gpui::test]
fn crash_and_manifest_errors_toast(cx: &mut TestAppContext) {
  let dir = init_test(cx);
  let window = cx.update(|cx| {
    gpui_kit::init(cx);
    proton_components::assets::load(cx).unwrap();
    proton_profiles::init_at(cx, dir.join("data")).unwrap();
    toasts(cx);
    gpui_kit::open_window(Default::default(), cx, |_, cx| {
      cx.new(|_| Placeholder("x".into()))
    })
    .unwrap()
    .0
  });
  let count = |cx: &mut TestAppContext| {
    cx.update_window(window, |_, window, cx| window.notifications(cx).len())
      .unwrap()
  };
  assert_eq!(count(cx), 0);
  cx.update(|cx| {
    cx.profiles().events.clone().update(cx, |_, cx| {
      cx.emit(ProfilesEvent::InstanceCrashed {
        profile_name: "Survival".into(),
      })
    })
  });
  cx.run_until_parked();
  assert_eq!(count(cx), 1);
  cx.update(|cx| cx.offline().handle.manifest_refresh_failed());
  cx.run_until_parked();
  assert_eq!(count(cx), 2);
}

#[gpui::test]
fn shell_data_mirrors_offline_only_on_change(cx: &mut TestAppContext) {
  init_test(cx);
  cx.update(|cx| {
    crate::shell::init(cx);
    let offline = cx.offline().offline.clone();
    mirror(cx, offline, |v, d| d.offline = *v);
  });
  let notes = Rc::new(Cell::new(0));
  let seen = notes.clone();
  cx.update(|cx| {
    cx.observe(&cx.shell_data(), move |_, _| seen.set(seen.get() + 1))
      .detach();
    let offline = cx.offline().offline.clone();
    offline.write_changed(cx, true);
  });
  cx.run_until_parked();
  assert!(cx.update(|cx| cx.shell_data().read(cx).offline));
  // a settings-like unrelated notify does not touch shell data
  cx.update(|cx| cx.offline().offline.clone().update(cx, |_, cx| cx.notify()));
  cx.run_until_parked();
  assert_eq!(notes.get(), 1);
}

#[gpui::test]
fn saved_window_bounds_restore_when_they_touch_a_display(cx: &mut TestAppContext) {
  init_test(cx);
  let display = cx.update(|cx| cx.displays()[0].bounds());
  let state = |x, y, width, height, maximized| WindowState {
    x,
    y,
    width,
    height,
    maximized,
  };
  let restore = |saved: Option<WindowState>, cx: &mut TestAppContext| {
    cx.update(|cx| {
      proton_config::update(cx, |s| s.window = saved);
      restored_bounds(cx)
    })
  };
  let centered = |w: f32, h: f32, cx: &mut TestAppContext| {
    cx.update(|cx| Bounds::centered(None, size(px(w), px(h)), cx))
  };

  // nothing saved: the default size, centered
  let expected = WindowBounds::Windowed(centered(1300., 800., cx));
  assert_eq!(restore(None, cx), expected);

  let on_screen = state(10., 20., 900., 700., false);
  assert_eq!(
    restore(Some(on_screen), cx),
    WindowBounds::Windowed(Bounds::new(
      point(px(10.), px(20.)),
      size(px(900.), px(700.))
    ))
  );
  assert_eq!(
    restore(Some(state(10., 20., 900., 700., true)), cx),
    WindowBounds::Maximized(Bounds::new(
      point(px(10.), px(20.)),
      size(px(900.), px(700.))
    ))
  );

  // a display that is gone: the saved size, centered
  let far = f32::from(display.size.width) * 4.;
  let expected = WindowBounds::Windowed(centered(900., 700., cx));
  assert_eq!(
    restore(Some(state(far, far, 900., 700., false)), cx),
    expected
  );

  // too small (or garbage) sizes are raised to the minimum
  let tiny = restore(Some(state(10., 20., 10., -5., false)), cx);
  assert_eq!(
    tiny,
    WindowBounds::Windowed(Bounds::new(
      point(px(10.), px(20.)),
      size(px(600.), px(400.))
    ))
  );
}

#[gpui::test]
fn bridge_mirrors_names_and_only_profiles_with_updates(cx: &mut TestAppContext) {
  use proton_content::ContentUpdates;
  crate::pages::profiles::tests::init_fixture(cx);
  crate::pages::profiles::tests::poll(cx, |cx| *cx.profiles().loaded.read(cx));
  let data = cx.update(|cx| cx.shell_data());
  let names = data.read_with(cx, |d, _| d.names.profiles.len());
  assert_eq!(names, 4);

  let updates = cx.update(|cx| cx.global::<Content>().updates.clone());
  updates.update(cx, |u, cx| {
    *u = ContentUpdates(HashMap::from([("empty".to_string(), vec![])]));
    cx.notify();
  });
  cx.run_until_parked();
  // a profile whose list emptied gets no chip
  assert!(data.read_with(cx, |d, _| d.content_updates.is_empty()));

  cx.update(|cx| proton_config::update(cx, |s| s.hide_window_controls = true));
  cx.run_until_parked();
  assert!(data.read_with(cx, |d, _| d.hide_window_controls));
}
