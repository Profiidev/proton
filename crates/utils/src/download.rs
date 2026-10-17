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
  if !force && let Ok(bytes) = fs::read(path).await {
    return Ok(serde_json::from_slice(&bytes)?);
  }

  debug!("Downloading file: {}", url.as_str());
  let res = client.get(url).send().await?.error_for_status()?;
  let bytes = res.bytes().await?;
  // parse before writing, junk (e.g. a captive portal page) must not replace a good cache
  let parsed = serde_json::from_slice(&bytes)?;
  write(path, &bytes).await?;
  Ok(parsed)
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

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  };

  use serde::Deserialize;

  use super::*;
  use crate::{rt, test_http::FakeServer};

  const JSON: &str = r#"{"name":"ünïcode","count":3}"#;

  #[derive(Deserialize, Debug, PartialEq)]
  struct Item {
    name: String,
    count: u32,
  }

  fn url(server: &FakeServer, path: &str) -> Url {
    format!("{}{path}", server.origin()).parse().unwrap()
  }

  fn status(err: &anyhow::Error) -> Option<u16> {
    let err = err.downcast_ref::<reqwest::Error>()?;
    Some(err.status()?.as_u16())
  }

  #[test]
  fn no_hash_downloads_into_new_parent_dirs() {
    let server = FakeServer::start();
    server.route("GET", "/f", 200, "data");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a/b/f.txt");
    let bytes = rt::block_on(download_file_no_hash(
      &default_client(),
      &path,
      url(&server, "/f"),
      false,
    ))
    .unwrap();
    assert_eq!(bytes, b"data");
    assert_eq!(std::fs::read(&path).unwrap(), b"data");
  }

  #[test]
  fn no_hash_reads_an_existing_file_unless_forced() {
    let server = FakeServer::start();
    server.route("GET", "/f", 200, "fresh");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    std::fs::write(&path, "cached").unwrap();
    let client = default_client();

    let bytes = rt::block_on(download_file_no_hash(
      &client,
      &path,
      url(&server, "/f"),
      false,
    ))
    .unwrap();
    assert_eq!(bytes, b"cached");
    // the cached file short-circuits the request
    assert!(server.calls().is_empty());

    let bytes = rt::block_on(download_file_no_hash(
      &client,
      &path,
      url(&server, "/f"),
      true,
    ))
    .unwrap();
    assert_eq!(bytes, b"fresh");
    assert_eq!(std::fs::read(&path).unwrap(), b"fresh");
    assert_eq!(server.calls().len(), 1);
  }

  #[test]
  fn error_statuses_fail_and_write_nothing() {
    let server = FakeServer::start();
    let dir = tempfile::tempdir().unwrap();
    let client = default_client();
    for code in [400, 404, 500, 503] {
      let path_name = format!("/s{code}");
      server.route("GET", &path_name, code, "oops");
      let path = dir.path().join(code.to_string());

      let err = rt::block_on(download_file_no_hash(
        &client,
        &path,
        url(&server, &path_name),
        true,
      ))
      .unwrap_err();
      assert_eq!(status(&err), Some(code));

      let err = rt::block_on(download_file(
        &client,
        &path,
        url(&server, &path_name),
        &bytes_hash(b"oops"),
        Box::new(|_| {}),
      ))
      .unwrap_err();
      assert_eq!(status(&err), Some(code));

      let err = match rt::block_on(download_file_size(
        &client,
        path.clone(),
        url(&server, &path_name),
      )) {
        Ok(_) => panic!("{code} should fail"),
        Err(e) => e,
      };
      assert_eq!(status(&err), Some(code));
      assert!(!path.exists());
    }
  }

  #[test]
  fn connection_refused_is_an_error() {
    // a port nothing listens on
    let port = std::net::TcpListener::bind("127.0.0.1:0")
      .unwrap()
      .local_addr()
      .unwrap()
      .port();
    let dir = tempfile::tempdir().unwrap();
    let res = rt::block_on(download_file_no_hash(
      &default_client(),
      &dir.path().join("f"),
      format!("http://127.0.0.1:{port}/f").parse().unwrap(),
      true,
    ));
    assert!(res.is_err());
  }

  #[test]
  fn parse_no_hash_reads_json_and_rejects_garbage() {
    let server = FakeServer::start();
    server.route("GET", "/ok", 200, JSON);
    server.route("GET", "/bad", 200, "{not json");
    let dir = tempfile::tempdir().unwrap();
    let client = default_client();

    let item: Item = rt::block_on(download_and_parse_file_no_hash(
      &client,
      &dir.path().join("ok.json"),
      url(&server, "/ok"),
      false,
    ))
    .unwrap();
    assert_eq!(
      item,
      Item {
        name: "ünïcode".into(),
        count: 3
      }
    );

    let res: Result<Item> = rt::block_on(download_and_parse_file_no_hash(
      &client,
      &dir.path().join("bad.json"),
      url(&server, "/bad"),
      false,
    ));
    assert!(res.unwrap_err().is::<serde_json::Error>());
  }

  #[test]
  fn forced_refresh_with_junk_keeps_the_cached_json() {
    let server = FakeServer::start();
    server.route("GET", "/portal", 200, "<html>log in to the wifi</html>");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manifest.json");
    std::fs::write(&path, JSON).unwrap();

    let res: Result<Item> = rt::block_on(download_and_parse_file_no_hash(
      &default_client(),
      &path,
      url(&server, "/portal"),
      true,
    ));
    assert!(res.is_err());
    // the next offline start still reads the good cache
    assert_eq!(std::fs::read_to_string(&path).unwrap(), JSON);
  }

  #[test]
  fn hashed_download_checks_the_hash_and_reports_progress() {
    let server = FakeServer::start();
    let body = vec![7u8; 100_000];
    server.route("GET", "/big", 200, body.clone());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x/big");
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = seen.clone();

    let bytes = rt::block_on(download_file(
      &default_client(),
      &path,
      url(&server, "/big"),
      &bytes_hash(&body),
      Box::new(move |n| {
        counter.fetch_add(n, Ordering::SeqCst);
      }),
    ))
    .unwrap();
    assert_eq!(bytes, body);
    assert_eq!(seen.load(Ordering::SeqCst), body.len());
    assert_eq!(std::fs::read(&path).unwrap(), body);
  }

  #[test]
  fn hash_mismatch_keeps_the_old_file() {
    let server = FakeServer::start();
    server.route("GET", "/f", 200, "tampered");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    std::fs::write(&path, "old").unwrap();

    let err = rt::block_on(download_file(
      &default_client(),
      &path,
      url(&server, "/f"),
      &bytes_hash(b"expected"),
      Box::new(|_| {}),
    ))
    .unwrap_err();
    assert!(matches!(
      err.downcast_ref::<FileError>(),
      Some(FileError::HashMismatch)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
  }

  #[test]
  fn hashed_download_ignores_an_existing_file() {
    let server = FakeServer::start();
    server.route("GET", "/f", 200, "new");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    std::fs::write(&path, "new").unwrap();
    rt::block_on(download_file(
      &default_client(),
      &path,
      url(&server, "/f"),
      &bytes_hash(b"new"),
      Box::new(|_| {}),
    ))
    .unwrap();
    // unlike the no-hash variant, the hashed one always fetches
    assert_eq!(server.calls().len(), 1);
  }

  #[test]
  fn empty_body_matches_the_empty_hash() {
    let server = FakeServer::start();
    server.route("GET", "/empty", 200, "");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty");
    let bytes = rt::block_on(download_file(
      &default_client(),
      &path,
      url(&server, "/empty"),
      "da39a3ee5e6b4b0d3255bfef95601890afd80709",
      Box::new(|n| assert_eq!(n, 0, "no chunks for an empty body")),
    ))
    .unwrap();
    assert!(bytes.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), b"");
  }

  #[test]
  fn hashed_parse_reads_json_and_rejects_garbage() {
    let server = FakeServer::start();
    server.route("GET", "/ok", 200, JSON);
    server.route("GET", "/bad", 200, "[]");
    let dir = tempfile::tempdir().unwrap();
    let client = default_client();

    let item: Item = rt::block_on(download_and_parse_file(
      &client,
      &dir.path().join("ok"),
      url(&server, "/ok"),
      &bytes_hash(JSON.as_bytes()),
    ))
    .unwrap();
    assert_eq!(item.count, 3);

    // valid json, wrong shape
    let res: Result<Item> = rt::block_on(download_and_parse_file(
      &client,
      &dir.path().join("bad"),
      url(&server, "/bad"),
      &bytes_hash(b"[]"),
    ));
    assert!(res.unwrap_err().is::<serde_json::Error>());

    // a mismatch fails before parsing
    let res: Result<Item> = rt::block_on(download_and_parse_file(
      &client,
      &dir.path().join("ok2"),
      url(&server, "/ok"),
      "0000",
    ));
    assert!(res.unwrap_err().is::<FileError>());
  }

  #[test]
  fn sized_download_reports_the_length_then_writes_on_demand() {
    let server = FakeServer::start();
    server.route("GET", "/f", 200, "0123456789");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep/f");
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = seen.clone();

    rt::block_on(async {
      let (write, size) = download_file_size(&default_client(), path.clone(), url(&server, "/f"))
        .await
        .unwrap();
      assert_eq!(size, 10);
      // nothing is written until the returned future runs
      assert!(!path.exists());
      write(Box::new(move |n| {
        counter.fetch_add(n, Ordering::SeqCst);
      }))
      .await
      .unwrap();
    });
    assert_eq!(seen.load(Ordering::SeqCst), 10);
    assert_eq!(std::fs::read(&path).unwrap(), b"0123456789");
  }
}
