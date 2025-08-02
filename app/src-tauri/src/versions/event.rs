use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::utils::log::ResultLogExt;

const VERSION_CHECK_STATUS_EVENT: &str = "version-check-status";

/// format always (done, total)
#[derive(Serialize, Clone)]
pub enum DownloadCheckStatus {
  VersionManifestCheck,
  VersionManifestDownload,
  AssetsManifestCheck,
  AssetsManifestDownload,
  JavaManifestCheck,
  JavaManifestDownload,
  ClientCheck,
  ClientDownload,
  AssetsCheck(usize, usize),
  AssetsDownload(usize, usize),
  JavaCheck(usize, usize),
  JavaDownload(usize, usize),
  NativeLibraryCheck(usize, usize),
  NativeLibraryDownload(usize, usize),
  LibraryCheck(usize, usize),
  LibraryDownload(usize, usize),
  Done,
}

#[derive(Serialize, Clone)]
struct InternalStatus {
  id: usize,
  data: DownloadCheckStatus,
}

pub fn emit_download_check_status(handle: &AppHandle, data: DownloadCheckStatus, id: usize) {
  let _ = handle
    .emit(VERSION_CHECK_STATUS_EVENT, InternalStatus { id, data })
    .log();
}
