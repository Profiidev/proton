use std::{future::Future, path::PathBuf, pin::Pin};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
  loader::{
    fabric::{FabricLikeLoader, FabricLikeLoaderVersion},
    forge::{ForgeLikeLoader, ForgeLikeLoaderVersion},
  },
  maven::MavenArtifact,
  paths::{MCPath, MCVersionPath},
};
use proton_utils::download::DownloadFileSizeFuture;

pub mod fabric;
pub mod forge;
mod util;

type DownloadFuture =
  Pin<Box<dyn Future<Output = Result<(DownloadFileSizeFuture, usize)>> + Send + 'static>>;
pub(crate) type CheckFuture =
  Pin<Box<dyn Future<Output = Result<Option<DownloadFuture>>> + Send + 'static>>;

pub struct ClasspathEntry {
  pub artifact: MavenArtifact,
  pub path: PathBuf,
}

#[derive(Default)]
pub struct Arguments {
  pub jvm_args: Vec<String>,
  pub game_args: Vec<String>,
  pub overwrite_game_args: bool,
}

#[async_trait::async_trait]
pub trait LoaderVersion: Send + Sync + 'static {
  #[allow(clippy::ptr_arg)]
  async fn download(
    &self,
    client: &Client,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
    existing_libs: &[String],
  ) -> Result<Vec<CheckFuture>>;
  async fn preprocess(
    &self,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
    jre_bin: PathBuf,
  ) -> Result<()>;
  async fn classpath(
    &self,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
  ) -> Result<Vec<ClasspathEntry>>;
  async fn main_class(&self, version_path: &MCVersionPath) -> Result<String>;
  async fn arguments(&self, version_path: &MCVersionPath) -> Result<Arguments>;
}

#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
  async fn download_metadata(&self, client: &Client, version_path: &MCVersionPath) -> Result<()>;
  async fn supported_versions(
    &self,
    version_path: &MCVersionPath,
    stable: bool,
  ) -> Result<Vec<String>>;
  async fn loader_versions_for_mc_version(
    &self,
    mc_version: &str,
    version_path: &MCVersionPath,
    stable: bool,
  ) -> Result<Vec<String>>;

  async fn newest_loader_version_for_mc_version(
    &self,
    mc_version: &str,
    version_path: &MCVersionPath,
  ) -> Result<String> {
    let versions = self
      .loader_versions_for_mc_version(mc_version, version_path, true)
      .await?;
    if versions.is_empty() {
      return Err(anyhow::anyhow!(
        "No loader versions found for Minecraft version: {}",
        mc_version
      ));
    }
    Ok(versions[0].clone())
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoaderType {
  Vanilla,
  Fabric,
  Quilt,
  Forge,
  NeoForge,
}

impl LoaderType {
  pub fn loader(self) -> Option<Box<dyn Loader>> {
    match self {
      LoaderType::Fabric => Some(Box::new(FabricLikeLoader::fabric())),
      LoaderType::Quilt => Some(Box::new(FabricLikeLoader::quilt())),
      LoaderType::Forge => Some(Box::new(ForgeLikeLoader::forge())),
      LoaderType::NeoForge => Some(Box::new(ForgeLikeLoader::neoforge())),
      LoaderType::Vanilla => None,
    }
  }

  pub fn loader_version(
    self,
    mc_version: String,
    loader_version: String,
  ) -> Option<Box<dyn LoaderVersion>> {
    match self {
      LoaderType::Fabric => Some(Box::new(FabricLikeLoaderVersion::fabric(
        mc_version,
        loader_version,
      ))),
      LoaderType::Quilt => Some(Box::new(FabricLikeLoaderVersion::quilt(
        mc_version,
        loader_version,
      ))),
      LoaderType::Forge => Some(Box::new(ForgeLikeLoaderVersion::forge(
        mc_version,
        loader_version,
      ))),
      LoaderType::NeoForge => Some(Box::new(ForgeLikeLoaderVersion::neoforge(
        mc_version,
        loader_version,
      ))),
      LoaderType::Vanilla => None,
    }
  }

  pub fn mod_loaders() -> Vec<Box<dyn Loader>> {
    vec![
      LoaderType::Fabric.loader().unwrap(),
      LoaderType::Quilt.loader().unwrap(),
      LoaderType::Forge.loader().unwrap(),
      LoaderType::NeoForge.loader().unwrap(),
    ]
  }
}

impl ClasspathEntry {
  pub fn new(artifact: MavenArtifact, path: PathBuf) -> Self {
    Self { artifact, path }
  }

  pub fn from_name(name: &str, mc_path: &MCPath) -> Result<Self> {
    let artifact = MavenArtifact::new(name)?;
    let path = artifact.full_path(mc_path);
    Ok(Self { artifact, path })
  }
}

impl Arguments {
  pub fn new(jvm_args: Vec<String>, game_args: Vec<String>, overwrite_game_args: bool) -> Self {
    Self {
      jvm_args,
      game_args,
      overwrite_game_args,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Fixed(Vec<String>);

  #[async_trait::async_trait]
  impl Loader for Fixed {
    async fn download_metadata(&self, _: &Client, _: &MCVersionPath) -> Result<()> {
      Ok(())
    }
    async fn supported_versions(&self, _: &MCVersionPath, _: bool) -> Result<Vec<String>> {
      Ok(vec![])
    }
    async fn loader_versions_for_mc_version(
      &self,
      _: &str,
      _: &MCVersionPath,
      stable: bool,
    ) -> Result<Vec<String>> {
      // the newest version is asked for stable only
      assert!(stable);
      Ok(self.0.clone())
    }
  }

  #[test]
  fn vanilla_has_no_loader() {
    assert!(LoaderType::Vanilla.loader().is_none());
    assert!(
      LoaderType::Vanilla
        .loader_version("1.20.1".into(), "x".into())
        .is_none()
    );
    for ty in [
      LoaderType::Fabric,
      LoaderType::Quilt,
      LoaderType::Forge,
      LoaderType::NeoForge,
    ] {
      assert!(ty.loader().is_some());
      assert!(ty.loader_version("1.20.1".into(), "x".into()).is_some());
    }
    assert_eq!(LoaderType::mod_loaders().len(), 4);
  }

  #[test]
  fn loader_types_keep_their_stored_names() {
    // profiles in store.json hold these strings
    for (ty, name) in [
      (LoaderType::Vanilla, "Vanilla"),
      (LoaderType::Fabric, "Fabric"),
      (LoaderType::Quilt, "Quilt"),
      (LoaderType::Forge, "Forge"),
      (LoaderType::NeoForge, "NeoForge"),
    ] {
      assert_eq!(serde_json::to_value(ty).unwrap(), name);
      assert_eq!(
        serde_json::from_value::<LoaderType>(name.into()).unwrap(),
        ty
      );
    }
  }

  #[test]
  fn newest_loader_version_is_the_first_or_an_error() {
    let path = MCVersionPath::new(&PathBuf::from("/d"), "1.20.1");
    proton_utils::rt::block_on(async {
      let loader = Fixed(vec!["2".into(), "1".into()]);
      let newest = loader.newest_loader_version_for_mc_version("1.20.1", &path);
      assert_eq!(newest.await.unwrap(), "2");

      let err = Fixed(vec![])
        .newest_loader_version_for_mc_version("1.20.1", &path)
        .await
        .unwrap_err();
      assert_eq!(
        err.to_string(),
        "No loader versions found for Minecraft version: 1.20.1"
      );
    });
  }

  #[test]
  fn classpath_entries_resolve_maven_names() {
    let mc = MCPath::new(&PathBuf::from("/d"));
    let entry = ClasspathEntry::from_name("a.b:c:1", &mc).unwrap();
    assert_eq!(
      entry.path,
      PathBuf::from("/d/minecraft/lib/a/b/c/1/c-1.jar")
    );
    assert!(ClasspathEntry::from_name("broken", &mc).is_err());
    let args = Arguments::new(vec!["-a".into()], vec![], true);
    assert!(args.overwrite_game_args && args.game_args.is_empty());
  }
}
