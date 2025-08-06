use std::path::{Path, PathBuf};

use anyhow::Result;
use reqwest::Client;
use tauri::Url;

use crate::{
  path,
  utils::file::{download_file_no_hash_force, file_hash},
  versions::{
    LIBRARY_DIR, MC_DIR,
    loader::{CheckFuture, DownloadFuture},
  },
};

pub fn download_maven_future(
  data_dir: PathBuf,
  name: String,
  client: Client,
  base_url: String,
  sha1: Option<String>,
) -> CheckFuture {
  Box::pin(async move {
    let local_name = name.clone();
    let data = data_dir.clone();
    let download = Box::pin(async move {
      download_maven(&base_url, &data_dir, &local_name, &client).await?;
      anyhow::Ok(())
    }) as DownloadFuture;

    let maven = parse_maven_name(&name);
    let path = full_path_from_maven(&data, &maven);
    if let Some(sha1) = sha1 {
      if !file_hash(&sha1, &path).await? {
        Ok(Some(download))
      } else {
        Ok(None)
      }
    } else {
      Ok(Some(download))
    }
  }) as CheckFuture
}

pub async fn download_maven(
  base_url: &str,
  data_dir: &Path,
  name: &str,
  client: &Client,
) -> Result<()> {
  let maven = parse_maven_name(name);
  let loader_path = full_path_from_maven(data_dir, &maven);
  let loader_url = url_from_maven(base_url, &maven)?;
  download_file_no_hash_force(client, &loader_path, loader_url).await?;
  Ok(())
}

pub fn maven_classpath(data_dir: &Path, name: &str) -> PathBuf {
  let maven = parse_maven_name(name);
  path!(data_dir, MC_DIR, LIBRARY_DIR, path_from_maven(&maven))
}

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

pub struct MavenName {
  group: String,
  artifact: String,
  version: String,
}
