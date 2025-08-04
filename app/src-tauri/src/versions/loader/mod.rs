use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use anyhow::Result;
use reqwest::Client;
use tauri::Url;

use crate::path;

pub mod fabric;

#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
  async fn download(&self, client: &Client, data_dir: &Path) -> Result<()>;
  async fn classpath(&self, data_dir: &Path) -> Result<OsString>;
  async fn main_class(&self, data_dir: &Path) -> Result<String>;
}

fn parse_maven_name(name: &str) -> Result<MavenName> {
  let parts: Vec<&str> = name.split(':').collect();
  let version = parts[3..].join(":");

  Ok(MavenName {
    group: parts[1].to_string(),
    artifact: parts[2].to_string(),
    version,
  })
}

fn path_from_maven(maven: &MavenName) -> PathBuf {
  let mut path = path!();
  let mut group: &str = &maven.group;
  while let Some(segment) = group.find('.') {
    path = path!(path, &group[..segment]);
    group = &group[(segment + 1)..];
  }
  path!(path, format!("{}-{}.jar", maven.artifact, maven.version))
}

fn url_from_maven(base_url: &str, maven: &MavenName) -> Result<Url> {
  Ok(Url::parse(&format!(
    "{}/{}/{}/{}/{}-{}.jar",
    base_url,
    maven.group.replace('.', "/"),
    maven.artifact,
    maven.version,
    maven.artifact,
    maven.version
  ))?)
}

struct MavenName {
  group: String,
  artifact: String,
  version: String,
}
