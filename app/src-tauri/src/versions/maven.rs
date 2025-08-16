use std::{path::PathBuf, str::FromStr};

use anyhow::Result;
use tauri::Url;

use crate::{path, versions::paths::MCPath};

#[derive(Debug)]
pub struct MavenArtifact {
  pub group: String,
  pub artifact: String,
  pub version: String,
  pub version_ext: Option<String>,
  pub ext: String,
}

impl MavenArtifact {
  pub fn new(name: &str) -> Result<Self> {
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

    Ok(MavenArtifact {
      group: parts[0].to_string(),
      artifact: parts[1].to_string(),
      version,
      version_ext,
      ext,
    })
  }

  pub fn path(&self) -> PathBuf {
    let mut path = path!();
    let mut group: &str = &self.group;
    while let Some(segment) = group.find('.') {
      path = path!(path, &group[..segment]);
      group = &group[(segment + 1)..];
    }

    let version = if let Some(parts) = &self.version_ext {
      format!("{}-{}", self.version, parts)
    } else {
      self.version.clone()
    };

    path!(
      path,
      group,
      &self.artifact,
      &self.version,
      format!("{}-{}.{}", self.artifact, version, self.ext)
    )
  }

  pub fn full_path(&self, mc_path: &MCPath) -> PathBuf {
    path!(mc_path.library_path(), self.path())
  }

  pub fn url(&self, base_url: &str) -> Result<Url> {
    Ok(Url::parse(&format!(
      "{}/{}/{}/{}/{}-{}{}.{}",
      base_url,
      self.group.replace('.', "/"),
      self.artifact,
      self.version,
      self.artifact,
      self.version,
      self
        .version_ext
        .as_deref()
        .map(|ext| format!("-{}", ext))
        .unwrap_or_default(),
      self.ext
    ))?)
  }
}

impl FromStr for MavenArtifact {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> Result<Self> {
    MavenArtifact::new(s)
  }
}
