use std::path::{Path, PathBuf};

use gpui_kit::{
  self as gpui, AnyWindowHandle, AppContext, Entity, TestAppContext, WindowOptions,
  component::WindowExt, test::TestWindowExt,
};

use super::*;
use crate::router::{ProfileSettingsTab, QuickPlayTab, RouterExt};

/// Vanilla Survival: favorite, 2 worlds and 2 servers
const VANILLA: &str = "11111111-1111-4111-8111-111111111111";

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

/// Services on a copy of `fixtures/data`, a window showing `build`
fn open<V: 'static + gpui::Render>(
  cx: &mut TestAppContext,
  build: impl FnOnce(&mut gpui::Window, &mut gpui::Context<V>) -> V + 'static,
) -> (AnyWindowHandle, Entity<V>, PathBuf) {
  cx.executor().allow_parking();
  let home = tempfile::tempdir().unwrap().keep();
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data");
  copy_dir(&root, &home);
  // nextest runs every test in its own process, so the env var and the store do not leak
  unsafe { std::env::set_var("XDG_DATA_HOME", &home) };
  let data = home.join("io.profidev.proton");
  proton_store::init_at(data.join("store.json"));
  let (handle, view) = cx.update(|cx| {
    gpui_kit::init(cx);
    proton_components::assets::load(cx).unwrap();
    // animations run on the wall clock: a click mid dialog-entrance misses its target
    cx.set_reduce_motion(true);
    proton_config::init(cx);
    proton_versions::init(cx);
    proton_profiles::init_at(cx, data.clone()).unwrap();
    crate::init(cx);
    register(cx);
    gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
      cx.new(|cx| build(window, cx))
    })
    .unwrap()
  });
  cx.run_until_parked();
  (handle, view, data)
}

fn profile(cx: &mut TestAppContext, id: &str) -> Option<proton_profiles::config::Profile> {
  cx.update(|cx| find_profile(id, cx))
}

/// Lets tokio work finish and the listener apply it
fn settle(cx: &mut TestAppContext) {
  for _ in 0..20 {
    std::thread::sleep(std::time::Duration::from_millis(25));
    cx.run_until_parked();
  }
}

#[gpui::test]
async fn header_favorite_toggles(cx: &mut TestAppContext) {
  let (handle, _, _) = open(cx, |w, cx| ProfileHeader::new(VANILLA.into(), w, cx));
  assert!(profile(cx, VANILLA).unwrap().favorite);
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.click("favorite", cx);
  })
  .unwrap();
  settle(cx);
  assert!(!profile(cx, VANILLA).unwrap().favorite);
  // breadcrumbs know the name
  let name = cx.update(|cx| {
    cx.shell_data()
      .read(cx)
      .names
      .profiles
      .get(VANILLA)
      .cloned()
  });
  assert_eq!(name.as_deref(), Some("Vanilla Survival"));
}

#[gpui::test]
async fn header_delete_asks_then_removes(cx: &mut TestAppContext) {
  let (handle, _, _) = open(cx, |w, cx| ProfileHeader::new(VANILLA.into(), w, cx));
  cx.update(|cx| {
    cx.navigate(Route::Profile {
      id: VANILLA.into(),
      tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
    })
  });
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    header::delete(VANILLA.into(), "Vanilla Survival".into(), window, cx);
  })
  .unwrap();
  cx.run_until_parked();
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    assert!(window.has_active_dialog(cx));
    window.within("dialog").click("ok", cx);
  })
  .unwrap();
  settle(cx);
  assert!(profile(cx, VANILLA).is_none());
  // the open profile page goes back to the list
  assert_eq!(cx.update(|cx| cx.route()), Route::Profiles);
}

#[gpui::test]
async fn quick_play_lists_fixture_entries(cx: &mut TestAppContext) {
  let (handle, page, _) = open(cx, |w, cx| {
    QuickPlayPage::new(VANILLA.into(), QuickPlayTab::Singleplayer, w, cx)
  });
  settle(cx);
  let names = page.read_with(cx, |p, cx| {
    p.list(cx).into_iter().map(|q| q.name).collect::<Vec<_>>()
  });
  // newest first
  assert_eq!(names, ["New World", "Creative Test"]);
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    assert!(window.within("entry-0").try_find("play").is_some());
    assert!(window.within("entry-1").try_find("remove").is_some());
    assert!(window.try_find("entry-2").is_none());
  })
  .unwrap();
}

#[gpui::test]
async fn settings_rename_persists(cx: &mut TestAppContext) {
  let (_, page, data) = open(cx, |w, cx| {
    SettingsPage::new(VANILLA.into(), ProfileSettingsTab::General, w, cx)
  });
  cx.update_window(cx.windows()[0], |_, window, cx| {
    page.update(cx, |p, cx| {
      p.name
        .update(cx, |s, cx| s.set_value("  Renamed  ", window, cx));
      p.rename(cx);
    })
  })
  .unwrap();
  settle(cx);
  assert_eq!(profile(cx, VANILLA).unwrap().name, "Renamed");
  let json =
    std::fs::read_to_string(data.join(format!("profiles/{VANILLA}/profile.json"))).unwrap();
  assert!(json.contains("\"Renamed\""));

  // empty names are refused
  cx.update_window(cx.windows()[0], |_, window, cx| {
    page.update(cx, |p, cx| {
      p.name.update(cx, |s, cx| s.set_value("   ", window, cx));
      p.rename(cx);
    })
  })
  .unwrap();
  settle(cx);
  assert_eq!(profile(cx, VANILLA).unwrap().name, "Renamed");
}

// port of app/src/tests/unit/routes/profiles/info/quick_play/quick-play.test.ts
#[test]
fn players_label_and_note() {
  use proton_profiles::{config::QuickPlayInfo, server::ServerStatus};

  use super::quick_play::{players_label, show_note};

  let status = |online, players_online, players_max| ServerStatus {
    online,
    players_online,
    players_max,
  };
  assert_eq!(players_label(&status(false, 0, 0)), "Offline");
  assert_eq!(players_label(&status(true, 3, 20)), "3 / 20 online");

  let entry = |kind: &str| -> QuickPlayInfo {
    serde_json::from_value(serde_json::json!({
      "id": "x", "name": "x", "lastPlayedTime": "2024-01-01T00:00:00Z", "type": kind
    }))
    .unwrap()
  };
  assert!(show_note(&[]));
  assert!(show_note(&[entry("realms")]));
  assert!(!show_note(&[entry("singleplayer")]));
  assert!(!show_note(&[entry("multiplayer")]));
}

#[gpui::test]
async fn quick_play_remove_asks_then_drops_the_entry(cx: &mut TestAppContext) {
  let (handle, page, _) = open(cx, |w, cx| {
    QuickPlayPage::new(VANILLA.into(), QuickPlayTab::Singleplayer, w, cx)
  });
  settle(cx);
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.within("entry-1").click("remove", cx);
  })
  .unwrap();
  cx.run_until_parked();
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.within("dialog").click("ok", cx);
  })
  .unwrap();
  settle(cx);
  let names = page.read_with(cx, |p, cx| {
    p.list(cx).into_iter().map(|q| q.name).collect::<Vec<_>>()
  });
  assert_eq!(names, ["New World"]);
}

#[gpui::test]
async fn settings_override_switches_on(cx: &mut TestAppContext) {
  for (tab, java) in [
    (ProfileSettingsTab::Minecraft, false),
    (ProfileSettingsTab::Java, true),
  ] {
    let (handle, _, _) = open(cx, move |w, cx| {
      SettingsPage::new(VANILLA.into(), tab, w, cx)
    });
    let local = |cx: &mut TestAppContext| {
      let p = profile(cx, VANILLA).unwrap();
      if java {
        p.use_local_jvm
      } else {
        p.use_local_game
      }
    };
    assert!(!local(cx));
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.within("use-local").click("switch", cx);
    })
    .unwrap();
    settle(cx);
    assert!(local(cx), "{tab:?}");
  }
}
