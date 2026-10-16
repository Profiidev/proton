//! Cross-page tests on the fixture data: every route has a page, perf (render counts) and idle

use gpui_kit::{self as gpui, AnyWindowHandle, AppContext, TestAppContext, test::TestWindowExt};
use proton_components::render_count;
use proton_versions::{
  VersionsExt,
  event::{DownloadCheckStatus, DownloadMsg},
};

use super::profiles::tests::init_fixture;
use crate::{
  router::{Route, RouterExt, SettingsTab, all_routes},
  shell::{DownloadRowView, Header, HeaderActions, PageFactory, Placeholder, Shell, Sidebar},
};

const FABRIC: &str = "22222222-2222-4222-8222-222222222222";

fn open_shell(cx: &mut TestAppContext) -> AnyWindowHandle {
  init_fixture(cx);
  cx.update(super::register_all);
  let (handle, _) = super::profiles::tests::open(cx, Shell::new);
  draw(handle, cx);
  handle
}

/// A normal frame: clean cached views are replayed (`render_frame` refreshes everything)
fn draw(handle: AnyWindowHandle, cx: &mut TestAppContext) {
  cx.run_until_parked();
  cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))
    .unwrap();
}

#[gpui::test]
fn every_route_has_a_page(cx: &mut TestAppContext) {
  let handle = open_shell(cx);
  for route in all_routes(FABRIC) {
    let view = cx
      .update_window(handle, |_, window, cx| {
        PageFactory::build(&route, window, cx)
      })
      .unwrap();
    assert!(
      view.downcast::<Placeholder>().is_err(),
      "{route:?} is a placeholder"
    );
    // and it renders inside the shell
    cx.update(|cx| cx.navigate(route.clone()));
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
      .unwrap();
  }
}

#[gpui::test]
fn download_tick_re_renders_only_its_row(cx: &mut TestAppContext) {
  let handle = open_shell(cx);
  let downloads = cx.update(|cx| cx.versions().handle.clone());
  let a = downloads.start("A");
  let _b = downloads.start("B");
  draw(handle, cx);
  // the popover shows the rows
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.click("downloads".to_string(), cx);
  })
  .unwrap();
  draw(handle, cx);
  draw(handle, cx);

  let counts = || {
    [
      render_count::of::<Header>(),
      render_count::of::<Sidebar>(),
      render_count::of::<DownloadRowView>(),
    ]
  };
  let before = counts();
  downloads.send(DownloadMsg::Status {
    id: a,
    status: DownloadCheckStatus::AssetsCheck(1, 2),
  });
  draw(handle, cx);
  let delta: Vec<usize> = counts().iter().zip(before).map(|(a, b)| a - b).collect();
  // header and sidebar are cached and no ancestor of the downloads: they stay.
  // ponytail: the open popover's rows re-render with the panel (their parent view); they have
  // no fixed height to `.cached()` them with. Measure rows if the panel ever gets long.
  // The page root renders on every frame (see `Shell::render`), so it is not asserted.
  assert_eq!(delta[..2], [0, 0], "header, sidebar");
  assert!(delta[2] > 0, "the row shows the tick");
  // the actions bar (offline badge, downloads, controls) is the cheap uncached sibling
  assert!(render_count::of::<HeaderActions>() > 0);
}

/// After the entrance animations no frame is requested: an idle window costs no CPU
#[gpui::test]
fn idle_pages_request_no_frames(cx: &mut TestAppContext) {
  let handle = open_shell(cx);
  cx.update(|cx| cx.set_reduce_motion(false));
  for route in [
    Route::Home,
    Route::Profiles,
    Route::Settings(SettingsTab::Accounts),
    Route::Settings(SettingsTab::About),
  ] {
    cx.update(|cx| cx.navigate(route.clone()));
    let mut frames = 0;
    // animations run on the wall clock; deliver frames until nothing asks for another
    let start = std::time::Instant::now();
    loop {
      draw(handle, cx);
      let requested = cx
        .update_window(handle, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap();
      if requested == 0 {
        break;
      }
      frames += 1;
      assert!(
        start.elapsed() < std::time::Duration::from_secs(3),
        "{route:?} keeps animating ({frames} frames)"
      );
      std::thread::sleep(std::time::Duration::from_millis(16));
    }
    // still idle a while later
    std::thread::sleep(std::time::Duration::from_millis(50));
    draw(handle, cx);
    let requested = cx
      .update_window(handle, |_, window, cx| window.simulate_next_frame(cx))
      .unwrap();
    assert_eq!(requested, 0, "{route:?} requested a frame while idle");
  }
}
