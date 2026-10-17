use std::{cell::Cell, rc::Rc};

use gpui_kit::{
  self as gpui, AnyWindowHandle, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions,
  px, size, test::TestWindowExt,
};

use super::*;
use crate::router::{Kind, ProfileTab, QuickPlayTab, RouterExt};

/// Opens the shell; counts page builds
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Rc<Cell<usize>>) {
  let (handle, builds, _) = open_shell(cx);
  (handle, builds)
}

fn open_shell(cx: &mut TestAppContext) -> (AnyWindowHandle, Rc<Cell<usize>>, Entity<Shell>) {
  let builds = Rc::new(Cell::new(0));
  let counter = builds.clone();
  let dir = tempfile::tempdir().unwrap().keep();
  proton_store::init_at(dir.join("store.json"));
  let handle = cx.update(|cx| {
    gpui_kit::init(cx);
    proton_components::assets::load(cx).unwrap();
    proton_config::init(cx);
    proton_versions::init_state(cx);
    crate::init(cx);
    PageFactory::register(
      cx,
      |_| true,
      move |route, _, cx| {
        counter.set(counter.get() + 1);
        cx.new(|_| Placeholder(route.to_url().into())).into()
      },
    );
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
  });
  (handle.0, builds, handle.1)
}

fn click(handle: AnyWindowHandle, id: &str, cx: &mut TestAppContext) {
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    // new pages build on the next frame
    window.simulate_next_frame(cx);
    window.render_frame(cx);
    window.click(id.to_string(), cx);
  })
  .unwrap();
  cx.run_until_parked();
}

fn route(cx: &mut TestAppContext) -> Route {
  cx.update(|cx| cx.route())
}

#[gpui::test]
fn sidebar_navigates_and_pages_are_cached(cx: &mut TestAppContext) {
  let (handle, builds) = open(cx);
  click(handle, "nav-Profiles", cx);
  assert_eq!(route(cx), Route::Profiles);
  click(handle, "nav-Shaders", cx);
  assert_eq!(route(cx), Route::Discover(Kind::Shader));
  click(handle, "nav-Settings", cx);
  assert_eq!(route(cx), Route::Settings(SettingsTab::Accounts));
  // the nested settings nav
  click(handle, "tab-Java", cx);
  assert_eq!(route(cx), Route::Settings(SettingsTab::Java));
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.simulate_next_frame(cx);
  })
  .unwrap();
  // home, profiles, shaders, accounts, java
  assert_eq!(builds.get(), 5);

  let router = cx.update(|cx| cx.router());
  router.update(cx, |r, cx| r.back(cx));
  assert_eq!(route(cx), Route::Settings(SettingsTab::Accounts));
  router.update(cx, |r, cx| r.back(cx));
  assert_eq!(route(cx), Route::Discover(Kind::Shader));
  router.update(cx, |r, cx| r.forward(cx));
  assert_eq!(route(cx), Route::Settings(SettingsTab::Accounts));
  cx.update_window(handle, |_, window, cx| window.render_frame(cx))
    .unwrap();
  // revisited pages come from the cache
  assert_eq!(builds.get(), 5);
}

#[gpui::test]
fn profile_tabs_and_breadcrumbs(cx: &mut TestAppContext) {
  let (handle, _) = open(cx);
  cx.update(|cx| {
    cx.shell_data().update(cx, |d, _| {
      d.names.profiles.insert("p".into(), "Survival".into());
    });
    cx.navigate(Route::Profile {
      id: "p".into(),
      tab: ProfileTab::Logs,
    });
  });
  click(handle, "tab-Quick play", cx);
  assert_eq!(
    route(cx),
    Route::Profile {
      id: "p".into(),
      tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
    }
  );
  // crumbs: Profiles > Survival > Quick Play > Singleplayer, ids are `crumb-<index>`
  click(handle, "crumb-0", cx);
  assert_eq!(route(cx), Route::Profiles);
}

#[gpui::test]
fn offline_badge_asks_to_reconnect(cx: &mut TestAppContext) {
  let (handle, _) = open(cx);
  let asked = Rc::new(Cell::new(false));
  let flag = asked.clone();
  let data = cx.update(|cx| cx.shell_data());
  let _sub = cx.update(|cx| cx.subscribe(&data, move |_, ShellEvent::Reconnect, _| flag.set(true)));
  assert!(cx.update(|cx| {
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.try_find("offline").is_none()
    })
    .unwrap()
  }));
  data.update(cx, |d, cx| {
    d.offline = true;
    cx.notify();
  });
  cx.run_until_parked();
  click(handle, "offline", cx);
  assert!(asked.get());
}

#[gpui::test]
fn new_page_builds_on_the_next_frame(cx: &mut TestAppContext) {
  let (handle, builds) = open(cx);
  let frame = |cx: &mut TestAppContext| {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
      .unwrap();
  };
  frame(cx);
  let before = builds.get();
  cx.update(|cx| cx.navigate(Route::Profiles));
  frame(cx);
  // the spinner paints first
  assert_eq!(builds.get(), before);
  cx.update_window(handle, |_, window, cx| window.simulate_next_frame(cx))
    .unwrap();
  frame(cx);
  assert_eq!(builds.get(), before + 1);
}

fn has(handle: AnyWindowHandle, id: &str, cx: &mut TestAppContext) -> bool {
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.try_find(id.to_string()).is_some()
  })
  .unwrap()
}

/// Renders the route and lets its page build
fn visit(handle: AnyWindowHandle, route: Route, cx: &mut TestAppContext) {
  cx.update(|cx| cx.navigate(route));
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.simulate_next_frame(cx);
    window.render_frame(cx);
  })
  .unwrap();
}

#[gpui::test]
fn window_controls_follow_the_setting(cx: &mut TestAppContext) {
  let (handle, _) = open(cx);
  for id in ["minimize", "maximize", "close"] {
    assert!(has(handle, id, cx), "{id}");
  }
  cx.update(|cx| {
    cx.shell_data().update(cx, |d, cx| {
      d.hide_window_controls = true;
      cx.notify();
    })
  });
  cx.run_until_parked();
  for id in ["minimize", "maximize", "close"] {
    assert!(!has(handle, id, cx), "{id}");
  }
}

#[gpui::test]
fn header_re_renders_only_when_its_crumbs_change(cx: &mut TestAppContext) {
  let (handle, _) = open(cx);
  cx.update(|cx| {
    cx.navigate(Route::Profile {
      id: "p".into(),
      tab: ProfileTab::Logs,
    })
  });
  let frame = |cx: &mut TestAppContext| {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))
      .unwrap();
  };
  frame(cx);
  let before = proton_components::render_count::of::<Header>();
  let data = cx.update(|cx| cx.shell_data());
  // shell data the header does not show
  data.update(cx, |d, cx| {
    d.running = 3;
    d.content_updates.insert("p".into(), 1);
    cx.notify();
  });
  frame(cx);
  assert_eq!(proton_components::render_count::of::<Header>(), before);
  // the profile name is a crumb
  data.update(cx, |d, cx| {
    d.names.profiles.insert("p".into(), "Survival".into());
    cx.notify();
  });
  frame(cx);
  assert_eq!(proton_components::render_count::of::<Header>(), before + 1);
}

#[gpui::test]
fn the_current_crumb_is_not_a_link(cx: &mut TestAppContext) {
  let (handle, _) = open(cx);
  cx.update(|cx| cx.navigate(Route::Settings(SettingsTab::Java)));
  cx.run_until_parked();
  // Settings > Java
  click(handle, "crumb-1", cx);
  assert_eq!(route(cx), Route::Settings(SettingsTab::Java));
  click(handle, "crumb-0", cx);
  assert_eq!(route(cx), Route::Settings(SettingsTab::Accounts));
}

#[gpui::test]
fn the_least_recently_used_page_is_dropped(cx: &mut TestAppContext) {
  let (handle, builds, shell) = open_shell(cx);
  let routes: Vec<Route> = crate::router::all_routes("p")
    .into_iter()
    .take(CACHED_PAGES + 1)
    .collect();
  for route in &routes {
    visit(handle, route.clone(), cx);
  }
  assert_eq!(builds.get(), CACHED_PAGES + 1);
  assert_eq!(shell.read_with(cx, |s, _| s.pages.len()), CACHED_PAGES);
  // the newest is still cached, the first was evicted
  visit(handle, routes[CACHED_PAGES].clone(), cx);
  assert_eq!(builds.get(), CACHED_PAGES + 1);
  visit(handle, routes[0].clone(), cx);
  assert_eq!(builds.get(), CACHED_PAGES + 2);
}

#[gpui::test]
fn a_page_left_before_its_first_frame_is_never_built(cx: &mut TestAppContext) {
  let (handle, builds, shell) = open_shell(cx);
  visit(handle, Route::Home, cx);
  let before = builds.get();
  cx.update(|cx| cx.navigate(Route::Profiles));
  cx.update_window(handle, |_, window, cx| window.render_frame(cx))
    .unwrap();
  // gone on to the next page before the build frame
  cx.update(|cx| cx.navigate(Route::Instances));
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    window.simulate_next_frame(cx);
    window.render_frame(cx);
  })
  .unwrap();
  assert_eq!(builds.get(), before + 1);
  let cached: Vec<Route> =
    shell.read_with(cx, |s, _| s.pages.iter().map(|(r, _)| r.clone()).collect());
  assert!(!cached.contains(&Route::Profiles), "{cached:?}");
  assert!(cached.contains(&Route::Instances));
}

#[gpui::test]
fn profile_header_is_built_once_per_profile(cx: &mut TestAppContext) {
  let (handle, _) = open(cx);
  let headers = Rc::new(Cell::new(0));
  let counter = headers.clone();
  cx.update(|cx| {
    PageFactory::set_profile_header(cx, move |id, _, cx| {
      counter.set(counter.get() + 1);
      cx.new(|_| Placeholder(id.to_string().into())).into()
    })
  });
  let profile = |id: &str, tab| Route::Profile { id: id.into(), tab };
  visit(handle, profile("a", ProfileTab::Logs), cx);
  visit(handle, profile("a", ProfileTab::Instances), cx);
  assert_eq!(headers.get(), 1);
  visit(handle, profile("b", ProfileTab::Logs), cx);
  assert_eq!(headers.get(), 2);
}

#[gpui::test]
fn unregistered_routes_get_a_placeholder(cx: &mut TestAppContext) {
  use gpui_kit::{IntoElement, div};
  let (handle, _) = proton_components::test_view::open(cx, |_, _| div().into_any_element());
  let view = cx
    .update_window(handle, |_, window, cx| {
      PageFactory::build(&Route::Instances, window, cx)
    })
    .unwrap();
  let placeholder = view.downcast::<Placeholder>().unwrap();
  assert_eq!(placeholder.read_with(cx, |p, _| p.0.clone()), "/instances");
}

#[gpui::test]
fn first_matching_registration_wins(cx: &mut TestAppContext) {
  let (handle, builds) = open(cx);
  let late = Rc::new(Cell::new(0));
  let counter = late.clone();
  cx.update(|cx| {
    PageFactory::register(
      cx,
      |_| true,
      move |_, _, cx| {
        counter.set(counter.get() + 1);
        cx.new(|_| Placeholder("late".into())).into()
      },
    )
  });
  visit(handle, Route::Instances, cx);
  assert!(builds.get() > 0);
  assert_eq!(late.get(), 0);
}

#[gpui::test]
fn profile_nav_carries_the_content_update_badge_and_notifies_on_change(cx: &mut TestAppContext) {
  let (_, _, shell) = open_shell(cx);
  let nav = shell.read_with(cx, |s, _| s.profile_nav.clone());
  let notes = Rc::new(Cell::new(0));
  let seen = notes.clone();
  let _sub = cx.update(|cx| cx.observe(&nav, move |_, _| seen.set(seen.get() + 1)));
  cx.update(|cx| {
    cx.navigate(Route::Profile {
      id: "p".into(),
      tab: ProfileTab::Content(Kind::Shader),
    })
  });
  cx.run_until_parked();
  assert!(notes.get() > 0);
  assert_eq!(nav.read_with(cx, |n, _| n.active()), Some(1));
  // from here on only the data changes count
  notes.set(0);

  let data = cx.update(|cx| cx.shell_data());
  // updates of another profile change nothing here
  data.update(cx, |d, cx| {
    d.content_updates.insert("other".into(), 4);
    cx.notify();
  });
  cx.run_until_parked();
  assert_eq!(notes.get(), 0);
  data.update(cx, |d, cx| {
    d.content_updates.insert("p".into(), 2);
    cx.notify();
  });
  cx.run_until_parked();
  assert_eq!(notes.get(), 1);
  let badges: Vec<usize> = nav.read_with(cx, |n, _| n.items().iter().map(|i| i.badge).collect());
  assert_eq!(badges, [0, 2, 0, 0, 0]);

  // leaving the profile empties the nav
  cx.update(|cx| cx.navigate(Route::Home));
  cx.run_until_parked();
  assert!(nav.read_with(cx, |n, _| n.items().is_empty() && n.active().is_none()));
}
