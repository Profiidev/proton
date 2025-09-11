use std::{
  fmt::{Debug, Display},
  sync::atomic::{AtomicBool, Ordering},
};

use log::debug;
use tauri::{AppHandle, Emitter, Manager, Result, State};
use tokio::{net::TcpStream, spawn};

use crate::{
  async_setup_refresh,
  utils::{
    log::ResultLogExt,
    updater::{UpdateType, update_data},
  },
};

pub const MANIFEST_REFRESH_ERROR: &str = "manifest-refresh-error";

/// This uses AtomicBools so the online checks do not block each other
pub struct OfflineState {
  app: AppHandle,
  offline: AtomicBool,
  state_init: AtomicBool,
}

impl OfflineState {
  pub fn new(app: AppHandle) -> Self {
    Self {
      app,
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
          let handle = self.app.clone();
          // we need to move it into a different task to avoid blocking the freeing of the lock
          spawn(async move {
            if let Err(e) = async_setup_refresh(&handle).await.log() {
              log::error!("Failed to refresh manifests: {e}");
              let _ = handle.emit(MANIFEST_REFRESH_ERROR, ()).log();
            }
          });

          self.state_init();
        }
      }
      self.offline.store(false, Ordering::SeqCst);
      update_data(&self.app, UpdateType::Offline);
      true
    }
  }

  pub fn state_init(&self) {
    self.state_init.store(true, Ordering::SeqCst);
  }
}

#[tauri::command]
pub async fn is_offline(state: State<'_, OfflineState>) -> Result<bool> {
  Ok(state.offline.load(Ordering::SeqCst))
}

#[tauri::command]
pub async fn try_reconnect(state: State<'_, OfflineState>) -> Result<bool> {
  Ok(state.check_online_state().await)
}

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
