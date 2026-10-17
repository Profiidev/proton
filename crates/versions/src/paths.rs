use std::path::PathBuf;

use crate::meta::java::Component;
use proton_utils::path;

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
  mc_version: String,
  component: Component,
}

impl JavaVersionPath {
  pub fn new(data_dir: &PathBuf, component: Component, mc_version: String) -> Self {
    let base_path = path!(data_dir, JAVA_DIR, &component.to_string());
    let java_root = path!(data_dir, JAVA_DIR);

    JavaVersionPath {
      base_path,
      java_root,
      mc_version,
      component,
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

  pub fn native_path(&self) -> PathBuf {
    if let Component::JreLegacy = self.component {
      path!(&self.base_path, &self.mc_version)
    } else {
      self.library_path()
    }
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn layout_matches_the_tauri_data_dir() {
    let data = PathBuf::from("/home/ü ser/data");
    let p = |s: &str| PathBuf::from(format!("/home/ü ser/data/{s}"));

    let mc = MCPath::new(&data);
    assert_eq!(mc.mc_manifest(), p("minecraft/manifest.json"));
    assert_eq!(mc.library_path(), p("minecraft/lib"));
    assert_eq!(mc.assets_path(), p("minecraft/assets"));
    assert_eq!(mc.assets_objects_path(), p("minecraft/assets/objects"));
    assert_eq!(mc.assets_index_path(), p("minecraft/assets/indexes"));

    let version = MCVersionPath::new(&data, "1.20.1");
    assert_eq!(version.version_root(), &p("minecraft/versions"));
    assert_eq!(version.base_path(), &p("minecraft/versions/1.20.1"));
    assert_eq!(
      version.version_manifest(),
      p("minecraft/versions/1.20.1/1.20.1.json")
    );
    assert_eq!(
      version.client_jar(),
      p("minecraft/versions/1.20.1/1.20.1.jar")
    );
    // the empty version is how callers get at the version root
    let root = MCVersionPath::new(&data, "");
    assert_eq!(root.version_root(), root.base_path());

    let java = JavaVersionPath::new(&data, Component::JavaRuntimeGamma, "1.20.1".into());
    assert_eq!(java.java_manifest(), p("java/manifest.json"));
    assert_eq!(java.base_path(), &p("java/java-runtime-gamma"));
    assert_eq!(java.library_path(), p("java/java-runtime-gamma/lib"));
    assert_eq!(java.native_path(), java.library_path());
    #[cfg(target_family = "unix")]
    assert_eq!(java.bin_path(), p("java/java-runtime-gamma/bin/java"));
  }

  #[test]
  fn legacy_java_keeps_natives_per_minecraft_version() {
    let data = PathBuf::from("/d");
    let a = JavaVersionPath::new(&data, Component::JreLegacy, "1.8.9".into());
    let b = JavaVersionPath::new(&data, Component::JreLegacy, "1.12.2".into());
    assert_eq!(a.native_path(), PathBuf::from("/d/java/jre-legacy/1.8.9"));
    assert_ne!(a.native_path(), b.native_path());
    assert_eq!(a.library_path(), b.library_path());
  }
}
