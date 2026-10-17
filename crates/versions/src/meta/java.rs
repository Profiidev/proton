use std::{collections::HashMap, fmt::Display};

use serde::Deserialize;
use url::Url;

use super::Rule;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum Component {
  JavaRuntimeAlpha,
  JavaRuntimeBeta,
  JavaRuntimeDelta,
  JavaRuntimeEpsilon,
  JavaRuntimeGamma,
  JavaRuntimeGammaSnapshot,
  JreLegacy,
  #[serde(other)]
  Unknown,
}

impl Display for Component {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      Self::JavaRuntimeAlpha => "java-runtime-alpha",
      Self::JavaRuntimeBeta => "java-runtime-beta",
      Self::JavaRuntimeDelta => "java-runtime-delta",
      Self::JavaRuntimeEpsilon => "java-runtime-epsilon",
      Self::JavaRuntimeGamma => "java-runtime-gamma",
      Self::JavaRuntimeGammaSnapshot => "java-runtime-gamma-snapshot",
      Self::JreLegacy => "jre-legacy",
      Self::Unknown => "unknown",
    })
  }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Library {
  pub name: String,
  pub url: Option<Url>,
  pub downloads: Option<Download>,
  pub rules: Option<Vec<Rule>>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Download {
  pub artifact: Option<Artifact>,
  pub classifiers: Option<Classifiers>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct Classifiers {
  pub natives_linux: Option<Artifact>,
  pub natives_osx: Option<Artifact>,
  pub natives_windows: Option<Artifact>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
  pub url: Url,
  pub path: String,
  pub sha1: String,
  pub size: usize,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default)]
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub struct PlatformVersion {
  pub java_runtime_alpha: Vec<Version>,
  pub java_runtime_beta: Vec<Version>,
  pub java_runtime_delta: Vec<Version>,
  pub java_runtime_epsilon: Vec<Version>,
  pub java_runtime_gamma: Vec<Version>,
  pub java_runtime_gamma_snapshot: Vec<Version>,
  pub jre_legacy: Vec<Version>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
  pub manifest: super::minecraft::Download,
  pub version: VersionName,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VersionName {
  pub name: String,
  pub released: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Downloads {
  pub raw: super::minecraft::Download,
  pub lzma: Option<super::minecraft::Download>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Files {
  pub files: HashMap<String, File>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn components_display_as_their_serde_names() {
    let all = [
      Component::JavaRuntimeAlpha,
      Component::JavaRuntimeBeta,
      Component::JavaRuntimeDelta,
      Component::JavaRuntimeEpsilon,
      Component::JavaRuntimeGamma,
      Component::JavaRuntimeGammaSnapshot,
      Component::JreLegacy,
    ];
    for component in all {
      let name = component.to_string();
      assert_eq!(
        serde_json::from_value::<Component>(name.clone().into()).unwrap(),
        component,
        "{name}"
      );
    }
    // new runtimes parse as unknown, the download then reports not supported
    assert_eq!(
      serde_json::from_value::<Component>("java-runtime-zeta".into()).unwrap(),
      Component::Unknown
    );
    assert_eq!(Component::Unknown.to_string(), "unknown");
  }

  #[test]
  fn runtime_files_are_tagged_by_type() {
    let files: Files = serde_json::from_str(
      r#"{ "files": {
        "bin": { "type": "directory" },
        "lib/link": { "type": "link", "target": "../x" },
        "bin/java": { "type": "file", "executable": true, "downloads": {
          "raw": { "url": "https://x.invalid/java", "size": 1, "sha1": "a" },
          "lzma": { "url": "https://x.invalid/java.lzma", "size": 1, "sha1": "b" }
        } }
      } }"#,
    )
    .unwrap();
    assert_eq!(files.files["bin"], File::Directory);
    assert_eq!(
      files.files["lib/link"],
      File::Link {
        target: "../x".into()
      }
    );
    let File::File {
      executable,
      downloads,
    } = &files.files["bin/java"]
    else {
      panic!()
    };
    assert!(executable);
    assert_eq!(downloads.raw.sha1, "a");
    assert!(serde_json::from_str::<File>(r#"{ "type": "socket" }"#).is_err());
  }

  #[test]
  fn platform_lists_default_to_empty() {
    let versions = JavaVersions::default();
    assert!(versions.linux.java_runtime_gamma.is_empty());
    // mojang lists every platform, a missing one is an error
    assert!(serde_json::from_str::<JavaVersions>("{}").is_err());
  }
}
