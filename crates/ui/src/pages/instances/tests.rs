use std::{
  cell::RefCell,
  path::{Path, PathBuf},
  rc::Rc,
  time::{Duration, Instant},
};

use gpui_kit::{self as gpui, AnyView, AnyWindowHandle, TestAppContext, div, test::TestWindowExt};
use proton_components::test_view;
use proton_profiles::Profiles;
use proton_utils::rt;

use super::*;

const VANILLA: &str = "11111111-1111-4111-8111-111111111111";

/// A copy of the fixture data dir, its store, the services, and a window drawing whatever
/// view is put into the returned slot (uncached, like the shell does)
fn setup(cx: &mut TestAppContext) -> (AnyWindowHandle, Rc<RefCell<Option<AnyView>>>) {
  let dir = tempfile::tempdir().unwrap().keep();
  let fixture =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/io.profidev.proton");
  let ok = std::process::Command::new("cp")
    .arg("-r")
    .arg(&fixture)
    .arg(dir.join("data"))
    .status()
    .unwrap()
    .success();
  assert!(ok);
  let data: PathBuf = dir.join("data");
  proton_store::init_at(data.join("store.json"));

  let slot: Rc<RefCell<Option<AnyView>>> = Rc::default();
  let view = slot.clone();
  let (handle, _) = test_view::open(cx, move |_, _| {
    div()
      .size_full()
      .children(view.borrow().clone())
      .into_any_element()
  });
  cx.update(|cx| {
    proton_config::init(cx);
    proton_profiles::init_at(cx, data).unwrap();
    crate::init(cx);
    register(cx);
  });
  (handle, slot)
}

fn show<V: Render>(
  handle: AnyWindowHandle,
  slot: &Rc<RefCell<Option<AnyView>>>,
  cx: &mut TestAppContext,
  build: impl FnOnce(&mut Window, &mut Context<V>) -> V,
) -> Entity<V> {
  let page = cx
    .update_window(handle, |_, window, cx| cx.new(|cx| build(window, cx)))
    .unwrap();
  *slot.borrow_mut() = Some(page.clone().into());
  test_view::draw(handle, cx);
  page
}

/// Waits (with real time, tokio does the work) until `done`
fn wait(cx: &mut TestAppContext, what: &str, mut done: impl FnMut(&mut TestAppContext) -> bool) {
  let start = Instant::now();
  while !done(cx) {
    assert!(
      start.elapsed() < Duration::from_secs(10),
      "timed out: {what}"
    );
    std::thread::sleep(Duration::from_millis(20));
    cx.run_until_parked();
  }
}

/// Starts a fake game (a sleeping shell) for the vanilla fixture profile
async fn spawn_instance(profiles: &Profiles, cx: &mut TestAppContext) -> String {
  let handle = profiles.handle.clone();
  rt::spawn(async move {
    let child = tokio::process::Command::new("sh")
      .args(["-c", "sleep 30"])
      .kill_on_drop(true)
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()?;
    let store = handle.store.lock().await;
    let profile = store.profile(VANILLA).await?;
    store.add_instance(child, &profile, "Steve".into()).await
  })
  .await
  .unwrap()
  .unwrap();
  wait(cx, "instance listed", |cx| {
    cx.update(|cx| !cx.profiles().instances.read(cx).is_empty())
  });
  cx.update(|cx| cx.profiles().instances.read(cx)[0].id.clone())
}

#[gpui::test]
async fn instances_page_lists_a_running_instance(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let (handle, slot) = setup(cx);
  let page = show(handle, &slot, cx, |_, cx| InstancesPage::new(cx));
  assert_eq!(page.read_with(cx, |p, _| p.row_count()), 0);

  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = spawn_instance(&profiles, cx).await;
  test_view::draw(handle, cx);
  assert_eq!(page.read_with(cx, |p, _| p.row_count()), 1);
  assert_eq!(cx.update(|cx| cx.shell_data().read(cx).running), 1);
  cx.update_window(handle, |_, window, cx| {
    assert!(window.within(id.clone()).try_find("console").is_some());
    window.within(id.clone()).click("console", cx);
  })
  .unwrap();
  assert_eq!(cx.update(|cx| cx.route()), Route::InstanceLogs { id });

  profiles.stop_all(None).await.unwrap();
  wait(cx, "instance gone", |cx| {
    cx.update(|cx| cx.profiles().instances.read(cx).is_empty())
  });
  assert_eq!(page.read_with(cx, |p, _| p.row_count()), 0);
  assert_eq!(cx.update(|cx| cx.shell_data().read(cx).running), 0);
}

#[gpui::test]
async fn live_log_appends_without_rerendering_the_header(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let (handle, slot) = setup(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = spawn_instance(&profiles, cx).await;
  let page_id = id.clone();
  let page = show(handle, &slot, cx, move |window, cx| {
    InstanceLogsPage::new(page_id, window, cx)
  });
  test_view::draw(handle, cx);
  let buffer = cx.update(|cx| profiles.instance_logs(&id, cx)).unwrap();
  let bar = page.read_with(cx, |p, _| p.bar().clone());
  let renders = bar.read_with(cx, |b, _| b.renders);
  assert!(renders > 0);

  for i in 0..5 {
    buffer.update(cx, |b, cx| {
      b.extend(
        [format!("[12:00:0{i}] [main/INFO]: line {i}")]
          .iter()
          .map(String::as_str),
        cx,
      )
    });
    // a normal frame, clean cached views are replayed
    cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))
      .unwrap();
  }
  assert_eq!(bar.read_with(cx, |b, _| b.renders), renders);
  let log = page.read_with(cx, |p, _| p.log.clone().unwrap());
  assert_eq!(log.read_with(cx, |l, _| l.visible_len()), 5);
  // "Warnings" hides the INFO lines, "All" brings them back
  bar.update(cx, |b, cx| b.set_mode(1, cx));
  assert_eq!(log.read_with(cx, |l, _| l.visible_len()), 0);
  bar.update(cx, |b, cx| b.set_mode(0, cx));
  assert_eq!(log.read_with(cx, |l, _| l.visible_len()), 5);

  profiles.stop_all(None).await.unwrap();
  wait(cx, "instance gone", |cx| {
    cx.update(|cx| cx.profiles().instances.read(cx).is_empty())
  });
}

#[gpui::test]
async fn profile_logs_tab_loads_the_newest_run_and_filters(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let (handle, slot) = setup(cx);
  let page = show(handle, &slot, cx, |window, cx| {
    ProfileLogsPage::new(VANILLA.into(), window, cx)
  });
  let log = page.read_with(cx, |p, _| p.log().clone());
  let visible = |cx: &mut TestAppContext| log.read_with(cx, |l, _| l.visible_len());
  // the newest of the two fixture runs has 13 lines
  wait(cx, "run loaded", |cx| visible(cx) == 13);
  test_view::draw(handle, cx);

  page.update(cx, |p, cx| p.toggle_level("ERROR", cx));
  // the error and its 3 stack trace lines
  assert_eq!(visible(cx), 4);

  let search = page.read_with(cx, |p, _| p.search().clone());
  cx.update_window(handle, |_, window, cx| {
    search.update(cx, |s, cx| s.set_value("nullpointer", window, cx))
  })
  .unwrap();
  page.update(cx, |p, cx| p.apply_filter(cx));
  assert_eq!(visible(cx), 1);

  page.update(cx, |p, cx| p.toggle_level("ERROR", cx));
  cx.update_window(handle, |_, window, cx| {
    search.update(cx, |s, cx| s.set_value("render", window, cx))
  })
  .unwrap();
  page.update(cx, |p, cx| p.apply_filter(cx));
  assert_eq!(visible(cx), 5);
  test_view::draw(handle, cx);
}

#[test]
fn groups_and_summary() {
  use chrono::TimeZone;
  use proton_versions::loader::LoaderType;
  let at = |s| Utc.timestamp_opt(s, 0).unwrap();
  let info = |id: &str, profile: &str, t| InstanceInfo {
    id: id.into(),
    launched_at: at(t),
    profile_name: profile.into(),
    profile_id: profile.into(),
    version: "1.21".into(),
    loader: LoaderType::Vanilla,
    account_name: String::new(),
  };
  let groups = group_by_profile(&[info("c", "b", 3), info("a", "a", 2), info("x", "b", 1)]);
  let ids: Vec<Vec<&str>> = groups
    .iter()
    .map(|g| g.iter().map(|i| i.id.as_str()).collect())
    .collect();
  assert_eq!(ids, [vec!["x", "c"], vec!["a"]]);
  // `instance_number`: per profile, in launch order
  assert_eq!(console::sibling_label(&groups[0], 1), "Instance 2");
  assert_eq!(console::sibling_label(&groups[1], 0), "Instance 1");
  assert_eq!(running_summary(1, 1), "1 game running across 1 profile.");
  assert_eq!(running_summary(3, 2), "3 games running across 2 profiles.");
}

#[gpui::test]
async fn live_log_in_the_shell_re_renders_neither_header_nor_sidebar(cx: &mut TestAppContext) {
  use proton_components::{render_count, widgets::logs::LogWindow};

  use crate::shell::{Header, Shell, Sidebar};

  cx.executor().allow_parking();
  let (handle, slot) = setup(cx);
  cx.update(proton_versions::init_state);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = spawn_instance(&profiles, cx).await;
  cx.update(|cx| cx.navigate(Route::InstanceLogs { id: id.clone() }));
  show(handle, &slot, cx, Shell::new);
  cx.run_until_parked();
  for _ in 0..2 {
    cx.update_window(handle, |_, window, cx| {
      window.draw(cx).clear(cx);
      // the page builds on the next frame
      window.simulate_next_frame(cx);
    })
    .unwrap();
  }
  let buffer = cx.update(|cx| profiles.instance_logs(&id, cx)).unwrap();

  let counts = || {
    [
      render_count::of::<Header>(),
      render_count::of::<Sidebar>(),
      render_count::of::<LogWindow>(),
    ]
  };
  let before = counts();
  for i in 0..5 {
    buffer.update(cx, |b, cx| {
      b.push(&format!("[12:00:0{i}] [main/INFO]: line {i}"), cx)
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))
      .unwrap();
  }
  let after = counts();
  assert_eq!((after[0], after[1]), (before[0], before[1]));
  assert!(after[2] >= before[2] + 5, "{before:?} {after:?}");

  profiles.stop_all(None).await.unwrap();
  wait(cx, "instance gone", |cx| {
    cx.update(|cx| cx.profiles().instances.read(cx).is_empty())
  });
}

#[gpui::test]
async fn profile_instances_tab_shows_its_running_instance(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let (handle, slot) = setup(cx);
  let page = show(handle, &slot, cx, |_, cx| {
    ProfileInstancesPage::new(VANILLA.into(), cx)
  });
  assert!(page.read_with(cx, |p, _| p.rows.is_empty()));

  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = spawn_instance(&profiles, cx).await;
  test_view::draw(handle, cx);
  assert_eq!(page.read_with(cx, |p, _| p.rows.len()), 1);
  cx.update_window(handle, |_, window, cx| {
    window.within(id.clone()).click("console", cx);
  })
  .unwrap();
  assert_eq!(cx.update(|cx| cx.route()), Route::InstanceLogs { id });

  profiles.stop_all(None).await.unwrap();
  wait(cx, "instance gone", |cx| {
    cx.update(|cx| cx.profiles().instances.read(cx).is_empty())
  });
  assert!(page.read_with(cx, |p, _| p.rows.is_empty()));
}
