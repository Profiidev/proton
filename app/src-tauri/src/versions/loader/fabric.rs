use std::path::{Path, PathBuf};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::Url;

use crate::{
  path,
  utils::file::{
    download_and_parse_file_no_hash_force, download_file_no_hash_force, read_parse_file,
  },
  versions::{
    MC_DIR, VERSION_DIR,
    loader::{CheckFuture, Loader, LoaderVersion, util::download_maven_future},
    maven::{MavenName, full_path_from_maven, parse_maven_name},
  },
};

const API_BASE_URL_FABRIC: &str = "https://meta.fabricmc.net/v2/versions";
const API_BASE_URL_QUILT: &str = "https://meta.quiltmc.org/v3/versions";
const MAVEN_BASE_URL_FABRIC: &str = "https://maven.fabricmc.net";
const MAVEN_BASE_URL_QUILT: &str = "https://maven.quiltmc.org/repository/release";
const INDEX_FILE_NAME_FABRIC: &str = "fabric";
const INDEX_FILE_NAME_QUILT: &str = "quilt";

pub struct FabricLikeLoader {
  pub base_url: String,
  pub index_file_name: String,
}

impl FabricLikeLoader {
  pub fn fabric() -> Self {
    Self {
      base_url: API_BASE_URL_FABRIC.to_string(),
      index_file_name: INDEX_FILE_NAME_FABRIC.to_string(),
    }
  }

  pub fn quilt() -> Self {
    Self {
      base_url: API_BASE_URL_QUILT.to_string(),
      index_file_name: INDEX_FILE_NAME_QUILT.to_string(),
    }
  }

  fn loader(&self, data_dir: &Path) -> PathBuf {
    let filename = format!("{}-loader.json", self.index_file_name);
    path!(data_dir, MC_DIR, VERSION_DIR, filename)
  }

  fn game(&self, data_dir: &Path) -> PathBuf {
    let filename = format!("{}-game.json", self.index_file_name);
    path!(data_dir, MC_DIR, VERSION_DIR, filename)
  }
}

#[async_trait::async_trait]
impl Loader for FabricLikeLoader {
  async fn download_metadata(&self, client: &Client, data_dir: &Path) -> Result<()> {
    let url = Url::parse(&format!("{}/loader", self.base_url))?;
    let path = self.loader(data_dir);
    download_file_no_hash_force(client, &path, url).await?;

    let url = Url::parse(&format!("{}/game", self.base_url))?;
    let path = self.game(data_dir);
    download_file_no_hash_force(client, &path, url).await?;
    Ok(())
  }

  async fn supported_versions(&self, data_dir: &Path, stable: bool) -> Result<Vec<String>> {
    let path = self.game(data_dir);
    let versions = read_parse_file::<Vec<GameVersionMeta>>(&path)
      .await?
      .into_iter()
      .filter(|v| v.stable == stable || !stable)
      .map(|v| v.version)
      .collect::<Vec<_>>();
    Ok(versions)
  }

  async fn loader_versions_for_mc_version(&self, _: &str, data_dir: &Path) -> Result<Vec<String>> {
    let path = self.loader(data_dir);
    let versions = read_parse_file::<Vec<LoaderVersionMeta>>(&path)
      .await?
      .into_iter()
      .map(|v| v.version)
      .collect::<Vec<_>>();
    Ok(versions)
  }
}

#[derive(Deserialize, Serialize)]
struct LoaderVersionMeta {
  version: String,
}

#[derive(Serialize, Deserialize)]
struct GameVersionMeta {
  version: String,
  stable: bool,
}

pub struct FabricLikeLoaderVersion {
  mc_version: String,
  loader_version: String,
  base_url: String,
  maven_base_url: String,
}

impl FabricLikeLoaderVersion {
  pub fn fabric(mc_version: String, loader_version: String) -> Self {
    Self {
      mc_version,
      loader_version,
      base_url: API_BASE_URL_FABRIC.to_string(),
      maven_base_url: MAVEN_BASE_URL_FABRIC.to_string(),
    }
  }

  pub fn quilt(mc_version: String, loader_version: String) -> Self {
    Self {
      mc_version,
      loader_version,
      base_url: API_BASE_URL_QUILT.to_string(),
      maven_base_url: MAVEN_BASE_URL_QUILT.to_string(),
    }
  }

  fn meta_path(&self, data_dir: &Path) -> PathBuf {
    path!(
      data_dir,
      MC_DIR,
      VERSION_DIR,
      &self.mc_version,
      format!("fabric-{}.json", self.loader_version)
    )
  }
}

#[async_trait::async_trait]
impl LoaderVersion for FabricLikeLoaderVersion {
  async fn download(&self, client: &Client, data_dir: &PathBuf) -> Result<Vec<CheckFuture>> {
    let url = Url::parse(&format!(
      "{}/loader/{}/{}",
      self.base_url, self.mc_version, self.loader_version
    ))?;
    let path = self.meta_path(data_dir);
    let meta: FabricVersionMeta = download_and_parse_file_no_hash_force(client, &path, url).await?;

    let libraries = match meta.launcher_meta {
      FabricLauncherMeta::V1(FabricLauncherMetaV1 { libraries, .. }) => libraries,
      FabricLauncherMeta::V2(FabricLauncherMetaV2 { libraries, .. }) => libraries,
    };

    let mut futures = Vec::new();
    for lib in libraries.client.into_iter().chain(libraries.common) {
      let data_dir = data_dir.clone();
      let client = client.clone();
      futures.push(download_maven_future(
        data_dir,
        lib.name,
        client,
        lib
          .url
          .unwrap_or(Url::parse(&self.maven_base_url).unwrap())
          .to_string(),
        lib.sha1,
      ));
    }

    let mut libs = vec![meta.loader.maven, meta.intermediary.maven];
    if let Some(hashed) = meta.hashed {
      libs.push(hashed.maven);
    }

    for lib in libs {
      let data_dir = data_dir.clone();
      let client = client.clone();
      let base_url = if lib.contains("fabricmc") {
        MAVEN_BASE_URL_FABRIC.to_string()
      } else {
        self.maven_base_url.clone()
      };

      futures.push(download_maven_future(data_dir, lib, client, base_url, None));
    }

    Ok(futures)
  }

  async fn classpath(&self, data_dir: &Path) -> Result<Vec<(MavenName, PathBuf)>> {
    let meta_path = self.meta_path(data_dir);
    let meta: FabricVersionMeta = read_parse_file(&meta_path).await?;

    let libraries = match meta.launcher_meta {
      FabricLauncherMeta::V1(FabricLauncherMetaV1 { libraries, .. }) => libraries,
      FabricLauncherMeta::V2(FabricLauncherMetaV2 { libraries, .. }) => libraries,
    };

    let mut libs = Vec::new();
    for lib in libraries.client.into_iter().chain(libraries.common) {
      let maven = parse_maven_name(&lib.name);
      let path = full_path_from_maven(data_dir, &maven);
      libs.push((maven, path));
    }

    let loader_maven = parse_maven_name(&meta.loader.maven);
    let loader_path = full_path_from_maven(data_dir, &loader_maven);
    libs.push((loader_maven, loader_path));

    let intermediary_maven = parse_maven_name(&meta.intermediary.maven);
    let intermediary_path = full_path_from_maven(data_dir, &intermediary_maven);
    libs.push((intermediary_maven, intermediary_path));

    Ok(libs)
  }

  async fn main_class(&self, data_dir: &Path) -> Result<String> {
    let meta_path = self.meta_path(data_dir);
    let meta: FabricVersionMeta = read_parse_file(&meta_path).await?;

    match meta.launcher_meta {
      FabricLauncherMeta::V1(FabricLauncherMetaV1 { main_class, .. }) => Ok(main_class),
      FabricLauncherMeta::V2(FabricLauncherMetaV2 { main_class, .. }) => Ok(main_class.client),
    }
  }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FabricVersionMeta {
  loader: MavenLib,
  intermediary: MavenLib,
  hashed: Option<MavenLib>,
  launcher_meta: FabricLauncherMeta,
}

#[derive(Deserialize, Serialize)]
struct MavenLib {
  maven: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum FabricLauncherMeta {
  V1(FabricLauncherMetaV1),
  V2(FabricLauncherMetaV2),
}

impl<'de> Deserialize<'de> for FabricLauncherMeta {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    use serde::__private::de::{Content, ContentRefDeserializer};

    let content = Content::deserialize(deserializer)?;
    match FabricLauncherMetaV1::deserialize(ContentRefDeserializer::<D::Error>::new(&content)) {
      Ok(v1) => Ok(FabricLauncherMeta::V1(v1)),
      Err(_) => {
        match FabricLauncherMetaV2::deserialize(ContentRefDeserializer::<D::Error>::new(&content)) {
          Ok(v2) => Ok(FabricLauncherMeta::V2(v2)),
          Err(e) => Err(serde::de::Error::custom(format!(
            "Failed to deserialize FabricLauncherMeta: {e}"
          ))),
        }
      }
    }
  }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FabricLauncherMetaV1 {
  libraries: FabricLibraries,
  main_class: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FabricLauncherMetaV2 {
  libraries: FabricLibraries,
  main_class: FabricMainClass,
}

#[derive(Deserialize, Serialize)]
struct FabricLibraries {
  client: Vec<FabricLibrary>,
  server: Vec<FabricLibrary>,
  common: Vec<FabricLibrary>,
}

#[derive(Deserialize, Serialize)]
struct FabricLibrary {
  name: String,
  url: Option<Url>,
  sha1: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct FabricMainClass {
  client: String,
  server: String,
}
