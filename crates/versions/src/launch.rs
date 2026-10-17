use std::{
  collections::{HashMap, HashSet},
  ffi::OsString,
  path::PathBuf,
  process::Stdio,
};

use anyhow::Result;
use tokio::process::{Child, Command};
use tracing::debug;

use crate::{
  check_feature,
  loader::{Arguments, ClasspathEntry, LoaderVersion},
  maven::MavenArtifact,
  meta::{Features, minecraft::ArgumentValue},
  paths::{JavaVersionPath, MCPath, MCVersionPath, QUICK_PLAY},
};
use proton_config::settings::{GameSettings, JvmSettings};
use proton_utils::CLIENT_ID;
use proton_utils::file::read_parse_file;
use proton_utils::path;

#[cfg(all(not(debug_assertions), target_os = "windows"))]
use super::DETACHED_PROCESS;
use super::{
  SEPARATOR, check_rule,
  meta::{
    java::{Download, Library},
    minecraft::{Argument, Version},
  },
};

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
  pub game_settings: GameSettings,
  pub jvm_settings: JvmSettings,
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

    let mc_path = MCPath::new(&self.data_dir);

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
        &mc_path.assets_path().display().to_string(),
      )
      .replace("${assets_index_name}", &version.asset_index.id)
      .replace("${version_type}", &version.r#type.to_string())
      .replace("${launcher_version}", &self.launcher_version)
      .replace("${launcher_name}", &self.launcher_name)
      .replace(
        "${natives_directory}",
        &JavaVersionPath::new(
          &self.data_dir,
          version.java_version.component,
          self.version.clone(),
        )
        .native_path()
        .display()
        .to_string(),
      )
      .replace("${classpath}", classpath)
      .replace("${quickPlayPath}", QUICK_PLAY)
      .replace("${quickPlaySingleplayer}", &quick_singleplayer)
      .replace("${quickPlayMultiplayer}", &quick_multiplayer)
      .replace("${quickPlayRealms}", &quick_realms)
      .replace(
        "${library_directory}",
        &mc_path.library_path().display().to_string(),
      )
      .replace("${classpath_separator}", SEPARATOR)
      .replace("${user_properties}", "{}")
      .replace("${resolution_width}", &self.game_settings.width.to_string())
      .replace(
        "${resolution_height}",
        &self.game_settings.height.to_string(),
      )
  }

  async fn classpath(
    &self,
    version: &Version,
    mc_path: &MCPath,
    version_path: &MCVersionPath,
  ) -> Result<String> {
    classpath(version, mc_path, version_path, &self.loader)
      .await?
      .into_string()
      .map_err(|e| anyhow::anyhow!("Failed to convert classpath OsString to String: {:?}", e))
  }
}

pub async fn launch_minecraft_version(args: &LaunchArgs) -> Result<Child> {
  debug!(
    "Collecting args to start minecraft version: {}",
    args.version
  );
  let version_path = MCVersionPath::new(&args.data_dir, &args.version);
  let version: Version = read_parse_file(&version_path.version_manifest()).await?;
  let mc_path = MCPath::new(&args.data_dir);

  let java_path = JavaVersionPath::new(
    &args.data_dir,
    version.java_version.component,
    args.version.clone(),
  );
  let classpath = args.classpath(&version, &mc_path, &version_path).await?;

  let mut jvm_args = jvm_args(args, &version, &classpath);
  let mut game_args = game_args(args, &version, &classpath);
  let mut env_vars = HashMap::new();

  if let Some(loader) = &args.loader {
    debug!("Adding loader arguments to JVM args");
    let Arguments {
      jvm_args: loader_jvm_args,
      game_args: loader_game_args,
      overwrite_game_args,
    } = loader.arguments(&version_path).await?;

    for arg in &loader_jvm_args {
      jvm_args.push(args.replace_vars(&version, arg, &classpath));
    }

    if overwrite_game_args {
      game_args.clear();
    }
    for arg in &loader_game_args {
      game_args.push(args.replace_vars(&version, arg, &classpath));
    }
  }

  // custom jvm settings
  jvm_args.push(format!("-Xmx{}M", args.jvm_settings.mem_max));
  jvm_args.extend(args.jvm_settings.args.clone());
  env_vars.extend(args.jvm_settings.env_vars.clone());

  let main_class = if let Some(loader) = &args.loader {
    loader.main_class(&version_path).await?
  } else {
    version.main_class.clone()
  };

  let game_path = path!(&args.data_dir, &args.working_sub_dir);
  let jre_bin = java_path.bin_path();

  let mut command = Command::new(jre_bin);

  #[cfg(all(not(debug_assertions), target_os = "windows"))]
  Command::creation_flags(&mut command, DETACHED_PROCESS);

  command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .current_dir(game_path)
    .envs(env_vars)
    .args(jvm_args)
    .arg(main_class)
    .args(game_args);

  let command_fmt = format!("{command:?}");
  debug!(
    "Spawning minecraft with command: {}",
    command_fmt.replace(&args.access_token, "**REDACTED**")
  );

  Ok(command.spawn()?)
}

fn jvm_args(args: &LaunchArgs, version: &Version, classpath: &str) -> Vec<String> {
  let mut jvm_args = Vec::new();

  for arg in &version.arguments.jvm {
    match arg {
      Argument::String(arg) => jvm_args.push(args.replace_vars(version, arg, classpath)),
      // os gated, e.g. -XstartOnFirstThread on macOS
      Argument::Object(arg) if arg.rules.iter().all(check_rule) => match &arg.value {
        ArgumentValue::List(list) => jvm_args.extend(
          list
            .iter()
            .map(|s| args.replace_vars(version, s, classpath)),
        ),
        ArgumentValue::String(s) => jvm_args.push(args.replace_vars(version, s, classpath)),
      },
      Argument::Object(_) => {}
    }
  }

  jvm_args
}

fn game_args(args: &LaunchArgs, version: &Version, classpath: &str) -> Vec<String> {
  let mut game_args = Vec::new();

  let mut features = Features {
    has_quick_plays_support: Some(true),
    has_custom_resolution: Some(args.game_settings.use_custom),
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
  mc_path: &MCPath,
  version_path: &MCVersionPath,
  loader: &Option<Box<dyn LoaderVersion>>,
) -> Result<OsString> {
  let mut classpath = OsString::new();
  classpath.push(version_path.client_jar());

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

        let path = path!(mc_path.library_path(), &artifact.path);
        libraries.push(ClasspathEntry::new(MavenArtifact::new(&lib.name)?, path));
      }
      lib => {
        let maven = MavenArtifact::new(&lib.name)?;
        let path = maven.full_path(mc_path);
        libraries.push(ClasspathEntry::new(maven, path));
      }
    }
  }

  if let Some(loader) = loader {
    let loader_libs = loader.classpath(version_path, mc_path).await?;
    libraries.retain(|l| {
      !loader_libs.iter().any(|ll| {
        l.artifact.group == ll.artifact.group && l.artifact.artifact == ll.artifact.artifact
      })
    });
    libraries.extend(loader_libs);
  }

  let mut add_libs = HashSet::new();
  for ClasspathEntry { path, .. } in libraries {
    if add_libs.contains(&path) {
      continue; // Skip already added libraries
    }
    classpath.push(SEPARATOR);
    classpath.push(&path);
    add_libs.insert(path);
  }

  Ok(classpath)
}

#[cfg(test)]
mod tests {
  use proton_utils::rt;

  use super::*;
  use crate::{
    meta::java::Component,
    testing::{fake_java, java_calls},
  };

  fn artifact(path: &str) -> serde_json::Value {
    serde_json::json!({ "artifact": { "path": path, "url": "https://x.invalid/a.jar", "sha1": "a", "size": 1 } })
  }

  fn quick(feature: &str, flag: &str, var: &str) -> serde_json::Value {
    serde_json::json!({
      "rules": [{ "action": "allow", "features": { feature: true } }],
      "value": [flag, var]
    })
  }

  fn version_json() -> serde_json::Value {
    serde_json::json!({
      "id": "1.20.1",
      "arguments": {
        "game": [
          "--username", "${auth_player_name}", "--version", "${version_name}",
          "--gameDir", "${game_directory}", "--assetsDir", "${assets_root}",
          "--assetIndex", "${assets_index_name}", "--uuid", "${auth_uuid}",
          "--accessToken", "${auth_access_token}", "--clientId", "${clientid}",
          "--xuid", "${auth_xuid}", "--userType", "${user_type}", "--versionType", "${version_type}",
          { "rules": [{ "action": "allow", "features": { "is_demo_user": true } }], "value": "--demo" },
          {
            "rules": [{ "action": "allow", "features": { "has_custom_resolution": true } }],
            "value": ["--width", "${resolution_width}", "--height", "${resolution_height}"]
          },
          quick("has_quick_plays_support", "--quickPlayPath", "${quickPlayPath}"),
          quick("is_quick_play_singleplayer", "--quickPlaySingleplayer", "${quickPlaySingleplayer}"),
          quick("is_quick_play_multiplayer", "--quickPlayMultiplayer", "${quickPlayMultiplayer}"),
          quick("is_quick_play_realms", "--quickPlayRealms", "${quickPlayRealms}")
        ],
        "jvm": [
          { "rules": [{ "action": "allow", "os": { "name": "osx" } }], "value": ["-XstartOnFirstThread"] },
          { "rules": [{ "action": "allow", "os": { "name": "linux" } }], "value": "-Dlinux=yes" },
          { "rules": [{ "action": "allow", "os": { "arch": "x86" } }], "value": "-Xss1M" },
          "-Djava.library.path=${natives_directory}",
          "-Dminecraft.launcher.brand=${launcher_name}",
          "-Dminecraft.launcher.version=${launcher_version}",
          "-Dsep=${classpath_separator}",
          "-Dlib=${library_directory}",
          "-Dunknown=${not_a_var}",
          "-cp", "${classpath}"
        ]
      },
      "assetIndex": { "id": "5", "sha1": "a", "size": 1, "totalSize": 1, "url": "https://x.invalid/i.json" },
      "downloads": { "client": { "url": "https://x.invalid/c.jar", "size": 1, "sha1": "c" } },
      "javaVersion": { "component": "java-runtime-gamma", "majorVersion": 17 },
      "libraries": [
        { "name": "a.b:with-artifact:1", "downloads": artifact("custom/place.jar") },
        {
          "name": "a.b:mac-only:1", "downloads": artifact("mac.jar"),
          "rules": [{ "action": "allow", "os": { "name": "osx" } }]
        },
        { "name": "a.b:no-downloads:1" },
        { "name": "org.ow2.asm:asm:9.3", "downloads": artifact("org/ow2/asm/asm/9.3/asm-9.3.jar") },
        // same jar twice is on the classpath once
        { "name": "a.b:with-artifact:1", "downloads": artifact("custom/place.jar") }
      ],
      "mainClass": "net.minecraft.client.main.Main",
      "type": "release"
    })
  }

  struct Env {
    _dir: tempfile::TempDir,
    data: PathBuf,
  }

  fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("dätä dir");
    std::fs::create_dir_all(data.join("profiles/my profile")).unwrap();
    let manifest = MCVersionPath::new(&data, "1.20.1").version_manifest();
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    std::fs::write(&manifest, version_json().to_string()).unwrap();
    Env { _dir: dir, data }
  }

  impl Env {
    fn args(&self) -> LaunchArgs {
      LaunchArgs {
        launcher_version: "1.0.0".into(),
        launcher_name: "proton".into(),
        player_name: "Stëve".into(),
        player_uuid: "uuid".into(),
        user_type: "msa".into(),
        access_token: "secret token".into(),
        data_dir: self.data.clone(),
        version: "1.20.1".into(),
        working_sub_dir: "profiles/my profile".into(),
        quick_play: None,
        loader: None,
        game_settings: GameSettings::default(),
        jvm_settings: JvmSettings::default(),
      }
    }

    fn java(&self, code: i32) -> PathBuf {
      let java = JavaVersionPath::new(&self.data, Component::JavaRuntimeGamma, "1.20.1".into());
      fake_java(&java.bin_path(), code)
    }

    fn p(&self, rest: &str) -> String {
      self.data.join(rest).display().to_string()
    }
  }

  fn version() -> Version {
    serde_json::from_value(version_json()).unwrap()
  }

  /// Launches and waits, returns the exit code and the recorded args
  fn launch(env: &Env, args: &LaunchArgs, code: i32) -> (Option<i32>, Vec<String>) {
    let record = env.java(code);
    let status = rt::block_on(async {
      let child = launch_minecraft_version(args).await.unwrap();
      child.wait_with_output().await.unwrap().status
    });
    let mut calls = java_calls(&record);
    assert_eq!(calls.len(), 1);
    (status.code(), calls.remove(0))
  }

  #[cfg(target_os = "linux")]
  #[test]
  fn vanilla_launch_passes_every_placeholder() {
    let env = env();
    let mut args = env.args();
    args.quick_play = Some(QuickPlay::Singleplayer {
      world_name: "Wörld 1".into(),
    });
    args.game_settings = GameSettings {
      use_custom: true,
      width: 1280,
      height: 720,
    };
    args.jvm_settings = JvmSettings {
      args: vec!["-Dcustom=a b".into()],
      env_vars: [("PROTON_TEST_ENV".to_string(), "hi".to_string())].into(),
      mem_max: 4096,
    };

    let (code, call) = launch(&env, &args, 0);
    assert_eq!(code, Some(0));
    let classpath = [
      env.p("minecraft/versions/1.20.1/1.20.1.jar"),
      env.p("minecraft/lib/custom/place.jar"),
      env.p("minecraft/lib/a/b/no-downloads/1/no-downloads-1.jar"),
      env.p("minecraft/lib/org/ow2/asm/asm/9.3/asm-9.3.jar"),
    ]
    .join(":");
    let expected = [
      format!("cwd={}", env.p("profiles/my profile")),
      "env=hi".into(),
      // os gated jvm args only for this os
      "-Dlinux=yes".into(),
      format!(
        "-Djava.library.path={}",
        env.p("java/java-runtime-gamma/lib")
      ),
      "-Dminecraft.launcher.brand=proton".into(),
      "-Dminecraft.launcher.version=1.0.0".into(),
      "-Dsep=:".into(),
      format!("-Dlib={}", env.p("minecraft/lib")),
      // unknown placeholders stay as they are
      "-Dunknown=${not_a_var}".into(),
      "-cp".into(),
      classpath,
      "-Xmx4096M".into(),
      "-Dcustom=a b".into(),
      "net.minecraft.client.main.Main".into(),
      "--username".into(),
      "Stëve".into(),
      "--version".into(),
      "1.20.1".into(),
      "--gameDir".into(),
      env.p("profiles/my profile"),
      "--assetsDir".into(),
      env.p("minecraft/assets"),
      "--assetIndex".into(),
      "5".into(),
      "--uuid".into(),
      "uuid".into(),
      "--accessToken".into(),
      "secret token".into(),
      "--clientId".into(),
      CLIENT_ID.into(),
      "--xuid".into(),
      "0".into(),
      "--userType".into(),
      "msa".into(),
      "--versionType".into(),
      "release".into(),
      "--width".into(),
      "1280".into(),
      "--height".into(),
      "720".into(),
      "--quickPlayPath".into(),
      QUICK_PLAY.into(),
      "--quickPlaySingleplayer".into(),
      "Wörld 1".into(),
      "--userProperties".into(),
      "{}".into(),
    ];
    assert_eq!(call, expected);
  }

  #[test]
  fn the_game_exit_code_reaches_the_caller() {
    let env = env();
    let (code, _) = launch(&env, &env.args(), 7);
    assert_eq!(code, Some(7));
  }

  #[test]
  fn quick_play_turns_on_only_its_own_flag() {
    let env = env();
    let version = version();
    let cases = [
      (None, None),
      (
        Some(QuickPlay::Singleplayer {
          world_name: "w".into(),
        }),
        Some(("--quickPlaySingleplayer", "w")),
      ),
      (
        Some(QuickPlay::Multiplayer {
          uri: "mc.example:25565".into(),
        }),
        Some(("--quickPlayMultiplayer", "mc.example:25565")),
      ),
      (
        Some(QuickPlay::Realms {
          realm_id: "42".into(),
        }),
        Some(("--quickPlayRealms", "42")),
      ),
    ];
    for (quick_play, expected) in cases {
      let mut args = env.args();
      args.quick_play = quick_play;
      let game = game_args(&args, &version, "cp");
      // custom resolution is off by default, the demo flag never set
      assert!(!game.contains(&"--width".into()) && !game.contains(&"--demo".into()));
      // the path flag is always there, the launcher supports quick play
      assert!(game.contains(&"--quickPlayPath".into()));
      let flags: Vec<_> = game
        .iter()
        .filter(|a| a.starts_with("--quickPlay") && *a != "--quickPlayPath")
        .collect();
      match expected {
        None => assert!(flags.is_empty(), "{flags:?}"),
        Some((flag, value)) => {
          assert_eq!(flags, [flag]);
          let at = game.iter().position(|a| a == flag).unwrap();
          assert_eq!(game[at + 1], value);
        }
      }
      assert_eq!(game[game.len() - 2..], ["--userProperties", "{}"]);
    }
  }

  #[test]
  fn legacy_versions_use_the_default_arguments() {
    let env = env();
    let mut json = version_json();
    json.as_object_mut().unwrap().remove("arguments");
    let version: Version = serde_json::from_value(json).unwrap();
    let args = env.args();
    let jvm = jvm_args(&args, &version, "the-cp");
    assert_eq!(jvm[jvm.len() - 2..], ["-cp", "the-cp"]);
    let game = game_args(&args, &version, "the-cp");
    assert!(game.iter().all(|a| !a.contains("${")), "{game:?}");
  }

  struct FakeLoader {
    overwrite: bool,
    fail: bool,
  }

  #[async_trait::async_trait]
  impl LoaderVersion for FakeLoader {
    async fn download(
      &self,
      _: &reqwest::Client,
      _: &MCVersionPath,
      _: &MCPath,
      _: &[String],
    ) -> Result<Vec<crate::loader::CheckFuture>> {
      Ok(vec![])
    }
    async fn preprocess(&self, _: &MCVersionPath, _: &MCPath, _: PathBuf) -> Result<()> {
      Ok(())
    }
    async fn classpath(&self, _: &MCVersionPath, mc: &MCPath) -> Result<Vec<ClasspathEntry>> {
      Ok(vec![
        ClasspathEntry::from_name("org.ow2.asm:asm:9.6", mc)?,
        ClasspathEntry::from_name("loader:core:1", mc)?,
      ])
    }
    async fn main_class(&self, _: &MCVersionPath) -> Result<String> {
      Ok("loader.Main".into())
    }
    async fn arguments(&self, _: &MCVersionPath) -> Result<Arguments> {
      anyhow::ensure!(!self.fail, "no loader args");
      Ok(Arguments::new(
        vec!["-Dloader=${version_name}".into()],
        vec!["--tweak".into(), "${auth_player_name}".into()],
        self.overwrite,
      ))
    }
  }

  #[test]
  fn loader_replaces_libraries_main_class_and_maybe_game_args() {
    let env = env();
    for overwrite in [false, true] {
      let mut args = env.args();
      args.loader = Some(Box::new(FakeLoader {
        overwrite,
        fail: false,
      }));
      let (_, call) = launch(&env, &args, 0);
      std::fs::remove_file(
        JavaVersionPath::new(&env.data, Component::JavaRuntimeGamma, "1.20.1".into())
          .bin_path()
          .with_extension("calls"),
      )
      .unwrap();

      let cp = &call[call.iter().position(|a| a == "-cp").unwrap() + 1];
      // the loader's asm wins over vanilla's, loader libs go last
      assert!(
        !cp.contains("asm-9.3.jar") && cp.contains("asm-9.6.jar"),
        "{cp}"
      );
      assert!(cp.ends_with(&env.p("minecraft/lib/loader/core/1/core-1.jar")));

      let main = call.iter().position(|a| a == "loader.Main").unwrap();
      assert_eq!(call[main - 2..main], ["-Dloader=1.20.1", "-Xmx2048M"]);
      let game = &call[main + 1..];
      assert_eq!(game[game.len() - 2..], ["--tweak", "Stëve"]);
      // overwriting drops the vanilla args, user properties included
      assert_eq!(game.contains(&"--username".to_string()), !overwrite);
      assert_eq!(game.contains(&"--userProperties".to_string()), !overwrite);
    }
  }

  #[test]
  fn launch_errors_are_returned() {
    let env = env();
    // no java installed yet
    let err = rt::block_on(launch_minecraft_version(&env.args())).unwrap_err();
    assert!(err.downcast_ref::<std::io::Error>().is_some(), "{err}");

    let record = env.java(0);
    let mut args = env.args();
    args.loader = Some(Box::new(FakeLoader {
      overwrite: false,
      fail: true,
    }));
    let err = rt::block_on(launch_minecraft_version(&args)).unwrap_err();
    assert_eq!(err.to_string(), "no loader args");

    // the version is not downloaded
    let mut args = env.args();
    args.version = "1.0".into();
    assert!(rt::block_on(launch_minecraft_version(&args)).is_err());

    let mut args = env.args();
    args.working_sub_dir = "profiles/missing".into();
    assert!(rt::block_on(launch_minecraft_version(&args)).is_err());
    assert!(java_calls(&record).is_empty());
  }
}
