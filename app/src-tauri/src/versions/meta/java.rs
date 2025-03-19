use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::Url;

use super::Rule;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
  pub component: String,
  pub major_version: u16,
}

impl Default for JavaVersion {
  #[inline(always)]
  fn default() -> Self {
    Self {
      component: "jre-legacy".to_string(),
      major_version: 8,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Library {
  pub name: String,
  pub url: Option<Url>,
  pub downloads: Option<Download>,
  pub natives: Option<serde_json::Value>,
  pub rules: Option<Vec<Rule>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Download {
  pub artifact: Option<Artifact>,
  pub classifiers: Option<Classifiers>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct Classifiers {
  pub natives_linux: Option<Artifact>,
  pub natives_osx: Option<Artifact>,
  pub natives_windows: Option<Artifact>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
  pub url: Url,
  pub path: String,
  pub sha1: String,
  pub size: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct JavaVersions {
  pub linux: PlatformVersion,
  pub linux_i386: PlatformVersion,
  pub mac_os: PlatformVersion,
  pub mac_os_arm64: PlatformVersion,
  pub windows_arm64: PlatformVersion,
  pub windows_x64: PlatformVersion,
  pub windows_x86: PlatformVersion,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct PlatformVersion {
  pub java_runtime_alpha: Vec<Version>,
  pub java_runtime_beta: Vec<Version>,
  pub java_runtime_delta: Vec<Version>,
  pub java_runtime_gamma: Vec<Version>,
  pub java_runtime_gamma_snapshot: Vec<Version>,
  pub jre_legacy: Vec<Version>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
  pub manifest: super::minecraft::Download,
  pub version: VersionName,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VersionName {
  pub name: String,
  pub released: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
#[allow(clippy::large_enum_variant, clippy::enum_variant_names)]
pub enum File {
  Directory,
  Link {
    target: String,
  },
  File {
    executable: bool,
    downloads: Downloads,
  },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Downloads {
  pub raw: super::minecraft::Download,
  pub lzma: Option<super::minecraft::Download>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Files {
  files: HashMap<String, File>,
}
