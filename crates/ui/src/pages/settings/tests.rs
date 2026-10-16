//! Each page against a temp copy of `fixtures/data` (Steve_Fixture active, Alex_Fixture)

use std::{
  path::Path,
  time::{Duration, Instant},
};

use gpui_kit::{
  self as gpui, AnyWindowHandle, App, AppContext, Bounds, TestAppContext, WindowBounds,
  WindowOptions, component::WindowExt, px, size, test::TestWindowExt,
};
use proton_account::AccountsExt;
use proton_config::{OfflineExt, SettingsExt};

use crate::{
  router::{Route, RouterExt, SettingsTab},
  shell::Shell,
};

const STEVE: &str = "f1f1f1f1f1f14f1f8f1f1f1f1f1f1f1f";
const ALEX: &str = "a2a2a2a2a2a24a2a8a2a2a2a2a2a2a2a";

fn copy_dir(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap();
  for entry in std::fs::read_dir(from).unwrap() {
    let entry = entry.unwrap();
    std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
  }
}

/// The shell at `tab`, services loaded from the fixture data
fn open(cx: &mut TestAppContext, tab: SettingsTab) -> AnyWindowHandle {
  cx.executor().allow_parking();
  let fixtures =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/io.profidev.proton");
  let home = tempfile::tempdir().unwrap().keep();
  let data = home.join("io.profidev.proton");
  copy_dir(&fixtures.join("skins"), &data.join("skins"));
  std::fs::copy(fixtures.join("store.json"), data.join("store.json")).unwrap();
  // SAFETY: nextest runs every test in its own process, nothing else reads the env yet
  unsafe { std::env::set_var("XDG_DATA_HOME", &home) };
  proton_store::init_at(data.join("store.json"));

  cx.update(|cx| {
    gpui_kit::init(cx);
    proton_components::assets::load(cx).unwrap();
    proton_config::init(cx);
    proton_versions::init(cx);
    proton_account::init(cx).unwrap();
    crate::init(cx);
    super::register(cx);
    cx.navigate(Route::Settings(tab));
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
  })
}

/// Runs the app (and the tokio services) until `done`, at most 10s
fn settle(cx: &mut TestAppContext, mut done: impl FnMut(&mut App) -> bool) {
  let start = Instant::now();
  loop {
    cx.run_until_parked();
    if cx.update(&mut done) {
      return;
    }
    assert!(start.elapsed() < Duration::from_secs(10), "timed out");
    std::thread::sleep(Duration::from_millis(10));
  }
}

fn act(
  cx: &mut TestAppContext,
  handle: AnyWindowHandle,
  f: impl FnOnce(&mut gpui::Window, &mut App),
) {
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    f(window, cx);
  })
  .unwrap();
  cx.run_until_parked();
}

fn toasts(cx: &mut TestAppContext, handle: AnyWindowHandle) -> usize {
  cx.update_window(handle, |_, window, cx| window.notifications(cx).len())
    .unwrap()
}

fn active(cx: &App) -> Option<String> {
  cx.accounts().active_id(cx).cloned()
}

#[gpui::test]
fn accounts_select_and_remove(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Accounts);
  assert_eq!(cx.update(|cx| active(cx)).as_deref(), Some(STEVE));
  act(cx, handle, |w, cx| w.click(format!("select-{ALEX}"), cx));
  settle(cx, |cx| active(cx).as_deref() == Some(ALEX));

  act(cx, handle, |w, cx| w.click(format!("remove-{STEVE}"), cx));
  act(cx, handle, |w, cx| {
    assert!(w.has_active_dialog(cx));
    w.within("dialog").click("ok", cx);
  });
  settle(cx, |cx| {
    !cx.accounts().accounts.read(cx).contains_key(STEVE)
  });
  settle(cx, |_| true);
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
fn add_account_offline_warns(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Accounts);
  cx.update(|cx| {
    let offline = cx.offline().offline.clone();
    offline.update(cx, |o, cx| {
      *o = true;
      cx.notify();
    })
  });
  act(cx, handle, |w, cx| w.click("add-account", cx));
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
fn account_selector_switches(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::About);
  act(cx, handle, |w, cx| w.click("account-selector", cx));
  act(cx, handle, |w, cx| w.click(format!("account-{ALEX}"), cx));
  settle(cx, |cx| active(cx).as_deref() == Some(ALEX));
}

#[gpui::test]
fn skins_page_adds_from_file(cx: &mut TestAppContext) {
  // the idle animation would keep the executor busy forever
  cx.update(|cx| cx.set_reduce_motion(true));
  let handle = open(cx, SettingsTab::Skins);
  act(cx, handle, |w, _| {
    assert!(w.try_find("skin-animation").is_some());
  });
  act(cx, handle, |w, cx| w.click("add-skin", cx));
  assert!(cx.did_prompt_for_paths());
}

#[gpui::test]
fn capes_offline_change_warns(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Capes);
  cx.update(|cx| {
    let offline = cx.offline().offline.clone();
    offline.update(cx, |o, cx| {
      *o = true;
      cx.notify();
    })
  });
  // the grid shows once the skin texture is decoded
  settle(cx, |cx| {
    cx.update_window(handle, |_, w, cx| {
      w.render_frame(cx);
      w.try_find("cape-REMOVE_CAPE").is_some()
    })
    .unwrap()
  });
  act(cx, handle, |w, cx| w.click("cape-REMOVE_CAPE", cx));
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
fn minecraft_switches_save(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Minecraft);
  act(cx, handle, |w, cx| w.click("show-snapshots", cx));
  act(cx, handle, |w, cx| {
    w.within("game-settings").click("use-custom", cx)
  });
  cx.update(|cx| {
    let s = cx.settings().get(cx);
    assert!(s.minecraft.show_snapshots);
    assert!(s.minecraft.game_settings.use_custom);
  });
  act(cx, handle, |w, cx| w.click("add-folder", cx));
  cx.simulate_path_prompt_response(|_| Some(vec!["/tmp/dl".into()]));
  cx.run_until_parked();
  cx.update(|cx| {
    assert_eq!(
      cx.settings().get(cx).download_dirs,
      Some(vec!["/tmp/dl".into()])
    )
  });
  act(cx, handle, |w, cx| w.click("remove-dir-/tmp/dl", cx));
  cx.update(|cx| assert_eq!(cx.settings().get(cx).download_dirs, Some(vec![])));
}

#[gpui::test]
fn java_env_needs_key_and_value(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Java);
  act(cx, handle, |w, cx| {
    w.within("java-settings").click("add-env", cx)
  });
  assert_eq!(toasts(cx, handle), 1);
  cx.update(|cx| {
    assert!(
      cx.settings()
        .get(cx)
        .minecraft
        .jvm_settings
        .env_vars
        .is_empty()
    )
  });
}

#[gpui::test]
fn about_checks_updates_and_saves(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::About);
  act(cx, handle, |w, cx| w.click("check-updates", cx));
  settle(cx, |_| true);
  // stub updater: "No updates available"
  assert_eq!(toasts(cx, handle), 1);
  act(cx, handle, |w, cx| w.click("auto-update", cx));
  cx.update(|cx| assert!(cx.settings().get(cx).auto_update));
}
