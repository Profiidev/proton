use std::{path::PathBuf, pin::Pin};

use anyhow::Result;
use futures_util::StreamExt;
use log::debug;
use reqwest::Client;
use serde::de::DeserializeOwned;
use tauri::Url;
use tokio::fs::{self, File};

use crate::utils::file::{FileError, hash_bytes};

pub async fn download_file_no_hash(client: &Client, path: &PathBuf, url: Url) -> Result<Vec<u8>> {
  if File::open(path).await.is_ok() {
    return Ok(fs::read(path).await?);
  }

  debug!("Downloading file: {}", url.as_str());
  let bytes = client
    .get(url)
    .send()
    .await?
    .error_for_status()?
    .bytes()
    .await?;

  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;
  }
  fs::write(path, &bytes).await?;

  Ok(bytes.to_vec())
}

pub async fn download_file_no_hash_force(
  client: &Client,
  path: &PathBuf,
  url: Url,
) -> Result<Vec<u8>> {
  debug!("Downloading file: {}", url.as_str());
  let bytes = client
    .get(url)
    .send()
    .await?
    .error_for_status()?
    .bytes()
    .await?;

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
  let bytes = download_file_no_hash_force(client, path, url).await?;
  Ok(serde_json::from_slice(&bytes)?)
}

pub async fn download_and_parse_file_no_hash<R: DeserializeOwned>(
  client: &Client,
  path: &PathBuf,
  url: Url,
) -> Result<R> {
  let data = download_file_no_hash(client, path, url).await?;
  Ok(serde_json::from_slice(&data)?)
}

pub async fn download_file(
  client: &Client,
  path: &PathBuf,
  url: Url,
  hash: &str,
  progress: Box<dyn Fn(usize) + Send + 'static>,
) -> Result<Vec<u8>> {
  let res = client.get(url).send().await?.error_for_status()?;
  let size = res.content_length().unwrap_or_default() as usize;
  let mut stream = res.bytes_stream();

  let mut bytes = Vec::with_capacity(size);
  while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    progress(chunk.len());
    bytes.extend_from_slice(&chunk);
  }

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
  let data = download_file(client, path, url, hash, Box::new(|_| {})).await?;
  Ok(serde_json::from_slice(&data)?)
}

pub type DownloadFileSizeFuture = Box<
  dyn FnOnce(
      Box<dyn Fn(usize) + Send + 'static>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>
    + Send
    + 'static,
>;

pub async fn download_file_size(
  client: &Client,
  path: PathBuf,
  url: Url,
) -> Result<(DownloadFileSizeFuture, usize)> {
  let res = client.get(url).send().await?.error_for_status()?;
  let size = res.content_length().unwrap_or_default() as usize;

  Ok((
    Box::new(move |progress| {
      Box::pin(async move {
        let mut stream = res.bytes_stream();

        let mut bytes = Vec::with_capacity(size);
        while let Some(chunk) = stream.next().await {
          let chunk = chunk?;
          progress(chunk.len());
          bytes.extend_from_slice(&chunk);
        }

        if let Some(parent) = path.parent() {
          fs::create_dir_all(parent).await?;
        }
        fs::write(path, &bytes).await?;

        Ok(())
      })
    }),
    size,
  ))
}
