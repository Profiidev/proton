use std::{
  io::Write,
  path::{Path, PathBuf},
};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use sha1::{Digest, Sha1};
use tauri::{async_runtime::spawn_blocking, Url};
use thiserror::Error;
use tokio::fs::{self, File};

#[derive(Error, Debug)]
pub enum FileError {
  #[error("HashMismatch")]
  HashMismatch,
}

pub async fn download_file_no_hash_force(
  client: &Client,
  path: &PathBuf,
  url: Url,
) -> Result<Vec<u8>> {
  debug!("Downloading file: {}", url.as_str());
  let bytes = client.get(url).send().await?.bytes().await?;

  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;
  }
  fs::write(path, &bytes).await?;

  Ok(bytes.to_vec())
}

pub async fn download_file_no_hash(client: &Client, path: &PathBuf, url: Url) -> Result<Vec<u8>> {
  if File::open(path).await.is_ok() {
    return Ok(fs::read(path).await?);
  }

  debug!("Downloading file: {}", url.as_str());
  let bytes = client.get(url).send().await?.bytes().await?;

  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;
  }
  fs::write(path, &bytes).await?;

  Ok(bytes.to_vec())
}

pub async fn download_and_parse_file_no_hash_force<R: DeserializeOwned>(
  client: &Client,
  path: &PathBuf,
  url: Url,
) -> Result<R> {
  let data = download_file_no_hash_force(client, path, url).await?;
  Ok(serde_json::from_slice(&data)?)
}

pub async fn download_and_parse_file_no_hash<R: DeserializeOwned>(
  client: &Client,
  path: &PathBuf,
  url: Url,
) -> Result<R> {
  let data = download_file_no_hash(client, path, url).await?;
  Ok(serde_json::from_slice(&data)?)
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

pub async fn write_file<T: Serialize>(path: &PathBuf, data: &T) -> Result<()> {
  let data = serde_json::to_string(data)?;
  fs::write(path, data).await?;
  Ok(())
}

pub fn create_or_open_file_std(path: &PathBuf) -> Result<std::fs::File> {
  let path = Path::new(path);
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  Ok(std::fs::File::create(path)?)
}

pub async fn download_file(
  client: &Client,
  path: &PathBuf,
  url: Url,
  hash: &str,
) -> Result<Vec<u8>> {
  let bytes = client.get(url).send().await?.bytes().await?;
  if !hash_bytes(hash, &bytes)? {
    return Err(FileError::HashMismatch.into());
  }

  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;
  }
  fs::write(path, &bytes).await?;

  Ok(bytes.to_vec())
}

pub async fn download_and_parse_file<R: DeserializeOwned>(
  client: &Client,
  path: &PathBuf,
  url: Url,
  hash: &str,
) -> Result<R> {
  let data = download_file(client, path, url, hash).await?;
  Ok(serde_json::from_slice(&data)?)
}
