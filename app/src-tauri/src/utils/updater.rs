use std::time::Duration;

use log::trace;
use reqwest::Client;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::utils::log::ResultLogExt;

const UPDATE_EVENT: &str = "data-update";

#[derive(Serialize, Clone, Copy, Debug)]
pub enum UpdateType {
  //accounts
  Accounts,
  AccountActive,
  AccountSkins,
  //versions
  Versions,
  //profiles
  Profiles,
  ProfileLogs,
  ProfileQuickPlay,
  //instances
  Instances,
  InstanceLogs,
  //settings
  Settings,
  //offline
  Offline,
}

pub fn update_data(handle: &AppHandle, r#type: UpdateType) {
  trace!("Send update event for type {type:?}");
  let _ = handle.emit(UPDATE_EVENT, r#type).log();
}

pub fn default_client() -> Client {
  Client::builder()
    .connect_timeout(Duration::from_secs(10))
    .timeout(Duration::from_secs(10))
    .build()
    .unwrap()
}
