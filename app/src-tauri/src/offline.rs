use std::fmt::{Debug, Display};

use tauri::{AppHandle, Emitter, Manager, Result, State};
use tokio::{net::TcpStream, sync::Mutex};

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
        if self.state_init
          && let Err(e) = async_setup_refresh(&self.app).await.log()
        {
          log::error!("Failed to refresh manifests: {e}");
          let _ = self.app.emit(MANIFEST_REFRESH_ERROR, ()).log();
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
  async fn check_online_state(self, handle: &AppHandle) -> Self;
}

impl<T, E: Debug + Display> OfflineResultExt for std::result::Result<T, E> {
  async fn check_online_state(self, handle: &AppHandle) -> Self {
    if self.is_err() {
      let state = handle.state::<Mutex<OfflineState>>();
      let mut state = state.lock().await;
      state.check_online_state().await;
    }
    self.log()
  }
}
