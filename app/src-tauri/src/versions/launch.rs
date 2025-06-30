use std::{
  ffi::OsString,
  path::PathBuf,
  process::{Child, Command, Stdio},
};

use anyhow::Result;
use log::debug;

use crate::{path, utils::file::read_parse_file, CLIENT_ID};

use super::{
  check_rule,
  meta::{
    java::{Download, Library},
    minecraft::{Argument, Version},
  },
  ASSETS_DIR, JAVA_DIR, LIBRARY_DIR, MC_DIR, NATIVE_DIR, SEPARATOR, VERSION_DIR,
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
}

impl LaunchArgs {
  fn replace_vars(&self, version: &Version, arg: &str) -> String {
    let classpath = if arg.contains("${classpath}") {
      classpath(version, &path!(&self.data_dir, MC_DIR))
        .into_string()
        .expect("Invalid Classpath")
    } else {
      String::new()
    };

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
        &path!(&self.data_dir, MC_DIR, NATIVE_DIR)
          .display()
          .to_string(),
      )
      .replace("${classpath}", &classpath)
  }
}

pub fn launch_minecraft_version(args: &LaunchArgs) -> Result<Child> {
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
  let version: Version = read_parse_file(&path)?;

  let jvm_args = jvm_args(args, &version);
  let game_args = game_args(args, &version);

  let game_path = path!(&args.data_dir, &args.working_sub_dir);
  let main_class = &version.main_class;
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
  std::os::windows::process::CommandExt::creation_flags(&mut command, DETACHED_PROCESS);

  command
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .current_dir(game_path)
    .args(jvm_args)
    .arg(main_class)
    .args(game_args);
  debug!("Spawning minecraft with command: {command:?}");

  Ok(command.spawn()?)
}

fn jvm_args(args: &LaunchArgs, version: &Version) -> Vec<String> {
  let mut jvm_args = Vec::new();

  for arg in &version.arguments.jvm {
    if let Argument::String(arg) = arg {
      jvm_args.push(args.replace_vars(version, arg));
    }
  }

  jvm_args
}

fn game_args(args: &LaunchArgs, version: &Version) -> Vec<String> {
  let mut jvm_args = Vec::new();

  for arg in &version.arguments.game {
    if let Argument::String(arg) = arg {
      jvm_args.push(args.replace_vars(version, arg));
    }
  }

  jvm_args.push("--userProperties".into());
  jvm_args.push("{}".into());

  jvm_args
}

fn classpath(version: &Version, mc_dir: &PathBuf) -> OsString {
  let mut classpath = OsString::new();
  classpath.push(path!(
    mc_dir,
    VERSION_DIR,
    &version.id,
    format!("{}.jar", version.id)
  ));

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

        classpath.push(SEPARATOR);
        let path = path!(mc_dir, LIBRARY_DIR, &artifact.path);
        classpath.push(path);
      }
      lib => {
        let mut original_name: &str = &lib.name;
        let mut name = "";
        let mut version = "";
        let mut path = path!();

        if let Some(i) = original_name.find(":") {
          let mut paths = &original_name[..i];
          original_name = &original_name[(i + 1)..];
          while let Some(i) = paths.find(".") {
            path = path!(path, &paths[..i]);
            paths = &paths[(i + 1)..];
          }
          path = path!(path, &paths);
        }
        if let Some(i) = original_name.find(":") {
          name = &original_name[..i];
          version = &original_name[(i + 1)..];
        }
        classpath.push(SEPARATOR);
        let path = path!(
          mc_dir,
          LIBRARY_DIR,
          &path,
          name,
          version,
          format!("{name}-{version}.jar")
        );
        classpath.push(path);
      }
    }
  }

  classpath
}
