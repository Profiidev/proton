//! gpui-free half of the downloads: the status enum and a cheap handle tokio code reports to.
//! The gpui half (`Downloads` entity + listener) lives in lib.rs/listener.rs.

use std::{
  collections::HashMap,
  sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use serde::Serialize;

use proton_utils::future::UpdateLimiter;

/// Progress ticks of one download are delivered at most this often
const DEBOUNCE: Duration = Duration::from_millis(50);

/// format always (done, total)
#[derive(Serialize, Clone, Debug, PartialEq)]
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
  ContentResolve,
  /// bytes
  ContentDownload(usize, usize),
  /// file counts
  ContentFiles(usize, usize),
  ModpackOverrides(usize, usize),
  Done,
}

/// What the handle reports to the gpui side, drained by `listener::listen`
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadMsg {
  Start {
    id: usize,
    title: String,
  },
  Status {
    id: usize,
    status: DownloadCheckStatus,
  },
  Finish {
    id: usize,
    ok: bool,
  },
}

/// Clone it into tokio tasks. Status ticks are debounced per download id.
#[derive(Clone)]
pub struct DownloadsHandle {
  tx: flume::Sender<DownloadMsg>,
  next_id: Arc<AtomicUsize>,
  limiters: Arc<Mutex<HashMap<usize, UpdateLimiter<DownloadCheckStatus>>>>,
}

impl DownloadsHandle {
  /// The receiver goes to the listener. Tests may drop it, sends are then ignored.
  pub fn new() -> (Self, flume::Receiver<DownloadMsg>) {
    let (tx, rx) = flume::unbounded();
    let handle = Self {
      tx,
      next_id: Default::default(),
      limiters: Default::default(),
    };
    (handle, rx)
  }

  /// Adds a running row to the downloads panel and returns its id (also the cancel id)
  pub fn start(&self, title: impl Into<String>) -> usize {
    let id = self.next_id.fetch_add(1, Ordering::SeqCst);
    self.send(DownloadMsg::Start {
      id,
      title: title.into(),
    });
    id
  }

  /// Marks the row done or failed, canceled wins when a cancel was requested
  pub fn finish(&self, id: usize, ok: bool) {
    self.send(DownloadMsg::Finish { id, ok });
  }

  /// Reports a progress tick, debounced. Needs the tokio runtime (`proton_utils::rt`).
  pub fn status(&self, id: usize, status: DownloadCheckStatus) {
    let mut limiters = self.limiters.lock().unwrap();
    let done = status == DownloadCheckStatus::Done;
    let limiter = limiters.entry(id).or_insert_with(|| {
      let tx = self.tx.clone();
      UpdateLimiter::new(DEBOUNCE, move |status| {
        let _ = tx.send(DownloadMsg::Status { id, status });
      })
    });
    limiter.call(status);
    if done {
      // dropping the limiter still delivers the last value
      limiters.remove(&id);
    }
  }

  pub(crate) fn send(&self, msg: DownloadMsg) {
    // the listener is gone only when the app quits
    let _ = self.tx.send(msg);
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use super::*;

  #[test]
  fn status_ticks_are_debounced_and_done_is_delivered() {
    let (handle, rx) = DownloadsHandle::new();
    let id = handle.start("Test");
    assert_eq!(handle.start("Other"), id + 1);
    proton_utils::rt::block_on(async {
      for i in 0..100 {
        handle.status(id, DownloadCheckStatus::AssetsCheck(i, 100));
      }
      handle.status(id, DownloadCheckStatus::Done);
      tokio::time::sleep(Duration::from_millis(200)).await;
    });
    let msgs: Vec<_> = rx.try_iter().collect();
    let statuses = msgs
      .iter()
      .filter(|m| matches!(m, DownloadMsg::Status { .. }))
      .count();
    assert!(statuses < 10, "{statuses} ticks were not debounced");
    assert_eq!(
      msgs.last(),
      Some(&DownloadMsg::Status {
        id,
        status: DownloadCheckStatus::Done
      })
    );
    assert!(handle.limiters.lock().unwrap().is_empty());
  }
}
