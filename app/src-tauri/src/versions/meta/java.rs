use std::{collections::HashMap, fmt::Display};

use serde::{Deserialize, Serialize};
use serde_json::{value::Serializer, Value};
use tauri::Url;

use super::Rule;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
  pub component: Component,
  pub major_version: u16,
}

impl Default for JavaVersion {
  #[inline(always)]
  fn default() -> Self {
    Self {
      component: Component::JreLegacy,
      major_version: 8,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum Component {
  JavaRuntimeAlpha,
  JavaRuntimeBeta,
  JavaRuntimeDelta,
  JavaRuntimeGamma,
  JavaRuntimeGammaSnapshot,
  JreLegacy,
  #[serde(other)]
  Unknown,
}

impl Display for Component {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let value = serde_json::to_value(self).unwrap();
    let Value::String(value) = value.serialize(Serializer).unwrap() else {
      unreachable!()
    };

    write!(f, "{value}")
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
  pub files: HashMap<String, File>,
}
