use std::{io, path::PathBuf};

use anyhow::{Context, Result};
use tokio::fs;

/// The tauri bundle identifier, every app dir is named after it
pub const IDENTIFIER: &str = "io.profidev.proton";

// Same resolution as tauri 2.12 `PathResolver::app_*_dir` (src/path/desktop.rs), so the data of
// the tauri app is found as it is.

fn app_dir(base: Option<PathBuf>) -> Result<PathBuf> {
  Ok(base.context("Unknown path")?.join(IDENTIFIER))
}

/// `$XDG_DATA_HOME/io.profidev.proton`, `~/Library/Application Support/..`, `%APPDATA%\..`
pub fn data_dir() -> Result<PathBuf> {
  app_dir(dirs::data_dir())
}

pub fn cache_dir() -> Result<PathBuf> {
  app_dir(dirs::cache_dir())
}

/// `<local data>/io.profidev.proton/logs`, on macOS `~/Library/Logs/io.profidev.proton`
pub fn log_dir() -> Result<PathBuf> {
  #[cfg(target_os = "macos")]
  return app_dir(dirs::home_dir().map(|home| home.join("Library/Logs")));
  #[cfg(not(target_os = "macos"))]
  Ok(app_dir(dirs::data_local_dir())?.join("logs"))
}

/// The system Downloads folder, not app specific
pub fn download_dir() -> Result<PathBuf> {
  dirs::download_dir().context("Unknown path")
}

/// Moves app data from the old `de.proton.mc` identifier folder, call before anything touches
/// [`data_dir`]
pub fn migrate() {
  let Some(base) = dirs::data_dir() else {
    return;
  };
  let old = base.join("de.proton.mc");
  let new = base.join(IDENTIFIER);
  if old.exists()
    && !new.exists()
    && let Err(e) = std::fs::rename(&old, &new)
  {
    eprintln!("Failed to migrate data dir: {e}");
  }
}

async fn list_dirs_in_dir_path(path: PathBuf) -> io::Result<Vec<PathBuf>> {
  let mut dirs = Vec::new();
  let mut stream = fs::read_dir(path).await?;
  while let Some(entry) = stream.next_entry().await? {
    if entry.file_type().await?.is_dir() {
      dirs.push(entry.path());
    }
  }
  Ok(dirs)
}

pub async fn list_dirs_in_dir(path: PathBuf) -> io::Result<Vec<String>> {
  let paths = list_dirs_in_dir_path(path).await?;
  let mut dirs = Vec::new();
  for path in paths {
    if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
      dirs.push(dir_name.to_string());
    }
  }
  Ok(dirs)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(target_os = "linux")]
  #[test]
  fn dirs_match_tauri() {
    let local = dirs::data_local_dir().unwrap();
    assert_eq!(
      data_dir().unwrap(),
      dirs::data_dir().unwrap().join(IDENTIFIER)
    );
    assert_eq!(log_dir().unwrap(), local.join(IDENTIFIER).join("logs"));
    assert_eq!(
      cache_dir().unwrap(),
      dirs::cache_dir().unwrap().join(IDENTIFIER)
    );
  }
}
