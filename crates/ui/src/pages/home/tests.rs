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

fn played(id: &str, last_played: Option<&str>, favorite: bool) -> Profile {
  let mut p = profile(id);
  p.last_played = last_played.map(|t| t.parse().unwrap());
  p.favorite = favorite;
  p
}

fn entry(id: &str, last_played: &str) -> proton_profiles::config::QuickPlayInfo {
  serde_json::from_value(serde_json::json!({
    "id": id, "name": id, "lastPlayedTime": last_played, "history": true, "type": "multiplayer"
  }))
  .unwrap()
}

// ports of app/src/tests/unit/lib/profiles.test.ts
#[test]
fn card_meta_shows_mods_played_and_quick_play() {
  use crate::pages::profiles::card_meta;
  use proton_versions::loader::LoaderType;

  let ago = chrono::Utc::now() - chrono::Duration::hours(2);
  let mut p = played("a", None, false);
  p.last_played = Some(ago);
  assert_eq!(
    card_meta(&p, None, Some(24)),
    "24 mods · played 2 hours ago"
  );
  p.last_played = None;
  p.loader = LoaderType::Forge;
  assert_eq!(card_meta(&p, None, Some(1)), "1 mod · never played");

  p.loader = LoaderType::Vanilla;
  p.last_played = Some(ago);
  assert_eq!(card_meta(&p, None, Some(3)), "played 2 hours ago");
  p.loader = LoaderType::Quilt;
  p.last_played = None;
  assert_eq!(card_meta(&p, None, None), "never played");

  p.loader = LoaderType::Fabric;
  p.name = "SMP".into();
  let mut qp = entry("x", "2024-01-01T00:00:00Z");
  qp.last_played_time = ago;
  assert_eq!(
    card_meta(&p, Some(&qp), Some(5)),
    "SMP · played 2 hours ago"
  );
}

#[test]
fn latest_played_and_summary() {
  use crate::pages::profiles::profiles_summary;

  let profiles = [
    played("a", Some("2024-05-01T10:00:00Z"), false),
    played("b", None, false),
    played("c", Some("2024-05-02T09:00:00+02:00"), false),
    played("d", Some("2024-04-30T00:00:00Z"), false),
  ];
  assert_eq!(latest_played(&profiles).unwrap().id, "c");
  assert!(latest_played(&[played("a", None, false)]).is_none());
  assert!(latest_played(&[]).is_none());

  let list = [played("a", None, true), played("b", None, false)];
  assert_eq!(profiles_summary(&list), "2 profiles · 1 favorite");
  assert_eq!(profiles_summary(&[]), "0 profiles · 0 favorites");
}

#[test]
fn continue_quick_play_cases() {
  let at = |t: &str| Some(t.parse().unwrap());
  let mut p = played("a", Some("2024-01-03T00:00:00Z"), false);
  p.last_played_non_quick_play = at("2024-01-01T00:00:00Z");
  p.quick_play = vec![
    entry("old", "2024-01-02T00:00:00Z"),
    entry("new", "2024-01-03T00:00:00Z"),
  ];
  assert_eq!(continue_quick_play(&p).unwrap().id, "new");

  p.last_played_non_quick_play = at("2024-01-04T00:00:00Z");
  p.quick_play = vec![entry("old", "2024-01-02T00:00:00Z")];
  assert!(continue_quick_play(&p).is_none());

  assert!(continue_quick_play(&played("a", None, false)).is_none());
}
