use std::path::{Path, PathBuf};

use anyhow::Result;
use reqwest::Client;
use tauri::Url;

use crate::{
  path,
  utils::file::download_file_no_hash_force,
  versions::{LIBRARY_DIR, MC_DIR},
};

pub async fn download_maven(
  base_url: &str,
  data_dir: &Path,
  name: &str,
  client: &Client,
) -> Result<()> {
  let maven = parse_maven_name(name);
  let loader_path = path!(data_dir, MC_DIR, LIBRARY_DIR, path_from_maven(&maven));
  let loader_url = url_from_maven(base_url, &maven)?;
  download_file_no_hash_force(client, &loader_path, loader_url).await?;
  Ok(())
}

pub fn maven_classpath(data_dir: &Path, name: &str) -> PathBuf {
  let maven = parse_maven_name(name);
  path!(data_dir, MC_DIR, LIBRARY_DIR, path_from_maven(&maven))
}

fn parse_maven_name(name: &str) -> MavenName {
  let parts: Vec<&str> = name.split(':').collect();
  let version = parts[2..].join(":");

  MavenName {
    group: parts[0].to_string(),
    artifact: parts[1].to_string(),
    version,
  }
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
    format!("{}-{}.jar", maven.artifact, maven.version)
  )
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
