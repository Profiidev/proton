use std::path::PathBuf;

use crate::{path, versions::meta::java::Component};

const JAVA_DIR: &str = "java";
const MC_DIR: &str = "minecraft";
const VERSION_DIR: &str = "versions";
const LIBRARY_DIR: &str = "lib";
const ASSETS_DIR: &str = "assets";
const ASSETS_OBJECTS_DIR: &str = "objects";
const ASSETS_INDEX_DIR: &str = "indexes";

pub const QUICK_PLAY: &str = "quick_play.json";
const MANIFEST_NAME: &str = "manifest.json";

#[derive(Clone)]
pub struct JavaVersionPath {
  base_path: PathBuf,
  java_root: PathBuf,
}

impl JavaVersionPath {
  pub fn new(data_dir: &PathBuf, component: Component) -> Self {
    let base_path = path!(data_dir, JAVA_DIR, &component.to_string());
    let java_root = path!(data_dir, JAVA_DIR);
    JavaVersionPath {
      base_path,
      java_root,
    }
  }

  pub fn java_manifest(&self) -> PathBuf {
    path!(&self.java_root, MANIFEST_NAME)
  }

  pub fn base_path(&self) -> &PathBuf {
    &self.base_path
  }

  pub fn library_path(&self) -> PathBuf {
    path!(&self.base_path, LIBRARY_DIR)
  }

  pub fn bin_path(&self) -> PathBuf {
    #[cfg(target_family = "unix")]
    let jre_path = path!(&self.base_path, "bin", "java");
    #[cfg(target_family = "windows")]
    let jre_path = path!(&self.base_path, "bin", "java.exe");

    jre_path
  }
}

#[derive(Clone)]
pub struct MCPath {
  base_path: PathBuf,
}

impl MCPath {
  pub fn new(data_dir: &PathBuf) -> Self {
    let base_path = path!(data_dir, MC_DIR);
    MCPath { base_path }
  }

  pub fn mc_manifest(&self) -> PathBuf {
    path!(&self.base_path, MANIFEST_NAME)
  }

  pub fn library_path(&self) -> PathBuf {
    path!(&self.base_path, LIBRARY_DIR)
  }

  pub fn assets_path(&self) -> PathBuf {
    path!(&self.base_path, ASSETS_DIR)
  }

  pub fn assets_objects_path(&self) -> PathBuf {
    path!(&self.assets_path(), ASSETS_OBJECTS_DIR)
  }

  pub fn assets_index_path(&self) -> PathBuf {
    path!(&self.assets_path(), ASSETS_INDEX_DIR)
  }
}

#[derive(Clone)]
pub struct MCVersionPath {
  base_path: PathBuf,
  version: String,
  version_root: PathBuf,
}

impl MCVersionPath {
  pub fn new(data_dir: &PathBuf, version: &str) -> Self {
    let base_path = path!(data_dir, MC_DIR, VERSION_DIR, version);
    let version_root = path!(data_dir, MC_DIR, VERSION_DIR);
    MCVersionPath {
      base_path,
      version: version.to_string(),
      version_root,
    }
  }

  pub fn version_root(&self) -> &PathBuf {
    &self.version_root
  }

  pub fn base_path(&self) -> &PathBuf {
    &self.base_path
  }

  pub fn version_manifest(&self) -> PathBuf {
    path!(&self.base_path, format!("{}.json", self.version))
  }

  pub fn client_jar(&self) -> PathBuf {
    path!(&self.base_path, format!("{}.jar", self.version))
  }
}
