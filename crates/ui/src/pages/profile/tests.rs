use std::path::{Path, PathBuf};

use gpui_kit::{
  self as gpui, AnyWindowHandle, AppContext, Entity, TestAppContext, WindowOptions,
  component::WindowExt, test::TestWindowExt,
};

use super::*;
use crate::{
  router::{ProfileSettingsTab, QuickPlayTab, RouterExt},
  shell::ShellDataExt,
};
use gpui_kit::{App, component::input::InputEvent};

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
    crate::app::bridge(cx);
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

const FORGE: &str = "33333333-3333-4333-8333-333333333333";
/// Fabulously Optimized, a modpack profile
const PACK: &str = "44444444-4444-4444-8444-444444444444";

fn toasts(cx: &mut TestAppContext, handle: AnyWindowHandle) -> usize {
  cx.update_window(handle, |_, window, cx| window.notifications(cx).len())
    .unwrap()
}

fn click(
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

#[gpui::test]
async fn header_shows_play_and_the_pack_action_only_for_modpacks(cx: &mut TestAppContext) {
  for (id, items) in [(VANILLA, 5), (PACK, 6)] {
    let (handle, _, _) = open(cx, move |w, cx| ProfileHeader::new(id.into(), w, cx));
    click(cx, handle, |w, cx| {
      // nothing runs
      assert!(w.try_find("play").is_some());
      assert!(w.try_find("stop").is_none());
      assert!(w.try_find("launch-another").is_none());
      w.click("menu", cx);
    });
    click(cx, handle, |w, _| {
      let menu = w.within("popup-menu");
      assert!(menu.try_find(items - 1).is_some(), "{id}");
      assert!(menu.try_find(items).is_none(), "{id}");
    });
  }
}

#[gpui::test]
async fn header_of_a_missing_profile_renders_nothing_and_leaves(cx: &mut TestAppContext) {
  let (handle, _, _) = open(cx, |w, cx| {
    // a stale saved route
    cx.navigate(Route::Profile {
      id: "gone".into(),
      tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
    });
    ProfileHeader::new("gone".into(), w, cx)
  });
  settle(cx);
  assert_eq!(cx.update(|cx| cx.route()), Route::Profiles);
  click(cx, handle, |w, _| assert!(w.try_find("play").is_none()));
}

#[gpui::test]
async fn header_does_not_leave_when_another_profile_is_open(cx: &mut TestAppContext) {
  let route = Route::Profile {
    id: VANILLA.into(),
    tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
  };
  let shown = route.clone();
  open(cx, move |w, cx| {
    cx.navigate(shown);
    ProfileHeader::new("gone".into(), w, cx)
  });
  settle(cx);
  assert_eq!(cx.update(|cx| cx.route()), route);
}

#[gpui::test]
async fn quick_play_tabs_navigate_and_realms_list_joins(cx: &mut TestAppContext) {
  let (handle, page, _) = open(cx, |w, cx| {
    QuickPlayPage::new(VANILLA.into(), QuickPlayTab::Realms, w, cx)
  });
  settle(cx);
  let names = page.read_with(cx, |p, cx| {
    p.list(cx).into_iter().map(|q| q.name).collect::<Vec<_>>()
  });
  assert_eq!(names, ["Fixture Realm"]);
  click(cx, handle, |w, cx| {
    // realms join, they have no favorite or remove buttons
    assert!(w.within("entry-0").try_find("play").is_some());
    assert!(w.within("entry-0").try_find("remove").is_none());
    w.within("quick-play-tabs").click("Singleplayer", cx);
  });
  assert_eq!(
    cx.update(|cx| cx.route()),
    Route::Profile {
      id: VANILLA.into(),
      tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
    }
  );
}

#[gpui::test]
async fn quick_play_without_entries_shows_the_empty_box(cx: &mut TestAppContext) {
  for tab in [
    QuickPlayTab::Singleplayer,
    QuickPlayTab::Multiplayer,
    QuickPlayTab::Realms,
  ] {
    let (handle, page, _) = open(cx, move |w, cx| {
      QuickPlayPage::new(FORGE.into(), tab, w, cx)
    });
    settle(cx);
    assert!(page.read_with(cx, |p, cx| p.list(cx).is_empty()));
    click(cx, handle, |w, _| {
      assert!(w.try_find("entry-0").is_none(), "{tab:?}");
      assert!(w.try_find("quick-play-tabs").is_some());
    });
  }
}

#[gpui::test]
async fn quick_play_favorite_toggles_the_world(cx: &mut TestAppContext) {
  let (handle, page, _) = open(cx, |w, cx| {
    QuickPlayPage::new(VANILLA.into(), QuickPlayTab::Singleplayer, w, cx)
  });
  settle(cx);
  let favorites = |cx: &mut TestAppContext| {
    page.read_with(cx, |p, cx| {
      p.list(cx)
        .into_iter()
        .map(|q| q.favorite)
        .collect::<Vec<_>>()
    })
  };
  assert_eq!(favorites(cx), [true, false]);
  click(cx, handle, |w, cx| {
    w.within("entry-1").click("favorite", cx)
  });
  settle(cx);
  assert_eq!(favorites(cx), [true, true]);
  click(cx, handle, |w, cx| {
    w.within("entry-0").click("favorite", cx)
  });
  settle(cx);
  assert_eq!(favorites(cx), [false, true]);
}

#[gpui::test]
async fn quick_play_server_status_is_fetched_once_per_ttl(cx: &mut TestAppContext) {
  use proton_utils::test_http::FakeServer;

  let server = FakeServer::start();
  server.route(
    "GET",
    "/api.mcstatus.io/v2/status/java/mc.hypixel.net",
    200,
    r#"{"online":true,"players":{"online":3,"max":20}}"#,
  );
  let build = |w: &mut gpui::Window, cx: &mut gpui::Context<QuickPlayPage>| {
    QuickPlayPage::new(VANILLA.into(), QuickPlayTab::Multiplayer, w, cx)
  };
  let (handle, _, _) = open(cx, build);
  settle(cx);
  let status = |server: &FakeServer| -> Vec<String> {
    server
      .calls()
      .into_iter()
      .map(|c| c.path)
      .filter(|p| p.contains("/status/"))
      .collect()
  };
  let mut paths = status(&server);
  paths.sort();
  // the unknown server 404s and shows no chip, the address is one encoded segment
  assert_eq!(
    paths,
    [
      "/api.mcstatus.io/v2/status/java/mc.hypixel.net",
      "/api.mcstatus.io/v2/status/java/play.example.org:25565",
    ]
  );
  click(cx, handle, |w, _| {
    assert!(w.within("entry-0").try_find("play").is_some());
    assert!(w.within("entry-1").try_find("play").is_some());
  });
  // another page within the ttl asks the cache, and a reload does too
  cx.update_window(handle, |_, w, cx| {
    let _ = cx.new(|cx| build(w, cx));
    let entries = cx.profiles().clone().quick_play(VANILLA, cx);
    entries.update(cx, |_, cx| cx.notify());
  })
  .unwrap();
  settle(cx);
  assert_eq!(status(&server).len(), 2);
}

#[gpui::test]
async fn settings_tabs_navigate(cx: &mut TestAppContext) {
  let (handle, _, _) = open(cx, |w, cx| {
    SettingsPage::new(VANILLA.into(), ProfileSettingsTab::General, w, cx)
  });
  for (label, tab) in [
    ("Java", ProfileSettingsTab::Java),
    ("Minecraft", ProfileSettingsTab::Minecraft),
    ("General", ProfileSettingsTab::General),
  ] {
    click(cx, handle, |w, cx| {
      w.within("profile-settings-tabs").click(label, cx)
    });
    assert_eq!(
      cx.update(|cx| cx.route()),
      Route::Profile {
        id: VANILLA.into(),
        tab: ProfileTab::Settings(tab),
      }
    );
  }
}

#[gpui::test]
async fn settings_rename_on_enter_and_blur_and_toast_when_empty(cx: &mut TestAppContext) {
  let (handle, page, _) = open(cx, |w, cx| {
    SettingsPage::new(VANILLA.into(), ProfileSettingsTab::General, w, cx)
  });
  let set = |text: &'static str, event: InputEvent, cx: &mut TestAppContext| {
    cx.update_window(handle, |_, window, cx| {
      let name = page.read(cx).name.clone();
      name.update(cx, |s, cx| {
        s.set_value(text, window, cx);
        cx.emit(event);
      });
    })
    .unwrap();
    settle(cx);
  };
  set(
    "Entered ✓",
    InputEvent::PressEnter {
      secondary: false,
      shift: false,
    },
    cx,
  );
  assert_eq!(profile(cx, VANILLA).unwrap().name, "Entered ✓");
  set("Blurred", InputEvent::Blur, cx);
  assert_eq!(profile(cx, VANILLA).unwrap().name, "Blurred");
  // other events do not save
  set("Focused", InputEvent::Focus, cx);
  assert_eq!(profile(cx, VANILLA).unwrap().name, "Blurred");

  assert_eq!(toasts(cx, handle), 0);
  set("", InputEvent::Blur, cx);
  assert_eq!(profile(cx, VANILLA).unwrap().name, "Blurred");
  // ponytail: `toast` from inside a window callback is dropped (the window is borrowed), so
  // the empty-name toast is checked from outside; see the report on `actions::toast`
  page.update(cx, |p, cx| p.rename(cx));
  cx.run_until_parked();
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
async fn settings_override_switches_back_off_and_keeps_the_values(cx: &mut TestAppContext) {
  let (handle, _, _) = open(cx, |w, cx| {
    SettingsPage::new(VANILLA.into(), ProfileSettingsTab::Minecraft, w, cx)
  });
  for expected in [true, false] {
    click(cx, handle, |w, cx| {
      w.within("use-local").click("switch", cx)
    });
    settle(cx);
    let p = profile(cx, VANILLA).unwrap();
    assert_eq!(p.use_local_game, expected);
    // the copied global settings stay for the next time
    assert!(p.game.is_some());
  }
}

#[gpui::test]
async fn settings_change_icon_saves_png_and_toasts_garbage(cx: &mut TestAppContext) {
  let (handle, _, data) = open(cx, |w, cx| {
    SettingsPage::new(FORGE.into(), ProfileSettingsTab::General, w, cx)
  });
  let dir = tempfile::tempdir().unwrap();
  let icon = data.join(format!("profiles/{FORGE}/image.png"));
  let pick = |path: Option<PathBuf>, cx: &mut TestAppContext| {
    click(cx, handle, |w, cx| w.click("change-icon", cx));
    cx.simulate_path_prompt_response(move |_| path.map(|p| vec![p]));
    settle(cx);
  };

  // canceled: nothing happens
  let before = std::fs::read(&icon).unwrap();
  pick(None, cx);
  assert_eq!(std::fs::read(&icon).unwrap(), before);
  assert_eq!(toasts(cx, handle), 0);

  let garbage = dir.path().join("garbage.png");
  std::fs::write(&garbage, b"nope").unwrap();
  pick(Some(garbage), cx);
  assert_eq!(toasts(cx, handle), 1);
  assert_eq!(std::fs::read(&icon).unwrap(), before);

  // a missing file toasts too
  pick(Some(dir.path().join("missing.png")), cx);
  assert_eq!(toasts(cx, handle), 2);

  let new = data.join(format!("profiles/{VANILLA}/image.png"));
  pick(Some(new.clone()), cx);
  assert_eq!(std::fs::read(&icon).unwrap(), std::fs::read(&new).unwrap());
  assert_eq!(toasts(cx, handle), 2);
}

#[gpui::test]
async fn settings_of_a_missing_profile_shows_only_the_tabs(cx: &mut TestAppContext) {
  let (handle, _, _) = open(cx, |w, cx| {
    SettingsPage::new("gone".into(), ProfileSettingsTab::General, w, cx)
  });
  click(cx, handle, |w, _| {
    assert!(w.try_find("profile-settings-tabs").is_some());
    assert!(w.try_find("change-icon").is_none());
  });
}
