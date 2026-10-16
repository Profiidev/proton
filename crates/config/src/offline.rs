//! gpui-free half of the offline service: a cheap handle for tokio code. The gpui half
//! (`Offline` global + listener) lives in lib.rs/listener.rs.

use std::{
  fmt::Debug,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use proton_utils::log::ErrorLogExt;
use tokio::net::TcpStream;
use tracing::{debug, info};

/// What the handle reports to the gpui side, drained by `listener::listen`
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OfflineMsg {
  Offline(bool),
  /// First reconnect before the startup refresh succeeded, the app refreshes the manifests
  Reconnected,
  ManifestRefreshError,
}

/// Clone it into tokio tasks. The atomics make reads instant and checks not block each other.
#[derive(Clone)]
pub struct OfflineHandle {
  offline: Arc<AtomicBool>,
  init: Arc<AtomicBool>,
  tx: flume::Sender<OfflineMsg>,
}

impl OfflineHandle {
  /// The receiver goes to the listener. Tests may drop it, sends are then ignored.
  pub fn new() -> (Self, flume::Receiver<OfflineMsg>) {
    let (tx, rx) = flume::unbounded();
    let handle = Self {
      offline: Default::default(),
      init: Default::default(),
      tx,
    };
    (handle, rx)
  }

  pub fn is_offline(&self) -> bool {
    self.offline.load(Ordering::SeqCst)
  }

  /// Checks the connection and returns whether we are online. Needs the tokio runtime.
  pub async fn check(&self) -> bool {
    let online = TcpStream::connect("detectportal.firefox.com:80")
      .await
      .is_ok();
    self.report(online);
    online
  }

  pub(crate) fn report(&self, online: bool) {
    let was_offline = self.offline.swap(!online, Ordering::SeqCst);
    if was_offline == !online {
      return;
    }
    if online {
      info!("Reconnected to the internet");
      if !self.init.swap(true, Ordering::SeqCst) {
        self.send(OfflineMsg::Reconnected);
      }
    } else {
      info!("Offline state detected");
    }
    self.send(OfflineMsg::Offline(!online));
  }

  /// The startup refresh succeeded, later reconnects do not need to refresh again
  pub fn set_init(&self) {
    self.init.store(true, Ordering::SeqCst);
  }

  pub fn manifest_refresh_failed(&self) {
    self.send(OfflineMsg::ManifestRefreshError);
  }

  fn send(&self, msg: OfflineMsg) {
    // the listener is gone only when the app quits
    let _ = self.tx.send(msg);
  }
}

#[allow(async_fn_in_trait)]
pub trait OfflineResultExt {
  /// Rechecks the connection when a request failed while online or worked while offline, logs
  /// the error and passes the result on
  async fn check_online_state(self, handle: &OfflineHandle) -> Self;
}

impl<T, E: Debug> OfflineResultExt for Result<T, E> {
  async fn check_online_state(self, handle: &OfflineHandle) -> Self {
    if self.is_err() != handle.is_offline() {
      debug!("Checking online state, request ok: {}", self.is_ok());
      handle.check().await;
    }
    self.log_err()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reports_only_changes_and_reconnects_once() {
    let (handle, rx) = OfflineHandle::new();
    handle.report(true);
    assert!(rx.try_recv().is_err());

    handle.report(false);
    handle.report(false);
    handle.report(true);
    handle.report(false);
    handle.report(true);
    let msgs: Vec<_> = rx.try_iter().collect();
    use OfflineMsg::*;
    assert_eq!(
      msgs,
      [
        Offline(true),
        Reconnected,
        Offline(false),
        Offline(true),
        Offline(false)
      ]
    );
  }

  #[test]
  fn no_reconnect_after_init() {
    let (handle, rx) = OfflineHandle::new();
    handle.set_init();
    handle.report(false);
    handle.report(true);
    assert!(!rx.try_iter().any(|m| m == OfflineMsg::Reconnected));
  }
}
