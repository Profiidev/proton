//! Self update via cargo-packager-updater: `latest.json` on the GitHub release (written by
//! `.github/workflows/app-release.yml`), bundles signed with the old Tauri updater key.
//! Port of the old `+layout.svelte` updater flow (BEHAVIOR §2 "Updater flow").

use std::{path::PathBuf, time::Duration};

use cargo_packager_updater::{Config, Update, UpdaterBuilder, semver::Version, url::Url};
use gpui_kit::{
  App, AppContext, Context, Entity, Global, SharedString, Task,
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

fn builder(endpoint: &str) -> anyhow::Result<cargo_packager_updater::Updater> {
  let config = Config {
    endpoints: vec![Url::parse(endpoint)?],
    pubkey: PUBKEY.into(),
    ..Default::default()
  };
  Ok(
    UpdaterBuilder::new(Version::parse(env!("CARGO_PKG_VERSION"))?, config)
      .timeout(Duration::from_secs(30))
      .build()?,
  )
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
  // a deb/nix install would get the AppImage written over its binary, only AppImages self update
  #[cfg(target_os = "linux")]
  let relaunch = std::env::var_os("APPIMAGE")
    .map(PathBuf::from)
    .ok_or_else(|| anyhow::anyhow!("Only the AppImage updates itself, use your package manager"))?;
  #[cfg(not(target_os = "linux"))]
  let relaunch = std::env::current_exe()?;

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
  // windows: runs the nsis installer and exits the process
  update.install(bytes)?;
  Ok(relaunch)
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
  use std::{
    io::{Read, Write},
    net::TcpListener,
  };

  use super::*;

  /// Serves `body` as the endpoint once, returns its url
  fn serve(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
      let (mut conn, _) = listener.accept().unwrap();
      let _ = conn.read(&mut [0; 4096]);
      let res = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
      );
      conn.write_all(res.as_bytes()).unwrap();
    });
    format!("http://{addr}/latest.json")
  }

  /// The shape `app-release.yml` writes
  fn latest_json(version: &str) -> String {
    let platform = |format: &str| {
      serde_json::json!({
        "url": "https://github.com/Profiidev/proton/releases/download/v9/Proton.bin",
        "signature": "c2ln",
        "format": format,
      })
    };
    serde_json::json!({
      "version": version,
      "notes": "Fixes",
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

  #[test]
  fn newer_release_is_an_update() {
    let update = fetch(&serve(latest_json("v99.0.0"))).unwrap().unwrap();
    assert_eq!(update.version, "99.0.0");
    assert_eq!(update.body.as_deref(), Some("Fixes"));
    assert_eq!(update.signature, "c2ln");
  }

  #[test]
  fn same_or_older_release_is_none() {
    let current = env!("CARGO_PKG_VERSION");
    assert!(fetch(&serve(latest_json(current))).unwrap().is_none());
    assert!(fetch(&serve(latest_json("0.0.1"))).unwrap().is_none());
  }

  #[test]
  fn garbage_is_an_error() {
    assert!(fetch(&serve("{\"version\":\"nope\"}".into())).is_err());
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
}
