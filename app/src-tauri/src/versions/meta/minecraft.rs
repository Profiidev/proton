use std::{collections::HashMap, fmt::Display};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{value::Serializer, Value};
use tauri::Url;

use super::{
  java::{JavaVersion, Library},
  Rule,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
  pub latest: LatestVersion,
  pub versions: Vec<ManifestVersion>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LatestVersion {
  pub release: String,
  pub snapshot: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
  pub id: String,
  pub r#type: VersionType,
  pub url: Url,
  pub time: DateTime<Utc>,
  pub release_time: DateTime<Utc>,
  pub sha1: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
  Release,
  Snapshot,
  OldBeta,
  OldAlpha,
}

impl Display for VersionType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let value = serde_json::to_value(self).unwrap();
    let Value::String(value) = value.serialize(Serializer).unwrap() else {
      unreachable!()
    };

    write!(f, "{}", value)
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
  pub id: String,
  #[serde(default)]
  pub arguments: Arguments,
  pub asset_index: AssetIndex,
  pub downloads: Downloads,
  #[serde(default)]
  pub java_version: JavaVersion,
  pub libraries: Vec<Library>,
  pub main_class: String,
  pub r#type: VersionType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Downloads {
  pub client: Download,
  pub server: Option<Download>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Download {
  pub url: Url,
  pub size: usize,
  pub sha1: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
  pub id: String,
  pub sha1: String,
  pub size: usize,
  pub total_size: usize,
  pub url: Url,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Arguments {
  pub game: Vec<Argument>,
  pub jvm: Vec<Argument>,
}

impl Default for Arguments {
  fn default() -> Self {
    Arguments {
      game: vec![
        Argument::String("--username".to_string()),
        Argument::String("${auth_player_name}".to_string()),
        Argument::String("--version".to_string()),
        Argument::String("${version_name}".to_string()),
        Argument::String("--gameDir".to_string()),
        Argument::String("${game_directory}".to_string()),
        Argument::String("--assetsDir".to_string()),
        Argument::String("${assets_root}".to_string()),
        Argument::String("--assetIndex".to_string()),
        Argument::String("${assets_index_name}".to_string()),
        Argument::String("--uuid".to_string()),
        Argument::String("${auth_uuid}".to_string()),
        Argument::String("--accessToken".to_string()),
        Argument::String("${auth_access_token}".to_string()),
        Argument::String("--clientId".to_string()),
        Argument::String("${clientid}".to_string()),
        Argument::String("--xuid".to_string()),
        Argument::String("${auth_xuid}".to_string()),
        Argument::String("--userType".to_string()),
        Argument::String("${user_type}".to_string()),
        Argument::String("--versionType".to_string()),
        Argument::String("${version_type}".to_string()),
      ],
      jvm: vec![
        Argument::String("-Djava.library.path=${natives_directory}".to_string()),
        Argument::String("-Djna.tmpdir=${natives_directory}".to_string()),
        Argument::String(
          "-Dorg.lwjgl.system.SharedLibraryExtractPath=${natives_directory}".to_string(),
        ),
        Argument::String("-Dio.netty.native.workdir=${natives_directory}".to_string()),
        Argument::String("-Dminecraft.launcher.brand=${launcher_name}".to_string()),
        Argument::String("-Dminecraft.launcher.version=${launcher_version}".to_string()),
        Argument::String("-cp".to_string()),
        Argument::String("${classpath}".to_string()),
      ],
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Argument {
  String(String),
  Object(ArgumentRule),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentRule {
  pub rules: Vec<Rule>,
  pub value: ArgumentValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ArgumentValue {
  String(String),
  List(Vec<String>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Assets {
  pub objects: HashMap<String, Asset>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
  pub hash: String,
  pub size: usize,
}
