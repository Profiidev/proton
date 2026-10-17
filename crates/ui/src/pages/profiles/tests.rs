use std::{
  fs,
  path::{Path, PathBuf},
  time::Duration,
};

use gpui_kit::{
  self as gpui, AnyWindowHandle, AppContext, Bounds, Context, Entity, Render, TestAppContext,
  Window, WindowBounds, WindowOptions, px, size, test::TestWindowExt,
};

use super::{create::CreateDialog, *};

fn copy_dir(from: &Path, to: &Path) {
  fs::create_dir_all(to).unwrap();
  for entry in fs::read_dir(from).unwrap() {
    let entry = entry.unwrap();
    let target = to.join(entry.file_name());
    if entry.file_type().unwrap().is_dir() {
      copy_dir(&entry.path(), &target);
    } else {
      fs::copy(entry.path(), target).unwrap();
    }
  }
}

/// A temp copy of `fixtures/data` with every service initialized like `main.rs`
pub(crate) fn init_fixture(cx: &mut TestAppContext) -> PathBuf {
  cx.executor().allow_parking();
  let tmp = tempfile::tempdir().unwrap().keep();
  copy_dir(
    &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data"),
    &tmp,
  );
  // nextest runs each test in its own process; the versions service reads the data dir
  unsafe { std::env::set_var("XDG_DATA_HOME", &tmp) };
  let data = tmp.join("io.profidev.proton");
  proton_store::init_at(data.join("store.json"));
  cx.update(|cx| {
    gpui_kit::init(cx);
    proton_components::assets::load(cx).unwrap();
    // animations run on the wall clock: a click mid dialog-entrance misses its target
    cx.set_reduce_motion(true);
    proton_config::init(cx);
    proton_versions::init(cx);
    proton_account::init(cx).unwrap();
    proton_profiles::init_at(cx, data.clone()).unwrap();
    proton_content::init(cx);
    crate::init(cx);
    crate::app::bridge(cx);
  });
  data
}

pub(crate) fn open<V: Render>(
  cx: &mut TestAppContext,
  build: impl FnOnce(&mut Window, &mut Context<V>) -> V + 'static,
) -> (AnyWindowHandle, Entity<V>) {
  cx.update(|cx| {
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(1300.), px(800.)),
        cx,
      ))),
      ..Default::default()
    };
    gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| build(window, cx))).unwrap()
  })
}

/// Waits for tokio-side work: the test clock does not move while parked on it
pub(crate) fn poll(cx: &mut TestAppContext, mut ready: impl FnMut(&mut App) -> bool) {
  for _ in 0..500 {
    cx.run_until_parked();
    if cx.update(&mut ready) {
      return;
    }
    std::thread::sleep(Duration::from_millis(10));
  }
  panic!("condition not met within 5s");
}

const FORGE: &str = "33333333-3333-4333-8333-333333333333";

fn names(page: &Entity<ProfilesPage>, cx: &mut TestAppContext) -> Vec<String> {
  cx.update(|cx| {
    page
      .read(cx)
      .visible(cx)
      .into_iter()
      .map(|p| p.name)
      .collect()
  })
}

#[gpui::test]
fn search_and_filters(cx: &mut TestAppContext) {
  init_fixture(cx);
  let (handle, page) = open(cx, ProfilesPage::new);
  cx.run_until_parked();
  cx.update_window(handle, |_, w, cx| {
    w.render_frame(cx);
    assert!(w.within(FORGE).try_find("open").is_some());
  })
  .unwrap();
  assert_eq!(names(&page, cx).len(), 4);

  let set = |text: &'static str, cx: &mut TestAppContext| {
    cx.update_window(handle, |_, window, cx| {
      let search = page.read(cx).search.clone();
      search.update(cx, |s, cx| s.set_value(text, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
  };
  set("forge", cx);
  assert_eq!(names(&page, cx), ["Forge Classic"]);
  // typo, the letters still appear in order
  set("perfrmance", cx);
  assert_eq!(names(&page, cx), ["Fabric Performance"]);
  set("zzz", cx);
  assert!(names(&page, cx).is_empty());
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    assert!(window.try_find("open").is_none());
  })
  .unwrap();

  set("", cx);
  page.update(cx, |p, _| p.loader_filter = vec!["Fabric".into()]);
  let mut fabric = names(&page, cx);
  fabric.sort();
  assert_eq!(fabric, ["Fabric Performance", "Fabulously Optimized"]);
  page.update(cx, |p, _| p.version_filter = vec!["1.20.1".into()]);
  assert!(names(&page, cx).is_empty());
}

#[gpui::test]
fn create_validates_and_creates(cx: &mut TestAppContext) {
  init_fixture(cx);
  let (handle, page) = open(cx, ProfilesPage::new);
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.click("new-profile", cx);
    window.render_frame(cx);
    assert!(window.try_find("create-profile").is_some());
  })
  .unwrap();

  let (handle, dialog) = open(cx, CreateDialog::new);
  // the vanilla list comes from the fixture manifest
  poll(cx, |cx| {
    dialog.read(cx).version.read(cx).value(cx).is_some()
  });

  cx.update_window(handle, |_, window, cx| window.click("create", cx))
    .unwrap();
  cx.run_until_parked();
  dialog.read_with(cx, |d, _| {
    assert_eq!(d.name_error.as_deref(), Some("Name is required"));
    assert!(d.version_error.is_none());
  });

  cx.update_window(handle, |_, window, cx| {
    let name = dialog.read(cx).name.clone();
    name.update(cx, |s, cx| s.set_value("Test SMP", window, cx));
    window.click("create", cx);
  })
  .unwrap();
  let profiles = cx.update(|cx| cx.profiles().profiles.clone());
  poll(cx, |cx| {
    profiles.read(cx).iter().any(|p| p.name == "Test SMP")
  });
  let version = cx.update(|cx| dialog.read(cx).version.read(cx).value(cx).unwrap());
  let created = cx.update(|cx| {
    profiles
      .read(cx)
      .iter()
      .find(|p| p.name == "Test SMP")
      .cloned()
      .unwrap()
  });
  assert_eq!(created.loader, LoaderType::Vanilla);
  assert_eq!(created.version, version.as_ref());
  cx.run_until_parked();
  assert!(names(&page, cx).contains(&"Test SMP".to_string()));
}
