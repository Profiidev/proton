use std::{path::PathBuf, pin::Pin, time::Duration};

use anyhow::Result;
use futures_util::StreamExt;
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use tokio::fs;
use tracing::debug;
use url::Url;

use crate::file::{FileError, bytes_hash};

pub fn default_client() -> Client {
  Client::builder()
    .connect_timeout(Duration::from_secs(10))
    .build()
    .unwrap()
}

/// Downloads `url` to `path` and returns the bytes. Without `force` an existing file is read
/// instead.
pub async fn download_file_no_hash(
  client: &Client,
  path: &PathBuf,
  url: Url,
  force: bool,
) -> Result<Vec<u8>> {
  if !force && let Ok(bytes) = fs::read(path).await {
    return Ok(bytes);
  }

  debug!("Downloading file: {}", url.as_str());
  let res = client.get(url).send().await?.error_for_status()?;
  let bytes = res.bytes().await?.to_vec();
  write(path, &bytes).await?;
  Ok(bytes)
}

pub async fn download_and_parse_file_no_hash<R: DeserializeOwned>(
  client: &Client,
  path: &PathBuf,
  url: Url,
  force: bool,
) -> Result<R> {
  let bytes = download_file_no_hash(client, path, url, force).await?;
  Ok(serde_json::from_slice(&bytes)?)
}

pub async fn download_file(
  client: &Client,
  path: &PathBuf,
  url: Url,
  hash: &str,
  progress: Box<dyn Fn(usize) + Send + 'static>,
) -> Result<Vec<u8>> {
  let res = client.get(url).send().await?.error_for_status()?;
  let bytes = read_body(res, progress).await?;
  if bytes_hash(&bytes) != hash {
    return Err(FileError::HashMismatch.into());
  }
  write(path, &bytes).await?;
  Ok(bytes)
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

/// Streams the body, reporting each chunk's size
async fn read_body(res: Response, progress: impl Fn(usize)) -> Result<Vec<u8>> {
  let mut bytes = Vec::with_capacity(res.content_length().unwrap_or_default() as usize);
  let mut stream = res.bytes_stream();
  while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    progress(chunk.len());
    bytes.extend_from_slice(&chunk);
  }
  Ok(bytes)
}

/// Writes `bytes`, creating the parent dirs
async fn write(path: &PathBuf, bytes: &[u8]) -> Result<()> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;
  }
  fs::write(path, bytes).await?;
  Ok(())
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
        let bytes = read_body(res, progress).await?;
        write(&path, &bytes).await
      })
    }),
    size,
  ))
}
