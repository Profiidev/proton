use std::{
  cmp::Ordering,
  collections::{HashMap, HashSet},
  ffi::OsString,
  path::{Path, PathBuf},
  process::Stdio,
};

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use tokio::{fs, process::Command};
use tracing::debug;
use url::Url;

#[cfg(all(not(debug_assertions), target_os = "windows"))]
use crate::DETACHED_PROCESS;
use crate::{
  SEPARATOR,
  loader::{
    CheckFuture, ClasspathEntry, Loader, LoaderVersion,
    util::{
      compare_mc_versions, download_maven_future, extract_and_save_file_from_zip,
      extract_file_from_zip, main_class_from_jar,
    },
  },
  maven::MavenArtifact,
  paths::{MCPath, MCVersionPath},
};
use proton_utils::{
  download::download_file_no_hash,
  file::{read_parse_file, read_parse_xml_file},
};
use proton_utils::{http, path};

const INDEX_BASE_URL_FORGE: &str =
  "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const INDEX_BASE_URL_NEOFORGE: &str =
  "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const INSTALLER_URL_FORGE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/{loader_version}/forge-{loader_version}-installer.jar";
const INSTALLER_URL_NEOFORGE: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar";
const MAVEN_BASE_URL_FORGE: &str = "https://maven.minecraftforge.net";
const MAVEN_BASE_URL_NEOFORGE: &str = "https://maven.neoforged.net";
const INDEX_FILE_NAME_FORGE: &str = "forge";
const INDEX_FILE_NAME_NEOFORGE: &str = "neoforge";

pub struct ForgeLikeLoader {
  index_url: String,
  index_file_name: String,
}

impl ForgeLikeLoader {
  pub fn forge() -> Self {
    Self {
      index_url: http::url(INDEX_BASE_URL_FORGE).into_owned(),
      index_file_name: INDEX_FILE_NAME_FORGE.to_string(),
    }
  }

  pub fn neoforge() -> Self {
    Self {
      index_url: http::url(INDEX_BASE_URL_NEOFORGE).into_owned(),
      index_file_name: INDEX_FILE_NAME_NEOFORGE.to_string(),
    }
  }

  fn index(&self, version_path: &MCVersionPath) -> PathBuf {
    let filename = format!("{}-index.json", self.index_file_name);
    path!(version_path.version_root(), filename)
  }

  async fn neoforge_version_lists(&self, version_path: &MCVersionPath) -> Result<Vec<String>> {
    let path = self.index(version_path);
    let forge_versions = read_parse_xml_file::<NeoForgeIndex>(&path).await?;

    let forge_versions_parsed = forge_versions.versioning.versions.version;
    Ok(forge_versions_parsed)
  }

  async fn neoforge_version_pairs(
    &self,
    version_path: &MCVersionPath,
  ) -> Result<Vec<(String, String)>> {
    let versions = self.neoforge_version_lists(version_path).await?;

    let neoforge_versions_parsed = versions
      .into_iter()
      .flat_map(|v| neoforge_version_pair(&v))
      .collect::<Vec<_>>();

    Ok(neoforge_versions_parsed)
  }
}

#[async_trait::async_trait]
impl Loader for ForgeLikeLoader {
  async fn download_metadata(&self, client: &Client, version_path: &MCVersionPath) -> Result<()> {
    let url = Url::parse(&self.index_url)?;
    let path = self.index(version_path);
    download_file_no_hash(client, &path, url, true).await?;

    Ok(())
  }

  async fn supported_versions(
    &self,
    version_path: &MCVersionPath,
    stable: bool,
  ) -> Result<Vec<String>> {
    let path = self.index(version_path);
    if self.index_file_name == INDEX_FILE_NAME_FORGE {
      let versions = read_parse_file::<VersionIndex>(&path)
        .await?
        .keys()
        .filter(|v| {
          // all versions 1.5.1 and below are not supported because they require manual jar patching
          (!v.contains("pre") || !stable)
            && compare_mc_versions(&"1.5.1".to_string(), v) == Ordering::Less
        })
        .cloned()
        .collect::<Vec<_>>();

      Ok(versions)
    } else {
      let versions = self.neoforge_version_pairs(version_path).await?;
      let mut versions: Vec<String> = versions.into_iter().map(|(mc, _)| mc).collect();
      versions.sort();
      versions.dedup();

      Ok(versions)
    }
  }

  async fn loader_versions_for_mc_version(
    &self,
    mc_version: &str,
    version_path: &MCVersionPath,
    stable: bool,
  ) -> Result<Vec<String>> {
    let path = self.index(version_path);
    if self.index_file_name == INDEX_FILE_NAME_FORGE {
      let versions = read_parse_file::<VersionIndex>(&path)
        .await?
        .get(mc_version)
        .cloned()
        .unwrap_or_default();

      let mut versions: Vec<String> = versions
        .into_iter()
        .filter(|v| !v.contains("pre") || !stable)
        .flat_map(|v| anyhow::Ok(forge_version_pair(&v)?.1))
        .collect();
      versions.reverse();

      Ok(versions)
    } else {
      let versions = self.neoforge_version_pairs(version_path).await?;
      let mut versions: Vec<String> = versions
        .into_iter()
        .filter(|(mc, loader)| mc == mc_version && (!stable || !loader.contains("beta")))
        .map(|(_, neoforge)| neoforge)
        .collect();

      versions.reverse();

      Ok(versions)
    }
  }
}

pub struct ForgeLikeLoaderVersion {
  mc_version: String,
  loader_version: String,
  installer_base_url: String,
  index_file_name: String,
  maven_base_url: String,
}

impl ForgeLikeLoaderVersion {
  pub fn forge(mc_version: String, loader_version: String) -> Self {
    Self {
      mc_version,
      loader_version,
      installer_base_url: http::url(INSTALLER_URL_FORGE).into_owned(),
      index_file_name: INDEX_FILE_NAME_FORGE.to_string(),
      maven_base_url: http::url(MAVEN_BASE_URL_FORGE).into_owned(),
    }
  }

  pub fn neoforge(mc_version: String, loader_version: String) -> Self {
    Self {
      mc_version,
      loader_version,
      installer_base_url: http::url(INSTALLER_URL_NEOFORGE).into_owned(),
      index_file_name: INDEX_FILE_NAME_NEOFORGE.to_string(),
      maven_base_url: http::url(MAVEN_BASE_URL_NEOFORGE).into_owned(),
    }
  }

  async fn loader_version(&self, version_path: &MCVersionPath) -> Result<String> {
    Ok(if self.index_file_name == INDEX_FILE_NAME_FORGE {
      let loader = ForgeLikeLoader::forge();
      let path = loader.index(version_path);
      let versions = read_parse_file::<VersionIndex>(&path)
        .await?
        .get(&self.mc_version)
        .cloned()
        .unwrap_or_default();

      versions
        .into_iter()
        .find(|v| v.contains(&format!("{}-{}", self.mc_version, self.loader_version)))
        .ok_or_else(|| {
          anyhow::anyhow!(
            "Loader version {} not found for Minecraft version {}",
            self.loader_version,
            self.mc_version
          )
        })?
    } else {
      let loader = ForgeLikeLoader::neoforge();
      let neoforge = loader.neoforge_version_lists(version_path).await?;

      let mc_version_parts = self.mc_version.split('.').collect::<Vec<_>>();
      // first part of neoforge version are the major and minor version of Minecraft
      // e.g., "1.16.5" => "16.5", "1.16" => "16.0"
      let mc_version_part = if mc_version_parts.len() > 2 {
        format!("{}.{}", mc_version_parts[1], mc_version_parts[2])
      } else {
        format!("{}.0", mc_version_parts[1])
      };

      neoforge
        .into_iter()
        .find(|v| v.contains(&format!("{mc_version_part}.{}", self.loader_version)))
        .ok_or_else(|| {
          anyhow::anyhow!(
            "Loader version {} not found for Minecraft version {}",
            self.loader_version,
            self.mc_version
          )
        })?
    })
  }

  async fn installer_path(&self, version_path: &MCVersionPath) -> Result<PathBuf> {
    let loader_version = self.loader_version(version_path).await?;
    Ok(path!(
      version_path.base_path(),
      format!("{}-{}", self.index_file_name, loader_version),
    ))
  }

  async fn installer_url(&self, version_path: &MCVersionPath) -> Result<String> {
    let loader_version = self.loader_version(version_path).await?;
    let url = self
      .installer_base_url
      .replace("{loader_version}", &loader_version);
    Ok(url)
  }

  async fn version_json(&self, version_path: &MCVersionPath) -> Result<ForgeVersion> {
    let installer_path = self.installer_path(version_path).await?;
    Ok(
      if let Ok(data) = read_parse_file(&installer_path.join(VERSION_JSON_PATH)).await {
        data
      } else {
        read_parse_file::<ForgeVersionManifestOld>(&installer_path.join(INSTALLER_PROFILE_PATH))
          .await?
          .into_new()?
      },
    )
  }
}

const INSTALLER_PATH: &str = "installer.jar";
const INSTALLER_PROFILE_PATH: &str = "install_profile.json";
const VERSION_JSON_PATH: &str = "version.json";

#[async_trait::async_trait]
impl LoaderVersion for ForgeLikeLoaderVersion {
  async fn download(
    &self,
    client: &Client,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
    existing_libs: &[String],
  ) -> Result<Vec<CheckFuture>> {
    let installer_path = self.installer_path(version_path).await?;

    let url = Url::parse(&self.installer_url(version_path).await?)?;
    let path = installer_path.join(INSTALLER_PATH);
    download_file_no_hash(client, &path, url, true).await?;

    let profile_path = installer_path.join(INSTALLER_PROFILE_PATH);
    extract_and_save_file_from_zip(&path, INSTALLER_PROFILE_PATH, &profile_path).await?;

    let mut futures = Vec::new();
    let mut added_libs = HashSet::new();

    let version_json: ForgeVersion =
      if let Ok(profile) = read_parse_file::<ForgeInstallerProfile>(&profile_path).await {
        let version_json_path = installer_path.join(VERSION_JSON_PATH);
        extract_and_save_file_from_zip(&path, &profile.json[1..], &version_json_path).await?;

        for entry in profile.data.values() {
          if entry.client.starts_with("/") {
            let file_path = installer_path.join(&entry.client[1..]);
            extract_and_save_file_from_zip(&path, &entry.client[1..], &file_path).await?;
          }
        }

        for library in profile.libraries {
          if library.downloads.artifact.url.is_none() {
            try_extract_lib_from_zip(mc_path, &library, &path).await?;
            continue; // Skip libraries without a URL
          }

          futures.push(download_maven_future(
            mc_path.clone(),
            library.name.clone(),
            client.clone(),
            self.maven_base_url.clone(),
            library.downloads.artifact.sha1,
            library.downloads.artifact.url,
          ));
          added_libs.insert(library.name);
        }

        read_parse_file(&version_json_path).await?
      } else {
        let old = read_parse_file::<ForgeVersionManifestOld>(&profile_path).await?;
        let old_path = MavenArtifact::new(&old.install.path)?.full_path(mc_path);

        extract_and_save_file_from_zip(&path, &old.install.file_path, &old_path).await?;
        added_libs.insert(old.install.path.clone());

        old.into_new()?
      };

    for library in version_json.libraries {
      if existing_libs.contains(&library.name) {
        continue; // Skip already specified libraries from vanilla
      }
      if library.downloads.artifact.url.is_none() {
        try_extract_lib_from_zip(mc_path, &library, &path).await?;
        continue; // Skip libraries without a URL
      }
      if added_libs.contains(&library.name) {
        continue; // Skip already added libraries
      }

      futures.push(download_maven_future(
        mc_path.clone(),
        library.name.clone(),
        client.clone(),
        self.maven_base_url.clone(),
        library.downloads.artifact.sha1,
        library.downloads.artifact.url,
      ));
      added_libs.insert(library.name);
    }

    Ok(futures)
  }

  async fn preprocess(
    &self,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
    jre_bin: PathBuf,
  ) -> Result<()> {
    let installer_path = self.installer_path(version_path).await?;
    let profile_path = installer_path.join(INSTALLER_PROFILE_PATH);
    if let Ok(profile) = read_parse_file::<ForgeInstallerProfile>(&profile_path).await {
      let mut data = profile.data;

      for entry in data.values_mut() {
        if entry.client.starts_with("/") {
          entry.client = installer_path
            .join(&entry.client[1..])
            .to_string_lossy()
            .into_owned();
        }
      }
      default_data(&mut data, &self.mc_version, version_path, mc_path);

      for processor in profile.processors {
        if let Some(sides) = processor.sides
          && !sides.contains(&"client".to_string())
        {
          continue; // Skip processors that are not for the client side
        }

        let jar_path = MavenArtifact::new(&processor.jar)?.full_path(mc_path);
        let main_class = main_class_from_jar(&jar_path).await?;

        let mut classpath = OsString::new();
        classpath.push(jar_path);

        for lib in processor.classpath {
          let path = MavenArtifact::new(&lib)?.full_path(mc_path);
          classpath.push(SEPARATOR);
          classpath.push(path);
        }

        let mut args = Vec::new();
        for arg in processor.args {
          let arg = if arg.starts_with("{") && arg.ends_with("}") {
            let arg_name = &arg[1..arg.len() - 1];
            if let Some(value) = data.get(arg_name) {
              value.client.clone()
            } else {
              return Err(anyhow::anyhow!(
                "Argument {} not found in profile data",
                arg_name
              ));
            }
          } else {
            arg
          };

          if arg.starts_with("[") && arg.ends_with("]") {
            let arg = &arg[1..arg.len() - 1];
            let path = MavenArtifact::new(arg)?.full_path(mc_path);
            args.push(path.to_string_lossy().into_owned());
          } else {
            args.push(arg);
          }
        }

        let mut command = Command::new(&jre_bin);

        command
          .current_dir(&installer_path)
          .stdout(Stdio::piped())
          .stderr(Stdio::piped())
          .arg("-cp")
          .arg(classpath)
          .arg(&main_class)
          .args(&args);

        #[cfg(all(not(debug_assertions), target_os = "windows"))]
        Command::creation_flags(&mut command, DETACHED_PROCESS);

        debug!("Running processor command: {command:?}");

        let command = command.spawn()?;

        let output = command.wait_with_output().await?;

        debug!("Processor command finished with status: {}", output.status);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug!("Processor stdout: {}", stdout);
        debug!("Processor stderr: {}", stderr);

        if !output.status.success() {
          return Err(anyhow::anyhow!(
            "Processor command failed with status: {}",
            output.status
          ));
        }
      }
    } else {
      // just test the old version so when the new version fails to parse because of a corrupted file
      // and the old version is not valid there is still an error returned
      read_parse_file::<ForgeVersionManifestOld>(&profile_path).await?;
    }

    Ok(())
  }

  async fn classpath(
    &self,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
  ) -> Result<Vec<ClasspathEntry>> {
    let version_json = self.version_json(version_path).await?;
    let mut classpath = Vec::new();
    let mut added_libs = HashSet::new();

    for library in version_json.libraries {
      if added_libs.contains(&library.name) {
        continue; // Skip already added libraries
      }

      classpath.push(ClasspathEntry::from_name(&library.name, mc_path)?);
      added_libs.insert(library.name);
    }

    Ok(classpath)
  }

  async fn main_class(&self, version_path: &MCVersionPath) -> Result<String> {
    let version_json = self.version_json(version_path).await?;
    Ok(version_json.main_class)
  }

  async fn arguments(&self, version_path: &MCVersionPath) -> Result<super::Arguments> {
    let version_json = self.version_json(version_path).await?;
    let mut jvm_args = Vec::new();
    let mut game_args = Vec::new();
    let mut overwrite_game_args = false;

    if let Some(arguments) = version_json.arguments {
      if let Some(jvm) = arguments.jvm {
        jvm_args.extend(jvm);
      }
      if let Some(game) = arguments.game {
        game_args.extend(game);
      }
    } else if let Some(minecraft_args) = version_json.minecraft_arguments {
      game_args.extend(
        minecraft_args
          .split(' ')
          .map(String::from)
          .collect::<Vec<_>>(),
      );
      overwrite_game_args = true;
    }

    Ok(super::Arguments::new(
      jvm_args,
      game_args,
      overwrite_game_args,
    ))
  }
}

async fn try_extract_lib_from_zip(mc_path: &MCPath, library: &Library, zip: &Path) -> Result<()> {
  let path = format!("maven/{}", library.downloads.artifact.path);
  if let Ok(data) = extract_file_from_zip(zip, &path).await {
    let library_path = MavenArtifact::new(&library.name)?.full_path(mc_path);
    let parent = library_path.parent().unwrap();
    fs::create_dir_all(parent).await?;
    fs::write(&library_path, &data).await?;
  }
  Ok(())
}

fn default_data(
  data: &mut HashMap<String, DataEntry>,
  mc_version: &str,
  version_path: &MCVersionPath,
  mc_path: &MCPath,
) {
  // Default data: SIDE, MINECRAFT_VERSION, MINECRAFT_JAR, ROOT, LIBRARY_DIR
  data.insert(
    "SIDE".to_string(),
    DataEntry {
      client: "client".to_string(),
    },
  );

  data.insert(
    "MINECRAFT_VERSION".to_string(),
    DataEntry {
      client: mc_version.to_string(),
    },
  );

  data.insert(
    "MINECRAFT_JAR".to_string(),
    DataEntry {
      client: version_path.client_jar().to_string_lossy().into_owned(),
    },
  );

  data.insert(
    "ROOT".to_string(),
    DataEntry {
      client: version_path.base_path().to_string_lossy().into_owned(),
    },
  );

  data.insert(
    "LIBRARY_DIR".to_string(),
    DataEntry {
      client: mc_path.library_path().to_string_lossy().into_owned(),
    },
  );
}

fn forge_version_pair(version_string: &str) -> Result<(String, String)> {
  // Forge version format: x-y where x is the Minecraft version and y is the Forge version
  // e.g., "1.16.5-36.2.39" => ("1.16.5", "36.2.39")
  // or "1.16.5-36.2.39-1.16.5" => ("1.16.5", "36.2.39")
  let parts: Vec<&str> = version_string.split('-').collect();
  if parts.len() < 2 {
    return Err(anyhow::anyhow!(
      "Invalid forge version string: {}",
      version_string
    ));
  }
  let mc_version = parts[0].to_string();
  let forge_version = parts[1].to_string();
  Ok((mc_version, forge_version))
}

fn neoforge_version_pair(version_string: &str) -> Result<(String, String)> {
  // format: x.y.z where x is x.y is the Minecraft version and z is the NeoForge version
  // the mc version is presented as "1.x.y" and the NeoForge version is presented as "z"
  // e.g., "16.5.30" => ("1.16.5", "30")
  let parts: Vec<&str> = version_string.split('.').collect();
  if parts.len() < 3 || parts[0].parse::<u32>().is_err() || parts[1].parse::<u32>().is_err() {
    return Err(anyhow::anyhow!(
      "Invalid NeoForge version string: {}",
      version_string
    ));
  }
  // "21.0.1" is minecraft "1.21", there is no "1.21.0"
  let mc_version = match parts[1] {
    "0" => format!("1.{}", parts[0]),
    minor => format!("1.{}.{minor}", parts[0]),
  };
  let neoforge_version = parts[2..].join(".");
  Ok((mc_version, neoforge_version))
}

type VersionIndex = HashMap<String, Vec<String>>;

#[derive(Deserialize, Debug, Clone)]
struct NeoForgeIndex {
  versioning: NeoForgeVersioning,
}

#[derive(Deserialize, Debug, Clone)]
struct NeoForgeVersioning {
  versions: NeoForgeVersions,
}

#[derive(Deserialize, Debug, Clone)]
struct NeoForgeVersions {
  version: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct ForgeInstallerProfile {
  data: HashMap<String, DataEntry>,
  processors: Vec<Processor>,
  libraries: Vec<Library>,
  json: String,
}

#[derive(Deserialize, Debug, Clone)]
struct DataEntry {
  client: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Processor {
  sides: Option<Vec<String>>,
  jar: String,
  classpath: Vec<String>,
  args: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct Library {
  name: String,
  downloads: Downloads,
}

#[derive(Deserialize, Debug, Clone)]
struct Downloads {
  artifact: Artifact,
}

#[derive(Deserialize, Debug, Clone)]
struct Artifact {
  path: String,
  #[serde(deserialize_with = "deserialize_url_option")]
  url: Option<Url>,
  sha1: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ForgeVersion {
  main_class: String,
  libraries: Vec<Library>,
  arguments: Option<Arguments>,
  minecraft_arguments: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ForgeVersionManifestOld {
  install: ForgeInstallOld,
  version_info: ForgeVersionOld,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ForgeInstallOld {
  path: String,
  file_path: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ForgeVersionOld {
  main_class: String,
  minecraft_arguments: String,
  libraries: Vec<LibraryOld>,
}

#[derive(Deserialize, Debug, Clone)]
struct LibraryOld {
  name: String,
  url: Option<Url>,
}

#[derive(Deserialize, Debug, Clone)]
struct Arguments {
  game: Option<Vec<String>>,
  jvm: Option<Vec<String>>,
}

fn deserialize_url_option<'de, D>(deserializer: D) -> Result<Option<Url>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  let url = String::deserialize(deserializer)?;
  if let Ok(parsed_url) = Url::parse(&url) {
    Ok(Some(parsed_url))
  } else {
    Ok(None)
  }
}

impl ForgeVersionManifestOld {
  const MINECRAFT_MAVEN: &str = "https://libraries.minecraft.net";

  fn into_new(self) -> Result<ForgeVersion> {
    let minecraft_maven = http::url(Self::MINECRAFT_MAVEN);
    let library_results = self
      .version_info
      .libraries
      .into_iter()
      .map(|lib| {
        let maven = MavenArtifact::new(&lib.name)?;
        let base_url = lib
          .url
          .as_ref()
          .map(|u| u.as_str())
          .unwrap_or(&minecraft_maven);

        let url = Some(maven.url(base_url)?);
        let path = maven.path().to_string_lossy().into_owned();

        anyhow::Ok(Library {
          name: lib.name,
          downloads: Downloads {
            artifact: Artifact {
              path,
              url,
              sha1: None,
            },
          },
        })
      })
      .collect::<Vec<_>>();

    let mut libraries = Vec::new();
    for result in library_results {
      libraries.push(result?);
    }

    Ok(ForgeVersion {
      main_class: self.version_info.main_class,
      libraries,
      minecraft_arguments: Some(self.version_info.minecraft_arguments),
      arguments: None,
    })
  }
}

#[cfg(test)]
mod tests {
  use proton_utils::{download::default_client, rt, test_http::FakeServer};

  use super::*;
  use crate::{
    loader::CheckFuture,
    testing::{fake_java, java_calls, write_zip},
  };

  const FORGE_INDEX: &str = r#"{
    "1.20.1": ["1.20.1-47.1.0", "1.20.1-47.1.3"],
    "1.7.10_pre4": ["1.7.10_pre4-10.12.2.1149-prerelease"],
    "1.7.10": ["1.7.10-10.13.4.1614-1.7.10"],
    "1.5.2": ["1.5.2-7.8.0.684"],
    "1.5.1": ["1.5.1-7.7.0.598"],
    "1.4.7": ["1.4.7-6.6.2.534"]
  }"#;
  const NEOFORGE_INDEX: &str = "<metadata><groupId>net.neoforged</groupId><versioning>\
    <latest>21.0.2-beta</latest><versions>\
    <version>20.2.3-beta</version><version>20.4.80-beta</version><version>20.4.237</version>\
    <version>21.0.1-beta</version><version>21.0.2</version><version>broken</version>\
    </versions></versioning></metadata>";

  struct Env {
    _dir: tempfile::TempDir,
    data: PathBuf,
    mc: MCPath,
  }

  fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    // spaces and unicode reach the processor args
    let data = dir.path().join("dätä dir");
    Env {
      mc: MCPath::new(&data),
      data,
      _dir: dir,
    }
  }

  impl Env {
    fn version(&self, mc: &str) -> MCVersionPath {
      MCVersionPath::new(&self.data, mc)
    }

    fn lib(&self, name: &str) -> PathBuf {
      MavenArtifact::new(name).unwrap().full_path(&self.mc)
    }

    async fn index(&self, name: &str, body: &str) {
      let root = self.version("").version_root().clone();
      fs::create_dir_all(&root).await.unwrap();
      fs::write(root.join(format!("{name}-index.json")), body)
        .await
        .unwrap();
    }
  }

  async fn run(futures: Vec<CheckFuture>) -> Result<()> {
    for check in futures {
      if let Some(download) = check.await? {
        let (write, _) = download.await?;
        write(Box::new(|_| {})).await?;
      }
    }
    Ok(())
  }

  #[test]
  fn version_strings_split_into_mc_and_loader() {
    let forge = [
      ("1.16.5-36.2.39", Some(("1.16.5", "36.2.39"))),
      (
        "1.7.10-10.13.4.1614-1.7.10",
        Some(("1.7.10", "10.13.4.1614")),
      ),
      ("1.16.5", None),
      ("", None),
    ];
    for (input, expected) in forge {
      let pair = forge_version_pair(input).ok();
      let pair = pair.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
      assert_eq!(pair, expected, "{input}");
    }
    let neoforge = [
      ("20.4.237", Some(("1.20.4", "237"))),
      ("20.2.3-beta", Some(("1.20.2", "3-beta"))),
      // the .0 minor is the first release of a major
      ("21.0.1-beta", Some(("1.21", "1-beta"))),
      ("21.1.1.5", Some(("1.21.1", "1.5"))),
      ("20.4", None),
      ("x.4.1", None),
      ("20.y.1", None),
      ("", None),
    ];
    for (input, expected) in neoforge {
      let pair = neoforge_version_pair(input).ok();
      let pair = pair.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
      assert_eq!(pair, expected, "{input}");
    }
  }

  #[test]
  fn forge_lists_skip_prereleases_and_jar_patching_versions() {
    let env = env();
    let path = env.version("");
    let loader = ForgeLikeLoader::forge();
    rt::block_on(async {
      assert!(loader.supported_versions(&path, true).await.is_err());
      env.index("forge", FORGE_INDEX).await;

      let mut stable = loader.supported_versions(&path, true).await.unwrap();
      stable.sort();
      // 1.5.1 and older need manual jar patching
      assert_eq!(stable, ["1.20.1", "1.5.2", "1.7.10"]);
      let mut all = loader.supported_versions(&path, false).await.unwrap();
      all.sort();
      assert_eq!(all, ["1.20.1", "1.5.2", "1.7.10", "1.7.10_pre4"]);

      let versions = |mc, stable| loader.loader_versions_for_mc_version(mc, &path, stable);
      // newest first
      assert_eq!(
        versions("1.20.1", true).await.unwrap(),
        ["47.1.3", "47.1.0"]
      );
      assert_eq!(versions("1.7.10", true).await.unwrap(), ["10.13.4.1614"]);
      assert!(versions("1.7.10_pre4", true).await.unwrap().is_empty());
      assert_eq!(
        versions("1.7.10_pre4", false).await.unwrap(),
        ["10.12.2.1149"]
      );
      assert!(versions("9.9", false).await.unwrap().is_empty());
    });
  }

  #[test]
  fn neoforge_lists_come_from_maven_metadata() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml",
      200,
      NEOFORGE_INDEX,
    );
    let env = env();
    let path = env.version("");
    let loader = ForgeLikeLoader::neoforge();
    rt::block_on(async {
      loader
        .download_metadata(&default_client(), &path)
        .await
        .unwrap();
      // sorted and deduped, the broken entry is dropped
      assert_eq!(
        loader.supported_versions(&path, true).await.unwrap(),
        ["1.20.2", "1.20.4", "1.21"]
      );
      let versions = |mc, stable| loader.loader_versions_for_mc_version(mc, &path, stable);
      assert_eq!(versions("1.20.4", true).await.unwrap(), ["237"]);
      assert_eq!(versions("1.20.4", false).await.unwrap(), ["237", "80-beta"]);
      assert_eq!(versions("1.21", false).await.unwrap(), ["2", "1-beta"]);
      assert!(versions("1.20.2", true).await.unwrap().is_empty());

      // the latest route wins, a failed refresh is an error
      server.route(
        "GET",
        "/maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml",
        500,
        "",
      );
      assert!(
        loader
          .download_metadata(&default_client(), &path)
          .await
          .is_err()
      );
      // corrupt xml
      fs::write(loader.index(&path), "<metadata>").await.unwrap();
      assert!(loader.supported_versions(&path, true).await.is_err());
    });
  }

  #[test]
  fn installer_urls_use_the_full_loader_version() {
    let env = env();
    rt::block_on(async {
      env.index("forge", FORGE_INDEX).await;
      let forge = ForgeLikeLoaderVersion::forge("1.7.10".into(), "10.13.4.1614".into());
      let path = env.version("1.7.10");
      assert!(forge.installer_url(&path).await.unwrap().ends_with(
        "/net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar"
      ));
      assert_eq!(
        forge.installer_path(&path).await.unwrap(),
        path.base_path().join("forge-1.7.10-10.13.4.1614-1.7.10")
      );
      let missing = ForgeLikeLoaderVersion::forge("1.7.10".into(), "1.0".into());
      let err = missing.installer_url(&path).await.unwrap_err();
      assert_eq!(
        err.to_string(),
        "Loader version 1.0 not found for Minecraft version 1.7.10"
      );

      env.index("neoforge", NEOFORGE_INDEX).await;
      for (mc, loader, full) in [
        ("1.20.4", "237", "20.4.237"),
        ("1.20.4", "80-beta", "20.4.80-beta"),
        // "1.21" maps to the 21.0 line
        ("1.21", "1-beta", "21.0.1-beta"),
      ] {
        let neo = ForgeLikeLoaderVersion::neoforge(mc.into(), loader.into());
        let url = neo.installer_url(&env.version(mc)).await.unwrap();
        assert!(
          url.ends_with(&format!("/neoforge/{full}/neoforge-{full}-installer.jar")),
          "{url}"
        );
      }
      let neo = ForgeLikeLoaderVersion::neoforge("1.20.4".into(), "999".into());
      assert!(neo.installer_url(&env.version("1.20.4")).await.is_err());
    });
  }

  fn lib_json(name: &str, path: &str, url: &str, sha1: Option<&str>) -> serde_json::Value {
    let mut artifact = serde_json::json!({ "path": path, "url": url });
    if let Some(sha1) = sha1 {
      artifact["sha1"] = sha1.into();
    }
    serde_json::json!({ "name": name, "downloads": { "artifact": artifact } })
  }

  /// A 1.20.1 style installer with processors, served by `server`
  async fn modern_installer(env: &Env, server: &FakeServer) {
    let origin = server.origin();
    let maven = format!("{origin}/maven.minecraftforge.net");
    let profile = serde_json::json!({
      "data": {
        "MAPPINGS": { "client": "[de.oceanlabs.mcp:mcp_config:1.20.1@zip]", "server": "x" },
        "BINPATCH": { "client": "/data/client.lzma", "server": "/data/server.lzma" },
        "LITERAL": { "client": "'ü lit'", "server": "" }
      },
      "processors": [
        // never run, its jar does not even exist
        { "sides": ["server"], "jar": "x:server-only:1", "classpath": [], "args": [] },
        {
          "sides": ["client", "server"],
          "jar": "net.minecraftforge:installertools:1.0",
          "classpath": ["a.b:cp-lib:1"],
          "args": [
            "--task", "{MAPPINGS}", "--patch", "{BINPATCH}", "--jar", "{MINECRAFT_JAR}",
            "--side", "{SIDE}", "--out", "[a.b:out:1@txt]", "--lit", "{LITERAL}",
            "--version", "{MINECRAFT_VERSION}", "--root", "{ROOT}", "--libs", "{LIBRARY_DIR}"
          ]
        }
      ],
      "libraries": [
        lib_json(
          "net.minecraftforge:installertools:1.0",
          "net/minecraftforge/installertools/1.0/installertools-1.0.jar",
          &format!("{maven}/net/minecraftforge/installertools/1.0/installertools-1.0.jar"),
          None,
        ),
        // no url: it ships inside the installer
        lib_json(
          "net.minecraftforge:forge:1.20.1-47.1.0:universal",
          "net/minecraftforge/forge/1.20.1-47.1.0/forge-1.20.1-47.1.0-universal.jar",
          "",
          None,
        ),
      ],
      "json": "/version.json"
    });
    let version = serde_json::json!({
      "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
      "libraries": [
        lib_json("org.lwjgl:lwjgl:3.3.1", "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar", &format!("{maven}/vanilla.jar"), None),
        lib_json(
          "net.minecraftforge:installertools:1.0",
          "net/minecraftforge/installertools/1.0/installertools-1.0.jar",
          &format!("{maven}/net/minecraftforge/installertools/1.0/installertools-1.0.jar"),
          None,
        ),
        lib_json(
          "cpw.mods:securejarhandler:2.1.10",
          "cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar",
          &format!("{maven}/cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar"),
          Some("0000"),
        ),
        lib_json(
          "net.minecraftforge:forge:1.20.1-47.1.0",
          "net/minecraftforge/forge/1.20.1-47.1.0/forge-1.20.1-47.1.0.jar",
          "",
          None,
        ),
        // no url and not in the installer: silently skipped
        lib_json("x:not-shipped:1", "x/not-shipped/1/not-shipped-1.jar", "", None),
        lib_json(
          "cpw.mods:securejarhandler:2.1.10",
          "cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar",
          &format!("{maven}/cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar"),
          None,
        ),
      ],
      "arguments": { "game": ["--launchTarget", "forgeclient"], "jvm": ["-Dlibs=${library_directory}"] }
    });

    let jar = env.data.join("installer.jar");
    write_zip(
      &jar,
      &[
        ("install_profile.json", profile.to_string().as_bytes()),
        ("version.json", version.to_string().as_bytes()),
        ("data/client.lzma", b"patch"),
        (
          "maven/net/minecraftforge/forge/1.20.1-47.1.0/forge-1.20.1-47.1.0-universal.jar",
          b"universal",
        ),
        (
          "maven/net/minecraftforge/forge/1.20.1-47.1.0/forge-1.20.1-47.1.0.jar",
          b"forge",
        ),
      ],
    )
    .await;
    server
      .route(
        "GET",
        "/maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.1.0/forge-1.20.1-47.1.0-installer.jar",
        200,
        fs::read(&jar).await.unwrap(),
      )
      .route(
        "GET",
        "/maven.minecraftforge.net/net/minecraftforge/installertools/1.0/installertools-1.0.jar",
        200,
        "tools",
      )
      .route(
        "GET",
        "/maven.minecraftforge.net/cpw/mods/securejarhandler/2.1.10/securejarhandler-2.1.10.jar",
        200,
        "sjh",
      );
    env.index("forge", FORGE_INDEX).await;
  }

  #[test]
  fn modern_installer_extracts_downloads_and_runs_client_processors() {
    let server = FakeServer::start();
    let env = env();
    let path = env.version("1.20.1");
    let forge = ForgeLikeLoaderVersion::forge("1.20.1".into(), "47.1.0".into());
    let installer = path.base_path().join("forge-1.20.1-47.1.0");

    rt::block_on(async {
      modern_installer(&env, &server).await;
      let client = default_client();
      let vanilla = ["org.lwjgl:lwjgl:3.3.1".to_string()];
      let futures = forge
        .download(&client, &path, &env.mc, &vanilla)
        .await
        .unwrap();
      // installertools once (profile + version.json), securejarhandler once, vanilla skipped
      assert_eq!(futures.len(), 2);
      run(futures).await.unwrap();

      assert_eq!(
        fs::read(installer.join("data/client.lzma")).await.unwrap(),
        b"patch"
      );
      let universal = env.lib("net.minecraftforge:forge:1.20.1-47.1.0:universal");
      assert_eq!(fs::read(universal).await.unwrap(), b"universal");
      let forge_jar = env.lib("net.minecraftforge:forge:1.20.1-47.1.0");
      assert_eq!(fs::read(forge_jar).await.unwrap(), b"forge");
      assert!(!env.lib("x:not-shipped:1").exists());
      assert_eq!(
        fs::read(env.lib("cpw.mods:securejarhandler:2.1.10"))
          .await
          .unwrap(),
        b"sjh"
      );

      // the processor jar names its main class
      let tools = env.lib("net.minecraftforge:installertools:1.0");
      write_zip(
        &tools,
        &[(
          "META-INF/MANIFEST.MF",
          b"Main-Class: net.minecraftforge.installertools.ConsoleTool\n",
        )],
      )
      .await;
      let java = env.data.join("jre/bin/java");
      let record = fake_java(&java, 0);
      forge
        .preprocess(&path, &env.mc, java.clone())
        .await
        .unwrap();

      let calls = java_calls(&record);
      assert_eq!(calls.len(), 1, "server processors are skipped");
      let s = |p: PathBuf| p.to_string_lossy().into_owned();
      let expected = [
        format!("cwd={}", installer.display()),
        "env=".into(),
        "-cp".into(),
        format!(
          "{}{SEPARATOR}{}",
          tools.display(),
          env.lib("a.b:cp-lib:1").display()
        ),
        "net.minecraftforge.installertools.ConsoleTool".into(),
        "--task".into(),
        s(env.lib("de.oceanlabs.mcp:mcp_config:1.20.1@zip")),
        "--patch".into(),
        s(installer.join("data/client.lzma")),
        "--jar".into(),
        s(path.client_jar()),
        "--side".into(),
        "client".into(),
        "--out".into(),
        s(env.lib("a.b:out:1@txt")),
        "--lit".into(),
        "'ü lit'".into(),
        "--version".into(),
        "1.20.1".into(),
        "--root".into(),
        s(path.base_path().clone()),
        "--libs".into(),
        s(env.mc.library_path()),
      ];
      assert_eq!(calls[0], expected);

      // a failing processor stops the install
      let record = fake_java(&java, 3);
      let err = forge
        .preprocess(&path, &env.mc, java.clone())
        .await
        .unwrap_err();
      assert!(
        err.to_string().contains("Processor command failed"),
        "{err}"
      );
      assert_eq!(java_calls(&record).len(), 2);

      // a missing java binary is an error, not a panic
      let gone = forge.preprocess(&path, &env.mc, env.data.join("nope/java"));
      assert!(gone.await.is_err());

      // launching uses the extracted version.json, deduped
      let classpath: Vec<_> = forge
        .classpath(&path, &env.mc)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.artifact.artifact)
        .collect();
      assert_eq!(
        classpath,
        [
          "lwjgl",
          "installertools",
          "securejarhandler",
          "forge",
          "not-shipped"
        ]
      );
      assert_eq!(
        forge.main_class(&path).await.unwrap(),
        "cpw.mods.bootstraplauncher.BootstrapLauncher"
      );
      let args = forge.arguments(&path).await.unwrap();
      assert_eq!(args.jvm_args, ["-Dlibs=${library_directory}"]);
      assert_eq!(args.game_args, ["--launchTarget", "forgeclient"]);
      assert!(!args.overwrite_game_args);
    });
  }

  #[test]
  fn processor_args_must_be_in_the_profile_data() {
    let env = env();
    let path = env.version("1.20.1");
    let forge = ForgeLikeLoaderVersion::forge("1.20.1".into(), "47.1.0".into());
    let installer = path.base_path().join("forge-1.20.1-47.1.0");
    rt::block_on(async {
      env.index("forge", FORGE_INDEX).await;
      let tools = env.lib("a:tools:1");
      write_zip(&tools, &[("META-INF/MANIFEST.MF", b"Main-Class: T\n")]).await;
      let profile = serde_json::json!({
        "data": {},
        "processors": [{ "jar": "a:tools:1", "classpath": [], "args": ["{NOPE}"] }],
        "libraries": [],
        "json": "/version.json"
      });
      fs::create_dir_all(&installer).await.unwrap();
      fs::write(installer.join(INSTALLER_PROFILE_PATH), profile.to_string())
        .await
        .unwrap();
      let java = env.data.join("bin/java");
      let record = fake_java(&java, 0);
      let err = forge.preprocess(&path, &env.mc, java).await.unwrap_err();
      assert_eq!(err.to_string(), "Argument NOPE not found in profile data");
      assert!(java_calls(&record).is_empty());

      // a processor jar without a manifest fails before spawning
      let profile = serde_json::json!({
        "data": {},
        "processors": [{ "jar": "a:missing:1", "classpath": [], "args": [] }],
        "libraries": [],
        "json": "/version.json"
      });
      fs::write(installer.join(INSTALLER_PROFILE_PATH), profile.to_string())
        .await
        .unwrap();
      let java = env.data.join("bin/java");
      assert!(forge.preprocess(&path, &env.mc, java).await.is_err());

      // neither format parses
      fs::write(installer.join(INSTALLER_PROFILE_PATH), "{}")
        .await
        .unwrap();
      let java = env.data.join("bin/java");
      assert!(forge.preprocess(&path, &env.mc, java).await.is_err());
    });
  }

  #[test]
  fn legacy_installer_converts_the_old_profile() {
    let server = FakeServer::start();
    let origin = server.origin();
    let env = env();
    let path = env.version("1.7.10");
    let forge = ForgeLikeLoaderVersion::forge("1.7.10".into(), "10.13.4.1614".into());
    let profile = serde_json::json!({
      "install": {
        "path": "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10",
        "filePath": "forge-1.7.10-10.13.4.1614-1.7.10-universal.jar"
      },
      "versionInfo": {
        "mainClass": "net.minecraft.launchwrapper.Launch",
        "minecraftArguments": "--username ${auth_player_name} --tweakClass cpw.mods.fml.common.launcher.FMLTweaker",
        "libraries": [
          { "name": "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10", "url": "http://files.minecraftforge.net/maven/" },
          { "name": "net.minecraft:launchwrapper:1.12" },
          { "name": "org.scala-lang:scala-library:2.11.1", "url": format!("{origin}/maven.minecraftforge.net/") }
        ]
      }
    });

    rt::block_on(async {
      env.index("forge", FORGE_INDEX).await;
      let jar = env.data.join("legacy.jar");
      write_zip(
        &jar,
        &[
          ("install_profile.json", profile.to_string().as_bytes()),
          (
            "forge-1.7.10-10.13.4.1614-1.7.10-universal.jar",
            b"universal",
          ),
        ],
      )
      .await;
      server
        .route(
          "GET",
          "/maven.minecraftforge.net/net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar",
          200,
          fs::read(&jar).await.unwrap(),
        )
        .route(
          "GET",
          "/libraries.minecraft.net/net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar",
          200,
          "lw",
        )
        .route(
          "GET",
          "/maven.minecraftforge.net/org/scala-lang/scala-library/2.11.1/scala-library-2.11.1.jar",
          200,
          "scala",
        );

      let futures = forge
        .download(&default_client(), &path, &env.mc, &[])
        .await
        .unwrap();
      // the forge jar itself comes out of the installer
      assert_eq!(futures.len(), 2);
      run(futures).await.unwrap();
      let universal = env.lib("net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10");
      assert_eq!(fs::read(universal).await.unwrap(), b"universal");
      assert_eq!(
        fs::read(env.lib("net.minecraft:launchwrapper:1.12"))
          .await
          .unwrap(),
        b"lw"
      );

      // old installers have no processors
      let java = env.data.join("bin/java");
      let record = fake_java(&java, 1);
      forge.preprocess(&path, &env.mc, java).await.unwrap();
      assert!(java_calls(&record).is_empty());

      assert_eq!(
        forge.main_class(&path).await.unwrap(),
        "net.minecraft.launchwrapper.Launch"
      );
      let args = forge.arguments(&path).await.unwrap();
      // old minecraftArguments replace the vanilla game args
      assert!(args.overwrite_game_args);
      assert_eq!(
        args.game_args,
        [
          "--username",
          "${auth_player_name}",
          "--tweakClass",
          "cpw.mods.fml.common.launcher.FMLTweaker"
        ]
      );
      assert!(args.jvm_args.is_empty());
      assert_eq!(forge.classpath(&path, &env.mc).await.unwrap().len(), 3);
    });
  }

  #[test]
  fn broken_installers_are_errors() {
    let server = FakeServer::start();
    let env = env();
    let path = env.version("1.20.1");
    let forge = ForgeLikeLoaderVersion::forge("1.20.1".into(), "47.1.0".into());
    let url = "/maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.1.0/forge-1.20.1-47.1.0-installer.jar";
    rt::block_on(async {
      env.index("forge", FORGE_INDEX).await;
      let client = default_client();
      let download = || forge.download(&client, &path, &env.mc, &[]);

      server.route("GET", url, 404, "");
      assert!(download().await.is_err());
      server.route("GET", url, 200, "not a zip");
      assert!(download().await.is_err());

      let jar = env.data.join("empty.jar");
      write_zip(&jar, &[("other.txt", b"")]).await;
      server.route("GET", url, 200, fs::read(&jar).await.unwrap());
      let err = download().await.err().unwrap();
      assert_eq!(
        err.to_string(),
        "File 'install_profile.json' not found in zip"
      );

      write_zip(&jar, &[("install_profile.json", b"{}")]).await;
      server.route("GET", url, 200, fs::read(&jar).await.unwrap());
      assert!(download().await.is_err());

      // a version.json without the profile pointing at it
      let profile = r#"{ "data": {}, "processors": [], "libraries": [], "json": "/version.json" }"#;
      write_zip(&jar, &[("install_profile.json", profile.as_bytes())]).await;
      server.route("GET", url, 200, fs::read(&jar).await.unwrap());
      assert!(download().await.is_err());
      // nothing installed yet, launching parts fail
      assert!(forge.classpath(&path, &env.mc).await.is_err());
    });
  }
}
