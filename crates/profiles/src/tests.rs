use std::{
  cell::{Cell, RefCell},
  path::PathBuf,
  rc::Rc,
  time::{Duration, Instant},
};

use gpui_kit::{self as gpui, TestAppContext};

use proton_utils::test_http::FakeServer;
use proton_versions::{
  VersionsExt,
  downloads::DownloadStatus,
  testing::{fake_java, java_calls},
};

use super::*;
use crate::{
  PROFILE_LOGS,
  config::{ProfileInfo, QuickPlayType},
  store::ProfilesMsg,
};

/// A profile as an old version wrote it, without the fields added later
pub(crate) const OLD_PROFILE: &str = r#"{"id":"x","name":"x","created_at":"2024-01-01T00:00:00Z","last_played":null,
  "quick_play":[{"id":"w","name":"W","lastPlayedTime":"2024-01-01T00:00:00Z","type":"singleplayer"}],
  "version":"1.21","loader":"Vanilla","loader_version":null,"downloaded":true,
  "use_local_game":false,"game":null,"use_local_jvm":false,"jvm":null}"#;

/// [`OLD_PROFILE`] with this id and no quick play entries
pub(crate) fn profile(id: &str) -> Profile {
  let mut profile: Profile = serde_json::from_str(OLD_PROFILE).unwrap();
  profile.id = id.into();
  profile.quick_play.clear();
  profile
}

pub(crate) fn quick_play(id: &str, r#type: QuickPlayType) -> QuickPlayInfo {
  QuickPlayInfo {
    id: id.into(),
    name: id.into(),
    last_played_time: Utc::now(),
    favorite: false,
    history: false,
    r#type,
  }
}

/// A fake game: `sh -c script` with piped output, call inside the tokio runtime
pub(crate) fn sh(script: &str) -> tokio::process::Child {
  tokio::process::Command::new("sh")
    .args(["-c", script])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .unwrap()
}

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
    account_name: String::new(),
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
    store.add_instance(child, &profile, "acc".into()).await
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
  let mut profile: Profile = serde_json::from_str(OLD_PROFILE).unwrap();
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

#[gpui::test]
async fn init_loads_in_the_background_and_early_calls_wait(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let dir = tempfile::tempdir().unwrap().keep();
  // SAFETY: nextest runs every test in its own process, nothing else reads the env yet
  unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
  proton_store::init_at(dir.join("store.json"));
  cx.update(init);
  let profiles = cx.update(|cx| cx.profiles().clone());
  assert!(!cx.update(|cx| *profiles.loaded.read(cx)));

  // queued behind the initial load
  profiles
    .create(
      "Early".into(),
      None,
      "1.21".into(),
      LoaderType::Vanilla,
      None,
    )
    .await
    .unwrap();
  cx.run_until_parked();
  cx.update(|cx| {
    assert!(*profiles.loaded.read(cx));
    assert_eq!(profiles.profiles.read(cx)[0].name, "Early");
  });
}

/// Runs the gpui side until `done` holds, tokio work lands in between
fn wait_until(
  cx: &mut TestAppContext,
  what: &str,
  mut done: impl FnMut(&mut TestAppContext) -> bool,
) {
  let start = Instant::now();
  loop {
    cx.run_until_parked();
    if done(cx) {
      return;
    }
    assert!(
      start.elapsed() < Duration::from_secs(10),
      "timed out: {what}"
    );
    std::thread::sleep(Duration::from_millis(20));
  }
}

async fn create(profiles: &Profiles, name: &str) -> String {
  profiles
    .create(name.into(), None, "1.21".into(), LoaderType::Vanilla, None)
    .await
    .unwrap()
}

#[gpui::test]
fn listener_ignores_messages_for_unknown_entities(cx: &mut TestAppContext) {
  init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let tx = profiles.handle.tx.clone();
  let loaded = count_notifies(cx, &profiles.loaded);
  let instances = count_notifies(cx, &profiles.instances);

  // nobody asked for these lists or logs yet, so there is nothing to update
  tx.send(ProfilesMsg::QuickPlay("x".into(), vec![])).unwrap();
  tx.send(ProfilesMsg::Runs("x".into(), vec![Utc::now()]))
    .unwrap();
  tx.send(ProfilesMsg::InstanceLines("x".into(), vec!["l".into()]))
    .unwrap();
  // removing an unknown instance still notifies, harmless
  tx.send(ProfilesMsg::InstanceRemoved("x".into())).unwrap();
  // already loaded, write_changed keeps it quiet
  tx.send(ProfilesMsg::Profiles {
    profiles: vec![],
    favorites: vec![],
    history: vec![],
  })
  .unwrap();
  cx.run_until_parked();

  assert_eq!(loaded.get(), 0);
  assert_eq!(instances.get(), 1);
  let lazy = cx.update(|cx| {
    let l = profiles.lazy.read(cx);
    (l.quick_play.len(), l.runs.len(), l.logs.len())
  });
  assert_eq!(lazy, (0, 0, 0));
}

#[gpui::test]
fn listener_turns_content_changes_into_events(cx: &mut TestAppContext) {
  init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let events = Rc::new(RefCell::new(Vec::new()));
  let seen = events.clone();
  cx.update(|cx| {
    cx.subscribe(&profiles.events, move |_, e: &ProfilesEvent, _| {
      seen.borrow_mut().push(e.clone())
    })
    .detach()
  });
  let tx = profiles.handle.tx.clone();
  tx.send(ProfilesMsg::ContentChanged("p".into())).unwrap();
  tx.send(ProfilesMsg::InstanceCrashed {
    profile_name: "P".into(),
  })
  .unwrap();
  cx.run_until_parked();
  assert_eq!(
    *events.borrow(),
    [
      ProfilesEvent::ContentChanged("p".into()),
      ProfilesEvent::InstanceCrashed {
        profile_name: "P".into()
      }
    ]
  );
}

#[gpui::test]
async fn quick_play_and_runs_load_lazily_once(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let data_dir = init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = create(&profiles, "P").await;

  let launched = Utc::now();
  let log_dir = ProfileInfo::log_dir(&data_dir, &id);
  std::fs::create_dir_all(&log_dir).unwrap();
  std::fs::write(profile::run_log_file(&log_dir, launched), "line").unwrap();
  {
    let entry = quick_play("srv", QuickPlayType::Multiplayer);
    let handle = profiles.handle.clone();
    let pid = id.clone();
    rt::block_on(async move {
      let store = handle.store.lock().await;
      let mut profile = store.profile(&pid).await.unwrap();
      profile.quick_play = vec![entry];
      profile.update(store.data_dir()).await.unwrap();
    });
  }

  let quick = cx.update(|cx| profiles.quick_play(&id, cx));
  let runs = cx.update(|cx| profiles.runs(&id, cx));
  wait_until(cx, "lazy lists", |cx| {
    cx.update(|cx| quick.read(cx).len() == 1 && runs.read(cx).len() == 1)
  });
  // the second call reuses the entity
  assert_eq!(cx.update(|cx| profiles.quick_play(&id, cx)), quick);
  assert_eq!(cx.update(|cx| profiles.runs(&id, cx)), runs);
  assert_eq!(profiles.logs(id.clone(), launched).await.unwrap(), ["line"]);

  profiles.clear_logs(id.clone()).await.unwrap();
  cx.run_until_parked();
  assert!(cx.update(|cx| runs.read(cx).is_empty()));
}

#[gpui::test]
async fn favorites_and_quick_play_edits_reach_the_home_lists(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = create(&profiles, "P").await;
  let entry = quick_play("srv", QuickPlayType::Multiplayer);
  {
    let handle = profiles.handle.clone();
    let (pid, entry) = (id.clone(), entry.clone());
    rt::block_on(async move {
      let store = handle.store.lock().await;
      let mut profile = store.profile(&pid).await.unwrap();
      profile.quick_play = vec![entry];
      profile.update(store.data_dir()).await.unwrap();
    });
  }
  let quick = cx.update(|cx| profiles.quick_play(&id, cx));

  profiles.set_favorite(id.clone(), None, true).await.unwrap();
  profiles
    .set_favorite(id.clone(), Some(entry.clone()), true)
    .await
    .unwrap();
  cx.run_until_parked();
  let favorites = cx.update(|cx| profiles.favorites.read(cx).clone());
  assert_eq!(favorites.len(), 2);
  assert!(cx.update(|cx| quick.read(cx)[0].favorite));

  profiles.remove_quick_play(id.clone(), entry).await.unwrap();
  cx.run_until_parked();
  assert!(cx.update(|cx| quick.read(cx).is_empty()));
  let favorites = cx.update(|cx| profiles.favorites.read(cx).clone());
  assert_eq!(favorites.len(), 1);
  assert!(favorites[0].quick_play.is_none());
}

#[gpui::test]
async fn quick_play_file_change_refreshes_the_lists(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let data_dir = init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = create(&profiles, "P").await;
  let quick = cx.update(|cx| profiles.quick_play(&id, cx));
  cx.run_until_parked();

  // the game writes its quick play file, the watcher merges it
  let entries = vec![quick_play("srv", QuickPlayType::Multiplayer)];
  let file = data_dir
    .join(PROFILE_DIR)
    .join(&id)
    .join(proton_versions::paths::QUICK_PLAY);
  std::fs::write(file, serde_json::to_string(&entries).unwrap()).unwrap();
  // the quick play list and the profile list refresh one after the other
  wait_until(cx, "merged quick play", |cx| {
    cx.update(|cx| quick.read(cx).len() == 1 && profiles.profiles.read(cx)[0].quick_play.len() == 1)
  });
}

#[gpui::test]
async fn edits_icons_and_removal_go_through_the_store(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let data_dir = init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = create(&profiles, "P").await;
  cx.run_until_parked();
  let count = count_notifies(cx, &profiles.profiles);

  profiles
    .update(crate::config::ProfileUpdate {
      id: id.clone(),
      name: "Renamed".into(),
      version: "1.20".into(),
      loader_version: None,
      use_local_game: false,
      game: None,
      use_local_jvm: false,
      jvm: None,
    })
    .await
    .unwrap();
  cx.run_until_parked();
  assert_eq!(
    cx.update(|cx| profiles.profiles.read(cx)[0].name.clone()),
    "Renamed"
  );

  assert_eq!(profiles.icon(id.clone()).await.unwrap(), None);
  assert!(
    profiles
      .update_icon(id.clone(), b"x".to_vec())
      .await
      .is_err()
  );
  let mut png = std::io::Cursor::new(Vec::new());
  image::RgbaImage::new(2, 2)
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
  let png = png.into_inner();
  profiles.update_icon(id.clone(), png.clone()).await.unwrap();
  assert_eq!(profiles.icon(id.clone()).await.unwrap(), Some(png));
  cx.run_until_parked();
  // update and the new icon both reload the list
  assert_eq!(count.get(), 2);

  std::fs::create_dir_all(data_dir.join(PROFILE_DIR).join(&id).join("mods")).unwrap();
  std::fs::write(data_dir.join(PROFILE_DIR).join(&id).join("mods/a.jar"), "").unwrap();
  let mods = profiles.mods_count().await.unwrap();
  assert_eq!(mods.get(&id), Some(&1));

  profiles.remove(id.clone()).await.unwrap();
  cx.run_until_parked();
  assert!(cx.update(|cx| profiles.profiles.read(cx).is_empty()));
  // every call on a removed profile reports it
  assert!(profiles.icon(id.clone()).await.is_err());
  assert!(profiles.clear_logs(id.clone()).await.is_err());
  assert!(profiles.logs(id.clone(), Utc::now()).await.is_err());
  assert!(profiles.remove(id.clone()).await.is_err());
  assert!(profiles.set_favorite(id.clone(), None, true).await.is_err());
  assert!(profiles.open_path(id).await.is_err());
}

#[gpui::test]
async fn instances_stop_one_by_one_or_all_together(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  init_test(cx);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let a = create(&profiles, "A").await;
  let b = create(&profiles, "B").await;
  for id in [&a, &a, &b] {
    let handle = profiles.handle.clone();
    let id = id.clone();
    rt::block_on(async move {
      let store = handle.store.lock().await;
      let profile = store.profile(&id).await.unwrap();
      store
        .add_instance(sh("sleep 30"), &profile, "acc".into())
        .await
        .unwrap();
    });
  }
  cx.run_until_parked();
  let running = |cx: &mut TestAppContext| cx.update(|cx| profiles.instances.read(cx).clone());
  assert_eq!(running(cx).len(), 3);
  assert_eq!(profiles.instances_memory().await.unwrap().len(), 3);

  let first = running(cx).into_iter().find(|i| i.profile_id == a).unwrap();
  assert!(
    profiles
      .stop_instance(b.clone(), first.id.clone())
      .await
      .is_err()
  );
  profiles
    .stop_instance(a.clone(), first.id.clone())
    .await
    .unwrap();
  wait_until(cx, "one stopped", |cx| running(cx).len() == 2);

  profiles.stop_all(Some(b.clone())).await.unwrap();
  wait_until(cx, "b stopped", |cx| {
    running(cx).iter().all(|i| i.profile_id == a)
  });
  assert_eq!(running(cx).len(), 1);
  profiles.stop_all(None).await.unwrap();
  wait_until(cx, "all stopped", |cx| running(cx).is_empty());
  // killed games are no crashes, so no events but the run logs
  assert!(profiles.instances_memory().await.unwrap().is_empty());
}

#[gpui::test]
async fn world_icons_and_server_status_resolve_through_the_service(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let data_dir = init_test(cx);
  cx.update(proton_config::init);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = create(&profiles, "P").await;
  let world = data_dir.join(PROFILE_DIR).join(&id).join("saves/w");
  std::fs::create_dir_all(&world).unwrap();
  std::fs::write(world.join("icon.png"), "icon").unwrap();

  let entry = quick_play("w", QuickPlayType::Singleplayer);
  let icon = cx
    .update(|cx| profiles.quick_play_icon(cx, id.clone(), entry.clone()))
    .await
    .unwrap();
  assert_eq!(icon, Some(b"icon".to_vec()));
  let missing = cx.update(|cx| profiles.quick_play_icon(cx, "nope".into(), entry));
  assert!(missing.await.is_err());

  let server = proton_utils::test_http::FakeServer::start();
  server.route(
    "GET",
    "/api.mcstatus.io/v2/status/java/a.b",
    200,
    r#"{"online":true,"players":{"online":1,"max":2}}"#,
  );
  let status = Profiles::server_status("a.b".into()).await.unwrap();
  assert_eq!((status.online, status.players_online), (true, 1));
  assert!(Profiles::server_status("c.d".into()).await.is_none());
}

/// A served vanilla 1.20.1 without assets or libraries whose java is [`fake_java`]. Returns the
/// data dir and the file the fake java records its calls in.
fn serve_game(cx: &mut TestAppContext, server: &FakeServer, java_exit: i32) -> (PathBuf, PathBuf) {
  use proton_utils::file::bytes_hash;
  use serde_json::json;

  let base = tempfile::tempdir().unwrap().keep();
  // SAFETY: nextest runs every test in its own process, versions resolve the data dir from it
  unsafe { std::env::set_var("XDG_DATA_HOME", &base) };
  let data_dir = proton_utils::dir::data_dir().unwrap();
  proton_store::init_at(base.join("store.json"));

  let origin = server.origin();
  let serve = |path: &str, body: &[u8]| {
    server.route("GET", path, 200, body.to_vec());
    json!({ "url": format!("{origin}{path}"), "size": body.len(), "sha1": bytes_hash(body) })
  };

  // the downloaded java is the fake, it records into a file next to this script
  let script = base.join("java/java");
  let record = fake_java(&script, java_exit);
  let java = serve("/java/bin/java", &std::fs::read(&script).unwrap());
  let files = json!({ "files": {
    "bin/java": { "type": "file", "executable": true, "downloads": { "raw": java } }
  } })
  .to_string();
  let runtime = json!([{
    "manifest": serve("/java/manifest.json", files.as_bytes()),
    "version": { "name": "17.0.8", "released": "2023-01-01" }
  }]);
  let mut platform = json!({
    "java-runtime-alpha": [], "java-runtime-beta": [], "java-runtime-delta": [],
    "java-runtime-epsilon": [], "java-runtime-gamma-snapshot": [], "jre-legacy": []
  });
  platform["java-runtime-gamma"] = runtime;
  let platforms = [
    "linux",
    "linux-i386",
    "mac-os",
    "mac-os-arm64",
    "windows-arm64",
    "windows-x64",
    "windows-x86",
  ];
  let all: serde_json::Map<_, _> = platforms
    .into_iter()
    .map(|p| (p.to_string(), platform.clone()))
    .collect();
  server.route(
    "GET",
    "/launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
    200,
    serde_json::Value::Object(all).to_string(),
  );

  let index = r#"{"objects":{}}"#;
  let mut asset_index = serve("/indexes/5.json", index.as_bytes());
  asset_index["id"] = "5".into();
  asset_index["totalSize"] = 0.into();
  let version = json!({
    "id": "1.20.1",
    "assetIndex": asset_index,
    "downloads": { "client": serve("/client.jar", b"client") },
    "javaVersion": { "component": "java-runtime-gamma", "majorVersion": 17 },
    "libraries": [],
    "mainClass": "net.minecraft.client.main.Main",
    "type": "release"
  })
  .to_string();
  let mut entry = serve("/v/1.20.1.json", version.as_bytes());
  entry["id"] = "1.20.1".into();
  entry["type"] = "release".into();
  server.route(
    "GET",
    "/launchermeta.mojang.com/mc/game/version_manifest_v2.json",
    200,
    json!({ "versions": [entry] }).to_string(),
  );

  cx.update(|cx| {
    proton_config::init(cx);
    proton_versions::init(cx);
    init_at(cx, data_dir.clone()).unwrap();
  });
  wait_until(cx, "version manifests", |cx| {
    cx.update(|cx| cx.versions().store(cx).is_ok())
  });
  (data_dir, record)
}

fn download_states(cx: &mut TestAppContext) -> Vec<DownloadStatus> {
  cx.update(|cx| {
    let downloads = cx.versions().downloads.read(cx);
    downloads.rows().map(|r| r.read(cx).status).collect()
  })
}

fn auth() -> LaunchInfo {
  LaunchInfo {
    id: "uuid-1".into(),
    name: "Steve".into(),
    access_token: "secret-token".into(),
  }
}

#[gpui::test]
async fn launch_downloads_once_then_starts_the_game(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let server = FakeServer::start();
  let (data_dir, record) = serve_game(cx, &server, 0);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = profiles
    .create(
      "Game".into(),
      None,
      "1.20.1".into(),
      LoaderType::Vanilla,
      None,
    )
    .await
    .unwrap();

  let launch = cx.update(|cx| profiles.launch(cx, id.clone(), None, auth()));
  launch.await.unwrap();
  wait_until(cx, "game exited", |_| java_calls(&record).len() == 1);
  let call = &java_calls(&record)[0];
  // the game runs inside its profile dir
  let cwd = data_dir.join(PROFILE_DIR).join(&id);
  assert_eq!(call[0], format!("cwd={}", cwd.display()));
  for arg in ["net.minecraft.client.main.Main", "-Xmx2048M"] {
    assert!(call.iter().any(|a| a == arg), "{arg} missing in {call:?}");
  }

  cx.run_until_parked();
  let profile = cx.update(|cx| profiles.profiles.read(cx)[0].clone());
  assert!(profile.downloaded);
  assert!(profile.last_played.is_some() && profile.last_played_non_quick_play.is_some());
  assert_eq!(cx.update(|cx| profiles.history.read(cx).len()), 1);
  assert_eq!(download_states(cx), [DownloadStatus::Done]);

  // downloaded: only the meta is checked, nothing is fetched again
  let fetched = server.calls().len();
  let launch = cx.update(|cx| profiles.launch(cx, id.clone(), None, auth()));
  launch.await.unwrap();
  wait_until(cx, "second run", |_| java_calls(&record).len() == 2);
  assert_eq!(server.calls().len(), fetched);
}

#[gpui::test]
async fn launch_uses_the_profile_jvm_settings_and_reports_a_crash(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let server = FakeServer::start();
  let (_, record) = serve_game(cx, &server, 1);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = profiles
    .create(
      "Crashy".into(),
      None,
      "1.20.1".into(),
      LoaderType::Vanilla,
      None,
    )
    .await
    .unwrap();
  let jvm = proton_config::settings::JvmSettings {
    args: vec!["-Dproton=1".into()],
    env_vars: [("PROTON_TEST_ENV".to_string(), "local".to_string())].into(),
    mem_max: 1234,
  };
  profiles
    .update(crate::config::ProfileUpdate {
      id: id.clone(),
      name: "Crashy".into(),
      version: "1.20.1".into(),
      loader_version: None,
      use_local_game: false,
      game: None,
      use_local_jvm: true,
      jvm: Some(jvm),
    })
    .await
    .unwrap();
  let events = Rc::new(RefCell::new(Vec::new()));
  let seen = events.clone();
  cx.update(|cx| {
    cx.subscribe(&profiles.events, move |_, e: &ProfilesEvent, _| {
      seen.borrow_mut().push(e.clone())
    })
    .detach()
  });

  let launch = cx.update(|cx| profiles.launch(cx, id.clone(), None, auth()));
  launch.await.unwrap();
  wait_until(cx, "crash event", |_| !events.borrow().is_empty());
  let call = &java_calls(&record)[0];
  assert_eq!(call[1], "env=local");
  for arg in ["-Dproton=1", "-Xmx1234M"] {
    assert!(call.iter().any(|a| a == arg), "{arg} missing in {call:?}");
  }
  assert_eq!(
    *events.borrow(),
    [ProfilesEvent::InstanceCrashed {
      profile_name: "Crashy".into()
    }]
  );
}

#[gpui::test]
async fn repair_downloads_without_starting_and_unknown_versions_fail(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  let server = FakeServer::start();
  let (data_dir, record) = serve_game(cx, &server, 0);
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = profiles
    .create("P".into(), None, "1.20.1".into(), LoaderType::Vanilla, None)
    .await
    .unwrap();

  let repair = cx.update(|cx| profiles.repair(cx, id.clone()));
  repair.await.unwrap();
  cx.run_until_parked();
  assert!(cx.update(|cx| profiles.profiles.read(cx)[0].downloaded));
  assert!(
    proton_versions::paths::MCVersionPath::new(&data_dir, "1.20.1")
      .client_jar()
      .exists()
  );
  assert!(java_calls(&record).is_empty());

  // a version the manifest does not know fails the download row and the launch
  let other = profiles
    .create("Old".into(), None, "9.9".into(), LoaderType::Vanilla, None)
    .await
    .unwrap();
  let launch = cx.update(|cx| profiles.launch(cx, other.clone(), None, auth()));
  assert!(launch.await.is_err());
  let repair = cx.update(|cx| profiles.repair(cx, other.clone()));
  assert!(repair.await.is_err());
  cx.run_until_parked();
  assert_eq!(
    download_states(cx),
    [
      DownloadStatus::Done,
      DownloadStatus::Error,
      DownloadStatus::Error
    ]
  );
  let old = profiles_named(cx, &profiles, "Old");
  assert!(!old.downloaded && old.last_played.is_none());
  assert!(java_calls(&record).is_empty());
}

fn profiles_named(cx: &mut TestAppContext, profiles: &Profiles, name: &str) -> Profile {
  cx.update(|cx| {
    profiles
      .profiles
      .read(cx)
      .iter()
      .find(|p| p.name == name)
      .unwrap()
      .clone()
  })
}

#[gpui::test]
async fn launch_fails_while_the_manifests_are_not_loaded(cx: &mut TestAppContext) {
  cx.executor().allow_parking();
  init_test(cx);
  cx.update(|cx| {
    proton_config::init(cx);
    proton_versions::init_state(cx);
  });
  let profiles = cx.update(|cx| cx.profiles().clone());
  let id = create(&profiles, "P").await;
  let launch = cx.update(|cx| profiles.launch(cx, id.clone(), None, auth()));
  assert!(launch.await.is_err());
  let repair = cx.update(|cx| profiles.repair(cx, id));
  assert!(repair.await.is_err());
  // no download row was started
  assert!(download_states(cx).is_empty());
}
