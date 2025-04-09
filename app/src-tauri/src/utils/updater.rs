use serde::Serialize;
use tauri::{AppHandle, Emitter};

const UPDATE_EVENT: &str = "data-update";

#[derive(Serialize, Clone)]
pub enum UpdateType {
  //accounts
  Accounts,
  AccountActive,
  AccountSkins,
  //versions
  Versions,
  //profiles
  Profiles,
  //instances
  Instances
}

pub fn update_data(handle: &AppHandle, r#type: UpdateType) {
  let _ = handle.emit(UPDATE_EVENT, r#type);
}
