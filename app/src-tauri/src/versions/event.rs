use std::{collections::HashMap, sync::Mutex, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::utils::{future::UpdateLimiter, log::ResultLogExt};

const VERSION_CHECK_STATUS_EVENT: &str = "version-check-status";

#[derive(Default)]
pub struct UpdateLimiterStore {
  debounce: HashMap<usize, UpdateLimiter<DownloadCheckStatus>>,
}

/// format always (done, total)
#[derive(Serialize, Clone, PartialEq)]
pub enum DownloadCheckStatus {
  VersionManifestCheck,
  VersionManifestDownload,
  AssetsManifestCheck,
  AssetsManifestDownload,
  JavaManifestCheck,
  JavaManifestDownload,
  ClientCheck,
  ClientDownload(usize, usize),
  AssetsCheck(usize, usize),
  AssetsDownload(usize, usize),
  JavaCheck(usize, usize),
  JavaDownload(usize, usize),
  NativeLibraryCheck(usize, usize),
  NativeLibraryDownload(usize, usize),
  LibraryCheck(usize, usize),
  LibraryDownload(usize, usize),
  ModLoaderMeta,
  ModLoaderFilesCheck(usize, usize),
  ModLoaderFilesDownloadInfo,
  ModLoaderFilesDownload(usize, usize),
  ModLoaderPreprocess,
  ModLoaderPreprocessDone,
  Done,
}

#[derive(Serialize, Clone)]
struct InternalStatus {
  id: usize,
  data: DownloadCheckStatus,
}

pub fn emit_download_check_status(handle: &AppHandle, data: DownloadCheckStatus, id: usize) {
  let debounce_state = handle.state::<Mutex<UpdateLimiterStore>>();
  let mut debounce_state = debounce_state.lock().unwrap();
  let func = debounce_state.debounce.entry(id).or_insert_with(|| {
    let handle = handle.clone();
    UpdateLimiter::new(Duration::from_millis(50), move |data| {
      let _ = handle
        .emit(VERSION_CHECK_STATUS_EVENT, InternalStatus { id, data })
        .log();
    })
  });

  let done = data == DownloadCheckStatus::Done;
  let _ = func.call(data);

  if done {
    debounce_state.debounce.remove(&id);
  }
}
