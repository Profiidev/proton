use serde::Serialize;
use tauri::{AppHandle, Emitter};

const VERSION_CHECK_STATUS_EVENT: &str = "version-check-status";

#[derive(Serialize, Clone)]
pub enum CheckStatus {
  Manifest(usize),
  Client,
  Assets(usize, usize),
  Java(usize, usize),
  NativeLibrary(usize, usize),
  Library(usize, usize),
  Done,
}

pub fn emit_check_status(handle: &AppHandle, data: CheckStatus) {
  let _ = handle.emit(VERSION_CHECK_STATUS_EVENT, data);
}
