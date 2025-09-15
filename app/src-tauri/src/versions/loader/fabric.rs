use std::path::PathBuf;

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::Url;

use crate::{
  path,
  utils::{
    download::{download_and_parse_file_no_hash_force, download_file_no_hash_force},
    file::read_parse_file,
  },
  versions::{
    loader::{
      Arguments, CheckFuture, ClasspathEntry, Loader, LoaderVersion, util::download_maven_future,
    },
    paths::{MCPath, MCVersionPath},
  },
};

const API_BASE_URL_FABRIC: &str = "https://meta.fabricmc.net/v2/versions";
const API_BASE_URL_QUILT: &str = "https://meta.quiltmc.org/v3/versions";
const MAVEN_BASE_URL_FABRIC: &str = "https://maven.fabricmc.net";
const MAVEN_BASE_URL_QUILT: &str = "https://maven.quiltmc.org/repository/release";
const INDEX_FILE_NAME_FABRIC: &str = "fabric";
const INDEX_FILE_NAME_QUILT: &str = "quilt";

pub struct FabricLikeLoader {
  base_url: String,
  index_file_name: String,
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

  fn loader(&self, version_path: &MCVersionPath) -> PathBuf {
    let filename = format!("{}-loader.json", self.index_file_name);
    path!(version_path.version_root(), filename)
  }

  fn game(&self, version_path: &MCVersionPath) -> PathBuf {
    let filename = format!("{}-game.json", self.index_file_name);
    path!(version_path.version_root(), filename)
  }
}

#[async_trait::async_trait]
impl Loader for FabricLikeLoader {
  async fn download_metadata(&self, client: &Client, version_path: &MCVersionPath) -> Result<()> {
    let url = Url::parse(&format!("{}/loader", self.base_url))?;
    let path = self.loader(version_path);
    download_file_no_hash_force(client, &path, url).await?;

    let url = Url::parse(&format!("{}/game", self.base_url))?;
    let path = self.game(version_path);
    download_file_no_hash_force(client, &path, url).await?;
    Ok(())
  }

  async fn supported_versions(
    &self,
    version_path: &MCVersionPath,
    stable: bool,
  ) -> Result<Vec<String>> {
    let path = self.game(version_path);
    let versions = read_parse_file::<Vec<GameVersionMeta>>(&path)
      .await?
      .into_iter()
      .filter(|v| v.stable == stable || !stable)
      .map(|v| v.version)
      .collect::<Vec<_>>();
    Ok(versions)
  }

  async fn loader_versions_for_mc_version(
    &self,
    _: &str,
    version_path: &MCVersionPath,
    stable: bool,
  ) -> Result<Vec<String>> {
    let path = self.loader(version_path);
    if self.index_file_name == INDEX_FILE_NAME_FABRIC {
      let versions = read_parse_file::<Vec<GameVersionMeta>>(&path)
        .await?
        .into_iter()
        .filter(|v| v.stable == stable || !stable)
        .map(|v| v.version)
        .collect::<Vec<_>>();
      Ok(versions)
    } else {
      let versions = read_parse_file::<Vec<LoaderVersionMeta>>(&path)
        .await?
        .into_iter()
        .filter(|v| {
          (v.version.contains("beta") != stable && v.version.contains("pre") != stable) || !stable
        })
        .map(|v| v.version)
        .collect::<Vec<_>>();
      Ok(versions)
    }
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
  meta_file_name: String,
}

impl FabricLikeLoaderVersion {
  pub fn fabric(mc_version: String, loader_version: String) -> Self {
    Self {
      meta_file_name: format!("{}-{}.json", INDEX_FILE_NAME_FABRIC, loader_version),
      mc_version,
      loader_version,
      base_url: API_BASE_URL_FABRIC.to_string(),
      maven_base_url: MAVEN_BASE_URL_FABRIC.to_string(),
    }
  }

  pub fn quilt(mc_version: String, loader_version: String) -> Self {
    Self {
      meta_file_name: format!("{}-{}.json", INDEX_FILE_NAME_QUILT, loader_version),
      mc_version,
      loader_version,
      base_url: API_BASE_URL_QUILT.to_string(),
      maven_base_url: MAVEN_BASE_URL_QUILT.to_string(),
    }
  }

  fn meta_path(&self, version_path: &MCVersionPath) -> PathBuf {
    path!(version_path.base_path(), &self.meta_file_name)
  }
}

#[async_trait::async_trait]
impl LoaderVersion for FabricLikeLoaderVersion {
  async fn download(
    &self,
    client: &Client,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
    // fabric does not specify libraries also specified by vanilla, so we can skip them
    _: &[String],
  ) -> Result<Vec<CheckFuture>> {
    let url = Url::parse(&format!(
      "{}/loader/{}/{}",
      self.base_url, self.mc_version, self.loader_version
    ))?;
    let path = self.meta_path(version_path);
    let meta: FabricVersionMeta = download_and_parse_file_no_hash_force(client, &path, url).await?;

    let libraries = match meta.launcher_meta {
      FabricLauncherMeta::V1(FabricLauncherMetaV1 { libraries, .. }) => libraries,
      FabricLauncherMeta::V2(FabricLauncherMetaV2 { libraries, .. }) => libraries,
    };

    let mut futures = Vec::new();
    for lib in libraries.client.into_iter().chain(libraries.common) {
      futures.push(download_maven_future(
        mc_path.clone(),
        lib.name,
        client.clone(),
        lib
          .url
          .unwrap_or(Url::parse(&self.maven_base_url).unwrap())
          .to_string(),
        lib.sha1,
        None,
      ));
    }

    let mut libs = vec![meta.loader.maven, meta.intermediary.maven];
    if let Some(hashed) = meta.hashed {
      libs.push(hashed.maven);
    }

    for lib in libs {
      let base_url = if lib.contains("fabricmc") {
        MAVEN_BASE_URL_FABRIC.to_string()
      } else {
        self.maven_base_url.clone()
      };

      futures.push(download_maven_future(
        mc_path.clone(),
        lib,
        client.clone(),
        base_url,
        None,
        None,
      ));
    }

    Ok(futures)
  }

  async fn preprocess(&self, _: &MCVersionPath, _: &MCPath, _: PathBuf) -> Result<()> {
    // Fabric versions do not require preprocessing
    Ok(())
  }

  async fn classpath(
    &self,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
  ) -> Result<Vec<ClasspathEntry>> {
    let meta_path = self.meta_path(version_path);
    let meta: FabricVersionMeta = read_parse_file(&meta_path).await?;

    let libraries = match meta.launcher_meta {
      FabricLauncherMeta::V1(FabricLauncherMetaV1 { libraries, .. }) => libraries,
      FabricLauncherMeta::V2(FabricLauncherMetaV2 { libraries, .. }) => libraries,
    };

    let mut libs = Vec::new();
    for lib in libraries.client.into_iter().chain(libraries.common) {
      libs.push(ClasspathEntry::from_name(&lib.name, mc_path)?);
    }

    libs.push(ClasspathEntry::from_name(&meta.loader.maven, mc_path)?);
    libs.push(ClasspathEntry::from_name(
      &meta.intermediary.maven,
      mc_path,
    )?);

    Ok(libs)
  }

  async fn main_class(&self, version_path: &MCVersionPath) -> Result<String> {
    let meta_path = self.meta_path(version_path);
    let meta: FabricVersionMeta = read_parse_file(&meta_path).await?;

    match meta.launcher_meta {
      FabricLauncherMeta::V1(FabricLauncherMetaV1 { main_class, .. }) => Ok(main_class),
      FabricLauncherMeta::V2(FabricLauncherMetaV2 { main_class, .. }) => Ok(main_class.client),
    }
  }

  async fn arguments(&self, _: &MCVersionPath) -> Result<Arguments> {
    Ok(Arguments::default()) // Fabric does not require additional arguments
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

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum FabricLauncherMeta {
  V1(FabricLauncherMetaV1),
  V2(FabricLauncherMetaV2),
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
