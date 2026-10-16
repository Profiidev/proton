use std::{
  fmt::{Debug, Display},
  sync::atomic::{AtomicBool, Ordering},
};

use log::debug;
use tauri::{AppHandle, Manager};
use tokio::net::TcpStream;

use proton_utils::{
  log::ResultLogExt,
  updater::{UpdateType, update_data},
};

pub const MANIFEST_REFRESH_ERROR: &str = "manifest-refresh-error";

/// This uses AtomicBools so the online checks do not block each other
pub struct OfflineState {
  app: AppHandle,
  /// Runs once when the first reconnect happens before the startup refresh succeeded
  on_reconnect: fn(AppHandle),
  offline: AtomicBool,
  state_init: AtomicBool,
}

impl OfflineState {
  pub fn new(app: AppHandle, on_reconnect: fn(AppHandle)) -> Self {
    Self {
      app,
      on_reconnect,
      offline: AtomicBool::new(false),
      state_init: AtomicBool::new(false),
    }
  }

  pub async fn check_online_state(&self) -> bool {
    if TcpStream::connect("detectportal.firefox.com:80")
      .await
      .is_err()
    {
      if !self.offline.load(Ordering::SeqCst) {
        log::info!("Offline state detected");
      }
      self.offline.store(true, Ordering::SeqCst);
      update_data(&self.app, UpdateType::Offline);
      false
    } else {
      if self.offline.load(Ordering::SeqCst) {
        log::info!("Reconnected to the internet");
        if !self.state_init.load(Ordering::SeqCst) {
          (self.on_reconnect)(self.app.clone());

          self.state_init();
        }
      }
      self.offline.store(false, Ordering::SeqCst);
      update_data(&self.app, UpdateType::Offline);
      true
    }
  }

  pub fn is_offline(&self) -> bool {
    self.offline.load(Ordering::SeqCst)
  }

  pub fn state_init(&self) {
    self.state_init.store(true, Ordering::SeqCst);
  }
}

#[allow(async_fn_in_trait)]
pub trait OfflineResultExt {
  async fn check_online_state(self, handle: &AppHandle) -> Self;
}

impl<T, E: Debug + Display> OfflineResultExt for std::result::Result<T, E> {
  async fn check_online_state(self, handle: &AppHandle) -> Self {
    let state = handle.state::<OfflineState>();
    if self.is_err() && !state.offline.load(Ordering::SeqCst) {
      debug!("Checking online state due to error");
      state.check_online_state().await;
    } else if self.is_ok() && state.offline.load(Ordering::SeqCst) {
      debug!("Checking online state due to success");
      state.check_online_state().await;
    }
    self.log()
  }
}
