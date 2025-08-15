use std::{
  collections::HashMap,
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
    LIBRARY_DIR, MC_DIR, SEPARATOR, VERSION_DIR,
    loader::{
      CheckFuture, Loader, LoaderVersion,
      util::{compare_mc_versions, download_maven_future, extract_file_from_zip},
    },
    maven::{MavenName, full_path_from_maven, parse_maven_name},
  },
};

const INDEX_BASE_URL_FORGE: &str =
  "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const INDEX_BASE_URL_NEOFORGE_FORGE: &str =
  "https://maven.neoforged.net/net/neoforged/forge/maven-metadata.xml";
const INDEX_BASE_URL_NEOFORGE_NEOFORGE: &str =
  "https://maven.neoforged.net/net/neoforged/neoforge/maven-metadata.xml";
const INSTALLER_URL_FORGE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/{loader_version}/forge-{loader_version}-installer.jar";
const INSTALLER_URL_NEOFORGE: &str = "https://maven.neoforged.net/net/neoforged/forge/{loader_version}/forge-{loader_version}-installer.jar";
const MAVEN_BASE_URL_FORGE: &str = "https://maven.minecraftforge.net";
const MAVEN_BASE_URL_NEOFORGE: &str = "https://maven.neoforged.net";
const INDEX_FILE_NAME_FORGE: &str = "forge";
const INDEX_FILE_NAME_NEOFORGE: &str = "neoforge";

pub struct ForgeLikeLoader {
  index_url: String,
  index_url_2: Option<String>,
  index_file_name: String,
}

impl ForgeLikeLoader {
  pub fn forge() -> Self {
    Self {
      index_url: INDEX_BASE_URL_FORGE.to_string(),
      index_url_2: None,
      index_file_name: INDEX_FILE_NAME_FORGE.to_string(),
    }
  }

  pub fn neoforge() -> Self {
    Self {
      index_url: INDEX_BASE_URL_NEOFORGE_FORGE.to_string(),
      index_url_2: Some(INDEX_BASE_URL_NEOFORGE_NEOFORGE.to_string()),
      index_file_name: INDEX_FILE_NAME_NEOFORGE.to_string(),
    }
  }

  fn index(&self, data_dir: &Path) -> PathBuf {
    let filename = format!("{}-index.json", self.index_file_name);
    path!(data_dir, MC_DIR, VERSION_DIR, filename)
  }

  fn index_2(&self, data_dir: &Path) -> PathBuf {
    let filename = format!("{}-index-2.json", self.index_file_name);
    path!(data_dir, MC_DIR, VERSION_DIR, filename)
  }

  async fn neoforge_version_pairs(&self, data_dir: &Path) -> Result<Vec<(String, String)>> {
    let path = self.index(data_dir);
    let forge_versions = read_parse_xml_file::<NeoForgeIndex>(&path).await?;
    let path = self.index_2(data_dir);
    let neoforge_versions = read_parse_xml_file::<NeoForgeIndex>(&path).await?;

    let forge_versions_parsed = forge_versions
      .versioning
      .versions
      .version
      .into_iter()
      .flat_map(|v| forge_version_pair(&v))
      .collect::<Vec<_>>();

    let neoforge_versions_parsed = neoforge_versions
      .versioning
      .versions
      .version
      .into_iter()
      .flat_map(|v| neoforge_version_pair(&v))
      .collect::<Vec<_>>();

    let mut versions = forge_versions_parsed;
    versions.extend(neoforge_versions_parsed);
    Ok(versions)
  }
}

#[async_trait::async_trait]
impl Loader for ForgeLikeLoader {
  async fn download_metadata(&self, client: &Client, data_dir: &Path) -> Result<()> {
    let url = Url::parse(&self.index_url)?;
    let path = self.index(data_dir);
    download_file_no_hash_force(client, &path, url).await?;

    if let Some(url_2) = &self.index_url_2 {
      let url = Url::parse(url_2)?;
      let path = self.index_2(data_dir);
      download_file_no_hash_force(client, &path, url).await?;
    }

    Ok(())
  }

  async fn supported_versions(&self, data_dir: &Path, _: bool) -> Result<Vec<String>> {
    let path = self.index(data_dir);
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
      let versions = self.neoforge_version_pairs(data_dir).await?;
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
    data_dir: &Path,
    _: bool,
  ) -> Result<Vec<String>> {
    let path = self.index(data_dir);
    if self.index_file_name == INDEX_FILE_NAME_FORGE {
      let versions = read_parse_file::<VersionIndex>(&path)
        .await?
        .get(mc_version)
        .cloned()
        .unwrap_or_default();

      let mut versions: Vec<String> = versions
        .into_iter()
        .filter(|v| !v.contains("pre"))
        .flat_map(|v| anyhow::Ok(forge_version_pair(&v)?.1))
        .collect();
      versions.reverse();

      Ok(versions)
    } else {
      let versions = self.neoforge_version_pairs(data_dir).await?;
      let mut versions: Vec<String> = versions
        .into_iter()
        .filter(|(mc, _)| mc == mc_version)
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
  installer_url: String,
  index_file_name: String,
  maven_base_url: String,
}

impl ForgeLikeLoaderVersion {
  pub fn forge(mc_version: String, loader_version: String) -> Self {
    let installer_url = INSTALLER_URL_FORGE.replace(
      "{loader_version}",
      &format!("{}-{}", mc_version, loader_version),
    );
    Self {
      mc_version,
      loader_version,
      installer_url,
      index_file_name: INDEX_FILE_NAME_FORGE.to_string(),
      maven_base_url: MAVEN_BASE_URL_FORGE.to_string(),
    }
  }

  pub fn neoforge(mc_version: String, loader_version: String) -> Self {
    let installer_url = INSTALLER_URL_NEOFORGE.replace(
      "{loader_version}",
      &format!("{}-{}", mc_version, loader_version),
    );
    Self {
      mc_version,
      loader_version,
      installer_url,
      index_file_name: INDEX_FILE_NAME_NEOFORGE.to_string(),
      maven_base_url: MAVEN_BASE_URL_NEOFORGE.to_string(),
    }
  }

  fn installer_path(&self, data_dir: &Path) -> PathBuf {
    path!(
      data_dir,
      MC_DIR,
      VERSION_DIR,
      &self.mc_version,
      format!("{}-{}", self.index_file_name, self.loader_version)
    )
  }
}

const INSTALLER_PATH: &str = "installer.jar";
const INSTALLER_PROFILE_PATH: &str = "install_profile.json";

#[async_trait::async_trait]
impl LoaderVersion for ForgeLikeLoaderVersion {
  async fn download(&self, client: &Client, data_dir: &PathBuf) -> Result<Vec<CheckFuture>> {
    let url = Url::parse(&self.installer_url)?;
    let path = self.installer_path(data_dir).join(INSTALLER_PATH);
    download_file_no_hash_force(client, &path, url).await?;

    let profile_path = self.installer_path(data_dir).join(INSTALLER_PROFILE_PATH);
    let installer_data = extract_file_from_zip(&path, INSTALLER_PROFILE_PATH).await?;
    fs::write(&profile_path, &installer_data).await?;

    let profile = read_parse_file::<ForgeInstallerProfile>(&profile_path).await?;

    for entry in profile.data.values() {
      if entry.client.starts_with("/") {
        let file_path = self.installer_path(data_dir).join(&entry.client[1..]);
        let data = extract_file_from_zip(&path, &entry.client[1..]).await?;

        let parent = file_path.parent().unwrap();
        fs::create_dir_all(parent).await?;
        fs::write(&file_path, &data).await?;
      }
    }

    let mut futures = Vec::new();
    for library in profile.libraries {
      let data_dir = data_dir.clone();
      let base_url = self.maven_base_url.clone();
      let client = client.clone();

      futures.push(download_maven_future(
        data_dir,
        library.name,
        client,
        base_url,
        Some(library.downloads.artifact.sha1),
      ))
    }

    Ok(futures)
  }

  async fn preprocess(&self, data_dir: &Path, jre_bin: PathBuf) -> Result<()> {
    let profile_path = self.installer_path(data_dir).join(INSTALLER_PROFILE_PATH);
    let profile: ForgeInstallerProfile = read_parse_file(&profile_path).await?;
    let mut data = profile.data;

    for entry in data.values_mut() {
      if entry.client.starts_with("/") {
        entry.client = self
          .installer_path(data_dir)
          .join(&entry.client[1..])
          .to_string_lossy()
          .into_owned();
      }
      if entry.server.starts_with("/") {
        entry.server = self
          .installer_path(data_dir)
          .join(&entry.server[1..])
          .to_string_lossy()
          .into_owned();
      }
    }
    default_data(&mut data, &self.mc_version, data_dir);

    for processor in profile.processors {
      if let Some(sides) = processor.sides
        && !sides.contains(&"client".to_string())
      {
        continue; // Skip processors that are not for the client side
      }

      let jar_maven = parse_maven_name(&processor.jar)?;
      let jar_path = full_path_from_maven(data_dir, &jar_maven);

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
        let path = full_path_from_maven(data_dir, &maven);
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
          let path = full_path_from_maven(data_dir, &maven);
          args.push(path.to_string_lossy().into_owned());
        } else {
          args.push(arg);
        }
      }

      let mut command = Command::new(&jre_bin);

      command
        .current_dir(self.installer_path(data_dir))
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

  async fn classpath(&self, data_dir: &Path) -> Result<Vec<(MavenName, PathBuf)>> {
    Ok(vec![])
  }

  async fn main_class(&self, data_dir: &Path) -> Result<String> {
    Ok("net.minecraftforge.fml.common.Mod".into())
  }
}

fn default_data(data: &mut HashMap<String, DataEntry>, mc_version: &str, data_dir: &Path) {
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
      client: path!(
        data_dir,
        MC_DIR,
        VERSION_DIR,
        mc_version,
        format!("{}.jar", mc_version)
      )
      .to_string_lossy()
      .into_owned(),
      server: String::new(),
    },
  );

  data.insert(
    "ROOT".to_string(),
    DataEntry {
      client: path!(data_dir, MC_DIR, VERSION_DIR, "root")
        .to_string_lossy()
        .into_owned(),
      server: String::new(),
    },
  );

  data.insert(
    "LIBRARY_DIR".to_string(),
    DataEntry {
      client: path!(data_dir, MC_DIR, LIBRARY_DIR)
        .to_string_lossy()
        .into_owned(),
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
  let forge_version = parts[1..].join("-");
  Ok((mc_version, forge_version))
}

fn neoforge_version_pair(version_string: &str) -> Result<(String, String)> {
  // format: x.y.z where x is x.y is the Minecraft version and z is the NeoForge version
  // the mc version is presented as "1.x.y" and the NeoForge version is presented as "z"
  // e.g., "16.5-36.2.39" => ("1.16.5", "36.2.39")
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
  spec: usize,
  profile: String,
  version: String,
  path: String,
  minecraft: String,
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
  url: Url,
  sha1: String,
  size: u64,
}
