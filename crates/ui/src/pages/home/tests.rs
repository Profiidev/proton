use gpui_kit::{self as gpui, TestAppContext, test::TestWindowExt};

use super::*;
use crate::{
  pages::profiles::tests::{init_fixture, open, poll},
  router::{ProfileTab, QuickPlayTab},
};

const VANILLA: &str = "11111111-1111-4111-8111-111111111111";

#[gpui::test]
fn renders_fixture_favorites_and_opens(cx: &mut TestAppContext) {
  init_fixture(cx);
  let (handle, _) = open(cx, |_, cx| HomePage::new(cx));
  let favorites = cx.update(|cx| cx.profiles().favorites.read(cx).len());
  assert_eq!(favorites, 4);

  let card = format!("favorites-{VANILLA}-");
  let world = format!("favorites-{VANILLA}-Singleplayer-New World");
  let history_world = format!("history-{VANILLA}-Singleplayer-New World");
  let server = format!("favorites-{VANILLA}-Multiplayer-mc.hypixel.net");
  poll(cx, |cx| !cx.profiles().profiles.read(cx).is_empty());
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    assert!(window.try_find("continue-play").is_some());
    assert!(
      window
        .within("favorites")
        .within(card.clone())
        .try_find("open")
        .is_some()
    );
    assert!(
      window
        .within("favorites")
        .within(world.clone())
        .try_find("open")
        .is_some()
    );
    assert!(
      window
        .within("history")
        .within(history_world.clone())
        .try_find("open")
        .is_some()
    );
    window
      .within("favorites")
      .within(server.clone())
      .click("open", cx);
  })
  .unwrap();
  assert_eq!(
    cx.update(|cx| cx.route()),
    Route::Profile {
      id: VANILLA.into(),
      tab: ProfileTab::QuickPlay(QuickPlayTab::Multiplayer),
    }
  );
}

#[test]
fn continue_quick_play_only_when_newer() {
  let p: Profile = serde_json::from_str(include_str!(
    "../../../../../fixtures/data/io.profidev.proton/profiles/11111111-1111-4111-8111-111111111111/profile.json"
  ))
  .unwrap();
  let mut plain = p.clone();
  plain.last_played_non_quick_play = Some(chrono::Utc::now());
  assert!(continue_quick_play(&plain).is_none());
  let mut qp = p;
  qp.last_played_non_quick_play = None;
  assert!(continue_quick_play(&qp).is_some());
}

fn profile(id: &str) -> Profile {
  serde_json::from_value(serde_json::json!({
    "id": id, "name": format!("P {id}"), "created_at": "2026-01-01T00:00:00Z",
    "last_played": null, "version": "1.21.8", "loader": "Fabric", "loader_version": null,
    "downloaded": true, "use_local_game": false, "game": null, "use_local_jvm": false, "jvm": null
  }))
  .unwrap()
}

fn update(profile: &str, kind: &str) -> proton_content::model::UpdateInfo {
  serde_json::from_value(serde_json::json!({
    "profile": profile, "kind": kind, "fileName": "x.jar", "source": "modrinth",
    "projectId": "p", "name": "X", "current": "1",
    "latest": {
      "source": "modrinth", "projectId": "p", "id": "v", "name": "v", "versionNumber": "2",
      "channel": "release", "gameVersions": [], "loaders": [], "publishedAt": "",
      "files": [], "dependencies": []
    }
  }))
  .unwrap()
}

// port of `updates_banner` in app/src/tests/unit/lib/content.test.ts
#[test]
fn updates_banner_counts_and_names() {
  let list = [profile("a"), profile("b"), profile("c")];
  let empty = ContentUpdates(Default::default());
  assert_eq!(updates_banner(&empty, &list), None);
  let only_empty = ContentUpdates([("a".into(), vec![])].into());
  assert_eq!(updates_banner(&only_empty, &list), None);

  let updates = ContentUpdates(
    [
      ("b".into(), vec![update("b", "mod"), update("b", "mod")]),
      ("c".into(), vec![update("c", "mod")]),
      ("gone".into(), vec![update("g", "mod")]),
    ]
    .into(),
  );
  let b = updates_banner(&updates, &list).unwrap();
  assert_eq!(b.title, "3 mod updates available");
  assert_eq!(b.description, "P b and P c have newer compatible versions.");
  assert_eq!(b.target.to_url(), "/profiles/info/content/mods?id=b");

  let mixed = ContentUpdates([("a".into(), vec![update("a", "shader")])].into());
  let b = updates_banner(&mixed, &list).unwrap();
  assert_eq!(b.title, "1 update available");
  assert_eq!(b.description, "P a has newer compatible versions.");
  assert_eq!(b.target.to_url(), "/profiles/info/content/shaders?id=a");
}
