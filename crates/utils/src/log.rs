use std::{fs::File, sync::Mutex};

use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::dir;

/// Same rule as the old tauri-plugin-log setup: everything of proton, but not the noisy crates
const DEFAULT_FILTER: &str = "info,proton=trace,notify=off,serde_xml_rs=off";

/// Logs to stdout and to a new file per run in [`dir::log_dir`], named like the tauri app did
/// (`2026-01-31T12-00-00.log`, local time). `RUST_LOG` overrides the filter.
pub fn init() {
  let filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| DEFAULT_FILTER.into());
  let file = dir::log_dir().ok().and_then(|dir| {
    std::fs::create_dir_all(&dir).ok()?;
    let name = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S.log");
    File::create(dir.join(name.to_string())).ok()
  });

  let registry = tracing_subscriber::registry().with(fmt::layer().with_filter(filter()));
  let file_layer = file.map(|file| {
    fmt::layer()
      .with_ansi(false)
      .with_writer(Mutex::new(file))
      .with_filter(filter())
  });
  // ignore a second init (tests)
  let _ = registry.with(file_layer).try_init();
}

/// Logs the error of a result and passes it on
pub trait ErrorLogExt {
  fn log_err(self) -> Self;
}

impl<T, E: std::fmt::Debug> ErrorLogExt for Result<T, E> {
  fn log_err(self) -> Self {
    if let Err(e) = &self {
      tracing::error!("{e:?}");
    }
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn log_err_returns_its_input() {
    assert_eq!(Ok::<_, &str>(1).log_err(), Ok(1));
    assert_eq!(Err::<i32, _>("boom").log_err(), Err("boom"));
  }
}
