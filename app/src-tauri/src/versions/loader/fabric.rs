use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

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
    loader::{parse_maven_name, path_from_maven, url_from_maven, Loader},
    LIBRARY_DIR, MC_DIR, SEPARATOR, VERSION_DIR,
  },
};

const API_BASE_URL: &str = "https://meta.fabricmc.net/v2/versions/loader";
const MAVEN_BASE_URL: &str = "https://maven.fabricmc.net";

pub struct FabricLoader {
  version: String,
  build: String,
}

impl FabricLoader {
  pub fn new(version: String, build: String) -> Self {
    Self { version, build }
  }

  fn meta_path(&self, data_dir: &Path) -> PathBuf {
    path!(
      data_dir,
      MC_DIR,
      VERSION_DIR,
      &self.version,
      format!("fabric-{}.json", self.build)
    )
  }
}

#[async_trait::async_trait]
impl Loader for FabricLoader {
  async fn download(&self, client: &Client, data_dir: &Path) -> Result<()> {
    let url = Url::parse(&format!("{API_BASE_URL}/{}/{}", self.version, self.build))?;
    let path = self.meta_path(data_dir);
    let meta: FabricVersionMeta = download_and_parse_file_no_hash_force(client, &path, url).await?;

    let libraries = match meta.launcher_meta {
      FabricLauncherMeta::V1 { libraries, .. } => libraries,
      FabricLauncherMeta::V2 { libraries, .. } => libraries,
    };

    for lib in libraries.client.into_iter().chain(libraries.common) {
      let maven = parse_maven_name(&lib.name)?;
      let lib_path = path!(data_dir, MC_DIR, LIBRARY_DIR, path_from_maven(&maven));
      let lib_url = url_from_maven(MAVEN_BASE_URL, &maven)?;

      download_file_no_hash_force(client, &lib_path, lib_url).await?;
    }

    Ok(())
  }

  async fn classpath(&self, data_dir: &Path) -> Result<OsString> {
    let meta_path = self.meta_path(data_dir);
    let meta: FabricVersionMeta = read_parse_file(&meta_path).await?;

    let mut classpath = OsString::new();
    let libraries = match meta.launcher_meta {
      FabricLauncherMeta::V1 { libraries, .. } => libraries,
      FabricLauncherMeta::V2 { libraries, .. } => libraries,
    };

    for lib in libraries.client.into_iter().chain(libraries.common) {
      let maven = parse_maven_name(&lib.name)?;
      let lib_path = path!(data_dir, MC_DIR, LIBRARY_DIR, path_from_maven(&maven));
      classpath.push(lib_path);
      classpath.push(SEPARATOR);
    }

    Ok(classpath)
  }

  async fn main_class(&self, data_dir: &Path) -> Result<String> {
    let meta_path = self.meta_path(data_dir);
    let meta: FabricVersionMeta = read_parse_file(&meta_path).await?;

    match meta.launcher_meta {
      FabricLauncherMeta::V1 { main_class, .. } => Ok(main_class),
      FabricLauncherMeta::V2 { main_class, .. } => Ok(main_class.client),
    }
  }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FabricVersionMeta {
  loader: FabricLoaderMeta,
  intermediary: IntermediaryMeta,
  launcher_meta: FabricLauncherMeta,
}

#[derive(Deserialize, Serialize)]
struct FabricLoaderMeta {
  separator: String,
  build: usize,
  maven: String,
  version: String,
  stable: bool,
}

#[derive(Deserialize, Serialize)]
struct IntermediaryMeta {
  version: String,
  maven: String,
  stable: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "version")]
enum FabricLauncherMeta {
  #[serde(rename = "1")]
  V1 {
    libraries: FabricLibraries,
    main_class: String,
    arguments: FabricArguments,
    launchwrapper: FabricLauncherWrapper,
  },
  #[serde(rename = "2")]
  V2 {
    libraries: FabricLibraries,
    main_class: FabricMainClass,
  },
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
struct FabricArguments {
  client: Vec<String>,
  server: Vec<String>,
  common: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct FabricLauncherWrapper {
  tweakers: FabricTweakers,
}

#[derive(Deserialize, Serialize)]
struct FabricTweakers {
  client: Vec<String>,
  server: Vec<String>,
  common: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct FabricMainClass {
  client: String,
  server: String,
}
