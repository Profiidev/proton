use std::{ffi::OsString, path::Path};

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::versions::loader::fabric::{FabricLoader, FabricLoaderVersion};

pub mod fabric;
mod util;

#[async_trait::async_trait]
pub trait LoaderVersion: Send + Sync + 'static {
  async fn download(&self, client: &Client, data_dir: &Path) -> Result<()>;
  async fn classpath(&self, data_dir: &Path) -> Result<OsString>;
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
      LoaderType::Fabric => Some(Box::new(FabricLoader)),
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
      LoaderType::Fabric => Some(Box::new(FabricLoaderVersion::new(
        mc_version,
        loader_version,
      ))),
      LoaderType::Vanilla => None,
      _ => unimplemented!("LoaderType not implemented: {:?}", self),
    }
  }

  pub fn mod_loaders() -> Vec<Box<dyn Loader>> {
    vec![LoaderType::Fabric.loader().unwrap()]
  }
}
