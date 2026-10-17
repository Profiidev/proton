//! The HTTP client gpui loads images and other URLs with: reqwest plus an on-disk cache in
//! `<cache>/io.profidev.proton/http` that follows `Cache-Control`.

use std::io;

use reqwest_client::ReqwestClient;

pub use crate::client::CachedHttpClient;

mod cleanup;
mod client;
mod policy;
mod store;

pub fn client() -> io::Result<CachedHttpClient<ReqwestClient>> {
  let client = ReqwestClient::user_agent(concat!("proton/", env!("CARGO_PKG_VERSION")))
    .map_err(io::Error::other)?;
  let dir = dirs::cache_dir()
    .unwrap_or_else(std::env::temp_dir)
    .join("io.profidev.proton")
    .join("http");
  CachedHttpClient::new(client, dir)
}

#[cfg(test)]
fn temp_dir(name: &str) -> std::path::PathBuf {
  let dir = std::env::temp_dir().join(format!("proton-http-cache-{name}-{}", std::process::id()));
  std::fs::remove_dir_all(&dir).ok();
  std::fs::create_dir_all(&dir).unwrap();
  dir
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_client_constructs() {
    let client = client();
    assert!(client.is_ok());
  }
}
