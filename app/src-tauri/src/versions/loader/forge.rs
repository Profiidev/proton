use std::{
  collections::{HashMap, HashSet},
  ffi::OsString,
  path::{Path, PathBuf},
  process::Stdio,
};

use anyhow::Result;
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::Url;
use tokio::{fs, process::Command};

use crate::{
  path,
  utils::file::{download_file_no_hash_force, read_parse_file, read_parse_xml_file},
  versions::{
    SEPARATOR,
    loader::{
      CheckFuture, Loader, LoaderVersion,
      util::{compare_mc_versions, download_maven_future, extract_file_from_zip},
    },
    maven::{MavenName, full_path_from_maven, parse_maven_name},
    paths::{MCPath, MCVersionPath},
  },
};

const INDEX_BASE_URL_FORGE: &str =
  "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const INDEX_BASE_URL_NEOFORGE: &str =
  "https://maven.neoforged.net/net/neoforged/neoforge/maven-metadata.xml";
const INSTALLER_URL_FORGE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/{loader_version}/forge-{loader_version}-installer.jar";
const INSTALLER_URL_NEOFORGE: &str = "https://maven.neoforged.net/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar";
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
      index_url: INDEX_BASE_URL_FORGE.to_string(),
      index_file_name: INDEX_FILE_NAME_FORGE.to_string(),
    }
  }

  pub fn neoforge() -> Self {
    Self {
      index_url: INDEX_BASE_URL_NEOFORGE.to_string(),
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
    download_file_no_hash_force(client, &path, url).await?;

    Ok(())
  }

  async fn supported_versions(&self, version_path: &MCVersionPath, _: bool) -> Result<Vec<String>> {
    let path = self.index(version_path);
    if self.index_file_name == INDEX_FILE_NAME_FORGE {
      let mut versions = read_parse_file::<VersionIndex>(&path)
        .await?
        .keys()
        .filter(|v| !v.contains("pre"))
        .cloned()
        .collect::<Vec<_>>();

      versions.sort_by(compare_mc_versions);
      versions.reverse();

      Ok(versions)
    } else {
      let versions = self.neoforge_version_pairs(version_path).await?;
      let mut versions: Vec<String> = versions.into_iter().map(|(mc, _)| mc).collect();
      versions.sort_by(compare_mc_versions);
      versions.dedup();
      versions.reverse();

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
      installer_base_url: INSTALLER_URL_FORGE.to_string(),
      index_file_name: INDEX_FILE_NAME_FORGE.to_string(),
      maven_base_url: MAVEN_BASE_URL_FORGE.to_string(),
    }
  }

  pub fn neoforge(mc_version: String, loader_version: String) -> Self {
    Self {
      mc_version,
      loader_version,
      installer_base_url: INSTALLER_URL_NEOFORGE.to_string(),
      index_file_name: INDEX_FILE_NAME_NEOFORGE.to_string(),
      maven_base_url: MAVEN_BASE_URL_NEOFORGE.to_string(),
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
  ) -> Result<Vec<CheckFuture>> {
    let installer_path = self.installer_path(version_path).await?;

    let url = Url::parse(&self.installer_url(version_path).await?)?;
    let path = installer_path.join(INSTALLER_PATH);
    download_file_no_hash_force(client, &path, url).await?;

    let profile_path = installer_path.join(INSTALLER_PROFILE_PATH);
    let installer_data = extract_file_from_zip(&path, INSTALLER_PROFILE_PATH).await?;
    fs::write(&profile_path, &installer_data).await?;

    let profile = read_parse_file::<ForgeInstallerProfile>(&profile_path).await?;

    let version_json_path = installer_path.join(VERSION_JSON_PATH);
    let version_json_data = extract_file_from_zip(&path, &profile.json[1..]).await?;
    fs::write(&version_json_path, &version_json_data).await?;

    let version_json: ForgeVersion = read_parse_file(&version_json_path).await?;

    for entry in profile.data.values() {
      if entry.client.starts_with("/") {
        let file_path = installer_path.join(&entry.client[1..]);
        let data = extract_file_from_zip(&path, &entry.client[1..]).await?;

        let parent = file_path.parent().unwrap();
        fs::create_dir_all(parent).await?;
        fs::write(&file_path, &data).await?;
      }
    }

    let mut futures = Vec::new();
    let mut added_libs = HashSet::new();

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
        Some(library.downloads.artifact.sha1),
        library.downloads.artifact.url,
      ));
      added_libs.insert(library.name);
    }

    for library in version_json.libraries {
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
        Some(library.downloads.artifact.sha1),
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
    let profile: ForgeInstallerProfile = read_parse_file(&profile_path).await?;
    let mut data = profile.data;

    for entry in data.values_mut() {
      if entry.client.starts_with("/") {
        entry.client = installer_path
          .join(&entry.client[1..])
          .to_string_lossy()
          .into_owned();
      }
      if entry.server.starts_with("/") {
        entry.server = installer_path
          .join(&entry.server[1..])
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

      let jar_maven = parse_maven_name(&processor.jar)?;
      let jar_path = full_path_from_maven(mc_path, &jar_maven);

      //find Main-Class in the jar
      let manifest_data = extract_file_from_zip(&jar_path, "META-INF/MANIFEST.MF").await?;
      let manifest = String::from_utf8(manifest_data)?;
      let mut main_class = None;
      for line in manifest.lines() {
        if line.starts_with("Main-Class: ") {
          main_class = Some(line.strip_prefix("Main-Class: ").unwrap().to_string());
          break;
        }
      }
      let main_class = main_class.ok_or_else(|| anyhow::anyhow!("Main-Class not found"))?;

      let mut classpath = OsString::new();
      classpath.push(jar_path);

      for lib in processor.classpath {
        let maven = parse_maven_name(&lib)?;
        let path = full_path_from_maven(mc_path, &maven);
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
          let maven = parse_maven_name(arg)?;
          let path = full_path_from_maven(mc_path, &maven);
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

    Ok(())
  }

  async fn classpath(
    &self,
    version_path: &MCVersionPath,
    mc_path: &MCPath,
  ) -> Result<Vec<(MavenName, PathBuf)>> {
    let version_json_path = self
      .installer_path(version_path)
      .await?
      .join(VERSION_JSON_PATH);
    let version_json: ForgeVersion = read_parse_file(&version_json_path).await?;

    let mut classpath = Vec::new();
    let mut added_libs = HashSet::new();

    for library in version_json.libraries {
      if added_libs.contains(&library.name) {
        continue; // Skip already added libraries
      }

      let maven = parse_maven_name(&library.name)?;
      let path = full_path_from_maven(mc_path, &maven);
      classpath.push((maven, path));
      added_libs.insert(library.name);
    }

    Ok(classpath)
  }

  async fn main_class(&self, version_path: &MCVersionPath) -> Result<String> {
    let version_json_path = self
      .installer_path(version_path)
      .await?
      .join(VERSION_JSON_PATH);
    let version_json: ForgeVersion = read_parse_file(&version_json_path).await?;
    Ok(version_json.main_class)
  }

  async fn arguments(&self, version_path: &MCVersionPath) -> Result<(Vec<String>, Vec<String>)> {
    let version_json_path = self
      .installer_path(version_path)
      .await?
      .join(VERSION_JSON_PATH);
    let version_json: ForgeVersion = read_parse_file(&version_json_path).await?;

    Ok((
      version_json.arguments.jvm.unwrap_or_default(),
      version_json.arguments.game.unwrap_or_default(),
    ))
  }
}

async fn try_extract_lib_from_zip(mc_path: &MCPath, library: &Library, zip: &Path) -> Result<()> {
  let path = format!("maven/{}", library.downloads.artifact.path);
  if let Ok(data) = extract_file_from_zip(zip, &path).await {
    let maven = parse_maven_name(&library.name)?;
    let library_path = full_path_from_maven(mc_path, &maven);
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
      server: "server".to_string(),
    },
  );

  data.insert(
    "MINECRAFT_VERSION".to_string(),
    DataEntry {
      client: mc_version.to_string(),
      server: mc_version.to_string(),
    },
  );

  data.insert(
    "MINECRAFT_JAR".to_string(),
    DataEntry {
      client: version_path.client_jar().to_string_lossy().into_owned(),
      server: String::new(),
    },
  );

  data.insert(
    "ROOT".to_string(),
    DataEntry {
      client: path!(version_path.version_root(), "root")
        .to_string_lossy()
        .into_owned(),
      server: String::new(),
    },
  );

  data.insert(
    "LIBRARY_DIR".to_string(),
    DataEntry {
      client: mc_path.library_path().to_string_lossy().into_owned(),
      server: String::new(),
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
  let mc_version = format!("1.{}.{}", parts[0], parts[1]);
  let neoforge_version = parts[2..].join(".");
  Ok((mc_version, neoforge_version))
}

type VersionIndex = HashMap<String, Vec<String>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NeoForgeIndex {
  versioning: NeoForgeVersioning,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NeoForgeVersioning {
  versions: NeoForgeVersions,
  latest: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NeoForgeVersions {
  version: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ForgeInstallerProfile {
  //spec: usize,
  //profile: String,
  //version: String,
  //minecraft: String,
  data: HashMap<String, DataEntry>,
  processors: Vec<Processor>,
  libraries: Vec<Library>,
  json: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DataEntry {
  client: String,
  server: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Processor {
  sides: Option<Vec<String>>,
  jar: String,
  classpath: Vec<String>,
  args: Vec<String>,
  outputs: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Library {
  name: String,
  downloads: Downloads,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Downloads {
  artifact: Artifact,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Artifact {
  path: String,
  #[serde(deserialize_with = "deserialize_url_option")]
  url: Option<Url>,
  sha1: String,
  size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ForgeVersion {
  id: String,
  main_class: String,
  libraries: Vec<Library>,
  arguments: Arguments,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Arguments {
  game: Option<Vec<String>>,
  jvm: Option<Vec<String>>,
}
