use log::trace;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

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
}

pub fn update_data(handle: &AppHandle, r#type: UpdateType) {
  trace!("Send update event for type {type:?}");
  let _ = handle.emit(UPDATE_EVENT, r#type);
}
