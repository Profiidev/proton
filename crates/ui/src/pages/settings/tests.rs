//! Each page against a temp copy of `fixtures` (Steve_Fixture active, Alex_Fixture)

use std::{
  path::Path,
  time::{Duration, Instant},
};

use gpui_kit::{
  self as gpui, AnyWindowHandle, App, AppContext, Bounds, TestAppContext, WindowBounds,
  WindowOptions, component::WindowExt, px, size, test::TestWindowExt,
};
use proton_account::AccountsExt;
use proton_config::{OfflineExt, SettingsExt};

use crate::{
  router::{Route, RouterExt, SettingsTab},
  shell::Shell,
};

const STEVE: &str = "f1f1f1f1f1f14f1f8f1f1f1f1f1f1f1f";
const ALEX: &str = "a2a2a2a2a2a24a2a8a2a2a2a2a2a2a2a";

fn copy_dir(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap();
  for entry in std::fs::read_dir(from).unwrap() {
    let entry = entry.unwrap();
    std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
  }
}

/// The shell at `tab`, services loaded from the fixture data
fn open(cx: &mut TestAppContext, tab: SettingsTab) -> AnyWindowHandle {
  cx.executor().allow_parking();
  let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/io.profidev.proton");
  let home = tempfile::tempdir().unwrap().keep();
  let data = home.join("io.profidev.proton");
  copy_dir(&fixtures.join("skins"), &data.join("skins"));
  std::fs::copy(fixtures.join("store.json"), data.join("store.json")).unwrap();
  // SAFETY: nextest runs every test in its own process, nothing else reads the env yet
  unsafe { std::env::set_var("XDG_DATA_HOME", &home) };
  proton_store::init_at(data.join("store.json"));

  cx.update(|cx| {
    gpui_kit::init(cx);
    proton_components::assets::load(cx).unwrap();
    // animations run on the wall clock: a click mid dialog-entrance misses its target
    cx.set_reduce_motion(true);
    proton_config::init(cx);
    proton_versions::init(cx);
    proton_account::init(cx).unwrap();
    crate::init(cx);
    super::register(cx);
    cx.navigate(Route::Settings(tab));
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
    .0
  })
}

/// Runs the app (and the tokio services) until `done`, at most 10s
fn settle(cx: &mut TestAppContext, mut done: impl FnMut(&mut App) -> bool) {
  let start = Instant::now();
  loop {
    cx.run_until_parked();
    if cx.update(&mut done) {
      return;
    }
    assert!(start.elapsed() < Duration::from_secs(10), "timed out");
    std::thread::sleep(Duration::from_millis(10));
  }
}

fn act(
  cx: &mut TestAppContext,
  handle: AnyWindowHandle,
  f: impl FnOnce(&mut gpui::Window, &mut App),
) {
  cx.update_window(handle, |_, window, cx| {
    window.render_frame(cx);
    // new pages build on the next frame
    window.simulate_next_frame(cx);
    window.render_frame(cx);
    f(window, cx);
  })
  .unwrap();
  cx.run_until_parked();
}

/// Toasts shown, after the closing ones finished their exit
fn toasts(cx: &mut TestAppContext, handle: AnyWindowHandle) -> usize {
  cx.executor().advance_clock(Duration::from_secs(1));
  cx.run_until_parked();
  cx.update_window(handle, |_, window, cx| window.notifications(cx).len())
    .unwrap()
}

fn active(cx: &App) -> Option<String> {
  cx.accounts().active_id(cx).cloned()
}

#[gpui::test]
fn accounts_select_and_remove(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Accounts);
  assert_eq!(cx.update(|cx| active(cx)).as_deref(), Some(STEVE));
  act(cx, handle, |w, cx| w.click(format!("select-{ALEX}"), cx));
  settle(cx, |cx| active(cx).as_deref() == Some(ALEX));

  act(cx, handle, |w, cx| w.click(format!("remove-{STEVE}"), cx));
  act(cx, handle, |w, cx| {
    assert!(w.has_active_dialog(cx));
    w.within("dialog").click("ok", cx);
  });
  settle(cx, |cx| {
    !cx.accounts().accounts.read(cx).contains_key(STEVE)
  });
  settle(cx, |_| true);
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
fn add_account_offline_warns(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Accounts);
  cx.update(|cx| {
    let offline = cx.offline().offline.clone();
    offline.update(cx, |o, cx| {
      *o = true;
      cx.notify();
    })
  });
  act(cx, handle, |w, cx| w.click("add-account", cx));
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
fn account_selector_switches(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::About);
  act(cx, handle, |w, cx| w.click("account-selector", cx));
  act(cx, handle, |w, cx| w.click(format!("account-{ALEX}"), cx));
  settle(cx, |cx| active(cx).as_deref() == Some(ALEX));
}

#[gpui::test]
fn skins_page_adds_from_file(cx: &mut TestAppContext) {
  // the idle animation would keep the executor busy forever
  cx.update(|cx| cx.set_reduce_motion(true));
  let handle = open(cx, SettingsTab::Skins);
  act(cx, handle, |w, _| {
    assert!(w.try_find("skin-animation").is_some());
  });
  act(cx, handle, |w, cx| w.click("add-skin", cx));
  assert!(cx.did_prompt_for_paths());
}

#[gpui::test]
fn capes_offline_change_warns(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Capes);
  cx.update(|cx| {
    let offline = cx.offline().offline.clone();
    offline.update(cx, |o, cx| {
      *o = true;
      cx.notify();
    })
  });
  // the grid shows once the skin texture is decoded
  settle(cx, |cx| {
    cx.update_window(handle, |_, w, cx| {
      w.render_frame(cx);
      w.simulate_next_frame(cx);
      w.render_frame(cx);
      w.try_find("cape-REMOVE_CAPE").is_some()
    })
    .unwrap()
  });
  act(cx, handle, |w, cx| w.click("cape-REMOVE_CAPE", cx));
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
fn minecraft_switches_save(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Minecraft);
  act(cx, handle, |w, cx| w.click("show-snapshots", cx));
  act(cx, handle, |w, cx| {
    w.within("game-settings").click("use-custom", cx)
  });
  cx.update(|cx| {
    let s = cx.settings().get(cx);
    assert!(s.minecraft.show_snapshots);
    assert!(s.minecraft.game_settings.use_custom);
  });
  act(cx, handle, |w, cx| w.click("add-folder", cx));
  cx.simulate_path_prompt_response(|_| Some(vec!["/tmp/dl".into()]));
  cx.run_until_parked();
  cx.update(|cx| {
    assert_eq!(
      cx.settings().get(cx).download_dirs,
      Some(vec!["/tmp/dl".into()])
    )
  });
  act(cx, handle, |w, cx| w.click("remove-dir-/tmp/dl", cx));
  cx.update(|cx| assert_eq!(cx.settings().get(cx).download_dirs, Some(vec![])));
}

#[gpui::test]
fn java_env_needs_key_and_value(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Java);
  act(cx, handle, |w, cx| {
    w.within("java-settings").click("add-env", cx)
  });
  assert_eq!(toasts(cx, handle), 1);
  cx.update(|cx| {
    assert!(
      cx.settings()
        .get(cx)
        .minecraft
        .jvm_settings
        .env_vars
        .is_empty()
    )
  });
}

#[gpui::test]
fn about_checks_updates_and_saves(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::About);
  // nothing listens there: the check fails fast, offline-safe
  cx.update(|cx| {
    crate::updater::UpdaterExt::updater(cx).update(cx, |u, _| {
      u.endpoint = "http://127.0.0.1:9/latest.json".into()
    })
  });
  act(cx, handle, |w, cx| w.click("check-updates", cx));
  settle(cx, |cx| {
    let state = &crate::updater::UpdaterExt::updater(cx).read(cx).state;
    matches!(state, crate::updater::UpdateState::Error(_))
  });
  cx.run_until_parked();
  // "Failed to check for updates"
  assert_eq!(toasts(cx, handle), 1);
  act(cx, handle, |w, cx| w.click("auto-update", cx));
  cx.update(|cx| assert!(cx.settings().get(cx).auto_update));
}

/// Runs until `n` toasts show
fn wait_toasts(cx: &mut TestAppContext, handle: AnyWindowHandle, n: usize) {
  settle(cx, |cx| {
    cx.update_window(handle, |_, w, cx| w.notifications(cx).len() >= n)
      .unwrap()
  });
}

fn clear_toasts(cx: &mut TestAppContext, handle: AnyWindowHandle) {
  cx.update_window(handle, |_, w, cx| w.clear_notifications(cx))
    .unwrap();
  assert_eq!(toasts(cx, handle), 0);
}

fn data_dir() -> std::path::PathBuf {
  Path::new(&std::env::var_os("XDG_DATA_HOME").unwrap()).join("io.profidev.proton")
}

fn go_offline(cx: &mut TestAppContext) {
  cx.update(|cx| {
    let offline = cx.offline().offline.clone();
    offline.update(cx, |o, cx| {
      *o = true;
      cx.notify();
    })
  });
}

fn has(cx: &mut TestAppContext, handle: AnyWindowHandle, id: impl Into<gpui::ElementId>) -> bool {
  let id = id.into();
  cx.update_window(handle, |_, w, cx| {
    w.render_frame(cx);
    w.simulate_next_frame(cx);
    w.render_frame(cx);
    w.try_find(id).is_some()
  })
  .unwrap()
}

/// The fixture profile of `account` with `edit` applied, as the Minecraft API answers
fn profile_json(account: &str, edit: impl FnOnce(&mut serde_json::Value)) -> String {
  let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/io.profidev.proton");
  let store: serde_json::Value =
    serde_json::from_slice(&std::fs::read(fixtures.join("store.json")).unwrap()).unwrap();
  let mut profile = store["accounts"][account]["profile"].clone();
  edit(&mut profile);
  profile.to_string()
}

/// Requests to the Minecraft API (the versions service fetches its manifests on its own)
fn api_calls(server: &proton_utils::test_http::FakeServer) -> Vec<proton_utils::test_http::Call> {
  let calls = server.calls().into_iter();
  calls
    .filter(|c| c.path.starts_with("/api.minecraftservices.com"))
    .collect()
}

fn active_cape(cx: &App) -> Option<String> {
  let accounts = cx.accounts();
  let info = accounts.accounts.read(cx).get(STEVE)?.clone()?;
  super::textures::active_cape(&info).map(|c| c.alias.clone())
}

#[gpui::test]
fn accounts_mark_the_active_one_and_expired_ones_cannot_be_selected(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Accounts);
  cx.update(|cx| {
    cx.accounts().accounts.clone().update(cx, |list, cx| {
      list.insert("expired".into(), None);
      cx.notify();
    })
  });
  // the active account shows "Selected" instead of a button
  assert!(!has(cx, handle, format!("select-{STEVE}")));
  assert!(has(cx, handle, format!("select-{ALEX}")));
  // "Login failed" rows can only be removed
  assert!(!has(cx, handle, "select-expired"));
  assert!(has(cx, handle, "remove-expired"));
}

#[gpui::test]
fn account_changes_that_fail_to_save_toast(cx: &mut TestAppContext) {
  use std::os::unix::fs::PermissionsExt;
  let handle = open(cx, SettingsTab::Accounts);
  let data = data_dir();
  // the store writes a temp file next to itself
  std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o555)).unwrap();
  let before = cx.update(|cx| active(cx));
  act(cx, handle, |w, cx| w.click(format!("select-{ALEX}"), cx));
  // "Failed to switch Account", the old account stays active since nothing was saved
  wait_toasts(cx, handle, 1);
  assert_eq!(cx.update(|cx| active(cx)), before);

  act(cx, handle, |w, cx| w.click(format!("remove-{STEVE}"), cx));
  act(cx, handle, |w, cx| w.within("dialog").click("ok", cx));
  // "Failed to remove Account", the account is still listed
  wait_toasts(cx, handle, 2);
  assert!(cx.update(|cx| cx.accounts().accounts.read(cx).contains_key(STEVE)));
  std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[gpui::test]
fn account_selector_without_accounts_links_to_the_accounts_page(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::About);
  cx.update(|cx| {
    cx.accounts().accounts.clone().update(cx, |list, cx| {
      list.clear();
      cx.notify();
    })
  });
  act(cx, handle, |w, cx| w.click("account-selector", cx));
  act(cx, handle, |w, cx| w.click("add-one", cx));
  assert_eq!(
    cx.update(|cx| cx.route()),
    Route::Settings(SettingsTab::Accounts)
  );
}

#[gpui::test]
fn account_selector_lists_expired_accounts_without_selecting_them(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::About);
  cx.update(|cx| {
    cx.accounts().accounts.clone().update(cx, |list, cx| {
      list.insert("expired".into(), None);
      cx.notify();
    })
  });
  act(cx, handle, |w, cx| w.click("account-selector", cx));
  act(cx, handle, |w, cx| w.click("account-expired", cx));
  settle(cx, |_| true);
  assert_eq!(cx.update(|cx| active(cx)).as_deref(), Some(STEVE));
}

/// A fake `xdg-open` that writes the url it was asked to open into the returned file
fn fake_browser() -> std::path::PathBuf {
  use std::os::unix::fs::PermissionsExt;
  let dir = tempfile::tempdir().unwrap().keep();
  let opened = dir.join("opened");
  let script = dir.join("xdg-open");
  std::fs::write(
    &script,
    format!("#!/bin/sh\necho \"$1\" > {}\n", opened.display()),
  )
  .unwrap();
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
  let path = std::env::var("PATH").unwrap_or_default();
  // SAFETY: nextest runs every test in its own process
  unsafe { std::env::set_var("PATH", format!("{}:{path}", dir.display())) };
  opened
}

/// Waits for the browser to open the authorize url, returns its `state`
fn opened_state(cx: &mut TestAppContext, opened: &Path) -> String {
  settle(cx, |_| opened.exists());
  let url = std::fs::read_to_string(opened).unwrap();
  std::fs::remove_file(opened).unwrap();
  let url = url::Url::parse(url.trim()).unwrap();
  let (_, state) = url.query_pairs().find(|(k, _)| k == "state").unwrap();
  state.into_owned()
}

/// What Microsoft's redirect to the login listener looks like
fn redirect(state: &str) {
  use std::io::{Read, Write};
  let mut conn = std::net::TcpStream::connect(("127.0.0.1", 51701)).unwrap();
  write!(
    conn,
    "GET /?code=c0de&state={state} HTTP/1.1\r\nHost: localhost\r\n\r\n"
  )
  .unwrap();
  let mut res = String::new();
  conn.read_to_string(&mut res).unwrap();
  assert!(res.starts_with("HTTP/1.1 200"), "{res}");
}

#[gpui::test]
fn login_can_be_cancelled_fail_and_add_the_account(cx: &mut TestAppContext) {
  use proton_utils::test_http::FakeServer;
  // ponytail: the redirect port is fixed (51701), a parallel login test elsewhere can race it;
  // a nextest test group would serialize them
  let opened = fake_browser();
  let server = FakeServer::start();
  let handle = open(cx, SettingsTab::Accounts);
  let ms = "/login.microsoftonline.com/consumers/oauth2/v2.0/token";
  server
    .route("POST", ms, 500, "down")
    .route(
      "POST",
      "/user.auth.xboxlive.com/user/authenticate",
      200,
      r#"{"NotAfter":"2099-01-01T00:00:00Z","Token":"xbl","DisplayClaims":{"xui":[{"uhs":"hash"}]}}"#,
    )
    .route(
      "POST",
      "/xsts.auth.xboxlive.com/xsts/authorize",
      200,
      r#"{"NotAfter":"2099-01-01T00:00:00Z","Token":"xsts","DisplayClaims":{"xui":[{"uhs":"hash"}]}}"#,
    )
    .route(
      "POST",
      "/api.minecraftservices.com/authentication/login_with_xbox",
      200,
      r#"{"access_token":"mc","expires_in":86400}"#,
    )
    .route(
      "GET",
      "/api.minecraftservices.com/minecraft/profile",
      200,
      r#"{"id":"c3c3","name":"Notch_Fixture","skins":[],"capes":[]}"#,
    );

  // cancelled from the toast: no result toast
  act(cx, handle, |w, cx| w.click("add-account", cx));
  opened_state(cx, &opened);
  act(cx, handle, |w, cx| w.click("cancel-login", cx));
  settle(cx, |_| true);
  assert_eq!(toasts(cx, handle), 0);

  // the token exchange fails: "Failed to add Account"
  act(cx, handle, |w, cx| w.click("add-account", cx));
  redirect(&opened_state(cx, &opened));
  settle(cx, |cx| {
    matches!(
      cx.accounts().login.read(cx),
      proton_account::LoginState::Error(_)
    )
  });
  settle(cx, |_| true);
  assert_eq!(toasts(cx, handle), 1);
  clear_toasts(cx, handle);

  server.route(
    "POST",
    ms,
    200,
    r#"{"expires_in":3600,"access_token":"ms","refresh_token":"r"}"#,
  );
  act(cx, handle, |w, cx| w.click("add-account", cx));
  redirect(&opened_state(cx, &opened));
  settle(cx, |cx| {
    cx.accounts().accounts.read(cx).contains_key("c3c3")
  });
  settle(cx, |_| true);
  // the step toast is gone, "Successfully added Account" stays
  assert_eq!(toasts(cx, handle), 1);
  assert!(has(cx, handle, "select-c3c3"));
  let hits: Vec<_> = server.calls().into_iter().map(|c| c.path).collect();
  assert_eq!(
    hits[hits.len() - 5..],
    [
      ms,
      "/user.auth.xboxlive.com/user/authenticate",
      "/xsts.auth.xboxlive.com/xsts/authorize",
      "/api.minecraftservices.com/authentication/login_with_xbox",
      "/api.minecraftservices.com/minecraft/profile",
    ]
  );
}

#[test]
fn login_steps_name_the_next_service() {
  use proton_account::auth::LoginStatus;
  let cases = [
    (LoginStatus::Ms, "Logging in to Xbox"),
    (LoginStatus::Xbox, "Logging in to Xbox Security"),
    (LoginStatus::XboxSecurity, "Logging in to Minecraft"),
    (LoginStatus::Mc, "Retrieving Minecraft Profile"),
  ];
  for (step, text) in cases {
    assert_eq!(super::accounts::step_message(&step), text);
  }
}

const SKINS: &str = "/api.minecraftservices.com/minecraft/profile/skins";
const CAPES: &str = "/api.minecraftservices.com/minecraft/profile/capes/active";
/// Local library skin with Alex's texture url, Steve wears the other one
const ALEX_SKIN: &str = "f95f506bca4c42d2ee4b8dfbda008132427f9a18";
const STEVE_SKIN: &str = "fb1c862dba2791fcc53c491eb2ddf5f7b118850c";

#[gpui::test]
fn wearing_a_skin_posts_its_url_and_toasts_the_outcome(cx: &mut TestAppContext) {
  use proton_utils::test_http::FakeServer;
  let server = FakeServer::start();
  let alex_url = "http://textures.minecraft.net/texture/891129157b1eb45a749b9c4169db7d5eadc7c1e85277b6a00a71cb5ced4e3382";
  server.route("POST", SKINS, 500, "down");
  let handle = open(cx, SettingsTab::Skins);

  // the worn skin is not sent again
  act(cx, handle, |w, cx| {
    w.click(format!("skin-{STEVE_SKIN}"), cx)
  });
  settle(cx, |_| true);
  assert!(api_calls(&server).is_empty());

  act(cx, handle, |w, cx| w.click(format!("skin-{ALEX_SKIN}"), cx));
  wait_toasts(cx, handle, 1);
  // "Failed to change Skin"
  assert_eq!(toasts(cx, handle), 1);
  clear_toasts(cx, handle);
  let call = &api_calls(&server)[0];
  assert_eq!(
    call.header("authorization"),
    Some("Bearer fixture-mc-token-Steve_Fixture")
  );
  let body: serde_json::Value = serde_json::from_slice(&call.body).unwrap();
  assert_eq!(
    body,
    serde_json::json!({"variant": "CLASSIC", "url": alex_url})
  );

  let profile = profile_json(STEVE, |p| p["skins"][0]["url"] = alex_url.into());
  server.route("POST", SKINS, 200, profile);
  act(cx, handle, |w, cx| w.click(format!("skin-{ALEX_SKIN}"), cx));
  settle(cx, |cx| {
    let info = cx.accounts().accounts.read(cx)[STEVE].clone().unwrap();
    super::textures::active_skin(&info).unwrap().url.as_str() == alex_url
  });
  wait_toasts(cx, handle, 1);
  // "Successfully changed Skin"
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
fn skins_offline_change_warns_without_a_request(cx: &mut TestAppContext) {
  use proton_utils::test_http::FakeServer;
  let server = FakeServer::start();
  let handle = open(cx, SettingsTab::Skins);
  go_offline(cx);
  act(cx, handle, |w, cx| w.click(format!("skin-{ALEX_SKIN}"), cx));
  settle(cx, |_| true);
  assert_eq!(toasts(cx, handle), 1);
  assert!(api_calls(&server).is_empty());
}

#[gpui::test]
fn skins_are_added_from_png_files_and_removed(cx: &mut TestAppContext) {
  use proton_utils::test_http::FakeServer;
  let server = FakeServer::start();
  let handle = open(cx, SettingsTab::Skins);
  let skins = |cx: &mut TestAppContext| cx.update(|cx| cx.accounts().skins.read(cx).len());
  let before = skins(cx);

  // the prompt was cancelled
  act(cx, handle, |w, cx| w.click("add-skin", cx));
  cx.simulate_path_prompt_response(|_| None);
  settle(cx, |_| true);
  assert_eq!((skins(cx), toasts(cx, handle)), (before, 0));

  // not an image: "Failed to add Skin"
  let dir = tempfile::tempdir().unwrap();
  let bad = dir.path().join("skin.png");
  std::fs::write(&bad, "not a png").unwrap();
  act(cx, handle, |w, cx| w.click("add-skin", cx));
  cx.simulate_path_prompt_response(move |_| Some(vec![bad]));
  settle(cx, |cx| {
    cx.update_window(handle, |_, w, cx| !w.notifications(cx).is_empty())
      .unwrap()
  });
  assert_eq!((skins(cx), toasts(cx, handle)), (before, 1));
  // toasts sit above the add button
  clear_toasts(cx, handle);

  let good = dir.path().join("new.png");
  image::RgbaImage::from_pixel(64, 64, image::Rgba([200, 10, 10, 255]))
    .save(&good)
    .unwrap();
  act(cx, handle, |w, cx| w.click("add-skin", cx));
  cx.simulate_path_prompt_response(move |_| Some(vec![good]));
  settle(cx, |cx| cx.accounts().skins.read(cx).len() == before + 1);
  settle(cx, |_| true);
  assert_eq!(toasts(cx, handle), 1);
  clear_toasts(cx, handle);

  // the hover trash of a skin that is not worn
  let tile = gpui::ElementId::Name(format!("skin-{ALEX_SKIN}").into());
  let remove = gpui::ElementId::NamedChild(std::sync::Arc::new(tile), "remove".into());
  let worn = gpui::ElementId::NamedChild(
    std::sync::Arc::new(gpui::ElementId::Name(format!("skin-{STEVE_SKIN}").into())),
    "remove".into(),
  );
  assert!(!has(cx, handle, worn));
  act(cx, handle, |w, cx| w.click(remove, cx));
  settle(cx, |cx| cx.accounts().skins.read(cx).len() == before);
  wait_toasts(cx, handle, 1);
  // a wear attempt of the tile below would toast late
  std::thread::sleep(Duration::from_millis(500));
  settle(cx, |_| true);
  // only "Successfully removed Skin", the tile under the trash was not worn
  assert_eq!(api_calls(&server), []);
  assert_eq!(toasts(cx, handle), 1);
  assert!(!data_dir().join(format!("skins/{ALEX_SKIN}.png")).exists());
}

/// Opens capes and waits for the grid (it shows once the bare skin is decoded)
fn open_capes(cx: &mut TestAppContext) -> AnyWindowHandle {
  let handle = open(cx, SettingsTab::Capes);
  settle(cx, |cx| {
    cx.update_window(handle, |_, w, cx| {
      w.render_frame(cx);
      w.simulate_next_frame(cx);
      w.render_frame(cx);
      w.try_find("cape-REMOVE_CAPE").is_some()
    })
    .unwrap()
  });
  handle
}

#[gpui::test]
fn capes_are_put_removed_and_failures_toast(cx: &mut TestAppContext) {
  use proton_utils::test_http::FakeServer;
  const PAN: &str = "2ed5269a-076e-4a3c-834a-2837d5b578f2";
  const VANILLA: &str = "7412868d-7be3-429f-b629-ed5074d2455c";
  let server = FakeServer::start();
  let pan = profile_json(STEVE, |p| {
    p["capes"][0]["state"] = "INACTIVE".into();
    p["capes"][1]["state"] = "ACTIVE".into();
  });
  let none = profile_json(STEVE, |p| p["capes"][0]["state"] = "INACTIVE".into());
  server
    .route("PUT", CAPES, 200, pan)
    .route("DELETE", CAPES, 200, none);
  let handle = open_capes(cx);
  assert_eq!(cx.update(|cx| active_cape(cx)).as_deref(), Some("Vanilla"));

  // the worn cape is not sent again
  act(cx, handle, |w, cx| w.click(format!("cape-{VANILLA}"), cx));
  settle(cx, |_| true);
  assert!(api_calls(&server).is_empty());

  act(cx, handle, |w, cx| w.click(format!("cape-{PAN}"), cx));
  settle(cx, |cx| active_cape(cx).as_deref() == Some("Pan"));
  wait_toasts(cx, handle, 1);
  act(cx, handle, |w, cx| w.click("cape-REMOVE_CAPE", cx));
  settle(cx, |cx| active_cape(cx).is_none());
  wait_toasts(cx, handle, 2);
  // two "Successfully changed Cape"
  assert_eq!(toasts(cx, handle), 2);
  clear_toasts(cx, handle);
  let calls = api_calls(&server);
  let sent: Vec<_> = calls
    .iter()
    .map(|c| (c.method.as_str(), c.body_str()))
    .collect();
  assert_eq!(
    sent,
    [
      ("PUT", format!(r#"{{"capeId":"{PAN}"}}"#).as_str()),
      ("DELETE", r#"{"capeId":"REMOVE_CAPE"}"#),
    ]
  );

  server.route("PUT", CAPES, 403, "not owned");
  act(cx, handle, |w, cx| w.click(format!("cape-{PAN}"), cx));
  wait_toasts(cx, handle, 1);
  // "Failed to change Cape", nothing changed
  assert_eq!(toasts(cx, handle), 1);
  assert_eq!(cx.update(|cx| active_cape(cx)), None);
}

#[gpui::test]
fn capes_without_an_active_account_show_none(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Capes);
  let accounts = cx.update(|cx| cx.accounts().clone());
  let task = accounts.set_active("nobody".into());
  settle(cx, |cx| active(cx).as_deref() == Some("nobody"));
  drop(task);
  assert!(!has(cx, handle, "cape-REMOVE_CAPE"));
}

#[gpui::test]
fn about_offers_the_found_update_and_warns_offline(cx: &mut TestAppContext) {
  use proton_utils::test_http::FakeServer;
  let server = FakeServer::start();
  let latest = serde_json::json!({
    "version": "99.0.0",
    "notes": "Fixes",
    "pub_date": "2026-10-16T12:00:00Z",
    "platforms": {"linux-x86_64": {"url": format!("{}/x", server.origin()), "signature": "c2ln", "format": "appimage"}},
  });
  server.route("GET", "/latest.json", 200, latest.to_string());
  let handle = open(cx, SettingsTab::About);
  let endpoint = format!("{}/latest.json", server.origin());
  cx.update(|cx| crate::updater::UpdaterExt::updater(cx).update(cx, |u, _| u.endpoint = endpoint));
  act(cx, handle, |w, cx| w.click("check-updates", cx));
  settle(cx, |cx| {
    matches!(
      crate::updater::UpdaterExt::updater(cx).read(cx).state,
      crate::updater::UpdateState::Available { .. }
    )
  });
  // no toast for a found update, the button turns into "Install 99.0.0"
  assert_eq!(toasts(cx, handle), 0);
  assert!(has(cx, handle, "install-update"));
  assert!(!has(cx, handle, "check-updates"));

  act(cx, handle, |w, cx| w.click("hide-window-controls", cx));
  cx.update(|cx| assert!(cx.settings().get(cx).hide_window_controls));

  cx.update(|cx| {
    crate::updater::UpdaterExt::updater(cx).update(cx, |u, cx| {
      u.state = crate::updater::UpdateState::Idle;
      cx.notify()
    })
  });
  go_offline(cx);
  act(cx, handle, |w, cx| w.click("check-updates", cx));
  // the offline warning, no second request
  assert_eq!(toasts(cx, handle), 1);
  let checks = server
    .calls()
    .into_iter()
    .filter(|c| c.path == "/latest.json");
  assert_eq!(checks.count(), 1);
}

#[gpui::test]
fn about_says_when_there_is_no_update(cx: &mut TestAppContext) {
  use proton_utils::test_http::FakeServer;
  let server = FakeServer::start();
  let latest = serde_json::json!({
    "version": env!("CARGO_PKG_VERSION"),
    "pub_date": "2026-10-16T12:00:00Z",
    "platforms": {"linux-x86_64": {"url": format!("{}/x", server.origin()), "signature": "c2ln", "format": "appimage"}},
  });
  server.route("GET", "/latest.json", 200, latest.to_string());
  let handle = open(cx, SettingsTab::About);
  let endpoint = format!("{}/latest.json", server.origin());
  cx.update(|cx| crate::updater::UpdaterExt::updater(cx).update(cx, |u, _| u.endpoint = endpoint));
  act(cx, handle, |w, cx| w.click("check-updates", cx));
  settle(cx, |cx| {
    crate::updater::UpdaterExt::updater(cx).read(cx).state == crate::updater::UpdateState::UpToDate
  });
  settle(cx, |_| true);
  // "No updates available"
  assert_eq!(toasts(cx, handle), 1);
}

#[gpui::test]
fn download_folders_are_not_added_twice(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Minecraft);
  for _ in 0..2 {
    act(cx, handle, |w, cx| w.click("add-folder", cx));
    cx.simulate_path_prompt_response(|_| Some(vec!["/tmp/dl".into()]));
    cx.run_until_parked();
  }
  // a cancelled prompt changes nothing
  act(cx, handle, |w, cx| w.click("add-folder", cx));
  cx.simulate_path_prompt_response(|_| None);
  cx.run_until_parked();
  cx.update(|cx| {
    assert_eq!(
      cx.settings().get(cx).download_dirs,
      Some(vec!["/tmp/dl".into()])
    )
  });
}

#[gpui::test]
fn settings_changed_elsewhere_reach_the_open_forms(cx: &mut TestAppContext) {
  let handle = open(cx, SettingsTab::Minecraft);
  act(cx, handle, |_, _| {});
  cx.update(|cx| proton_config::update(cx, |s| s.minecraft.game_settings.use_custom = true));
  // the switch now reads on, a click turns it off again
  act(cx, handle, |w, cx| {
    w.within("game-settings").click("use-custom", cx)
  });
  cx.update(|cx| assert!(!cx.settings().get(cx).minecraft.game_settings.use_custom));
}
