use std::{fs, io, path::PathBuf};

pub fn list_dirs_in_dir_path(path: PathBuf) -> io::Result<Vec<PathBuf>> {
  let mut dirs = Vec::new();
  for entry in fs::read_dir(path)? {
    let entry = entry?;
    if entry.file_type()?.is_dir() {
      dirs.push(entry.path());
    }
  }
  Ok(dirs)
}

pub fn list_dirs_in_dir(path: PathBuf) -> io::Result<Vec<String>> {
  let paths = list_dirs_in_dir_path(path)?;
  let mut dirs = Vec::new();
  for path in paths {
    if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
      dirs.push(dir_name.to_string());
    }
  }
  Ok(dirs)
}
