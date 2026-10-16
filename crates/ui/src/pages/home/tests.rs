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
