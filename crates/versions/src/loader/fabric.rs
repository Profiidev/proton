use std::path::PathBuf;

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use url::Url;

use crate::{
  loader::{
    Arguments, CheckFuture, ClasspathEntry, Loader, LoaderVersion, util::download_maven_future,
  },
  paths::{MCPath, MCVersionPath},
};
use proton_utils::{
  download::{download_and_parse_file_no_hash, download_file_no_hash},
  file::read_parse_file,
};
use proton_utils::{http, path};

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
      base_url: http::url(API_BASE_URL_FABRIC).into_owned(),
      index_file_name: INDEX_FILE_NAME_FABRIC.to_string(),
    }
  }

  pub fn quilt() -> Self {
    Self {
      base_url: http::url(API_BASE_URL_QUILT).into_owned(),
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
    download_file_no_hash(client, &path, url, true).await?;

    let url = Url::parse(&format!("{}/game", self.base_url))?;
    let path = self.game(version_path);
    download_file_no_hash(client, &path, url, true).await?;
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

#[derive(Deserialize)]
struct LoaderVersionMeta {
  version: String,
}

#[derive(Deserialize)]
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
      base_url: http::url(API_BASE_URL_FABRIC).into_owned(),
      maven_base_url: http::url(MAVEN_BASE_URL_FABRIC).into_owned(),
    }
  }

  pub fn quilt(mc_version: String, loader_version: String) -> Self {
    Self {
      meta_file_name: format!("{}-{}.json", INDEX_FILE_NAME_QUILT, loader_version),
      mc_version,
      loader_version,
      base_url: http::url(API_BASE_URL_QUILT).into_owned(),
      maven_base_url: http::url(MAVEN_BASE_URL_QUILT).into_owned(),
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
    let meta: FabricVersionMeta = download_and_parse_file_no_hash(client, &path, url, true).await?;

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

    // unobfuscated versions (26.1+) have no intermediary on quilt
    let mut libs = vec![meta.loader.maven];
    libs.extend(meta.intermediary.map(|i| i.maven));
    libs.extend(meta.hashed.map(|h| h.maven));

    for lib in libs {
      let base_url = if lib.contains("fabricmc") {
        http::url(MAVEN_BASE_URL_FABRIC).into_owned()
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
    if let Some(intermediary) = meta.intermediary {
      libs.push(ClasspathEntry::from_name(&intermediary.maven, mc_path)?);
    }

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FabricVersionMeta {
  loader: MavenLib,
  intermediary: Option<MavenLib>,
  hashed: Option<MavenLib>,
  launcher_meta: FabricLauncherMeta,
}

#[derive(Deserialize)]
struct MavenLib {
  maven: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FabricLauncherMeta {
  V1(FabricLauncherMetaV1),
  V2(FabricLauncherMetaV2),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FabricLauncherMetaV1 {
  libraries: FabricLibraries,
  main_class: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FabricLauncherMetaV2 {
  libraries: FabricLibraries,
  main_class: FabricMainClass,
}

#[derive(Deserialize)]
struct FabricLibraries {
  client: Vec<FabricLibrary>,
  common: Vec<FabricLibrary>,
}

#[derive(Deserialize)]
struct FabricLibrary {
  name: String,
  url: Option<Url>,
  sha1: Option<String>,
}

#[derive(Deserialize)]
struct FabricMainClass {
  client: String,
}

#[cfg(test)]
mod tests {
  use proton_utils::{download::default_client, file::bytes_hash, rt, test_http::FakeServer};
  use tokio::fs;

  use super::*;
  use crate::loader::CheckFuture;

  const GAME: &str = r#"[
    { "version": "1.21", "stable": true },
    { "version": "24w01a", "stable": false },
    { "version": "1.20.1", "stable": true }
  ]"#;
  const FABRIC_LOADERS: &str = r#"[
    { "version": "0.16.0", "stable": false },
    { "version": "0.15.0", "stable": true }
  ]"#;
  const QUILT_LOADERS: &str = r#"[
    { "version": "0.27.0-beta.1" },
    { "version": "0.26.0-pre.2" },
    { "version": "0.25.0" }
  ]"#;

  fn version_path(data: &std::path::Path, mc: &str) -> MCVersionPath {
    MCVersionPath::new(&data.to_path_buf(), mc)
  }

  /// Runs the checks, then the downloads they ask for, returns how many were skipped
  async fn run(futures: Vec<CheckFuture>) -> Result<usize> {
    let mut skipped = 0;
    for check in futures {
      match check.await? {
        Some(download) => {
          let (write, _) = download.await?;
          write(Box::new(|_| {})).await?;
        }
        None => skipped += 1,
      }
    }
    Ok(skipped)
  }

  #[test]
  fn metadata_lands_in_the_version_root() {
    let server = FakeServer::start();
    server
      .route(
        "GET",
        "/meta.fabricmc.net/v2/versions/loader",
        200,
        FABRIC_LOADERS,
      )
      .route("GET", "/meta.fabricmc.net/v2/versions/game", 200, GAME)
      .route(
        "GET",
        "/meta.quiltmc.org/v3/versions/loader",
        200,
        QUILT_LOADERS,
      )
      .route("GET", "/meta.quiltmc.org/v3/versions/game", 200, GAME);
    let dir = tempfile::tempdir().unwrap();
    let path = version_path(dir.path(), "");
    let (fabric, quilt) = (FabricLikeLoader::fabric(), FabricLikeLoader::quilt());

    rt::block_on(async {
      let client = default_client();
      fabric.download_metadata(&client, &path).await.unwrap();
      quilt.download_metadata(&client, &path).await.unwrap();
      let root = path.version_root();
      for file in ["fabric-loader", "fabric-game", "quilt-loader", "quilt-game"] {
        assert!(root.join(format!("{file}.json")).exists(), "{file}");
      }

      assert_eq!(
        fabric.supported_versions(&path, true).await.unwrap(),
        ["1.21", "1.20.1"]
      );
      assert_eq!(
        quilt.supported_versions(&path, false).await.unwrap(),
        ["1.21", "24w01a", "1.20.1"]
      );
      let fabric_loaders = |stable| fabric.loader_versions_for_mc_version("1.21", &path, stable);
      assert_eq!(fabric_loaders(true).await.unwrap(), ["0.15.0"]);
      assert_eq!(fabric_loaders(false).await.unwrap(), ["0.16.0", "0.15.0"]);
      // quilt has no stable flag, betas and pre releases are unstable
      let quilt_loaders = |stable| quilt.loader_versions_for_mc_version("1.21", &path, stable);
      assert_eq!(quilt_loaders(true).await.unwrap(), ["0.25.0"]);
      assert_eq!(quilt_loaders(false).await.unwrap().len(), 3);

      // a refresh that fails keeps nothing half written and reports the error
      server.route("GET", "/meta.fabricmc.net/v2/versions/loader", 503, "down");
      assert!(fabric.download_metadata(&client, &path).await.is_err());
      assert_eq!(fabric_loaders(true).await.unwrap(), ["0.15.0"]);
    });
  }

  #[test]
  fn missing_or_corrupt_metadata_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = version_path(dir.path(), "");
    let loader = FabricLikeLoader::fabric();
    rt::block_on(async {
      assert!(loader.supported_versions(&path, true).await.is_err());
      fs::create_dir_all(path.version_root()).await.unwrap();
      fs::write(path.version_root().join("fabric-game.json"), "{")
        .await
        .unwrap();
      assert!(loader.supported_versions(&path, true).await.is_err());
      // quilt loader entries have no stable flag, the fabric shape is required
      fs::write(
        path.version_root().join("fabric-loader.json"),
        QUILT_LOADERS,
      )
      .await
      .unwrap();
      let versions = loader.loader_versions_for_mc_version("1.21", &path, true);
      assert!(versions.await.is_err());
    });
  }

  #[test]
  fn fabric_install_downloads_libraries_and_skips_hashed_ones() {
    let server = FakeServer::start();
    let origin = server.origin();
    let meta = format!(
      r#"{{
        "loader": {{ "maven": "net.fabricmc:fabric-loader:0.15.0" }},
        "intermediary": {{ "maven": "net.fabricmc:intermediary:1.20.1" }},
        "launcherMeta": {{
          "libraries": {{
            "client": [{{ "name": "a.b:client-lib:1", "url": "{origin}/custom/" }}],
            "common": [
              {{ "name": "a.b:cached:1", "sha1": "{}" }},
              {{ "name": "a.b:fresh:1" }}
            ]
          }},
          "mainClass": {{ "client": "net.fabricmc.loader.impl.launch.knot.KnotClient", "server": "x" }}
        }}
      }}"#,
      bytes_hash(b"cached")
    );
    server
      .route(
        "GET",
        "/meta.fabricmc.net/v2/versions/loader/1.20.1/0.15.0",
        200,
        meta,
      )
      .route(
        "GET",
        "/custom/a/b/client-lib/1/client-lib-1.jar",
        200,
        "client",
      )
      .route(
        "GET",
        "/maven.fabricmc.net/a/b/fresh/1/fresh-1.jar",
        200,
        "fresh",
      )
      .route(
        "GET",
        "/maven.fabricmc.net/net/fabricmc/fabric-loader/0.15.0/fabric-loader-0.15.0.jar",
        200,
        "loader",
      )
      .route(
        "GET",
        "/maven.fabricmc.net/net/fabricmc/intermediary/1.20.1/intermediary-1.20.1.jar",
        200,
        "intermediary",
      );

    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (path, mc) = (version_path(&data, "1.20.1"), MCPath::new(&data));
    let version = FabricLikeLoaderVersion::fabric("1.20.1".into(), "0.15.0".into());
    let lib = |name: &str| {
      crate::maven::MavenArtifact::new(name)
        .unwrap()
        .full_path(&mc)
    };

    rt::block_on(async {
      fs::create_dir_all(lib("a.b:cached:1").parent().unwrap())
        .await
        .unwrap();
      fs::write(lib("a.b:cached:1"), "cached").await.unwrap();

      let client = default_client();
      let futures = version.download(&client, &path, &mc, &[]).await.unwrap();
      assert_eq!(futures.len(), 5);
      assert_eq!(run(futures).await.unwrap(), 1);
      for (name, body) in [
        ("a.b:client-lib:1", "client"),
        ("a.b:fresh:1", "fresh"),
        ("net.fabricmc:fabric-loader:0.15.0", "loader"),
        ("net.fabricmc:intermediary:1.20.1", "intermediary"),
      ] {
        assert_eq!(fs::read_to_string(lib(name)).await.unwrap(), body, "{name}");
      }
      assert!(path.base_path().join("fabric-0.15.0.json").exists());

      let classpath: Vec<_> = version
        .classpath(&path, &mc)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.artifact.artifact)
        .collect();
      // libraries first, then the loader and intermediary
      assert_eq!(
        classpath,
        [
          "client-lib",
          "cached",
          "fresh",
          "fabric-loader",
          "intermediary"
        ]
      );
      assert_eq!(
        version.main_class(&path).await.unwrap(),
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
      );
      let args = version.arguments(&path).await.unwrap();
      assert!(args.jvm_args.is_empty() && args.game_args.is_empty());
      assert!(!args.overwrite_game_args);
      version
        .preprocess(&path, &mc, "/no/java".into())
        .await
        .unwrap();
    });

    // no double slash from the library url
    assert!(
      server.calls().iter().all(|c| !c.path.contains("//")),
      "{:?}",
      server.calls()
    );
  }

  #[test]
  fn quilt_install_without_intermediary_uses_the_quilt_maven() {
    let server = FakeServer::start();
    // launcher meta v1 has a plain main class and no intermediary on 26.1+
    let meta = r#"{
      "loader": { "maven": "org.quiltmc:quilt-loader:0.25.0" },
      "hashed": { "maven": "org.quiltmc:hashed:26.1" },
      "launcherMeta": {
        "libraries": { "client": [], "common": [{ "name": "net.fabricmc:tiny-mappings-parser:0.3.0" }] },
        "mainClass": "org.quiltmc.loader.impl.launch.knot.KnotClient"
      }
    }"#;
    server
      .route("GET", "/meta.quiltmc.org/v3/versions/loader/26.1/0.25.0", 200, meta)
      .route("GET", "/maven.quiltmc.org/repository/release/org/quiltmc/quilt-loader/0.25.0/quilt-loader-0.25.0.jar", 200, "q")
      .route("GET", "/maven.quiltmc.org/repository/release/org/quiltmc/hashed/26.1/hashed-26.1.jar", 200, "h")
      // a library without url comes from the loader maven even when it is a fabric one
      .route("GET", "/maven.quiltmc.org/repository/release/net/fabricmc/tiny-mappings-parser/0.3.0/tiny-mappings-parser-0.3.0.jar", 200, "t");

    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (path, mc) = (version_path(&data, "26.1"), MCPath::new(&data));
    let version = FabricLikeLoaderVersion::quilt("26.1".into(), "0.25.0".into());
    rt::block_on(async {
      let futures = version
        .download(&default_client(), &path, &mc, &[])
        .await
        .unwrap();
      assert_eq!(run(futures).await.unwrap(), 0);
      assert_eq!(
        version.main_class(&path).await.unwrap(),
        "org.quiltmc.loader.impl.launch.knot.KnotClient"
      );
      let classpath = version.classpath(&path, &mc).await.unwrap();
      // hashed mappings are downloaded but not on the classpath
      assert_eq!(classpath.len(), 2);
    });
  }

  #[test]
  fn fabric_artifacts_of_quilt_come_from_fabric_maven() {
    let server = FakeServer::start();
    let meta = r#"{
      "loader": { "maven": "org.quiltmc:quilt-loader:0.25.0" },
      "intermediary": { "maven": "net.fabricmc:intermediary:1.20.1" },
      "launcherMeta": { "libraries": { "client": [], "common": [] }, "mainClass": "m" }
    }"#;
    server
      .route("GET", "/meta.quiltmc.org/v3/versions/loader/1.20.1/0.25.0", 200, meta)
      .route("GET", "/maven.quiltmc.org/repository/release/org/quiltmc/quilt-loader/0.25.0/quilt-loader-0.25.0.jar", 200, "q")
      .route("GET", "/maven.fabricmc.net/net/fabricmc/intermediary/1.20.1/intermediary-1.20.1.jar", 200, "i");
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (path, mc) = (version_path(&data, "1.20.1"), MCPath::new(&data));
    let version = FabricLikeLoaderVersion::quilt("1.20.1".into(), "0.25.0".into());
    rt::block_on(async {
      let futures = version
        .download(&default_client(), &path, &mc, &[])
        .await
        .unwrap();
      assert_eq!(run(futures).await.unwrap(), 0);

      // a missing jar fails its download
      server.route(
        "GET",
        "/maven.fabricmc.net/net/fabricmc/intermediary/1.20.1/intermediary-1.20.1.jar",
        404,
        "",
      );
      let futures = version
        .download(&default_client(), &path, &mc, &[])
        .await
        .unwrap();
      assert!(run(futures).await.is_err());
    });
  }

  #[test]
  fn failed_meta_download_is_an_error() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/meta.fabricmc.net/v2/versions/loader/1.20.1/9.9.9",
      404,
      "",
    );
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (path, mc) = (version_path(&data, "1.20.1"), MCPath::new(&data));
    let version = FabricLikeLoaderVersion::fabric("1.20.1".into(), "9.9.9".into());
    rt::block_on(async {
      assert!(
        version
          .download(&default_client(), &path, &mc, &[])
          .await
          .is_err()
      );
      // nothing was installed, so launching parts fail too
      assert!(version.classpath(&path, &mc).await.is_err());
      assert!(version.main_class(&path).await.is_err());
    });
  }
}
