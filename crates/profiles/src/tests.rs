use std::{
  cell::{Cell, RefCell},
  path::PathBuf,
  rc::Rc,
  time::{Duration, Instant},
};

use gpui_kit::{self as gpui, TestAppContext};

use super::*;
use crate::{PROFILE_LOGS, store::ProfilesMsg};

/// Fresh store and data dir per test, never the real ones
fn init_test(cx: &mut TestAppContext) -> PathBuf {
  let dir = tempfile::tempdir().unwrap().keep();
  proton_store::init_at(dir.join("store.json"));
  cx.update(|cx| init_at(cx, dir.join("data")).unwrap());
  dir.join("data")
}

fn count_notifies<T: 'static>(cx: &mut TestAppContext, entity: &Entity<T>) -> Rc<Cell<usize>> {
  let count = Rc::new(Cell::new(0));
  let seen = count.clone();
  cx.update(|cx| {
    cx.observe(entity, move |_, _| seen.set(seen.get() + 1))
      .detach()
  });
  count
}

fn info(id: &str) -> InstanceInfo {
  InstanceInfo {
    id: id.into(),
    launched_at: Utc::now(),
    profile_name: "p".into(),
    profile_id: "pid".into(),
    version: "1.21".into(),
    loader: LoaderType::Vanilla,
    loader_version: None,
    account_name: None,
    account_id: None,
  }
}

#[gpui::test]
async fn create_notifies_profiles_once(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let count = count_notifies(cx, &profiles.profiles);

  let id = profiles
    .create(
      "Test".into(),
      None,
      "1.21".into(),
      LoaderType::Vanilla,
      None,
    )
    .await
    .unwrap();
  cx.run_until_parked();

  assert_eq!(count.get(), 1);
  let list = cx.update(|cx| profiles.profiles.read(cx).clone());
  assert_eq!(list.len(), 1);
  assert_eq!(
    (list[0].id.as_str(), list[0].name.as_str()),
    (id.as_str(), "Test")
  );
}

#[gpui::test]
fn log_batch_notifies_once(cx: &mut TestAppContext) {
  init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let tx = profiles.handle.tx.clone();
  let instances = count_notifies(cx, &profiles.instances);

  tx.send(ProfilesMsg::InstanceAdded(info("a"))).unwrap();
  cx.run_until_parked();
  assert_eq!(instances.get(), 1);
  let logs = cx.update(|cx| profiles.instance_logs("a", cx)).unwrap();
  let count = count_notifies(cx, &logs);

  let batch = vec!["one".to_string(), "two".into(), "three".into()];
  tx.send(ProfilesMsg::InstanceLines("a".into(), batch))
    .unwrap();
  cx.run_until_parked();
  assert_eq!(count.get(), 1);
  assert_eq!(cx.update(|cx| logs.read(cx).entries().len()), 3);
  // log lines do not wake the instance list
  assert_eq!(instances.get(), 1);

  tx.send(ProfilesMsg::InstanceRemoved("a".into())).unwrap();
  cx.run_until_parked();
  assert_eq!(instances.get(), 2);
  assert!(cx.update(|cx| profiles.instance_logs("a", cx)).is_none());
}

#[gpui::test]
async fn non_zero_exit_emits_crash(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let data_dir = init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = profiles
    .create(
      "Crashy".into(),
      None,
      "1.21".into(),
      LoaderType::Vanilla,
      None,
    )
    .await
    .unwrap();
  cx.run_until_parked();

  let events = Rc::new(RefCell::new(Vec::new()));
  let seen = events.clone();
  cx.update(|cx| {
    cx.subscribe(&profiles.events, move |_, e: &ProfilesEvent, _| {
      seen.borrow_mut().push(e.clone())
    })
    .detach()
  });
  let instances = count_notifies(cx, &profiles.instances);

  let handle = profiles.handle.clone();
  let profile_id = id.clone();
  rt::spawn(async move {
    let child = tokio::process::Command::new("sh")
      .args(["-c", "echo hi; exit 1"])
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::piped())
      .spawn()?;
    let store = handle.store.lock().await;
    let profile = store.profile(&profile_id).await?;
    store
      .add_instance(child, &profile, "acc".into(), "uuid".into())
      .await
  })
  .await
  .unwrap()
  .unwrap();

  let crash = ProfilesEvent::InstanceCrashed {
    profile_name: "Crashy".into(),
  };
  let start = Instant::now();
  while !events.borrow().contains(&crash) {
    assert!(start.elapsed() < Duration::from_secs(10), "no crash event");
    std::thread::sleep(Duration::from_millis(20));
    cx.run_until_parked();
  }
  // added + removed
  assert_eq!(instances.get(), 2);
  assert!(cx.update(|cx| profiles.instances.read(cx).is_empty()));

  // the run log is saved
  let log_dir = data_dir.join(PROFILE_DIR).join(&id).join(PROFILE_LOGS);
  let start = Instant::now();
  let runs = loop {
    let runs = rt::block_on(profile::list_runs(&log_dir)).unwrap();
    if !runs.is_empty() || start.elapsed() > Duration::from_secs(5) {
      break runs;
    }
    std::thread::sleep(Duration::from_millis(20));
  };
  assert_eq!(runs.len(), 1);
  let lines = profiles.logs(id, runs[0]).await.unwrap();
  assert_eq!(lines, ["hi"]);
}

#[test]
fn mark_played_updates_history() {
  let json = r#"{"id":"x","name":"x","created_at":"2024-01-01T00:00:00Z","last_played":null,
    "quick_play":[{"id":"w","name":"W","lastPlayedTime":"2024-01-01T00:00:00Z","type":"singleplayer"}],
    "version":"1.21","loader":"Vanilla","loader_version":null,"downloaded":true,
    "use_local_game":false,"game":null,"use_local_jvm":false,"jvm":null}"#;
  let mut profile: Profile = serde_json::from_str(json).unwrap();
  let now = Utc::now();
  let quick_play = profile.quick_play[0].clone();

  mark_played(&mut profile, Some(&quick_play), now);
  assert_eq!(profile.last_played, Some(now));
  assert!(profile.quick_play[0].history);
  assert_eq!(profile.quick_play[0].last_played_time, now);
  assert_eq!(profile.last_played_non_quick_play, None);

  mark_played(&mut profile, None, now);
  assert_eq!(profile.last_played_non_quick_play, Some(now));
}
