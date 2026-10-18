//! One running app per user: a local socket (unix socket in the runtime dir, a named pipe on
//! Windows). A second launch connects, sends its request (empty: focus, else a cli
//! [`LaunchRequest`] as JSON) and exits.

use std::{
  io::{self, Read, Write},
  path::PathBuf,
};

use gpui_kit::{AnyWindowHandle, App};
use interprocess::local_socket::{ListenerOptions, Name, Stream, prelude::*};

use crate::actions::{self, LaunchRequest};

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

/// The listener when we are the first instance. `None`: another one runs and was just sent
/// `request`.
// ponytail: two launches in the same instant can both win, the later bind replaces the socket
pub fn acquire(path: &std::path::Path, request: &[u8]) -> io::Result<Option<Listener>> {
  if let Ok(mut stream) = Stream::connect(name(path)?) {
    stream.write_all(request)?;
    return Ok(None);
  }
  // a stale socket file from a crashed run is replaced
  let listener = ListenerOptions::new()
    .name(name(path)?)
    .try_overwrite(true)
    .create_sync()?;
  Ok(Some(listener))
}

/// What each connection sent, read until it closes (older versions send nothing)
// ponytail: one reader thread, a client that never closes blocks later ones; fine for a local cli
pub fn requests(listener: Listener) -> flume::Receiver<String> {
  let (tx, rx) = flume::unbounded();
  std::thread::spawn(move || {
    for mut conn in listener.incoming().flatten() {
      let mut request = String::new();
      let _ = conn.read_to_string(&mut request);
      if tx.send(request).is_err() {
        break;
      }
    }
  });
  rx
}

/// Focuses `window` whenever a second launch connects, and runs its launch request
pub fn focus_on_request(listener: Listener, window: AnyWindowHandle, cx: &mut App) {
  let rx = requests(listener);
  cx.spawn(async move |cx| {
    while let Ok(request) = rx.recv_async().await {
      tracing::info!("Second launch, focusing the window");
      let _ = window.update(cx, |_, window, _| window.activate_window());
      if request.is_empty() {
        continue;
      }
      match serde_json::from_str::<LaunchRequest>(&request) {
        Ok(request) => cx.update(|cx| actions::launch_request(request, cx)),
        Err(e) => tracing::warn!("Invalid launch request: {e}"),
      }
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
    let first = acquire(&path, b"").unwrap().expect("first instance");
    let pings = requests(first);
    assert!(acquire(&path, b"").unwrap().is_none());
    assert_eq!(pings.recv_timeout(Duration::from_secs(5)).unwrap(), "");
    assert!(acquire(&path, b"{}").unwrap().is_none());
    assert_eq!(pings.recv_timeout(Duration::from_secs(5)).unwrap(), "{}");
  }

  #[cfg(unix)]
  #[test]
  fn stale_socket_file_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("proton.sock");
    drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
    assert!(path.exists());
    assert!(acquire(&path, b"").unwrap().is_some());
  }
}
