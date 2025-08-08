use std::{
  ffi::OsString,
  path::{Path, PathBuf},
  process::Stdio,
};

use anyhow::Result;
use log::debug;
use tokio::process::{Child, Command};

use crate::{
  CLIENT_ID, path,
  utils::file::read_parse_file,
  versions::{
    QUICK_PLAY, check_feature,
    loader::LoaderVersion,
    maven::{full_path_from_maven, parse_maven_name},
    meta::{Features, minecraft::ArgumentValue},
  },
};

use super::{
  ASSETS_DIR, JAVA_DIR, LIBRARY_DIR, MC_DIR, SEPARATOR, VERSION_DIR, check_rule,
  meta::{
    java::{Download, Library},
    minecraft::{Argument, Version},
  },
};

#[cfg(all(not(debug_assertions), target_os = "windows"))]
const DETACHED_PROCESS: u32 = 0x00000008;

pub struct LaunchArgs {
  pub launcher_version: String,
  pub launcher_name: String,
  pub player_name: String,
  pub player_uuid: String,
  pub user_type: String,
  pub access_token: String,
  pub data_dir: PathBuf,
  pub version: String,
  pub working_sub_dir: String,
  pub quick_play: Option<QuickPlay>,
  pub loader: Option<Box<dyn LoaderVersion>>,
}

pub enum QuickPlay {
  Singleplayer { world_name: String },
  Multiplayer { uri: String },
  Realms { realm_id: String },
}

impl LaunchArgs {
  fn replace_vars(&self, version: &Version, arg: &str, classpath: &str) -> String {
    let mut quick_singleplayer = String::new();
    let mut quick_multiplayer = String::new();
    let mut quick_realms = String::new();

    if let Some(quick_play) = &self.quick_play {
      match quick_play {
        QuickPlay::Singleplayer { world_name } => {
          quick_singleplayer = world_name.clone();
        }
        QuickPlay::Multiplayer { uri } => {
          quick_multiplayer = uri.clone();
        }
        QuickPlay::Realms { realm_id } => {
          quick_realms = realm_id.clone();
        }
      }
    }

    arg
      .replace("${clientid}", CLIENT_ID)
      .replace("${auth_player_name}", &self.player_name)
      .replace("${auth_uuid}", &self.player_uuid)
      .replace("${user_type}", &self.user_type)
      .replace("${auth_access_token}", &self.access_token)
      .replace("${auth_xuid}", "0")
      .replace(
        "${game_directory}",
        &path!(&self.data_dir, &self.working_sub_dir)
          .display()
          .to_string(),
      )
      .replace("${version_name}", &self.version)
      .replace(
        "${assets_root}",
        &path!(&self.data_dir, MC_DIR, ASSETS_DIR)
          .display()
          .to_string(),
      )
      .replace("${assets_index_name}", &version.asset_index.id)
      .replace("${version_type}", &version.r#type.to_string())
      .replace("${launcher_version}", &self.launcher_version)
      .replace("${launcher_name}", &self.launcher_name)
      .replace(
        "${natives_directory}",
        &path!(
          &self.data_dir,
          JAVA_DIR,
          &version.java_version.component.to_string(),
          LIBRARY_DIR
        )
        .display()
        .to_string(),
      )
      .replace("${classpath}", classpath)
      .replace("${quickPlayPath}", QUICK_PLAY)
      .replace("${quickPlaySingleplayer}", &quick_singleplayer)
      .replace("${quickPlayMultiplayer}", &quick_multiplayer)
      .replace("${quickPlayRealms}", &quick_realms)
  }

  async fn classpath(&self, version: &Version) -> Result<String> {
    classpath(
      version,
      &path!(&self.data_dir, MC_DIR),
      &self.data_dir,
      &self.loader,
    )
    .await?
    .into_string()
    .map_err(|e| anyhow::anyhow!("Failed to convert classpath OsString to String: {:?}", e))
  }
}

pub async fn launch_minecraft_version(args: &LaunchArgs) -> Result<Child> {
  debug!(
    "Collecting args to start minecraft version: {}",
    &args.version
  );
  let path = path!(
    &args.data_dir,
    MC_DIR,
    VERSION_DIR,
    &args.version,
    format!("{}.json", args.version)
  );
  let version: Version = read_parse_file(&path).await?;
  let classpath = args.classpath(&version).await?;

  let jvm_args = jvm_args(args, &version, &classpath);
  let game_args = game_args(args, &version, &classpath);

  let main_class = if let Some(loader) = &args.loader {
    loader.main_class(&args.data_dir).await?
  } else {
    version.main_class.clone()
  };

  let game_path = path!(&args.data_dir, &args.working_sub_dir);
  let java_component = &version.java_version.component;
  #[cfg(target_family = "unix")]
  let jre_bin = path!(
    &args.data_dir,
    JAVA_DIR,
    java_component.to_string(),
    "bin",
    "java"
  );
  #[cfg(target_family = "windows")]
  let jre_bin = path!(
    &args.data_dir,
    JAVA_DIR,
    java_component.to_string(),
    "bin",
    "java.exe"
  );

  let mut command = Command::new(jre_bin);

  #[cfg(all(not(debug_assertions), target_os = "windows"))]
  Command::creation_flags(&mut command, DETACHED_PROCESS);

  command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .current_dir(game_path)
    .args(jvm_args)
    .arg(main_class)
    .args(game_args);
  debug!("Spawning minecraft with command: {command:?}");

  Ok(command.spawn()?)
}

fn jvm_args(args: &LaunchArgs, version: &Version, classpath: &str) -> Vec<String> {
  let mut jvm_args = Vec::new();

  for arg in &version.arguments.jvm {
    if let Argument::String(arg) = arg {
      jvm_args.push(args.replace_vars(version, arg, classpath));
    }
  }

  jvm_args
}

fn game_args(args: &LaunchArgs, version: &Version, classpath: &str) -> Vec<String> {
  let mut game_args = Vec::new();

  let mut features = Features {
    has_quick_plays_support: Some(true),
    ..Default::default()
  };

  if let Some(quick_play) = &args.quick_play {
    match quick_play {
      QuickPlay::Singleplayer { .. } => {
        features.is_quick_play_singleplayer = Some(true);
      }
      QuickPlay::Multiplayer { .. } => {
        features.is_quick_play_multiplayer = Some(true);
      }
      QuickPlay::Realms { .. } => {
        features.is_quick_play_realms = Some(true);
      }
    }
  }

  for arg in &version.arguments.game {
    match arg {
      Argument::String(s) => game_args.push(args.replace_vars(version, s, classpath)),
      Argument::Object(arg) => {
        if arg.rules.iter().all(|rule| check_feature(rule, &features)) {
          match &arg.value {
            ArgumentValue::List(list) => {
              for s in list {
                game_args.push(args.replace_vars(version, s, classpath));
              }
            }
            ArgumentValue::String(s) => {
              game_args.push(args.replace_vars(version, s, classpath));
            }
          }
        }
      }
    }
  }

  game_args.push("--userProperties".into());
  game_args.push("{}".into());

  game_args
}

async fn classpath(
  version: &Version,
  mc_dir: &PathBuf,
  data_dir: &Path,
  loader: &Option<Box<dyn LoaderVersion>>,
) -> Result<OsString> {
  let mut classpath = OsString::new();
  classpath.push(path!(
    mc_dir,
    VERSION_DIR,
    &version.id,
    format!("{}.jar", version.id)
  ));

  let mut libraries = Vec::new();
  'l: for lib in &version.libraries {
    match lib {
      Library {
        downloads: Some(Download {
          artifact: Some(artifact),
          ..
        }),
        rules,
        ..
      } => {
        if let Some(rules) = rules {
          for rule in rules {
            if !check_rule(rule) {
              continue 'l;
            }
          }
        }

        let path = path!(mc_dir, LIBRARY_DIR, &artifact.path);
        libraries.push((parse_maven_name(&lib.name), path));
      }
      lib => {
        let maven = parse_maven_name(&lib.name);
        let path = full_path_from_maven(data_dir, &maven);
        libraries.push((maven, path));
      }
    }
  }

  if let Some(loader) = loader {
    let loader_libs = loader.classpath(data_dir).await?;
    libraries.retain(|(l, _)| {
      !loader_libs
        .iter()
        .any(|(ll, _)| l.group == ll.group && l.artifact == ll.artifact)
    });
    libraries.extend(loader_libs);
  }

  for (_, path) in libraries {
    classpath.push(SEPARATOR);
    classpath.push(path);
  }

  Ok(classpath)
}
