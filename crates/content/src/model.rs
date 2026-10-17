use serde::{Deserialize, Serialize};

use proton_versions::loader::LoaderType;

pub use proton_profiles::config::Source;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
  Mod,
  Modpack,
  ResourcePack,
  Shader,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
  Release,
  Beta,
  Alpha,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind {
  Required,
  Optional,
  Incompatible,
  Embedded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSort {
  Relevance,
  Downloads,
  Updated,
  Newest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
  Client,
  Server,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSummary {
  pub source: Source,
  pub id: String,
  pub kind: ContentKind,
  pub name: String,
  pub author: String,
  pub description: String,
  pub icon_url: Option<String>,
  pub categories: Vec<String>,
  pub loaders: Vec<String>,
  pub game_versions: Vec<String>,
  pub downloads: u64,
  pub updated_at: String,
  pub client_side: Option<String>,
  pub server_side: Option<String>,
  pub url: String,
}

#[derive(Debug, Clone)]
pub struct ProjectDetail {
  pub summary: ProjectSummary,
  pub body: String,
  pub gallery: Vec<GalleryImage>,
  pub links: ProjectLinks,
}

#[derive(Debug, Clone)]
pub struct GalleryImage {
  pub url: String,
  pub title: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectLinks {
  pub source: Option<String>,
  pub issues: Option<String>,
  pub wiki: Option<String>,
  pub discord: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileInfo {
  pub file_name: String,
  /// None for restricted CurseForge files
  pub url: Option<String>,
  pub sha1: String,
  pub sha512: Option<String>,
  pub size: u64,
  pub primary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
  pub project_id: String,
  pub version_id: Option<String>,
  pub kind: DependencyKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionInfo {
  pub source: Source,
  pub project_id: String,
  pub id: String,
  pub name: String,
  pub version_number: String,
  pub channel: Channel,
  pub game_versions: Vec<String>,
  pub loaders: Vec<String>,
  pub published_at: String,
  pub files: Vec<FileInfo>,
  pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchQuery {
  pub kind: ContentKind,
  pub query: String,
  pub sources: Vec<Source>,
  pub loaders: Vec<String>,
  pub game_version: Option<String>,
  pub categories: Vec<String>,
  pub environment: Option<Environment>,
  pub sort: SearchSort,
  pub offset: u32,
  pub limit: u32,
  pub compatible_with: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
  pub hits: Vec<ProjectSummary>,
  pub total: u64,
  pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Category {
  pub name: String,
  pub header: String,
  pub kind: ContentKind,
}

/// Hashes of a local file, used to identify it on both providers
#[derive(Debug, Clone)]
pub struct FileHashes {
  pub sha1: String,
  /// CurseForge Murmur2 fingerprint
  pub fingerprint: u32,
}

impl Channel {
  pub fn as_str(self) -> &'static str {
    match self {
      Channel::Release => "release",
      Channel::Beta => "beta",
      Channel::Alpha => "alpha",
    }
  }
}

impl ContentKind {
  /// Content folder inside the profile, None for modpacks (they are profiles)
  pub fn dir(self) -> Option<&'static str> {
    match self {
      ContentKind::Mod => Some("mods"),
      ContentKind::ResourcePack => Some("resourcepacks"),
      ContentKind::Shader => Some("shaderpacks"),
      ContentKind::Modpack => None,
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
  pub kind: ContentKind,
  pub file_name: String,
  pub enabled: bool,
  pub manual: bool,
  pub from_pack: bool,
  pub source: Option<Source>,
  pub project_id: Option<String>,
  pub version_id: Option<String>,
  pub name: Option<String>,
  pub icon_url: Option<String>,
  pub version_number: Option<String>,
  pub dependency_of: Vec<String>,
  pub order: Option<usize>,
  pub active: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct PickRef {
  pub source: Source,
  pub project_id: String,
  pub version_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ManualFile {
  pub source: Source,
  pub project_id: String,
  pub file_id: String,
  pub file_name: String,
  pub sha1: String,
  pub size: u64,
  /// Browser link that starts the download of this exact file
  pub download_url: String,
}

#[derive(Debug, Clone)]
pub struct PlanItem {
  pub name: String,
  pub icon_url: Option<String>,
  pub kind: ContentKind,
  pub version: VersionInfo,
  /// Project id that required this item
  pub dependency_of: Option<String>,
  /// File name this item replaces
  pub replaces: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRef {
  pub project_id: String,
  pub name: String,
  /// Project id of the item that requires this installed project
  pub dependency_of: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionalRef {
  pub source: Source,
  pub project_id: String,
  pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
  pub project_id: String,
  pub name: String,
  pub reason: String,
}

#[derive(Debug, Clone)]
pub struct InstallPlan {
  pub profile: String,
  pub items: Vec<PlanItem>,
  pub already_installed: Vec<ProjectRef>,
  pub optional: Vec<OptionalRef>,
  pub conflicts: Vec<Conflict>,
  pub manual: Vec<ManualFile>,
  pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PackSource {
  Remote {
    source: Source,
    project_id: String,
    version_id: Option<String>,
  },
  Path {
    path: std::path::PathBuf,
  },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackFile {
  pub path: String,
  pub size: u64,
  pub sha1: String,
  pub sha512: Option<String>,
  pub url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PackCounts {
  pub mods: usize,
  pub resourcepacks: usize,
  pub shaders: usize,
}

#[derive(Debug, Clone)]
pub struct PackPlan {
  pub name: String,
  pub version: String,
  pub summary: Option<String>,
  pub icon_url: Option<String>,
  /// Remote sources always carry the resolved version id
  pub source: PackSource,
  pub minecraft: String,
  pub loader: LoaderType,
  pub loader_version: Option<String>,
  pub files: Vec<PackFile>,
  pub manual: Vec<ManualFile>,
  pub counts: PackCounts,
  pub recommended_memory_mb: Option<u64>,
  pub total_bytes: u64,
  /// The local pack file, remote packs are cached until installed
  pub archive: std::path::PathBuf,
  /// Override folders inside the archive
  pub overrides: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateInfo {
  pub profile: String,
  pub kind: ContentKind,
  pub file_name: String,
  pub source: Source,
  pub project_id: String,
  pub name: String,
  /// Installed version number
  pub current: String,
  pub latest: VersionInfo,
}

/// A newer version of the pack a profile was installed from
#[derive(Debug, Clone, PartialEq)]
pub struct PackUpdate {
  /// Installed version number, empty when the provider no longer lists it
  pub current: String,
  pub latest: VersionInfo,
}

impl VersionInfo {
  /// The file that gets installed
  pub fn primary_file(&self) -> Option<&FileInfo> {
    self.files.iter().find(|f| f.primary).or(self.files.first())
  }
}
