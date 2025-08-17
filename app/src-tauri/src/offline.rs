use std::fmt::{Debug, Display};

use log::debug;
use tauri::{AppHandle, Emitter, Manager, Result, State};
use tokio::{net::TcpStream, spawn, sync::Mutex};

use crate::{
  async_setup_refresh,
  utils::{
    log::ResultLogExt,
    updater::{UpdateType, update_data},
  },
};

pub const MANIFEST_REFRESH_ERROR: &str = "manifest-refresh-error";

pub struct OfflineState {
  app: AppHandle,
  offline: bool,
  state_init: bool,
}

impl OfflineState {
  pub fn new(app: AppHandle) -> Self {
    Self {
      app,
      offline: false,
      state_init: false,
    }
  }

  pub async fn check_online_state(&mut self) -> bool {
    if TcpStream::connect("detectportal.firefox.com:80")
      .await
      .is_err()
    {
      if !self.offline {
        log::info!("Offline state detected");
      }
      self.offline = true;
      update_data(&self.app, UpdateType::Offline);
      false
    } else {
      if self.offline {
        log::info!("Reconnected to the internet");
        if !self.state_init {
          let handle = self.app.clone();
          // we need to move it into a different task to avoid blocking the freeing of the lock
          spawn(async move {
            if let Err(e) = async_setup_refresh(&handle).await.log() {
              log::error!("Failed to refresh manifests: {e}");
              let _ = handle.emit(MANIFEST_REFRESH_ERROR, ()).log();
            }
          });

          self.state_init = true;
        }
      }
      self.offline = false;
      update_data(&self.app, UpdateType::Offline);
      true
    }
  }

  pub fn state_init(&mut self) {
    self.state_init = true;
  }
}

#[tauri::command]
pub async fn is_offline(state: State<'_, Mutex<OfflineState>>) -> Result<bool> {
  let state = state.lock().await;
  Ok(state.offline)
}

#[tauri::command]
pub async fn try_reconnect(state: State<'_, Mutex<OfflineState>>) -> Result<bool> {
  let mut state = state.lock().await;
  Ok(state.check_online_state().await)
}

pub trait OfflineResultExt {
  /// ### Do NOT call this method if you hold a lock on the `OfflineState` state.
  async fn check_online_state(self, handle: &AppHandle) -> Self;
}

impl<T, E: Debug + Display> OfflineResultExt for std::result::Result<T, E> {
  async fn check_online_state(self, handle: &AppHandle) -> Self {
    let state = handle.state::<Mutex<OfflineState>>();
    let mut state = state.lock().await;
    if self.is_err() && !state.offline {
      debug!("Checking online state due to error");
      state.check_online_state().await;
    } else if self.is_ok() && state.offline {
      debug!("Checking online state due to success");
      state.check_online_state().await;
    }
    drop(state);
    self.log()
  }
}
