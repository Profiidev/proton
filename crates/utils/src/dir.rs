use std::{io, path::PathBuf};

use tokio::fs;

pub async fn list_dirs_in_dir_path(path: PathBuf) -> io::Result<Vec<PathBuf>> {
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
