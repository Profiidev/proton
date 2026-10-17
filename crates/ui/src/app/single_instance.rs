//! One running app per user: a local socket (unix socket in the runtime dir, a named pipe on
//! Windows). A second launch connects, which asks the first to focus its window, and exits.

use std::{io, path::PathBuf};

use gpui_kit::{AnyWindowHandle, App};
use interprocess::local_socket::{ListenerOptions, Name, Stream, prelude::*};

pub use interprocess::local_socket::Listener;

/// `$XDG_RUNTIME_DIR/io.profidev.proton.sock` (temp dir without one)
pub fn default_path() -> PathBuf {
  dirs::runtime_dir()
    .unwrap_or_else(std::env::temp_dir)
    .join("io.profidev.proton.sock")
}

fn name(path: &std::path::Path) -> io::Result<Name<'_>> {
  #[cfg(unix)]
  return path.to_fs_name::<interprocess::local_socket::GenericFilePath>();
  #[cfg(windows)]
  return path
    .file_name()
    .unwrap_or_default()
    .to_ns_name::<interprocess::local_socket::GenericNamespaced>();
}

/// The listener when we are the first instance. `None`: another one runs and was just asked
/// to focus.
// ponytail: two launches in the same instant can both win, the later bind replaces the socket
pub fn acquire(path: &std::path::Path) -> io::Result<Option<Listener>> {
  if Stream::connect(name(path)?).is_ok() {
    return Ok(None);
  }
  // a stale socket file from a crashed run is replaced
  let listener = ListenerOptions::new()
    .name(name(path)?)
    .try_overwrite(true)
    .create_sync()?;
  Ok(Some(listener))
}

/// Every connection is a focus request
pub fn requests(listener: Listener) -> flume::Receiver<()> {
  let (tx, rx) = flume::unbounded();
  std::thread::spawn(move || {
    for conn in listener.incoming() {
      if conn.is_ok() && tx.send(()).is_err() {
        break;
      }
    }
  });
  rx
}

/// Focuses `window` whenever a second launch connects
pub fn focus_on_request(listener: Listener, window: AnyWindowHandle, cx: &mut App) {
  let rx = requests(listener);
  cx.spawn(async move |cx| {
    while rx.recv_async().await.is_ok() {
      tracing::info!("Second launch, focusing the window");
      let _ = window.update(cx, |_, window, _| window.activate_window());
    }
  })
  .detach();
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use super::*;

  #[test]
  fn second_acquire_finds_the_first_and_pings_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("proton.sock");
    let first = acquire(&path).unwrap().expect("first instance");
    let pings = requests(first);
    assert!(acquire(&path).unwrap().is_none());
    pings.recv_timeout(Duration::from_secs(5)).unwrap();
  }

  #[cfg(unix)]
  #[test]
  fn stale_socket_file_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("proton.sock");
    drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
    assert!(path.exists());
    assert!(acquire(&path).unwrap().is_some());
  }
}
