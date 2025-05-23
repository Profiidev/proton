use std::{
  fs::{self, File},
  io::{self, Write},
  path::{Path, PathBuf},
};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use sha1::{Digest, Sha1};
use tauri::Url;
use thiserror::Error;

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
    fs::create_dir_all(parent)?;
  }
  fs::write(path, &bytes)?;

  Ok(bytes.to_vec())
}

pub async fn download_file_no_hash(client: &Client, path: &PathBuf, url: Url) -> Result<Vec<u8>> {
  if File::open(path).is_ok() {
    return Ok(fs::read(path)?);
  }

  debug!("Downloading file: {}", url.as_str());
  let bytes = client.get(url).send().await?.bytes().await?;

  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::write(path, &bytes)?;

  Ok(bytes.to_vec())
}

pub async fn download_file(
  client: &Client,
  path: &PathBuf,
  url: Url,
  hash: &str,
) -> Result<Vec<u8>> {
  if File::open(path).is_ok() && file_hash(hash, path)? {
    return Ok(fs::read(path)?);
  }

  debug!("Downloading file: {}", url.as_str());
  let bytes = client.get(url).send().await?.bytes().await?;
  if !hash_bytes(hash, &bytes)? {
    return Err(FileError::HashMismatch.into());
  }

  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::write(path, &bytes)?;

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

pub async fn download_and_parse_file<R: DeserializeOwned>(
  client: &Client,
  path: &PathBuf,
  url: Url,
  hash: &str,
) -> Result<R> {
  let data = download_file(client, path, url, hash).await?;
  Ok(serde_json::from_slice(&data)?)
}

pub fn file_hash(hash: &str, path: &PathBuf) -> Result<bool> {
  let mut file = File::open(path)?;
  let mut hasher = Sha1::new();
  io::copy(&mut file, &mut hasher)?;
  let found_hash = hex::encode(hasher.finalize());
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

pub fn read_parse_file<R: DeserializeOwned>(path: &PathBuf) -> Result<R> {
  let data = std::fs::read_to_string(path)?;
  Ok(serde_json::from_str(&data)?)
}

pub fn write_file<T: Serialize>(path: &PathBuf, data: &T) -> Result<()> {
  let data = serde_json::to_string(data)?;
  fs::write(path, data)?;
  Ok(())
}

pub fn create_or_open_file(path: &PathBuf) -> Result<File> {
  let path = Path::new(path);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  Ok(File::create(path)?)
}
