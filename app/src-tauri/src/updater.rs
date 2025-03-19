use serde::Serialize;
use tauri::{AppHandle, Emitter};

const UPDATE_EVENT: &str = "data-update";

#[derive(Serialize, Clone)]
pub enum UpdateType {
  Accounts,
  AccountActive,
  AccountSkins,
}

pub fn update_data(handle: &AppHandle, r#type: UpdateType) {
  let _ = handle.emit(UPDATE_EVENT, r#type);
}
