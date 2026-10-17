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
use proton_config::SettingsExt;

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

fn profile(name: &str, loader: LoaderType, version: &str) -> Profile {
  serde_json::from_value(serde_json::json!({
    "id": name, "name": name, "created_at": "2026-01-01T00:00:00Z",
    "last_played": null, "version": version, "loader": loader, "loader_version": null,
    "downloaded": true, "use_local_game": false, "game": null, "use_local_jvm": false, "jvm": null
  }))
  .unwrap()
}

#[test]
fn matches_words_loaders_versions_and_typos() {
  let p = profile("Fabric Performance", LoaderType::Fabric, "1.21.8");
  let unicode = profile("Überleben Welt", LoaderType::Vanilla, "1.20.1");
  for (profile, query, expected) in [
    (&p, "", true),
    (&p, "   ", true),
    (&p, "FABRIC", true),
    (&p, "perf 1.21", true),
    // every word has to match
    (&p, "perf forge", false),
    (&p, "1.21.8", true),
    (&p, "1.20", false),
    // typo tolerance: letters in order within the name
    (&p, "fbrc", true),
    (&p, "crbf", false),
    (&unicode, "überleben", true),
    (&unicode, "ÜBER", true),
    (&unicode, "vanilla", true),
    (&unicode, "welt 1.20.1", true),
  ] {
    assert_eq!(matches(profile, query), expected, "{query:?}");
  }
}

#[test]
fn compare_profiles_puts_played_first() {
  let at = |t: &str| Some(t.parse().unwrap());
  let mut old = profile("old", LoaderType::Vanilla, "1");
  old.last_played = at("2024-01-01T00:00:00Z");
  let mut new = profile("new", LoaderType::Vanilla, "1");
  new.last_played = at("2024-02-01T00:00:00Z");
  let never_old = profile("never-old", LoaderType::Vanilla, "1");
  let mut never_new = profile("never-new", LoaderType::Vanilla, "1");
  never_new.created_at = "2026-06-01T00:00:00Z".parse().unwrap();

  let mut list = [
    never_old.clone(),
    old.clone(),
    never_new.clone(),
    new.clone(),
  ];
  list.sort_by(compare_profiles);
  let ids: Vec<_> = list.iter().map(|p| p.id.as_str()).collect();
  assert_eq!(ids, ["new", "old", "never-new", "never-old"]);
  assert_eq!(compare_profiles(&old, &old), Ordering::Equal);
}

#[test]
fn quick_play_tab_follows_the_entry_type() {
  let entry = |kind: &str| -> QuickPlayInfo {
    serde_json::from_value(serde_json::json!({
      "id": "x", "name": "x", "lastPlayedTime": "2024-01-01T00:00:00Z", "type": kind
    }))
    .unwrap()
  };
  assert_eq!(quick_play_tab(None), QuickPlayTab::Singleplayer);
  for (kind, tab) in [
    ("singleplayer", QuickPlayTab::Singleplayer),
    ("multiplayer", QuickPlayTab::Multiplayer),
    ("realms", QuickPlayTab::Realms),
  ] {
    assert_eq!(quick_play_tab(Some(&entry(kind))), tab, "{kind}");
  }
}

#[test]
fn icon_image_detects_jpeg_else_png() {
  assert_eq!(
    icon_image(vec![0xff, 0xd8, 0xff]).format,
    gpui_kit::ImageFormat::Jpeg
  );
  for bytes in [vec![0x89, b'P', b'N', b'G'], vec![], vec![0xff]] {
    assert_eq!(icon_image(bytes).format, gpui_kit::ImageFormat::Png);
  }
}

#[gpui::test]
fn filters_toggle_off_again_and_show_the_empty_hint(cx: &mut TestAppContext) {
  init_fixture(cx);
  let (handle, page) = open(cx, ProfilesPage::new);
  cx.run_until_parked();
  page.update(cx, |p, cx| p.toggle(true, "Forge".into(), cx));
  assert_eq!(names(&page, cx), ["Forge Classic"]);
  // filters within one select are OR-ed
  page.update(cx, |p, cx| p.toggle(true, "Vanilla".into(), cx));
  assert_eq!(names(&page, cx).len(), 2);
  page.update(cx, |p, cx| {
    p.toggle(true, "Forge".into(), cx);
    p.toggle(true, "Vanilla".into(), cx);
  });
  assert_eq!(names(&page, cx).len(), 4);
  // across selects they are AND-ed
  page.update(cx, |p, cx| {
    p.toggle(true, "Forge".into(), cx);
    p.toggle(false, "1.21.8".into(), cx);
  });
  assert!(names(&page, cx).is_empty());
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    assert!(window.try_find("open").is_none());
    // the popover trigger shows the picked filters
    assert!(window.try_find("loader-filter").is_some());
  })
  .unwrap();
}

/// A [`CreateDialog`] window once the fixture's version list is in
fn create_dialog(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<CreateDialog>) {
  init_fixture(cx);
  let (handle, dialog) = open(cx, CreateDialog::new);
  poll(cx, |cx| {
    dialog.read(cx).version.read(cx).value(cx).is_some()
  });
  (handle, dialog)
}

fn set_name(
  handle: AnyWindowHandle,
  dialog: &Entity<CreateDialog>,
  name: &'static str,
  cx: &mut TestAppContext,
) {
  cx.update_window(handle, |_, window, cx| {
    let input = dialog.read(cx).name.clone();
    input.update(cx, |s, cx| s.set_value(name, window, cx));
  })
  .unwrap();
}

fn profile_names(cx: &mut TestAppContext) -> Vec<String> {
  cx.update(|cx| {
    cx.profiles()
      .profiles
      .read(cx)
      .iter()
      .map(|p| p.name.clone())
      .collect()
  })
}

#[gpui::test]
fn create_name_error_clears_on_typing(cx: &mut TestAppContext) {
  let (handle, dialog) = create_dialog(cx);
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.click("create", cx);
  })
  .unwrap();
  assert!(dialog.read_with(cx, |d, _| d.name_error.is_some()));
  cx.update_window(handle, |_, window, cx| {
    window.click("name", cx);
    window.input("a", cx);
  })
  .unwrap();
  cx.run_until_parked();
  assert!(dialog.read_with(cx, |d, _| d.name_error.is_none()));
}

#[gpui::test]
fn create_accepts_unicode_whitespace_and_duplicate_names(cx: &mut TestAppContext) {
  let (handle, dialog) = create_dialog(cx);
  let before = profile_names(cx).len();
  // zod `min(1)` does not trim, so a blank looking name passes like in the old app
  for (name, expected) in [("Überleben ✨ 世界", 1), ("Vanilla Survival", 2), (" ", 1)] {
    set_name(handle, &dialog, name, cx);
    cx.update_window(handle, |_, window, cx| {
      dialog.update(cx, |d, cx| d.submit(window, cx))
    })
    .unwrap();
    poll(cx, |cx| {
      cx.profiles()
        .profiles
        .read(cx)
        .iter()
        .filter(|p| p.name == name)
        .count()
        == expected
    });
    assert!(
      dialog.read_with(cx, |d, _| d.name_error.is_none()),
      "{name}"
    );
    // let the dialog leave its submitting state before the next name
    std::thread::sleep(Duration::from_millis(20));
    cx.run_until_parked();
  }
  let names = profile_names(cx);
  assert_eq!(names.len(), before + 3);
  assert_eq!(names.iter().filter(|n| *n == "Vanilla Survival").count(), 2);
}

#[gpui::test]
fn create_twice_while_submitting_makes_one_profile(cx: &mut TestAppContext) {
  let (handle, dialog) = create_dialog(cx);
  set_name(handle, &dialog, "Once", cx);
  cx.update_window(handle, |_, window, cx| {
    dialog.update(cx, |d, cx| {
      d.submit(window, cx);
      d.submit(window, cx);
    })
  })
  .unwrap();
  poll(cx, |cx| profile_by_name(cx, "Once").is_some());
  std::thread::sleep(Duration::from_millis(100));
  cx.run_until_parked();
  let names = profile_names(cx);
  assert_eq!(names.iter().filter(|n| *n == "Once").count(), 1);
}

#[gpui::test]
fn create_enter_in_the_name_submits(cx: &mut TestAppContext) {
  let (handle, dialog) = create_dialog(cx);
  set_name(handle, &dialog, "Entered", cx);
  cx.update(|cx| {
    let name = dialog.read(cx).name.clone();
    name.update(cx, |_, cx| {
      cx.emit(InputEvent::PressEnter {
        secondary: false,
        shift: false,
      })
    });
  });
  poll(cx, |cx| {
    cx.profiles()
      .profiles
      .read(cx)
      .iter()
      .any(|p| p.name == "Entered")
  });
}

#[gpui::test]
fn create_modded_profile_takes_the_loader_version(cx: &mut TestAppContext) {
  let (handle, dialog) = create_dialog(cx);
  let loader_version =
    |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).loader_version.read(cx).value(cx));
  // vanilla has no loader versions
  assert_eq!(loader_version(cx), None);
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.click(("loader", 1usize), cx);
  })
  .unwrap();
  assert_eq!(dialog.read_with(cx, |d, _| d.loader), LoaderType::Fabric);
  poll(cx, |cx| {
    dialog.read(cx).loader_version.read(cx).value(cx).is_some()
  });
  let picked = loader_version(cx).unwrap();

  // back to vanilla clears it, fabric again picks it again
  cx.update_window(handle, |_, window, cx| {
    dialog.update(cx, |d, cx| d.set_loader(LoaderType::Vanilla, window, cx))
  })
  .unwrap();
  poll(cx, |cx| {
    dialog.read(cx).loader_version.read(cx).value(cx).is_none()
  });
  cx.update_window(handle, |_, window, cx| {
    dialog.update(cx, |d, cx| d.set_loader(LoaderType::Fabric, window, cx))
  })
  .unwrap();
  poll(cx, |cx| {
    dialog.read(cx).loader_version.read(cx).value(cx).as_ref() == Some(&picked)
  });

  set_name(handle, &dialog, "Modded", cx);
  cx.update_window(handle, |_, window, cx| {
    dialog.update(cx, |d, cx| d.submit(window, cx))
  })
  .unwrap();
  poll(cx, |cx| profile_by_name(cx, "Modded").is_some());
  let created = cx.update(|cx| profile_by_name(cx, "Modded").unwrap());
  assert_eq!(created.loader, LoaderType::Fabric);
  assert_eq!(created.loader_version.as_deref(), Some(picked.as_ref()));
}

fn profile_by_name(cx: &App, name: &str) -> Option<Profile> {
  cx.profiles()
    .profiles
    .read(cx)
    .iter()
    .find(|p| p.name == name)
    .cloned()
}

#[gpui::test]
fn create_icon_picker_skips_non_images_and_reports_bad_ones(cx: &mut TestAppContext) {
  let (handle, dialog) = create_dialog(cx);
  let dir = tempfile::tempdir().unwrap();
  let pick = |path: PathBuf, cx: &mut TestAppContext| {
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.click("image-input", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.simulate_path_prompt_response(move |_| Some(vec![path]));
    cx.run_until_parked();
  };
  let picked = |cx: &mut TestAppContext| {
    cx.update(|cx| {
      dialog
        .read(cx)
        .icon
        .read(cx)
        .picked()
        .map(Path::to_path_buf)
    })
  };
  let submit = |cx: &mut TestAppContext| {
    cx.update_window(handle, |_, window, cx| {
      dialog.update(cx, |d, cx| {
        d.icon_error = None;
        d.submit(window, cx)
      })
    })
    .unwrap();
  };
  set_name(handle, &dialog, "Iconic", cx);

  // the picker only takes png/jpeg, like `accept=`
  pick(dir.path().join("icon.GIF"), cx);
  assert_eq!(picked(cx), None);

  // a vanished file
  pick(dir.path().join("missing.PNG"), cx);
  assert!(picked(cx).is_some());
  submit(cx);
  // the read fails right away, before anything is sent
  assert_eq!(
    dialog.read_with(cx, |d, _| d.icon_error.clone()).as_deref(),
    Some("Invalid image")
  );

  // not an image inside
  let garbage = dir.path().join("garbage.png");
  fs::write(&garbage, b"not a png").unwrap();
  pick(garbage, cx);
  submit(cx);
  // the service refuses it as ProfileError::InvalidImage
  poll(cx, |cx| {
    dialog.read(cx).icon_error.as_deref() == Some("Invalid image")
  });
  assert!(cx.update(|cx| profile_by_name(cx, "Iconic")).is_none());

  let real = dir.path().join("real.jpeg");
  fs::copy(
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
      "../../fixtures/data/io.profidev.proton/profiles/{FORGE}/image.png"
    )),
    &real,
  )
  .unwrap();
  pick(real, cx);
  submit(cx);
  poll(cx, |cx| profile_by_name(cx, "Iconic").is_some());
}

#[gpui::test]
fn create_snapshot_switch_follows_the_setting(cx: &mut TestAppContext) {
  let (handle, dialog) = create_dialog(cx);
  let version =
    |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).version.read(cx).value(cx));
  assert_eq!(version(cx).as_deref(), Some("26.3"));
  let click = |cx: &mut TestAppContext| {
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.click("snapshots", cx);
    })
    .unwrap();
    cx.run_until_parked();
  };
  click(cx);
  assert!(cx.update(|cx| cx.settings().get(cx).minecraft.show_snapshots));
  // a release stays selected when the list grows
  std::thread::sleep(Duration::from_millis(50));
  poll(cx, |_| true);
  assert_eq!(version(cx).as_deref(), Some("26.3"));

  // nothing picked: the newest entry, a snapshot in the fixture manifest
  cx.update_window(handle, |_, window, cx| {
    dialog.update(cx, |d, cx| {
      d.version
        .update(cx, |s, cx| s.set_options(vec![], None, window, cx));
      d.set_loader(LoaderType::Fabric, window, cx);
    })
  })
  .unwrap();
  poll(cx, |cx| {
    dialog.read(cx).version.read(cx).value(cx).as_deref() == Some("26.4-snapshot-4")
  });
  // the snapshot is gone from the stable list, so the first release is picked
  click(cx);
  assert!(!cx.update(|cx| cx.settings().get(cx).minecraft.show_snapshots));
  poll(cx, |cx| {
    dialog.read(cx).version.read(cx).value(cx).as_deref() == Some("26.3")
  });
}

#[gpui::test]
fn create_tabs_swap_the_body_and_hide_the_footer(cx: &mut TestAppContext) {
  // the modpack tab searches on open
  let _server = proton_utils::test_http::FakeServer::start();
  let (handle, dialog) = create_dialog(cx);
  for (label, tab, footer) in [
    ("Import file", CreateTab::ImportFile, false),
    ("From modpack", CreateTab::FromModpack, false),
    ("Custom", CreateTab::Custom, true),
  ] {
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.within("create-tabs").click(label, cx);
      window.render_frame(cx);
      assert_eq!(window.try_find("create").is_some(), footer, "{label}");
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(dialog.read_with(cx, |d, _| d.tab), tab);
  }
}

#[gpui::test]
fn create_dialog_closes_on_cancel_and_on_success(cx: &mut TestAppContext) {
  use gpui_kit::component::WindowExt;

  init_fixture(cx);
  let (handle, _) = open(cx, ProfilesPage::new);
  let open_dialog = |cx: &mut TestAppContext| {
    cx.update_window(handle, |_, window, cx| {
      create::open(window, cx);
      window.render_frame(cx);
      assert!(window.has_active_dialog(cx));
    })
    .unwrap();
  };
  open_dialog(cx);
  cx.update_window(handle, |_, window, cx| {
    window.click("cancel", cx);
    window.render_frame(cx);
    assert!(!window.has_active_dialog(cx));
  })
  .unwrap();

  open_dialog(cx);
  // the version list loads in the background
  poll(cx, |_| true);
  std::thread::sleep(Duration::from_millis(50));
  cx.run_until_parked();
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.click("name", cx);
    window.input("Closed", cx);
    window.click("create", cx);
  })
  .unwrap();
  poll(cx, |cx| profile_by_name(cx, "Closed").is_some());
  cx.run_until_parked();
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    assert!(!window.has_active_dialog(cx));
  })
  .unwrap();
}

#[gpui::test]
fn profile_meta_loads_icons_and_mods_and_notifies_on_change_only(cx: &mut TestAppContext) {
  init_fixture(cx);
  let meta = cx.update(profile_meta);
  // the same entity for every page
  assert_eq!(cx.update(profile_meta), meta);
  poll(cx, |cx| {
    let m = meta.read(cx);
    m.icon(FORGE).is_some() && m.mods(FORGE).is_some()
  });
  const FABRIC: &str = "22222222-2222-4222-8222-222222222222";
  assert_eq!(cx.update(|cx| meta.read(cx).mods(FABRIC)), Some(6));
  assert_eq!(cx.update(|cx| meta.read(cx).mods("nope")), None);

  let notified = std::rc::Rc::new(std::cell::Cell::new(0));
  let count = notified.clone();
  let _sub = cx.update(|cx| cx.observe(&meta, move |_, _| count.set(count.get() + 1)));
  // a profile list refresh with the same data reloads but does not notify
  cx.update(|cx| {
    let list = cx.profiles().profiles.clone();
    list.update(cx, |_, cx| cx.notify())
  });
  std::thread::sleep(Duration::from_millis(100));
  poll(cx, |_| true);
  assert_eq!(notified.get(), 0);

  let data = std::env::var("XDG_DATA_HOME").unwrap();
  fs::write(
    Path::new(&data).join(format!("io.profidev.proton/profiles/{FORGE}/mods/new.jar")),
    b"jar",
  )
  .unwrap();
  cx.update(|cx| {
    let list = cx.profiles().profiles.clone();
    list.update(cx, |_, cx| cx.notify())
  });
  let before = cx.update(|cx| meta.read(cx).mods(FORGE).unwrap());
  poll(cx, |cx| meta.read(cx).mods(FORGE) == Some(before + 1));
  assert_eq!(notified.get(), 1);
}
