use std::{collections::HashMap, fmt::Display};

use serde::Deserialize;
use url::Url;

use super::{
  Rule,
  java::{JavaVersion, Library},
};

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
  pub versions: Vec<ManifestVersion>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
  pub id: String,
  pub r#type: VersionType,
  pub url: Url,
  pub sha1: String,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
  Release,
  Snapshot,
  OldBeta,
  OldAlpha,
}

impl Display for VersionType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      Self::Release => "release",
      Self::Snapshot => "snapshot",
      Self::OldBeta => "old_beta",
      Self::OldAlpha => "old_alpha",
    })
  }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Downloads {
  pub client: Download,
  pub server: Option<Download>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Download {
  pub url: Url,
  pub size: usize,
  pub sha1: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndex {
  pub id: String,
  pub sha1: String,
  pub size: usize,
  pub total_size: usize,
  pub url: Url,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Argument {
  String(String),
  Object(ArgumentRule),
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentRule {
  pub rules: Vec<Rule>,
  pub value: ArgumentValue,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ArgumentValue {
  String(String),
  List(Vec<String>),
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Assets {
  pub objects: HashMap<String, Asset>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
  pub hash: String,
  pub size: usize,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::meta::java::Component;

  const LEGACY: &str = r#"{
    "id": "1.12.2",
    "assetIndex": { "id": "1.12", "sha1": "a", "size": 1, "totalSize": 2, "url": "https://x.invalid/i.json" },
    "downloads": { "client": { "url": "https://x.invalid/c.jar", "size": 3, "sha1": "c" } },
    "libraries": [],
    "mainClass": "net.minecraft.client.main.Main",
    "minecraftArguments": "--username ${auth_player_name}",
    "type": "release"
  }"#;

  #[test]
  fn legacy_versions_get_default_arguments_and_java_8() {
    let version: Version = serde_json::from_str(LEGACY).unwrap();
    assert_eq!(version.arguments, Arguments::default());
    assert_eq!(version.java_version.component, Component::JreLegacy);
    assert_eq!(version.java_version.major_version, 8);
    assert_eq!(version.downloads.server, None);

    // the defaults pair every flag with its placeholder
    let game = &version.arguments.game;
    assert_eq!(game.len() % 2, 0);
    for pair in game.chunks(2) {
      let [Argument::String(flag), Argument::String(value)] = pair else {
        panic!("{pair:?}");
      };
      assert!(
        flag.starts_with("--") && value.starts_with("${"),
        "{flag} {value}"
      );
    }
    let jvm = &version.arguments.jvm;
    assert_eq!(jvm[jvm.len() - 2], Argument::String("-cp".into()));
    assert_eq!(jvm[jvm.len() - 1], Argument::String("${classpath}".into()));
  }

  #[test]
  fn arguments_mix_strings_and_rule_objects() {
    let args: Arguments = serde_json::from_str(
      r#"{
        "game": ["--demo", { "rules": [{ "action": "allow", "features": { "is_demo_user": true } }], "value": "--x" }],
        "jvm": [{ "rules": [{ "action": "allow", "os": { "name": "osx" } }], "value": ["-XstartOnFirstThread", "-Da"] }]
      }"#,
    )
    .unwrap();
    assert_eq!(args.game[0], Argument::String("--demo".into()));
    let Argument::Object(rule) = &args.game[1] else {
      panic!()
    };
    assert_eq!(rule.value, ArgumentValue::String("--x".into()));
    let Argument::Object(rule) = &args.jvm[0] else {
      panic!()
    };
    assert_eq!(
      rule.value,
      ArgumentValue::List(vec!["-XstartOnFirstThread".into(), "-Da".into()])
    );
  }

  #[test]
  fn version_types_display_as_their_serde_names() {
    for (name, ty) in [
      ("release", VersionType::Release),
      ("snapshot", VersionType::Snapshot),
      ("old_beta", VersionType::OldBeta),
      ("old_alpha", VersionType::OldAlpha),
    ] {
      assert_eq!(ty.to_string(), name);
      assert_eq!(
        serde_json::from_value::<VersionType>(name.into()).unwrap(),
        ty
      );
    }
    // a new mojang type breaks the manifest instead of being dropped silently
    assert!(serde_json::from_value::<VersionType>("experiment".into()).is_err());
  }

  #[test]
  fn missing_required_fields_fail_to_parse() {
    let mut json: serde_json::Value = serde_json::from_str(LEGACY).unwrap();
    json.as_object_mut().unwrap().remove("mainClass");
    assert!(serde_json::from_value::<Version>(json).is_err());
    assert!(serde_json::from_str::<Manifest>("{}").is_err());
    assert!(serde_json::from_str::<Manifest>("not json").is_err());
  }
}
