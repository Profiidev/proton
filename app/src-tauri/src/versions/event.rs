use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::utils::log::ResultLogExt;

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

#[derive(Serialize, Clone)]
struct InternalStatus {
  id: usize,
  data: CheckStatus,
}

pub fn emit_check_status(handle: &AppHandle, data: CheckStatus, id: usize) {
  let _ = handle
    .emit(VERSION_CHECK_STATUS_EVENT, InternalStatus { id, data })
    .log();
}
