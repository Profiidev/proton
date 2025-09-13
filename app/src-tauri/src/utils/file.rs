use std::{
  io::Write,
  path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Serialize, de::DeserializeOwned};
use sha1::{Digest, Sha1};
use tauri::async_runtime::spawn_blocking;
use thiserror::Error;
use tokio::fs::{self, File};

#[derive(Error, Debug)]
pub enum FileError {
  #[error("HashMismatch")]
  HashMismatch,
}

pub async fn file_hash(hash: &str, path: &PathBuf) -> Result<bool> {
  let Ok(file) = File::open(path).await else {
    return Ok(false);
  };
  let mut file = file.into_std().await;
  let found_hash = spawn_blocking(move || {
    let mut hasher = Sha1::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok::<_, std::io::Error>(hex::encode(hasher.finalize()))
  })
  .await??;
  Ok(hash == found_hash)
}

pub fn hash_bytes(hash: &str, bytes: &[u8]) -> Result<bool> {
  let mut hasher = Sha1::new();
  hasher.write_all(bytes)?;
  let found_hash = hex::encode(hasher.finalize());
  Ok(hash == found_hash)
}

pub fn bytes_hash(bytes: &[u8]) -> Result<String> {
  let mut hasher = Sha1::new();
  hasher.write_all(bytes)?;
  let found_hash = hex::encode(hasher.finalize());
  Ok(found_hash)
}

pub async fn read_parse_file<R: DeserializeOwned>(path: &PathBuf) -> Result<R> {
  let data = fs::read_to_string(path).await?;
  Ok(serde_json::from_str(&data)?)
}

pub async fn read_parse_xml_file<R: DeserializeOwned>(path: &PathBuf) -> Result<R> {
  let data = fs::read_to_string(path).await?;
  Ok(serde_xml_rs::from_str(&data)?)
}

pub async fn write_file<T: Serialize>(path: &PathBuf, data: &T) -> Result<()> {
  let data = serde_json::to_string(data)?;
  fs::write(path, data).await?;
  Ok(())
}

pub async fn create_or_open_file(path: &PathBuf) -> Result<File> {
  let path = Path::new(path);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;
  }
  Ok(File::create(path).await?)
}

pub async fn last_modified_ago(path: &PathBuf) -> Result<Option<Duration>> {
  if !path.exists() {
    return Ok(None);
  }
  let metadata = fs::metadata(path).await?;
  let sys_time = metadata.modified()?;
  let time: DateTime<Utc> = sys_time.into();

  Ok(Some(Utc::now() - time))
}
