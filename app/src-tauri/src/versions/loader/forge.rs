use std::{
  collections::HashMap,
  path::{Path, PathBuf},
};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::Url;

use crate::{
  path,
  utils::file::{download_file_no_hash_force, read_parse_file, read_parse_xml_file},
  versions::{
    MC_DIR, VERSION_DIR,
    loader::{Loader, util::compare_mc_versions},
  },
};

const INDEX_BASE_URL_FORGE: &str =
  "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const INDEX_BASE_URL_NEOFORGE_FORGE: &str =
  "https://maven.neoforged.net/net/neoforged/forge/maven-metadata.xml";
const INDEX_BASE_URL_NEOFORGE_NEOFORGE: &str =
  "https://maven.neoforged.net/net/neoforged/neoforge/maven-metadata.xml";
const INDEX_FILE_NAME_FORGE: &str = "forge";
const INDEX_FILE_NAME_NEOFORGE: &str = "neoforge";

pub struct ForgeLikeLoader {
  index_url: String,
  index_url_2: Option<String>,
  index_file_name: String,
}

impl ForgeLikeLoader {
  pub fn forge() -> Self {
    Self {
      index_url: INDEX_BASE_URL_FORGE.to_string(),
      index_url_2: None,
      index_file_name: INDEX_FILE_NAME_FORGE.to_string(),
    }
  }

  pub fn neoforge() -> Self {
    Self {
      index_url: INDEX_BASE_URL_NEOFORGE_FORGE.to_string(),
      index_url_2: Some(INDEX_BASE_URL_NEOFORGE_NEOFORGE.to_string()),
      index_file_name: INDEX_FILE_NAME_NEOFORGE.to_string(),
    }
  }

  fn index(&self, data_dir: &Path) -> PathBuf {
    let filename = format!("{}-index.json", self.index_file_name);
    path!(data_dir, MC_DIR, VERSION_DIR, filename)
  }

  fn index_2(&self, data_dir: &Path) -> PathBuf {
    let filename = format!("{}-index-2.json", self.index_file_name);
    path!(data_dir, MC_DIR, VERSION_DIR, filename)
  }

  async fn neoforge_version_pairs(&self, data_dir: &Path) -> Result<Vec<(String, String)>> {
    let path = self.index(data_dir);
    let forge_versions = read_parse_xml_file::<NeoForgeIndex>(&path).await?;
    let path = self.index_2(data_dir);
    let neoforge_versions = read_parse_xml_file::<NeoForgeIndex>(&path).await?;

    let forge_versions_parsed = forge_versions
      .versioning
      .versions
      .version
      .into_iter()
      .flat_map(|v| forge_version_pair(&v))
      .collect::<Vec<_>>();

    let neoforge_versions_parsed = neoforge_versions
      .versioning
      .versions
      .version
      .into_iter()
      .flat_map(|v| neoforge_version_pair(&v))
      .collect::<Vec<_>>();

    let mut versions = forge_versions_parsed;
    versions.extend(neoforge_versions_parsed);
    Ok(versions)
  }
}

#[async_trait::async_trait]
impl Loader for ForgeLikeLoader {
  async fn download_metadata(&self, client: &Client, data_dir: &Path) -> Result<()> {
    let url = Url::parse(&self.index_url)?;
    let path = self.index(data_dir);
    download_file_no_hash_force(client, &path, url).await?;

    if let Some(url_2) = &self.index_url_2 {
      let url = Url::parse(url_2)?;
      let path = self.index_2(data_dir);
      download_file_no_hash_force(client, &path, url).await?;
    }

    Ok(())
  }

  async fn supported_versions(&self, data_dir: &Path, _: bool) -> Result<Vec<String>> {
    let path = self.index(data_dir);
    if self.index_file_name == INDEX_FILE_NAME_FORGE {
      let mut versions = read_parse_file::<VersionIndex>(&path)
        .await?
        .keys()
        .filter(|v| !v.contains("pre"))
        .cloned()
        .collect::<Vec<_>>();

      versions.sort_by(compare_mc_versions);
      versions.reverse();

      Ok(versions)
    } else {
      let versions = self.neoforge_version_pairs(data_dir).await?;
      let mut versions: Vec<String> = versions.into_iter().map(|(mc, _)| mc).collect();
      versions.sort_by(compare_mc_versions);
      versions.dedup();
      versions.reverse();

      Ok(versions)
    }
  }

  async fn loader_versions_for_mc_version(
    &self,
    mc_version: &str,
    data_dir: &Path,
    _: bool,
  ) -> Result<Vec<String>> {
    let path = self.index(data_dir);
    if self.index_file_name == INDEX_FILE_NAME_FORGE {
      let versions = read_parse_file::<VersionIndex>(&path)
        .await?
        .get(mc_version)
        .cloned()
        .unwrap_or_default();

      let mut versions: Vec<String> = versions
        .into_iter()
        .filter(|v| !v.contains("pre"))
        .flat_map(|v| anyhow::Ok(forge_version_pair(&v)?.1))
        .collect();
      versions.reverse();

      Ok(versions)
    } else {
      let versions = self.neoforge_version_pairs(data_dir).await?;
      let mut versions: Vec<String> = versions
        .into_iter()
        .filter(|(mc, _)| mc == mc_version)
        .map(|(_, neoforge)| neoforge)
        .collect();

      versions.reverse();

      Ok(versions)
    }
  }
}

fn forge_version_pair(version_string: &str) -> Result<(String, String)> {
  // Forge version format: x-y where x is the Minecraft version and y is the Forge version
  // e.g., "1.16.5-36.2.39" => ("1.16.5", "36.2.39")
  // or "1.16.5-36.2.39-1.16.5" => ("1.16.5", "36.2.39")
  let parts: Vec<&str> = version_string.split('-').collect();
  if parts.len() < 2 {
    return Err(anyhow::anyhow!(
      "Invalid forge version string: {}",
      version_string
    ));
  }
  let mc_version = parts[0].to_string();
  let forge_version = parts[1..].join("-");
  Ok((mc_version, forge_version))
}

fn neoforge_version_pair(version_string: &str) -> Result<(String, String)> {
  // format: x.y.z where x is x.y is the Minecraft version and z is the NeoForge version
  // the mc version is presented as "1.x.y" and the NeoForge version is presented as "z"
  // e.g., "16.5-36.2.39" => ("1.16.5", "36.2.39")
  let parts: Vec<&str> = version_string.split('.').collect();
  if parts.len() < 3 || parts[0].parse::<u32>().is_err() || parts[1].parse::<u32>().is_err() {
    return Err(anyhow::anyhow!(
      "Invalid NeoForge version string: {}",
      version_string
    ));
  }
  let mc_version = format!("1.{}.{}", parts[0], parts[1]);
  let neoforge_version = parts[2..].join(".");
  Ok((mc_version, neoforge_version))
}

type VersionIndex = HashMap<String, Vec<String>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NeoForgeIndex {
  versioning: NeoForgeVersioning,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NeoForgeVersioning {
  versions: NeoForgeVersions,
  latest: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NeoForgeVersions {
  version: Vec<String>,
}
