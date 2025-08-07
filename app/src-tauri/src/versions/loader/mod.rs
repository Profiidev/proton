use std::{
  future::Future,
  path::{Path, PathBuf},
  pin::Pin,
};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::versions::{
  loader::fabric::{FabricLikeLoader, FabricLikeLoaderVersion},
  maven::MavenName,
};

pub mod fabric;
mod util;

type DownloadFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;
type CheckFuture = Pin<Box<dyn Future<Output = Result<Option<DownloadFuture>>> + Send + 'static>>;

#[async_trait::async_trait]
pub trait LoaderVersion: Send + Sync + 'static {
  #[allow(clippy::ptr_arg)]
  async fn download(&self, client: &Client, data_dir: &PathBuf) -> Result<Vec<CheckFuture>>;
  async fn classpath(&self, data_dir: &Path) -> Result<Vec<(MavenName, PathBuf)>>;
  async fn main_class(&self, data_dir: &Path) -> Result<String>;
}

#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
  async fn download_metadata(&self, client: &Client, data_dir: &Path) -> Result<()>;
  async fn loader_versions_for_mc_version(
    &self,
    mc_version: &str,
    client: &Client,
    data_dir: &Path,
  ) -> Result<Vec<String>>;
  async fn newest_loader_version_for_mc_version(
    &self,
    mc_version: &str,
    data_dir: &Path,
  ) -> Result<String>;
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
      LoaderType::Vanilla => None,
      _ => unimplemented!("LoaderType not implemented: {:?}", self),
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
      LoaderType::Vanilla => None,
      _ => unimplemented!("LoaderType not implemented: {:?}", self),
    }
  }

  pub fn mod_loaders() -> Vec<Box<dyn Loader>> {
    vec![
      LoaderType::Fabric.loader().unwrap(),
      LoaderType::Quilt.loader().unwrap(),
    ]
  }
}
