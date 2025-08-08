use std::path::{Path, PathBuf};

use anyhow::Result;
use tauri::Url;

use crate::{
  path,
  versions::{LIBRARY_DIR, MC_DIR},
};

pub fn parse_maven_name(name: &str) -> MavenName {
  let parts: Vec<&str> = name.split(':').collect();
  let version = parts[2..].join(":");

  MavenName {
    group: parts[0].to_string(),
    artifact: parts[1].to_string(),
    version,
  }
}

pub fn full_path_from_maven(data_dir: &Path, maven: &MavenName) -> PathBuf {
  path!(data_dir, MC_DIR, LIBRARY_DIR, path_from_maven(maven))
}

fn path_from_maven(maven: &MavenName) -> PathBuf {
  let mut path = path!();
  let mut group: &str = &maven.group;
  while let Some(segment) = group.find('.') {
    path = path!(path, &group[..segment]);
    group = &group[(segment + 1)..];
  }
  path!(
    path,
    group,
    &maven.artifact,
    &maven.version,
    format!("{}-{}.jar", maven.artifact, maven.version)
  )
}

pub fn url_from_maven(base_url: &str, maven: &MavenName) -> Result<Url> {
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

pub struct MavenName {
  pub group: String,
  pub artifact: String,
  pub version: String,
}
