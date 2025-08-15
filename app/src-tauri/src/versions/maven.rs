use std::path::{Path, PathBuf};

use anyhow::Result;
use tauri::Url;

use crate::{
  path,
  versions::{LIBRARY_DIR, MC_DIR},
};

pub fn parse_maven_name(name: &str) -> Result<MavenName> {
  let parts: Vec<&str> = name.split(':').collect();
  if parts.len() < 3 {
    return Err(anyhow::anyhow!("Invalid Maven name format"));
  }

  let (version, version_ext, ext) = if parts.len() == 3 {
    let version_parts = parts[2].split('@').collect::<Vec<&str>>();

    let version = version_parts[0].to_string();
    let ext = if version_parts.len() > 1 {
      version_parts[1].to_string()
    } else {
      "jar".to_string()
    };

    (version, None, ext)
  } else {
    let version = parts[2].to_string();
    let version_ext_parts = parts[3].split('@').collect::<Vec<&str>>();

    let version_ext = version_ext_parts[0].to_string();
    let ext = if version_ext_parts.len() > 1 {
      version_ext_parts[1].to_string()
    } else {
      "jar".to_string()
    };

    (version, Some(version_ext), ext)
  };

  Ok(MavenName {
    group: parts[0].to_string(),
    artifact: parts[1].to_string(),
    version,
    version_ext,
    ext,
  })
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

  let version = if let Some(parts) = &maven.version_ext {
    format!("{}-{}", maven.version, parts)
  } else {
    maven.version.clone()
  };

  path!(
    path,
    group,
    &maven.artifact,
    &maven.version,
    format!("{}-{}.{}", maven.artifact, version, maven.ext)
  )
}

pub fn url_from_maven(base_url: &str, maven: &MavenName) -> Result<Url> {
  Ok(Url::parse(&format!(
    "{}/{}/{}/{}/{}-{}.{}",
    base_url,
    maven.group.replace('.', "/"),
    maven.artifact,
    maven.version,
    maven.artifact,
    maven.version,
    maven.ext
  ))?)
}

pub struct MavenName {
  pub group: String,
  pub artifact: String,
  pub version: String,
  pub version_ext: Option<String>,
  pub ext: String,
}
