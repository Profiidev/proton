use std::{future::Future, path::PathBuf, pin::Pin};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::versions::{
  loader::{
    fabric::{FabricLikeLoader, FabricLikeLoaderVersion},
    forge::{ForgeLikeLoader, ForgeLikeLoaderVersion},
  },
  maven::MavenArtifact,
  paths::{MCPath, MCVersionPath},
};

pub mod fabric;
pub mod forge;
mod util;

type DownloadFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;
type CheckFuture = Pin<Box<dyn Future<Output = Result<Option<DownloadFuture>>> + Send + 'static>>;

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
  ) -> Result<Vec<(MavenArtifact, PathBuf)>>;
  async fn main_class(&self, version_path: &MCVersionPath) -> Result<String>;
  async fn arguments(
    &self,
    version_path: &MCVersionPath,
  ) -> Result<(Vec<String>, Vec<String>, bool)>;
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
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
