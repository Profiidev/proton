//! Self update via cargo-packager-updater: `latest.json` on the GitHub release (written by
//! `.github/workflows/app-release.yml`), bundles signed with the old Tauri updater key.
//! Self-updater: checks, downloads and installs releases.

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use cargo_packager_updater::{Config, Update, UpdaterBuilder, semver::Version, url::Url};
use gpui_kit::{
  App, AppContext, Context, Entity, Global, SharedString, Styled, Task,
  component::{
    WindowExt,
    button::{Button, ButtonVariants},
    notification::Notification,
  },
};
use proton_config::SettingsExt;

use crate::{
  actions::toast,
  router::{Route, RouterExt, SettingsTab},
};

pub const ENDPOINT: &str =
  "https://github.com/Profiidev/proton/releases/latest/download/latest.json";
/// The minisign key of the old Tauri updater, cargo-packager signs and verifies the same way
pub const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDQ2QzRBNzRCNkZEMEQ5MzEKUldReDJkQnZTNmZFUmd0STI1dnRzYld4ekZsUCtQY3ZKV2dMV3ZsMmNnUTNUTHZJTWdDZm5ZekYK";
/// Set on the relaunched process, it waits for the old one to release the single instance socket
pub const RESTART_ENV: &str = "PROTON_RESTARTED";

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
  Idle,
  Checking,
  UpToDate,
  Available {
    version: SharedString,
    notes: Option<SharedString>,
  },
  Downloading {
    version: SharedString,
    percent: Option<u8>,
  },
  Installing {
    version: SharedString,
  },
  Error(SharedString),
}

pub struct Updater {
  pub state: UpdateState,
  /// `ENDPOINT`, tests point it elsewhere
  pub endpoint: String,
  pending: Option<Update>,
}

struct GlobalUpdater(Entity<Updater>);
impl Global for GlobalUpdater {}

pub trait UpdaterExt {
  fn updater(&self) -> Entity<Updater>;
}

impl UpdaterExt for App {
  fn updater(&self) -> Entity<Updater> {
    self.global::<GlobalUpdater>().0.clone()
  }
}

pub fn init(cx: &mut App) {
  let updater = cx.new(|_| Updater {
    state: UpdateState::Idle,
    endpoint: ENDPOINT.into(),
    pending: None,
  });
  cx.set_global(GlobalUpdater(updater));
}

/// Binary the deb package installs
const DEB_BINARY: &str = "/usr/bin/proton";
/// dpkg's file list of the `proton` package
const DPKG_LIST: &str = "/var/lib/dpkg/info/proton.list";

/// How this build updates itself
#[derive(Debug, Clone, Copy, PartialEq)]
enum Install {
  /// Replace the AppImage (Windows/macOS take the same path through cargo-packager)
  Bundle,
  /// Download the signed .deb and install it with `pkexec dpkg -i`, like the old Tauri app
  Deb,
  /// Managed elsewhere (nix, cargo run), refuse to self update
  Unmanaged,
}

/// True when `exe` is the file the proton deb installed, `dpkg_list` is its dpkg file list
fn deb_owns(exe: &Path, dpkg_list: &str) -> bool {
  exe == Path::new(DEB_BINARY) && dpkg_list.lines().any(|l| l == DEB_BINARY)
}

fn install_kind() -> Install {
  if !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some() {
    return Install::Bundle;
  }
  let exe = std::env::current_exe().unwrap_or_default();
  let list = std::fs::read_to_string(DPKG_LIST).unwrap_or_default();
  if deb_owns(&exe, &list) {
    Install::Deb
  } else {
    Install::Unmanaged
  }
}

fn builder(endpoint: &str) -> anyhow::Result<cargo_packager_updater::Updater> {
  let config = Config {
    endpoints: vec![Url::parse(endpoint)?],
    pubkey: PUBKEY.into(),
    ..Default::default()
  };
  let mut builder = UpdaterBuilder::new(Version::parse(env!("CARGO_PKG_VERSION"))?, config)
    .timeout(Duration::from_secs(30));
  // `linux-x86_64-deb` in latest.json (app-release.yml), the same entry old Tauri debs read
  if install_kind() == Install::Deb
    && let Some(target) = cargo_packager_updater::target()
  {
    builder = builder.target(format!("{target}-deb"));
  }
  Ok(builder.build()?)
}

/// The newer release at `endpoint`, `None` when this build is up to date (blocking)
fn fetch(endpoint: &str) -> anyhow::Result<Option<Update>> {
  Ok(builder(endpoint)?.check()?)
}

impl Updater {
  fn set(&mut self, state: UpdateState, cx: &mut Context<Self>) {
    if self.state != state {
      self.state = state;
      cx.notify();
    }
  }

  /// Looks for a newer release, `Some(version)` when there is one
  pub fn check(&mut self, cx: &mut Context<Self>) -> Task<anyhow::Result<Option<SharedString>>> {
    self.set(UpdateState::Checking, cx);
    let endpoint = self.endpoint.clone();
    let job = proton_utils::rt::spawn_blocking(move || fetch(&endpoint));
    cx.spawn(async move |this, cx| {
      let res = job.await.map_err(anyhow::Error::from).and_then(|r| r);
      this.update(cx, |this, cx| {
        let state = match &res {
          Ok(Some(u)) => UpdateState::Available {
            version: u.version.clone().into(),
            notes: u.body.clone().filter(|n| !n.is_empty()).map(Into::into),
          },
          Ok(None) => UpdateState::UpToDate,
          Err(e) => UpdateState::Error(format!("{e:#}").into()),
        };
        this.set(state, cx);
        res.map(|u| {
          let version = u.as_ref().map(|u| SharedString::from(u.version.clone()));
          this.pending = u;
          version
        })
      })?
    })
  }
}

/// Downloads, verifies and installs the pending update, then restarts. Progress and the outcome
/// are toasts like in the old app.
pub fn install(cx: &mut App) {
  let updater = cx.updater();
  let Some(update) = updater.read(cx).pending.clone() else {
    return;
  };
  let version: SharedString = update.version.clone().into();
  updater.update(cx, |u, cx| {
    u.set(
      UpdateState::Downloading {
        version: version.clone(),
        percent: Some(0),
      },
      cx,
    )
  });
  progress_toast(0, cx);

  let (tx, rx) = flume::unbounded::<Progress>();
  let job = proton_utils::rt::spawn_blocking(move || download_and_install(&update, tx));
  cx.spawn(async move |cx| {
    while let Ok(p) = rx.recv_async().await {
      cx.update(|cx| {
        let state = match p {
          Progress::Percent(percent) => {
            if let Some(p) = percent {
              progress_toast(p, cx);
            }
            UpdateState::Downloading {
              version: version.clone(),
              percent,
            }
          }
          Progress::Installing => UpdateState::Installing {
            version: version.clone(),
          },
        };
        updater.update(cx, |u, cx| u.set(state, cx));
      });
    }
    let res = job.await.map_err(anyhow::Error::from).and_then(|r| r);
    cx.update(|cx| {
      remove_progress_toast(cx);
      match res {
        Ok(relaunch) => {
          toast(
            Notification::success("Update downloaded successfully. Restarting ..."),
            cx,
          );
          restart(relaunch, cx);
        }
        Err(e) => {
          tracing::error!("Update failed: {e:#}");
          toast(Notification::error("Failed to download update"), cx);
          updater.update(cx, |u, cx| {
            u.set(
              UpdateState::Available {
                version: version.clone(),
                notes: u
                  .pending
                  .as_ref()
                  .and_then(|p| p.body.clone())
                  .map(Into::into),
              },
              cx,
            )
          });
        }
      }
    });
  })
  .detach();
}

enum Progress {
  Percent(Option<u8>),
  Installing,
}

/// Blocking download + signature check + install, returns the binary to relaunch
fn download_and_install(update: &Update, tx: flume::Sender<Progress>) -> anyhow::Result<PathBuf> {
  let kind = install_kind();
  let relaunch = match kind {
    Install::Deb => PathBuf::from(DEB_BINARY),
    Install::Bundle => std::env::var_os("APPIMAGE")
      .map(PathBuf::from)
      .map_or_else(std::env::current_exe, Ok)?,
    Install::Unmanaged => {
      anyhow::bail!("This install is not self updating, use your package manager")
    }
  };

  let done = std::cell::Cell::new(0usize);
  let last = std::cell::Cell::new(None::<u8>);
  let bytes = update.download_extended(
    |n, total| {
      done.set(done.get() + n);
      let percent = total.map(|t| (done.get() as u64 * 100 / t.max(1)).min(100) as u8);
      if percent != last.get() {
        last.set(percent);
        let _ = tx.send(Progress::Percent(percent));
      }
    },
    || {
      let _ = tx.send(Progress::Installing);
    },
  )?;
  if kind == Install::Deb {
    install_deb(&bytes)?;
  } else {
    // windows: runs the nsis installer and exits the process
    update.install(bytes)?;
  }
  Ok(relaunch)
}

/// `bytes` are signature checked by `download_extended`
// ponytail: untested, it needs root (pkexec dpkg -i); test with a fake `pkexec` early on PATH
fn install_deb(bytes: &[u8]) -> anyhow::Result<()> {
  let path = std::env::temp_dir().join(format!("proton-update-{}.deb", std::process::id()));
  std::fs::write(&path, bytes)?;
  let status = std::process::Command::new("pkexec")
    .args(["dpkg", "-i"])
    .arg(&path)
    .status();
  let _ = std::fs::remove_file(&path);
  let status =
    status.map_err(|e| anyhow::anyhow!("pkexec is needed to install the update: {e}"))?;
  anyhow::ensure!(status.success(), "dpkg failed: {status}");
  Ok(())
}

struct UpdateToast;

fn progress_toast(percent: u8, cx: &mut App) {
  let note = Notification::info(format!("Downloading update: {percent}%"))
    .id::<UpdateToast>()
    .autohide(false);
  toast(note, cx);
}

fn remove_progress_toast(cx: &mut App) {
  if let Some(window) = cx.windows().into_iter().next() {
    let _ = window.update(cx, |_, window, cx| {
      window.remove_notification::<UpdateToast>(cx)
    });
  }
}

/// Starts the updated binary and quits, the new process waits for our socket to go away
// ponytail: only the failed relaunch is tested, success quits the test process; a fake binary
// plus a quit hook would cover it
fn restart(binary: PathBuf, cx: &mut App) {
  match std::process::Command::new(&binary)
    .env(RESTART_ENV, "1")
    .spawn()
  {
    Ok(_) => cx.quit(),
    Err(e) => {
      tracing::error!("Failed to relaunch {}: {e}", binary.display());
      toast(
        Notification::warning("Update installed, please restart Proton"),
        cx,
      );
    }
  }
}

/// Startup check: installs right away with auto update on, otherwise offers it in a toast
pub fn check_on_start(cx: &mut App) {
  let task = cx.updater().update(cx, |u, cx| u.check(cx));
  cx.spawn(async move |cx| match task.await {
    Ok(Some(version)) => cx.update(|cx| {
      if cx.settings().get(cx).auto_update {
        install(cx);
      } else {
        let note = Notification::info(format!("Update available: {version}")).action(|_, _, cx| {
          Button::new("update")
            .cursor_pointer()
            .primary()
            .label("Update")
            .on_click(cx.listener(|note, _, window, cx| {
              note.dismiss(window, cx);
              cx.navigate(Route::Settings(SettingsTab::About));
            }))
        });
        toast(note, cx);
      }
    }),
    Ok(None) => {}
    Err(e) => tracing::warn!("Update check failed: {e:#}"),
  })
  .detach();
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use gpui_kit::{
    self as gpui, AnyWindowHandle, IntoElement, TestAppContext, div, test::TestWindowExt,
  };
  use proton_components::test_view;
  use proton_utils::test_http::FakeServer;

  use super::*;
  use crate::pages::profiles::tests::poll;

  /// The shape `app-release.yml` writes, every platform downloading `bundle`
  fn latest_json(version: &str, notes: &str, bundle: &str) -> String {
    let platform = |format: &str| {
      serde_json::json!({
        "url": bundle,
        "signature": "c2ln",
        "format": format,
      })
    };
    serde_json::json!({
      "version": version,
      "notes": notes,
      "pub_date": "2026-10-16T12:00:00Z",
      "platforms": {
        "linux-x86_64": platform("appimage"),
        "linux-aarch64": platform("appimage"),
        // for old Tauri deb installs, tagged appimage (see app-release.yml)
        "linux-x86_64-deb": platform("appimage"),
        "darwin-x86_64": platform("app"),
        "darwin-aarch64": platform("app"),
        "windows-x86_64": platform("nsis"),
        "macos-x86_64": platform("app"),
        "macos-aarch64": platform("app"),
      }
    })
    .to_string()
  }

  /// A server answering `/latest.json` with `version`, the bundle at `/Proton.AppImage`
  fn release(version: &str, notes: &str) -> (FakeServer, String) {
    let server = FakeServer::start();
    let bundle = format!("{}/Proton.AppImage", server.origin());
    server.route(
      "GET",
      "/latest.json",
      200,
      latest_json(version, notes, &bundle),
    );
    let endpoint = format!("{}/latest.json", server.origin());
    (server, endpoint)
  }

  fn paths(server: &FakeServer) -> Vec<String> {
    server.calls().into_iter().map(|c| c.path).collect()
  }

  #[test]
  fn newer_release_is_an_update() {
    let (_server, endpoint) = release("v99.0.0", "Fixes");
    let update = fetch(&endpoint).unwrap().unwrap();
    assert_eq!(update.version, "99.0.0");
    assert_eq!(update.body.as_deref(), Some("Fixes"));
    assert_eq!(update.signature, "c2ln");
  }

  #[test]
  fn same_or_older_release_is_none() {
    for version in [env!("CARGO_PKG_VERSION"), "0.0.1"] {
      let (_server, endpoint) = release(version, "");
      assert!(fetch(&endpoint).unwrap().is_none(), "{version}");
    }
  }

  #[test]
  fn garbage_missing_or_failing_endpoints_are_errors() {
    let server = FakeServer::start();
    server.route("GET", "/bad.json", 200, "{\"version\":\"nope\"}");
    server.route("GET", "/html.json", 200, "<html>");
    server.route("GET", "/down.json", 503, "busy");
    for path in ["/bad.json", "/html.json", "/down.json", "/missing.json"] {
      assert!(
        fetch(&format!("{}{path}", server.origin())).is_err(),
        "{path}"
      );
    }
    assert!(fetch("not a url").is_err());
  }

  #[test]
  fn pubkey_is_a_minisign_key() {
    use base64::Engine;
    let key = base64::engine::general_purpose::STANDARD
      .decode(PUBKEY)
      .unwrap();
    let key = String::from_utf8(key).unwrap();
    assert!(key.starts_with("untrusted comment: minisign public key"));
  }

  #[test]
  fn deb_detection_needs_the_binary_and_its_dpkg_entry() {
    let list = "/.\n/usr\n/usr/bin\n/usr/bin/proton\n/usr/share/applications/proton.desktop\n";
    assert!(deb_owns(Path::new("/usr/bin/proton"), list));
    assert!(!deb_owns(Path::new("/usr/bin/proton"), ""));
    assert!(!deb_owns(Path::new("/nix/store/x/bin/proton"), list));
    assert!(!deb_owns(
      Path::new("/usr/bin/proton"),
      "/usr/bin/proton-other\n"
    ));
  }

  #[test]
  fn tests_run_unmanaged_and_appimage_is_a_bundle() {
    // the test binary is neither the deb binary nor an AppImage
    unsafe { std::env::remove_var("APPIMAGE") };
    assert_eq!(install_kind(), Install::Unmanaged);
    unsafe { std::env::set_var("APPIMAGE", "/tmp/Proton.AppImage") };
    assert_eq!(install_kind(), Install::Bundle);
  }

  /// A window for the toasts, the updater pointed at `endpoint`
  fn open(cx: &mut TestAppContext, endpoint: &str) -> AnyWindowHandle {
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap().keep();
    proton_store::init_at(dir.join("store.json"));
    let (handle, _) = test_view::open(cx, |_, _| div().into_any_element());
    let endpoint = endpoint.to_string();
    cx.update(|cx| {
      proton_config::init(cx);
      crate::init(cx);
      cx.updater().update(cx, |u, _| u.endpoint = endpoint);
    });
    handle
  }

  /// Every state the updater notified with
  fn record(cx: &mut TestAppContext) -> Rc<RefCell<Vec<UpdateState>>> {
    let states = Rc::new(RefCell::new(vec![]));
    let seen = states.clone();
    cx.update(|cx| {
      cx.observe(&cx.updater(), move |u, cx| {
        seen.borrow_mut().push(u.read(cx).state.clone())
      })
      .detach()
    });
    states
  }

  fn state(cx: &mut TestAppContext) -> UpdateState {
    cx.update(|cx| cx.updater().read(cx).state.clone())
  }

  /// Toasts shown, after the closing ones finished their exit
  fn toasts(cx: &mut TestAppContext, handle: AnyWindowHandle) -> usize {
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.notifications(cx).len())
      .unwrap()
  }

  async fn check(cx: &mut TestAppContext) -> anyhow::Result<Option<SharedString>> {
    cx.update(|cx| cx.updater().update(cx, |u, cx| u.check(cx)))
      .await
  }

  fn available(version: &str, notes: Option<&str>) -> UpdateState {
    UpdateState::Available {
      version: version.to_string().into(),
      notes: notes.map(|n| n.to_string().into()),
    }
  }

  #[gpui::test]
  async fn check_moves_through_checking_and_keeps_the_update(cx: &mut TestAppContext) {
    let (_server, endpoint) = release("v99.0.0", "Fixes");
    open(cx, &endpoint);
    let states = record(cx);
    assert_eq!(check(cx).await.unwrap().as_deref(), Some("99.0.0"));
    assert_eq!(
      *states.borrow(),
      [UpdateState::Checking, available("99.0.0", Some("Fixes"))]
    );
    assert!(cx.update(|cx| cx.updater().read(cx).pending.is_some()));
  }

  #[gpui::test]
  async fn empty_notes_are_none_and_failures_keep_their_message(cx: &mut TestAppContext) {
    let (server, endpoint) = release("v99.0.0", "");
    open(cx, &endpoint);
    check(cx).await.unwrap();
    assert_eq!(state(cx), available("99.0.0", None));

    server.route("GET", "/latest.json", 500, "down");
    assert!(check(cx).await.is_err());
    let UpdateState::Error(message) = state(cx) else {
      panic!("{:?}", state(cx))
    };
    assert!(!message.is_empty());
  }

  #[gpui::test]
  fn same_state_does_not_notify(cx: &mut TestAppContext) {
    open(cx, "http://127.0.0.1:9/latest.json");
    let states = record(cx);
    cx.update(|cx| {
      cx.updater()
        .update(cx, |u, cx| u.set(UpdateState::Idle, cx))
    });
    cx.run_until_parked();
    assert!(states.borrow().is_empty());
  }

  #[gpui::test]
  fn install_without_a_pending_update_does_nothing(cx: &mut TestAppContext) {
    let handle = open(cx, "http://127.0.0.1:9/latest.json");
    cx.update(install);
    cx.run_until_parked();
    assert_eq!(state(cx), UpdateState::Idle);
    assert_eq!(toasts(cx, handle), 0);
  }

  /// Installs the pending update and waits until it is offered again
  fn install_and_fail(cx: &mut TestAppContext) {
    cx.update(install);
    poll(cx, |cx| {
      matches!(cx.updater().read(cx).state, UpdateState::Available { .. })
    });
    cx.run_until_parked();
  }

  #[gpui::test]
  async fn unmanaged_installs_refuse_without_downloading(cx: &mut TestAppContext) {
    unsafe { std::env::remove_var("APPIMAGE") };
    let (server, endpoint) = release("v99.0.0", "Fixes");
    let handle = open(cx, &endpoint);
    check(cx).await.unwrap();
    let states = record(cx);
    install_and_fail(cx);
    assert_eq!(
      *states.borrow(),
      [
        UpdateState::Downloading {
          version: "99.0.0".into(),
          percent: Some(0)
        },
        available("99.0.0", Some("Fixes")),
      ]
    );
    assert_eq!(paths(&server), ["/latest.json"]);
    // the progress toast is gone, "Failed to download update" stays
    assert_eq!(toasts(cx, handle), 1);
  }

  #[gpui::test]
  async fn bad_signature_is_rejected_after_the_download(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let appimage = dir.path().join("Proton.AppImage");
    std::fs::write(&appimage, "old").unwrap();
    unsafe { std::env::set_var("APPIMAGE", &appimage) };
    let (server, endpoint) = release("v99.0.0", "Fixes");
    server.route("GET", "/Proton.AppImage", 200, vec![7u8; 64 * 1024]);
    let handle = open(cx, &endpoint);
    check(cx).await.unwrap();
    let states = record(cx);
    install_and_fail(cx);

    let states = states.borrow();
    let downloading = |p| UpdateState::Downloading {
      version: "99.0.0".into(),
      percent: Some(p),
    };
    assert_eq!(states.first(), Some(&downloading(0)));
    assert!(states.contains(&downloading(100)), "{states:?}");
    // progress only goes up, the signature check runs after "installing"
    let percents: Vec<u8> = states
      .iter()
      .filter_map(|s| match s {
        UpdateState::Downloading {
          percent: Some(p), ..
        } => Some(*p),
        _ => None,
      })
      .collect();
    assert!(percents.is_sorted(), "{percents:?}");
    assert_eq!(
      &states[states.len() - 2..],
      [
        UpdateState::Installing {
          version: "99.0.0".into()
        },
        available("99.0.0", Some("Fixes")),
      ]
    );
    assert_eq!(paths(&server), ["/latest.json", "/Proton.AppImage"]);
    // the bundle was not touched
    assert_eq!(std::fs::read_to_string(&appimage).unwrap(), "old");
    assert_eq!(toasts(cx, handle), 1);
  }

  #[gpui::test]
  async fn failed_download_never_installs(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("APPIMAGE", dir.path().join("Proton.AppImage")) };
    // no bundle route: 404
    let (server, endpoint) = release("v99.0.0", "");
    let handle = open(cx, &endpoint);
    check(cx).await.unwrap();
    let states = record(cx);
    install_and_fail(cx);
    assert!(
      !states
        .borrow()
        .iter()
        .any(|s| matches!(s, UpdateState::Installing { .. })),
      "{states:?}"
    );
    assert_eq!(paths(&server), ["/latest.json", "/Proton.AppImage"]);
    assert_eq!(toasts(cx, handle), 1);
  }

  #[gpui::test]
  fn failed_relaunch_asks_for_a_manual_restart(cx: &mut TestAppContext) {
    let handle = open(cx, "http://127.0.0.1:9/latest.json");
    cx.update(|cx| restart(PathBuf::from("/nonexistent/proton"), cx));
    cx.run_until_parked();
    assert_eq!(toasts(cx, handle), 1);
  }

  #[gpui::test]
  fn startup_offers_the_update_and_opens_about(cx: &mut TestAppContext) {
    let (_server, endpoint) = release("v99.0.0", "Fixes");
    let handle = open(cx, &endpoint);
    cx.update(check_on_start);
    poll(cx, |cx| {
      matches!(cx.updater().read(cx).state, UpdateState::Available { .. })
    });
    cx.run_until_parked();
    assert_eq!(toasts(cx, handle), 1);
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.click("update", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
      cx.update(|cx| cx.route()),
      Route::Settings(SettingsTab::About)
    );
    // not installed without asking
    assert_eq!(state(cx), available("99.0.0", Some("Fixes")));
  }

  #[gpui::test]
  fn startup_with_auto_update_installs_right_away(cx: &mut TestAppContext) {
    unsafe { std::env::remove_var("APPIMAGE") };
    let (_server, endpoint) = release("v99.0.0", "Fixes");
    let handle = open(cx, &endpoint);
    cx.update(|cx| proton_config::update(cx, |s| s.auto_update = true));
    let states = record(cx);
    cx.update(check_on_start);
    poll(cx, |_| {
      states
        .borrow()
        .iter()
        .filter(|s| matches!(s, UpdateState::Available { .. }))
        .count()
        == 2
    });
    cx.run_until_parked();
    assert!(
      states
        .borrow()
        .iter()
        .any(|s| matches!(s, UpdateState::Downloading { .. }))
    );
    // only the failure toast (unmanaged test binary), no offer
    assert_eq!(toasts(cx, handle), 1);
  }

  #[gpui::test]
  fn startup_stays_quiet_when_up_to_date_or_failing(cx: &mut TestAppContext) {
    let (server, endpoint) = release(env!("CARGO_PKG_VERSION"), "");
    let handle = open(cx, &endpoint);
    cx.update(check_on_start);
    poll(cx, |cx| {
      cx.updater().read(cx).state == UpdateState::UpToDate
    });
    cx.run_until_parked();
    assert_eq!(toasts(cx, handle), 0);

    server.route("GET", "/latest.json", 500, "down");
    cx.update(check_on_start);
    poll(cx, |cx| {
      matches!(cx.updater().read(cx).state, UpdateState::Error(_))
    });
    cx.run_until_parked();
    assert_eq!(toasts(cx, handle), 0);
  }
}
