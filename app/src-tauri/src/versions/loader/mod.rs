use std::{
  future::Future,
  path::{Path, PathBuf},
  pin::Pin,
};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::versions::{
  loader::{
    fabric::{FabricLikeLoader, FabricLikeLoaderVersion},
    forge::{ForgeLikeLoader, ForgeLikeLoaderVersion},
  },
  maven::MavenName,
};

pub mod fabric;
pub mod forge;
mod util;

type DownloadFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;
type CheckFuture = Pin<Box<dyn Future<Output = Result<Option<DownloadFuture>>> + Send + 'static>>;

#[async_trait::async_trait]
pub trait LoaderVersion: Send + Sync + 'static {
  #[allow(clippy::ptr_arg)]
  async fn download(&self, client: &Client, data_dir: &PathBuf) -> Result<Vec<CheckFuture>>;
  async fn preprocess(&self, data_dir: &Path, jre_bin: PathBuf) -> Result<()>;
  async fn classpath(&self, data_dir: &Path) -> Result<Vec<(MavenName, PathBuf)>>;
  async fn main_class(&self, data_dir: &Path) -> Result<String>;
}

#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
  async fn download_metadata(&self, client: &Client, data_dir: &Path) -> Result<()>;
  async fn supported_versions(&self, data_dir: &Path, stable: bool) -> Result<Vec<String>>;
  async fn loader_versions_for_mc_version(
    &self,
    mc_version: &str,
    data_dir: &Path,
    stable: bool,
  ) -> Result<Vec<String>>;

  async fn newest_loader_version_for_mc_version(
    &self,
    mc_version: &str,
    data_dir: &Path,
  ) -> Result<String> {
    let versions = self
      .loader_versions_for_mc_version(mc_version, data_dir, true)
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
