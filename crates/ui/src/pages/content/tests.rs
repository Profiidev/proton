use std::{
  cell::RefCell,
  path::{Path, PathBuf},
  rc::Rc,
};

use gpui_kit::{
  self as gpui, AnyWindowHandle, FileDropEvent, PlatformInput, TestAppContext,
  component::WindowExt, test::TestWindowExt,
};
use proton_components::test_view;

use super::*;

const FABRIC: &str = "22222222-2222-4222-8222-222222222222";
const FABRIC_API: &str = "fabric-api-0.129.0+1.21.8.jar";

fn copy_dir(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap();
  for entry in std::fs::read_dir(from).unwrap() {
    let entry = entry.unwrap();
    let target = to.join(entry.file_name());
    if entry.file_type().unwrap().is_dir() {
      copy_dir(&entry.path(), &target);
    } else {
      std::fs::copy(entry.path(), target).unwrap();
    }
  }
}

fn fresh_data() -> (PathBuf, PathBuf) {
  let dir = tempfile::tempdir().unwrap().keep();
  let fixture =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/io.profidev.proton");
  copy_dir(&fixture, &dir);
  proton_store::init_at(dir.join("store.json"));
  let prof = dir.join("profiles").join(FABRIC);
  (dir, prof)
}

/// Services on the fixture data and a window showing the mods tab of the fabric profile
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<ContentPage>, PathBuf) {
  open_with(cx, 0)
}

/// [`open`] with `extra` generated `generated-<n>.jar` files in the mods folder
fn open_with(
  cx: &mut TestAppContext,
  extra: usize,
) -> (AnyWindowHandle, Entity<ContentPage>, PathBuf) {
  cx.executor().allow_parking();
  let (data, prof) = fresh_data();
  for n in 0..extra {
    std::fs::write(prof.join(format!("mods/generated-{n:03}.jar")), b"jar").unwrap();
  }
  cx.update(|cx| {
    proton_config::init(cx);
    let (handle, _) = proton_versions::event::DownloadsHandle::new();
    let versions = proton_versions::Versions {
      versions: cx.new(|_| Default::default()),
      downloads: cx.new(|_| Default::default()),
      handle,
    };
    cx.set_global(versions);
    proton_profiles::init_at(cx, data).unwrap();
    proton_content::init(cx);
    crate::init(cx);
  });
  let page = Rc::new(RefCell::new(None::<Entity<ContentPage>>));
  let slot = page.clone();
  let (handle, _) = test_view::open(cx, move |window, cx| {
    let page = slot
      .borrow_mut()
      .get_or_insert_with(|| {
        cx.new(|cx| ContentPage::new(FABRIC.into(), ContentKind::Mod, window, cx))
      })
      .clone();
    div().size_full().child(page).into_any_element()
  });
  draw(handle, cx);
  let page = page.borrow().clone().unwrap();
  wait(cx, || {
    let page = page.clone();
    move |cx: &mut App| page.read(cx).summary.total == 7 + extra
  });
  draw(handle, cx);
  (handle, page, prof)
}

fn draw(handle: AnyWindowHandle, cx: &mut TestAppContext) {
  cx.run_until_parked();
  test_view::draw(handle, cx);
}

/// Waits for tokio work (scans, file moves) to land
fn wait<F: Fn(&mut App) -> bool>(cx: &mut TestAppContext, done: impl Fn() -> F) {
  for _ in 0..2000 {
    cx.run_until_parked();
    if cx.update(done()) {
      return;
    }
    std::thread::sleep(std::time::Duration::from_millis(5));
  }
  panic!("timed out");
}

fn renders(page: &Entity<ContentPage>, cx: &mut TestAppContext) -> HashMap<String, usize> {
  cx.update(|cx| {
    page
      .read(cx)
      .row_views
      .iter()
      .map(|(f, v)| (f.clone(), v.read(cx).renders))
      .collect()
  })
}

#[gpui::test]
fn renders_the_fixture_mods(cx: &mut TestAppContext) {
  let (handle, page, profile_dir) = open(cx);
  let visible = cx.update(|cx| page.read(cx).summary.visible.clone());
  assert_eq!(visible[0], "debugify-1.21.8+1.0.jar");
  assert_eq!(visible[1], FABRIC_API);
  assert_eq!(visible.len(), 7);
  cx.update_window(handle, |_, window, cx| {
    assert!(window.try_find(SharedString::from(FABRIC_API)).is_some());
    assert_eq!(
      window
        .within(SharedString::from(FABRIC_API))
        .find("toggle")
        .checked(),
      Some(true)
    );
    // clicking the switch enables Debugify on disk
    window
      .within(SharedString::from("debugify-1.21.8+1.0.jar"))
      .click("toggle", cx);
  })
  .unwrap();
  let enabled = profile_dir.join("mods/debugify-1.21.8+1.0.jar");
  wait(cx, || {
    let enabled = enabled.clone();
    move |_: &mut App| enabled.exists()
  });
}

#[gpui::test]
fn toggle_disables_and_re_renders_only_that_row(cx: &mut TestAppContext) {
  let (handle, page, profile_dir) = open(cx);
  // plain frames: `render_frame` and hover changes refresh the whole window on purpose
  let frame = |cx: &mut TestAppContext| {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))
      .unwrap();
  };
  frame(cx);
  let before = renders(&page, cx);
  cx.update_window(handle, |_, window, cx| {
    page.update(cx, |p, cx| p.toggle(FABRIC_API.into(), false, window, cx));
  })
  .unwrap();
  let disabled = profile_dir.join(format!("mods/{FABRIC_API}.disabled"));
  wait(cx, || {
    let disabled = disabled.clone();
    move |_: &mut App| disabled.exists()
  });
  // the refresh after the command lands too
  wait(cx, || {
    let page = page.clone();
    move |cx: &mut App| {
      let installed = page.read(cx).installed.read(cx);
      installed
        .get(ContentKind::Mod)
        .iter()
        .any(|m| m.file_name.starts_with("fabric-api") && !m.enabled)
    }
  });
  frame(cx);
  frame(cx);

  let after = renders(&page, cx);
  for (file, n) in &after {
    let expected =
      before.get(file).copied().unwrap_or(2) + usize::from(file.starts_with("fabric-api"));
    assert_eq!(*n, expected, "{file}");
  }
}

#[gpui::test]
fn dropping_files_adds_them(cx: &mut TestAppContext) {
  let (handle, page, profile_dir) = open(cx);
  let dir = tempfile::tempdir().unwrap();
  let jar = dir.path().join("dropped.jar");
  std::fs::write(&jar, b"jar").unwrap();
  let position = cx
    .update_window(handle, |_, window, _| {
      window.find("drop-zone").bounds().center()
    })
    .unwrap();
  cx.update_window(handle, |_, window, cx| {
    let paths = ExternalPaths(vec![jar.clone(), dir.path().join("wrong.zip")].into());
    window.dispatch_event(
      PlatformInput::FileDrop(FileDropEvent::Entered { position, paths }),
      cx,
    );
    window.dispatch_event(
      PlatformInput::FileDrop(FileDropEvent::Submit { position }),
      cx,
    );
  })
  .unwrap();
  wait(cx, || {
    let page = page.clone();
    move |cx: &mut App| page.read(cx).summary.total == 8
  });
  assert!(profile_dir.join("mods/dropped.jar").exists());
}

#[gpui::test]
fn bulk_remove_of_a_manual_file_asks_first(cx: &mut TestAppContext) {
  let (handle, _, profile_dir) = open(cx);
  cx.update_window(handle, |_, window, cx| {
    window
      .within(SharedString::from("local-tweaks-1.0.jar"))
      .click("select", cx);
  })
  .unwrap();
  draw(handle, cx);
  cx.update_window(handle, |_, window, cx| {
    window.click("bulk-remove", cx);
  })
  .unwrap();
  draw(handle, cx);
  cx.update_window(handle, |_, window, cx| {
    assert!(window.has_active_dialog(cx));
  })
  .unwrap();
  assert!(profile_dir.join("mods/local-tweaks-1.0.jar").exists());
}

#[gpui::test]
fn a_500_mod_list_renders_only_visible_rows(cx: &mut TestAppContext) {
  let (handle, page, _) = open_with(cx, 500);
  let started = std::time::Instant::now();
  cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))
    .unwrap();
  // ponytail: generous for debug builds and CI; catches an O(n) render, not a slow one
  assert!(started.elapsed() < std::time::Duration::from_millis(500));
  let first = renders(&page, cx).values().filter(|n| **n > 0).count();
  // virtualized: the 800px window fits a dozen rows, the list renders those and some overdraw
  assert!(first < 50, "{first} of 507 rows rendered");

  cx.update_window(handle, |_, window, cx| {
    window.scroll(
      SharedString::from("generated-005.jar"),
      gpui_kit::ScrollDelta::Pixels(gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(-20_000.))),
      cx,
    );
    window.draw(cx).clear(cx);
  })
  .unwrap();
  let rendered = renders(&page, cx).values().filter(|n| **n > 0).count();
  assert!(
    rendered > first && rendered < 100,
    "{rendered} of 507 rows rendered after scrolling"
  );
}
