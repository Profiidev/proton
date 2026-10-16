use std::{
  collections::{BTreeMap, HashMap},
  path::{Component, Path, PathBuf},
};

use anyhow::Result;
use async_zip::{
  Compression, ZipEntryBuilder,
  base::{read1::seek::ZipArchiveReader, write::ZipFileWriter},
};
use chrono::Utc;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use tauri::async_runtime::spawn_blocking;
use tokio::{
  fs,
  io::{AsyncReadExt, BufReader},
  select,
  sync::Notify,
};
use tokio_util::compat::{
  Compat, FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt,
};
use uuid::Uuid;

use super::{
  ContentError, MANIFEST, TMP_DIR,
  curseforge::fingerprint,
  install::{Fetch, Progress, download_url, fetch_all, newest},
  model::{
    ContentKind, ManualFile, PackCounts, PackFile, PackPlan, PackSource, ProjectSummary, Source,
  },
  provider::{ContentProvider, Providers},
  store::{self, Entry, safe_name},
};
use proton_profiles::config::Profile;
use proton_utils::file::bytes_hash;
use proton_versions::{event::DownloadCheckStatus, loader::LoaderType};

const INDEX: &str = "modrinth.index.json";
const CF_MANIFEST: &str = "manifest.json";
const OVERRIDES: &str = "overrides";
/// Hosts mrpack files may be downloaded from
const MRPACK_HOSTS: &[&str] = &[
  "cdn.modrinth.com",
  "github.com",
  "raw.githubusercontent.com",
  "gitlab.com",
];
/// Top level entries that never go into an export
const EXPORT_SKIP: &[&str] = &[
  "profile.json",
  "image.png",
  "instance_logs",
  "logs",
  "crash-reports",
  "saves",
  TMP_DIR,
  MANIFEST,
];
/// Top level entries a duplicate does not copy
const DUPLICATE_SKIP: &[&str] = &["logs", "crash-reports", "instance_logs", TMP_DIR];
/// Top level entries that belong to Proton, overrides never write them
const OVERRIDE_SKIP: &[&str] = &["profile.json", "image.png", "instance_logs", MANIFEST];
const CONTENT_KINDS: [ContentKind; 3] = [
  ContentKind::Mod,
  ContentKind::ResourcePack,
  ContentKind::Shader,
];

type Zip = ZipArchiveReader<Compat<BufReader<fs::File>>>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
  format_version: u32,
  game: String,
  version_id: String,
  name: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  summary: Option<String>,
  files: Vec<IndexFile>,
  dependencies: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
  path: String,
  hashes: Hashes,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  env: Option<Env>,
  downloads: Vec<String>,
  file_size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Hashes {
  sha1: String,
  sha512: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Env {
  client: String,
  server: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
  minecraft: CfMinecraft,
  name: String,
  #[serde(default)]
  version: String,
  files: Vec<CfManifestFile>,
  #[serde(default = "overrides")]
  overrides: String,
}

fn overrides() -> String {
  OVERRIDES.into()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
  version: String,
  #[serde(default)]
  mod_loaders: Vec<CfLoader>,
  recommended_ram: Option<u64>,
}

#[derive(Deserialize)]
struct CfLoader {
  id: String,
  #[serde(default)]
  primary: bool,
}

#[derive(Deserialize)]
struct CfManifestFile {
  #[serde(rename = "projectID")]
  _project_id: u64,
  #[serde(rename = "fileID")]
  file_id: u64,
  #[serde(default = "yes")]
  required: bool,
}

fn yes() -> bool {
  true
}

/// What the archive says, CurseForge files are only ids
#[derive(Debug)]
struct Pack {
  name: String,
  version: String,
  summary: Option<String>,
  minecraft: String,
  loader: LoaderType,
  loader_version: Option<String>,
  files: Vec<PackFile>,
  curseforge: Vec<u64>,
  /// Folders extracted in this order, later ones win
  overrides: Vec<String>,
  memory: Option<u64>,
}

/// A relative path that stays inside its base: only normal components, no `\` or drive letters
pub fn safe_rel(path: &str) -> Option<PathBuf> {
  let p = Path::new(path);
  (!path.is_empty()
    && !path.contains(['\\', ':'])
    && p.components().all(|c| matches!(c, Component::Normal(_))))
  .then(|| p.to_path_buf())
}

pub fn mrpack_host(url: &str) -> bool {
  Url::parse(url)
    .is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some_and(|h| MRPACK_HOSTS.contains(&h)))
}

/// Loader version in the format of the loader lists: Forge `47.2.0`, NeoForge `77` of `21.1.77`
fn loader_version(loader: LoaderType, minecraft: &str, raw: &str) -> String {
  let raw = raw.strip_prefix(&format!("{minecraft}-")).unwrap_or(raw);
  match loader {
    LoaderType::NeoForge => raw.splitn(3, '.').nth(2).unwrap_or(raw).to_string(),
    _ => raw.to_string(),
  }
}

/// Inverse of `loader_version` for exports
fn pack_loader_version(loader: LoaderType, minecraft: &str, version: &str) -> String {
  match loader {
    LoaderType::NeoForge => {
      let parts: Vec<_> = minecraft.split('.').collect();
      let minor = parts.get(1).copied().unwrap_or("0");
      let patch = parts.get(2).copied().unwrap_or("0");
      format!("{minor}.{patch}.{version}")
    }
    _ => version.to_string(),
  }
}

/// mrpack dependency key of a loader
fn index_key(loader: LoaderType) -> Option<&'static str> {
  match loader {
    LoaderType::NeoForge => Some("neoforge"),
    LoaderType::Forge => Some("forge"),
    LoaderType::Quilt => Some("quilt-loader"),
    LoaderType::Fabric => Some("fabric-loader"),
    LoaderType::Vanilla => None,
  }
}

fn invalid<E>(_: E) -> ContentError {
  ContentError::InvalidPack
}

fn parse_index(data: &[u8]) -> Result<Pack> {
  let index: Index = serde_json::from_slice(data).map_err(invalid)?;
  if index.format_version != 1 || index.game != "minecraft" {
    return Err(ContentError::InvalidPack.into());
  }
  let minecraft = index
    .dependencies
    .get("minecraft")
    .ok_or(ContentError::InvalidPack)?
    .clone();
  let (loader, raw) = [
    LoaderType::NeoForge,
    LoaderType::Forge,
    LoaderType::Quilt,
    LoaderType::Fabric,
  ]
  .into_iter()
  .find_map(|l| Some((l, Some(index.dependencies.get(index_key(l)?)?))))
  .unwrap_or((LoaderType::Vanilla, None));

  let mut files = Vec::new();
  for file in index.files {
    if file.env.as_ref().is_some_and(|e| e.client == "unsupported") {
      continue;
    }
    safe_rel(&file.path).ok_or(ContentError::InvalidPack)?;
    let url = file
      .downloads
      .into_iter()
      .find(|u| mrpack_host(u))
      .ok_or(ContentError::InvalidPack)?;
    files.push(PackFile {
      path: file.path,
      size: file.file_size,
      sha1: file.hashes.sha1,
      sha512: Some(file.hashes.sha512),
      url: Some(url),
    });
  }

  Ok(Pack {
    name: index.name,
    version: index.version_id,
    summary: index.summary,
    loader_version: raw.map(|v| loader_version(loader, &minecraft, v)),
    minecraft,
    loader,
    files,
    curseforge: Vec::new(),
    overrides: vec![OVERRIDES.into(), "client-overrides".into()],
    memory: None,
  })
}

fn parse_cf_manifest(data: &[u8]) -> Result<Pack> {
  let manifest: CfManifest = serde_json::from_slice(data).map_err(invalid)?;
  let minecraft = manifest.minecraft.version;
  let loaders = &manifest.minecraft.mod_loaders;
  let (loader, raw) = match loaders
    .iter()
    .find(|l| l.primary)
    .or(loaders.first())
    .and_then(|l| l.id.split_once('-'))
  {
    None => (LoaderType::Vanilla, None),
    Some((name, version)) => {
      let loader = match name {
        "forge" => LoaderType::Forge,
        "neoforge" => LoaderType::NeoForge,
        "fabric" => LoaderType::Fabric,
        "quilt" => LoaderType::Quilt,
        _ => return Err(ContentError::Unsupported.into()),
      };
      (loader, Some(version))
    }
  };

  Ok(Pack {
    name: manifest.name,
    version: manifest.version,
    summary: None,
    loader_version: raw.map(|v| loader_version(loader, &minecraft, v)),
    minecraft,
    loader,
    files: Vec::new(),
    curseforge: manifest
      .files
      .iter()
      .filter(|f| f.required)
      .map(|f| f.file_id)
      .collect(),
    overrides: vec![manifest.overrides],
    memory: manifest.minecraft.recommended_ram,
  })
}

async fn open_zip(path: &Path) -> Result<Zip> {
  let file = fs::File::open(path)
    .await
    .map_err(|_| ContentError::NotFound)?;
  Ok(
    ZipArchiveReader::open(BufReader::new(file).compat())
      .await
      .map_err(invalid)?,
  )
}

async fn read_entry(zip: &mut Zip, index: usize) -> Result<Vec<u8>> {
  let mut bytes = Vec::new();
  zip
    .file(index)
    .await?
    .compat()
    .read_to_end(&mut bytes)
    .await?;
  Ok(bytes)
}

/// Parses `modrinth.index.json` or a CurseForge `manifest.json`
async fn read_pack(path: &Path) -> Result<Pack> {
  let mut zip = open_zip(path).await?;
  for (name, parse) in [
    (INDEX, parse_index as fn(&[u8]) -> Result<Pack>),
    (CF_MANIFEST, parse_cf_manifest),
  ] {
    let found = zip.find(name.as_bytes()).map_err(invalid)?.next();
    if let Some(i) = found {
      return parse(&read_entry(&mut zip, i).await?);
    }
  }
  Err(ContentError::InvalidPack.into())
}

/// Remote packs are cached per version until installed, install re-opens the cached file and
/// downloads it again when it is gone or does not match the sha1. Returns (archive, version id).
async fn fetch_archive(
  providers: &Providers,
  cache: &Path,
  source: Source,
  project: &str,
  version: Option<&str>,
) -> Result<(PathBuf, String)> {
  let path = |version: &str| -> Result<PathBuf> {
    let name = format!("{source:?}-{version}.zip");
    Ok(cache.join("modpacks").join(safe_name(&name)?))
  };
  let version = match (source, version) {
    (Source::CurseForge, Some(id)) => {
      let id = id.parse().map_err(|_| ContentError::NotFound)?;
      providers.curseforge().files(&[id]).await?.pop()
    }
    (_, version) => {
      let versions = providers.get(source).versions(project, &[], None).await?;
      match version {
        Some(id) => versions.into_iter().find(|v| v.id == id),
        None => newest(versions),
      }
    }
  }
  .filter(|v| v.project_id == project)
  .ok_or(ContentError::NotFound)?;
  let file = version.primary_file().ok_or(ContentError::NotFound)?;
  let url = file
    .url
    .as_deref()
    .ok_or(ContentError::NeedsManualDownload)?;
  let archive = path(&version.id)?;
  // the cached file is only used while it still matches
  if let Ok(bytes) = fs::read(&archive).await
    && spawn_blocking(move || bytes_hash(&bytes)).await?? == file.sha1
  {
    return Ok((archive, version.id));
  }
  let fetch = Fetch {
    source,
    url: download_url(source, url)?,
    path: archive.clone(),
    sha1: file.sha1.clone(),
    sha512: file.sha512.clone(),
  };
  fetch_all(
    providers,
    vec![fetch],
    file.size,
    std::sync::Arc::new(|_| {}),
  )
  .await?;
  Ok((archive, version.id))
}

/// Local path of the pack archive
pub async fn archive(providers: &Providers, cache: &Path, source: &PackSource) -> Result<PathBuf> {
  match source {
    PackSource::Path { path } => Ok(path.clone()),
    PackSource::Remote {
      source,
      project_id,
      version_id,
    } => Ok(
      fetch_archive(providers, cache, *source, project_id, version_id.as_deref())
        .await?
        .0,
    ),
  }
}

/// Resolves CurseForge file ids in bulk. Restricted files become (path, manual file).
async fn resolve_curseforge(
  providers: &Providers,
  ids: &[u64],
) -> Result<(Vec<PackFile>, Vec<(String, ManualFile)>)> {
  let cf = providers.curseforge();
  let versions = cf.files(ids).await?;
  if versions.len() < ids.len() {
    tracing::warn!(
      "CurseForge returned {} of {} pack files",
      versions.len(),
      ids.len()
    );
  }
  let mut project_ids: Vec<String> = versions.iter().map(|v| v.project_id.clone()).collect();
  project_ids.sort();
  project_ids.dedup();
  let projects: HashMap<String, ProjectSummary> = cf
    .summaries(&project_ids)
    .await?
    .into_iter()
    .map(|p| (p.id.clone(), p))
    .collect();

  let mut files = Vec::new();
  let mut manual = Vec::new();
  for version in versions {
    let Some(file) = version.primary_file() else {
      continue;
    };
    let project = projects.get(&version.project_id);
    let folder = project.and_then(|p| p.kind.dir()).unwrap_or("mods");
    let path = format!("{folder}/{}", safe_name(&file.file_name)?);
    match &file.url {
      Some(url) => files.push(PackFile {
        path,
        size: file.size,
        sha1: file.sha1.clone(),
        sha512: None,
        url: Some(url.clone()),
      }),
      None => {
        let project_url = project.map(|p| p.url.clone()).unwrap_or_default();
        manual.push((
          path,
          ManualFile {
            source: Source::CurseForge,
            project_id: version.project_id.clone(),
            file_id: version.id.clone(),
            file_name: file.file_name.clone(),
            sha1: file.sha1.clone(),
            size: file.size,
            download_url: format!("{project_url}/download/{}", version.id),
            project_url,
          },
        ))
      }
    }
  }
  Ok((files, manual))
}

fn count(counts: &mut PackCounts, path: &str) {
  match path.split_once('/').map(|(dir, _)| dir) {
    Some("mods") => counts.mods += 1,
    Some("resourcepacks") => counts.resourcepacks += 1,
    Some("shaderpacks") => counts.shaders += 1,
    _ => counts.other += 1,
  }
}

/// Reads the pack and resolves everything an install needs
pub async fn plan(providers: &Providers, cache: &Path, source: PackSource) -> Result<PackPlan> {
  let (archive, source, project) = match source {
    PackSource::Path { path } => (path.clone(), PackSource::Path { path }, None),
    PackSource::Remote {
      source,
      project_id,
      version_id,
    } => {
      let (archive, version_id) =
        fetch_archive(providers, cache, source, &project_id, version_id.as_deref()).await?;
      let project = providers
        .get(source)
        .summaries(std::slice::from_ref(&project_id))
        .await?
        .pop();
      let source = PackSource::Remote {
        source,
        project_id,
        version_id: Some(version_id),
      };
      (archive, source, project)
    }
  };

  let pack = read_pack(&archive).await?;
  let mut files = pack.files;
  let mut manual = Vec::new();
  if !pack.curseforge.is_empty() {
    let (resolved, restricted) = resolve_curseforge(providers, &pack.curseforge).await?;
    files.extend(resolved);
    manual = restricted;
  }

  let mut counts = PackCounts::default();
  for path in files
    .iter()
    .map(|f| &f.path)
    .chain(manual.iter().map(|m| &m.0))
  {
    count(&mut counts, path);
  }
  Ok(PackPlan {
    name: pack.name,
    version: pack.version,
    summary: pack
      .summary
      .or(project.as_ref().map(|p| p.description.clone())),
    icon_url: project.and_then(|p| p.icon_url),
    source,
    minecraft: pack.minecraft,
    loader: pack.loader,
    loader_version: pack.loader_version,
    total_bytes: files.iter().map(|f| f.size).sum(),
    files,
    manual: manual.into_iter().map(|m| m.1).collect(),
    counts,
    recommended_memory_mb: pack.memory,
  })
}

/// Downloads the pack icon for the new profile, best effort, `None` when missing or not an image
pub async fn icon(providers: &Providers, plan: &PackPlan) -> Option<Vec<u8>> {
  let url = Url::parse(plan.icon_url.as_deref()?).ok()?;
  if url.scheme() != "https" {
    return None;
  }
  let source = match plan.source {
    PackSource::Remote { source, .. } => source,
    PackSource::Path { .. } => Source::Modrinth,
  };
  let res = providers
    .download_client(source)
    .get(url)
    .send()
    .await
    .ok()?
    .error_for_status()
    .ok()?;
  let bytes = res.bytes().await.ok()?;
  image::load_from_memory(&bytes).ok()?;
  Some(bytes.to_vec())
}

/// (kind, file name without `.disabled`) for a path inside a content folder
fn content_file(rel: &str) -> Option<(ContentKind, String)> {
  let (dir, name) = rel.split_once('/')?;
  let kind = CONTENT_KINDS.into_iter().find(|k| k.dir() == Some(dir))?;
  if name.contains('/') {
    return None;
  }
  store::parse_name(kind, name).map(|(name, _)| (kind, name))
}

/// Manifest entry for a pack file, identified on the next lookup
fn pack_entry(kind: ContentKind, file_name: String, sha1: String, fingerprint: u32) -> Entry {
  Entry {
    kind,
    file_name,
    sha1,
    fingerprint,
    manual: true,
    from_pack: true,
    installed_at: Some(Utc::now().to_rfc3339()),
    source: None,
    project_id: None,
    version_id: None,
    name: None,
    icon_url: None,
    version_number: None,
    channel: None,
    dependency_of: Vec::new(),
    pending: true,
  }
}

/// Extracts `<prefix>/...` entries into `dir`, prefixes in order. Returns the content files
/// written as (relative path, sha1, fingerprint).
async fn extract_overrides(
  archive: &Path,
  prefixes: &[String],
  dir: &Path,
  progress: &Progress,
) -> Result<Vec<(String, String, u32)>> {
  let mut zip = open_zip(archive).await?;
  let mut jobs = Vec::new();
  for prefix in prefixes {
    for (i, cdr) in zip.cdrs().iter().enumerate() {
      let Some(rel) = cdr
        .insecure_file_name
        .as_str()
        .and_then(|n| n.strip_prefix(prefix.as_str())?.strip_prefix('/'))
      else {
        continue;
      };
      let top = rel.split('/').next().unwrap_or_default();
      if rel.is_empty() || rel.ends_with('/') || OVERRIDE_SKIP.contains(&top) {
        continue;
      }
      // zip-slip
      safe_rel(rel).ok_or(ContentError::InvalidPack)?;
      jobs.push((i, rel.to_string()));
    }
  }

  let total = jobs.len();
  let mut content = Vec::new();
  for (n, (i, rel)) in jobs.into_iter().enumerate() {
    progress(DownloadCheckStatus::ModpackOverrides(n, total));
    let bytes = read_entry(&mut zip, i).await?;
    let path = dir.join(&rel);
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).await?;
    }
    fs::write(&path, &bytes).await?;
    if content_file(&rel).is_some() {
      let (sha1, fingerprint) =
        spawn_blocking(move || Ok::<_, anyhow::Error>((bytes_hash(&bytes)?, fingerprint(&bytes))))
          .await??;
      content.push((rel, sha1, fingerprint));
    }
  }
  progress(DownloadCheckStatus::ModpackOverrides(total, total));
  Ok(content)
}

/// Downloads the pack files into the profile, replaces the files of a previously installed pack
/// version, applies the overrides and writes the manifest. Ok(false) when canceled.
pub async fn install_files(
  providers: &Providers,
  dir: &Path,
  archive: &Path,
  plan: &PackPlan,
  id: usize,
  cancel: &Notify,
  progress: Progress,
) -> Result<bool> {
  let run = async {
    let pack = read_pack(archive).await?;
    let tmp = dir.join(TMP_DIR).join(id.to_string());
    let mut fetches = Vec::new();
    for file in &plan.files {
      let rel = safe_rel(&file.path).ok_or(ContentError::InvalidPack)?;
      let url = file
        .url
        .as_deref()
        .ok_or(ContentError::NeedsManualDownload)?;
      let source = if mrpack_host(url) {
        Source::Modrinth
      } else {
        // only accepts the CurseForge CDN
        Source::CurseForge
      };
      fetches.push(Fetch {
        source,
        url: download_url(source, url)?,
        path: tmp.join(rel),
        sha1: file.sha1.clone(),
        sha512: file.sha512.clone(),
      });
    }
    let fingerprints = fetch_all(providers, fetches, plan.total_bytes, progress.clone()).await?;

    // a version change replaces the old pack files, only after the new ones are downloaded
    // ponytail: old pack files outside the content folders stay, track them when packs leave junk
    let mut manifest = store::load(dir).await;
    for entry in manifest.entries.iter().filter(|e| e.from_pack) {
      let path = store::disk_path(dir, entry.kind, &entry.file_name)?;
      if path.exists() {
        fs::remove_file(path).await?;
      }
    }
    manifest.entries.retain(|e| !e.from_pack);

    let total = plan.files.len();
    let mut entries = Vec::new();
    for (i, (file, fingerprint)) in plan.files.iter().zip(fingerprints).enumerate() {
      progress(DownloadCheckStatus::ContentFiles(i, total));
      let dest = dir.join(&file.path);
      if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
      }
      fs::rename(tmp.join(&file.path), &dest).await?;
      if let Some((kind, name)) = content_file(&file.path) {
        entries.push(pack_entry(kind, name, file.sha1.clone(), fingerprint));
      }
    }
    progress(DownloadCheckStatus::ContentFiles(total, total));
    let _ = fs::remove_dir_all(dir.join(TMP_DIR)).await;

    for (rel, sha1, fingerprint) in
      extract_overrides(archive, &pack.overrides, dir, &progress).await?
    {
      if let Some((kind, name)) = content_file(&rel) {
        entries.retain(|e| e.kind != kind || e.file_name != name);
        entries.push(pack_entry(kind, name, sha1, fingerprint));
      }
    }

    // names and icons, best effort, failed lookups stay pending for the next scan
    store::identify(providers, entries.iter_mut().collect()).await;
    // content the user added stays unless the pack now ships the same file
    manifest.entries.retain(|e| {
      !entries
        .iter()
        .any(|n| n.kind == e.kind && n.file_name == e.file_name)
    });
    manifest.entries.extend(entries);
    let _lock = store::lock().await;
    store::save(dir, &manifest).await
  };

  let res = select! {
    res = run => Some(res),
    _ = cancel.notified() => None,
  };
  match res {
    Some(Ok(())) => {
      progress(DownloadCheckStatus::Done);
      Ok(true)
    }
    Some(Err(e)) => Err(e),
    None => Ok(false),
  }
}

/// Index files for the enabled Modrinth entries whose installed file Modrinth still serves.
/// Returns the files and their relative paths.
async fn index_files(providers: &Providers, dir: &Path) -> Result<(Vec<IndexFile>, Vec<String>)> {
  let manifest = store::load(dir).await;
  // disabled mods have `.disabled` on disk and go to the overrides
  let candidates: Vec<(&Entry, String)> = manifest
    .entries
    .iter()
    .filter(|e| e.source == Some(Source::Modrinth) && e.version_id.is_some())
    .filter_map(|e| Some((e, format!("{}/{}", e.kind.dir()?, e.file_name))))
    .filter(|(_, rel)| dir.join(rel).is_file())
    .collect();

  let mut ids: Vec<String> = candidates
    .iter()
    .filter_map(|(e, _)| e.version_id.clone())
    .collect();
  ids.sort();
  ids.dedup();
  let versions = providers.modrinth().versions_by_id(&ids).await?;
  let mut projects: Vec<String> = versions.iter().map(|v| v.project_id.clone()).collect();
  projects.sort();
  projects.dedup();
  // env is optional, a failed lookup leaves it out
  let sides: HashMap<String, Env> = providers
    .modrinth()
    .summaries(&projects)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|p| Some((p.id, env(p.client_side?, p.server_side?)?)))
    .collect();
  let urls: HashMap<&str, (&str, &str)> = versions
    .iter()
    .flat_map(|v| {
      v.files
        .iter()
        .filter_map(|f| Some((f.sha1.as_str(), (f.url.as_deref()?, v.project_id.as_str()))))
    })
    .collect();

  let mut files = Vec::new();
  let mut paths = Vec::new();
  for (entry, rel) in candidates {
    let Some((url, project)) = urls.get(entry.sha1.as_str()) else {
      continue;
    };
    let bytes = fs::read(dir.join(&rel)).await?;
    let (sha1, sha512, size) = spawn_blocking(move || {
      Ok::<_, anyhow::Error>((
        bytes_hash(&bytes)?,
        hex::encode(Sha512::digest(&bytes)),
        bytes.len() as u64,
      ))
    })
    .await??;
    // changed since the manifest was written
    if sha1 != entry.sha1 {
      continue;
    }
    files.push(IndexFile {
      path: rel.clone(),
      hashes: Hashes { sha1, sha512 },
      env: sides.get(*project).cloned(),
      downloads: vec![url.to_string()],
      file_size: size,
    });
    paths.push(rel);
  }
  Ok((files, paths))
}

fn env(client: String, server: String) -> Option<Env> {
  let valid = |s: &str| matches!(s, "required" | "optional" | "unsupported");
  (valid(&client) && valid(&server)).then_some(Env { client, server })
}

/// Files below `dir/rel`, as relative paths with `/`
fn walk(dir: &Path, rel: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
  let path = dir.join(rel);
  if path.is_file() {
    let parts: Vec<_> = rel
      .components()
      .filter_map(|c| c.as_os_str().to_str())
      .collect();
    out.push(parts.join("/"));
  } else if path.is_dir() {
    for entry in std::fs::read_dir(path)? {
      walk(dir, &rel.join(entry?.file_name()), out)?;
    }
  }
  Ok(())
}

/// Content files not in the index plus the included paths, without the never-exported entries
fn override_paths(
  dir: &Path,
  indexed: &[String],
  include: &[String],
) -> std::io::Result<Vec<String>> {
  let mut out = Vec::new();
  for folder in CONTENT_KINDS.into_iter().filter_map(ContentKind::dir) {
    walk(dir, Path::new(folder), &mut out)?;
  }
  for rel in include.iter().filter_map(|i| safe_rel(i)) {
    walk(dir, &rel, &mut out)?;
  }
  out.retain(|p| {
    let top = p.split('/').next().unwrap_or_default();
    !indexed.contains(p) && !EXPORT_SKIP.contains(&top)
  });
  out.sort();
  out.dedup();
  Ok(out)
}

async fn write_mrpack(
  dir: &Path,
  out: &Path,
  index: &Index,
  overrides: &[String],
  progress: &Progress,
) -> Result<()> {
  let file = fs::File::create(out).await?;
  let mut zip = ZipFileWriter::new(file.compat_write());
  let entry = |name: String| ZipEntryBuilder::new(name.into(), Compression::Deflate);
  zip
    .write_entry_whole(entry(INDEX.into()), &serde_json::to_vec_pretty(index)?)
    .await?;
  let total = overrides.len();
  for (i, rel) in overrides.iter().enumerate() {
    progress(DownloadCheckStatus::ContentFiles(i, total));
    let data = fs::read(dir.join(rel)).await?;
    zip
      .write_entry_whole(entry(format!("{OVERRIDES}/{rel}")), &data)
      .await?;
  }
  progress(DownloadCheckStatus::ContentFiles(total, total));
  zip.close().await?;
  Ok(())
}

/// Writes the profile as `.mrpack`: Modrinth files go in the index, everything else and the
/// `include` paths into `overrides/`
pub async fn export_mrpack(
  providers: &Providers,
  dir: &Path,
  profile: &Profile,
  out: &Path,
  include: Vec<String>,
  progress: Progress,
) -> Result<()> {
  progress(DownloadCheckStatus::ContentResolve);
  let (files, indexed) = index_files(providers, dir).await?;
  let root = dir.to_path_buf();
  let overrides = spawn_blocking(move || override_paths(&root, &indexed, &include)).await??;

  let mut dependencies = BTreeMap::from([("minecraft".to_string(), profile.version.clone())]);
  if let (Some(key), Some(version)) = (index_key(profile.loader), &profile.loader_version) {
    dependencies.insert(
      key.into(),
      pack_loader_version(profile.loader, &profile.version, version),
    );
  }
  let index = Index {
    format_version: 1,
    game: "minecraft".into(),
    version_id: Utc::now().format("%Y.%m.%d").to_string(),
    name: profile.name.clone(),
    summary: None,
    files,
    dependencies,
  };

  let res = write_mrpack(dir, out, &index, &overrides, &progress).await;
  if res.is_err() {
    let _ = fs::remove_file(out).await;
  }
  res?;
  progress(DownloadCheckStatus::Done);
  Ok(())
}

fn copy_dir(src: &Path, dst: &Path, skip: &[&str]) -> std::io::Result<()> {
  std::fs::create_dir_all(dst)?;
  for entry in std::fs::read_dir(src)? {
    let entry = entry?;
    let name = entry.file_name();
    if name.to_str().is_some_and(|n| skip.contains(&n)) {
      continue;
    }
    let kind = entry.file_type()?;
    if kind.is_dir() {
      copy_dir(&entry.path(), &dst.join(&name), &[])?;
    } else if kind.is_file() {
      std::fs::copy(entry.path(), dst.join(&name))?;
    }
  }
  Ok(())
}

/// Copies the profile folder without logs and temp files, `dst` is removed again on error
pub async fn copy_profile(src: PathBuf, dst: PathBuf) -> Result<()> {
  spawn_blocking(move || {
    let res = copy_dir(&src, &dst, DUPLICATE_SKIP);
    if res.is_err() {
      let _ = std::fs::remove_dir_all(&dst);
    }
    res
  })
  .await??;
  Ok(())
}

/// The copy's config: new id and name, never played
pub fn duplicate(profile: &Profile, name: String) -> Profile {
  Profile {
    id: Uuid::new_v4().to_string(),
    name,
    created_at: Utc::now(),
    last_played: None,
    last_played_non_quick_play: None,
    ..profile.clone()
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use serde_json::json;

  use super::*;
  use crate::store::tests::temp_dir;

  async fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).await.unwrap();
    let mut zip = ZipFileWriter::new(file.compat_write());
    for (name, data) in entries {
      let entry = ZipEntryBuilder::new((*name).into(), Compression::Deflate);
      zip.write_entry_whole(entry, data).await.unwrap();
    }
    zip.close().await.unwrap();
  }

  fn index_file(path: &str, client: &str) -> serde_json::Value {
    json!({
      "path": path, "hashes": {"sha1": "s1", "sha512": "s512"},
      "env": {"client": client, "server": "required"},
      "downloads": ["https://evil.example/x", format!("https://cdn.modrinth.com/{path}")],
      "fileSize": 5
    })
  }

  #[test]
  fn safe_rel_rejects_escapes() {
    assert_eq!(safe_rel("mods/a.jar"), Some(PathBuf::from("mods/a.jar")));
    assert!(safe_rel("config/").is_some());
    for bad in [
      "",
      "../a",
      "mods/../../a",
      "/etc/passwd",
      "C:/x",
      "C:x",
      "mods\\a.jar",
      "./a",
    ] {
      assert_eq!(safe_rel(bad), None, "{bad}");
    }
  }

  #[test]
  fn maps_loader_ids() {
    let v = |l, raw| loader_version(l, "1.21.1", raw);
    assert_eq!(v(LoaderType::Forge, "47.2.0"), "47.2.0");
    assert_eq!(v(LoaderType::Forge, "1.21.1-47.2.0"), "47.2.0");
    assert_eq!(v(LoaderType::NeoForge, "21.1.77"), "77");
    assert_eq!(v(LoaderType::NeoForge, "21.1.0-beta"), "0-beta");
    assert_eq!(v(LoaderType::Fabric, "0.15.0"), "0.15.0");
    assert_eq!(
      pack_loader_version(LoaderType::NeoForge, "1.21.1", "77"),
      "21.1.77"
    );
    assert_eq!(
      pack_loader_version(LoaderType::NeoForge, "1.21", "5"),
      "21.0.5"
    );

    let cf = |id: &str| {
      let manifest = json!({
        "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "fabric-0.1.0"}, {"id": id, "primary": true}]},
        "name": "P", "files": []
      });
      let pack = parse_cf_manifest(manifest.to_string().as_bytes()).unwrap();
      (format!("{:?}", pack.loader), pack.loader_version.unwrap())
    };
    assert_eq!(cf("forge-47.2.0"), ("Forge".into(), "47.2.0".into()));
    assert_eq!(cf("neoforge-21.1.77"), ("NeoForge".into(), "77".into()));
    assert_eq!(cf("quilt-0.26.0"), ("Quilt".into(), "0.26.0".into()));
    assert_eq!(cf("fabric-0.15.0"), ("Fabric".into(), "0.15.0".into()));
  }

  #[tokio::test]
  async fn parses_mrpack_and_curseforge_manifest() {
    let dir = temp_dir("modpack-parse");
    let index = json!({
      "formatVersion": 1, "game": "minecraft", "versionId": "1.2", "name": "Pack", "summary": "S",
      "files": [index_file("mods/a.jar", "required"), index_file("mods/server.jar", "unsupported")],
      "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.0"}
    });
    let mrpack = dir.join("p.mrpack");
    write_zip(&mrpack, &[(INDEX, index.to_string().as_bytes())]).await;
    let pack = read_pack(&mrpack).await.unwrap();
    assert_eq!((pack.name.as_str(), pack.version.as_str()), ("Pack", "1.2"));
    assert!(matches!(pack.loader, LoaderType::Fabric));
    assert_eq!(pack.loader_version.as_deref(), Some("0.16.0"));
    assert_eq!(pack.overrides, ["overrides", "client-overrides"]);
    assert_eq!(pack.files.len(), 1);
    assert_eq!(
      pack.files[0].url.as_deref(),
      Some("https://cdn.modrinth.com/mods/a.jar")
    );

    let bad = json!({
      "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Bad",
      "files": [index_file("../escape.jar", "required")], "dependencies": {"minecraft": "1.21.1"}
    });
    let bad_pack = dir.join("bad.mrpack");
    write_zip(&bad_pack, &[(INDEX, bad.to_string().as_bytes())]).await;
    assert_eq!(
      read_pack(&bad_pack).await.unwrap_err().to_string(),
      "InvalidPack"
    );

    let manifest = json!({
      "minecraft": {"version": "1.20.1", "modLoaders": [{"id": "forge-47.2.0", "primary": true}], "recommendedRam": 6144},
      "manifestType": "minecraftModpack", "name": "CF Pack", "version": "3.0", "overrides": "ov",
      "files": [
        {"projectID": 1, "fileID": 10, "required": true},
        {"projectID": 2, "fileID": 20, "required": false}
      ]
    });
    let zip = dir.join("cf.zip");
    write_zip(&zip, &[(CF_MANIFEST, manifest.to_string().as_bytes())]).await;
    let pack = read_pack(&zip).await.unwrap();
    assert!(matches!(pack.loader, LoaderType::Forge));
    assert_eq!(pack.loader_version.as_deref(), Some("47.2.0"));
    assert_eq!(pack.curseforge, [10]);
    assert_eq!(pack.overrides, ["ov"]);
    assert_eq!(pack.memory, Some(6144));

    let other = dir.join("other.zip");
    write_zip(&other, &[("readme.txt", b"x")]).await;
    assert!(read_pack(&other).await.is_err());

    std::fs::remove_dir_all(&dir).unwrap();
  }

  #[tokio::test]
  async fn overrides_reject_zip_slip_and_apply_in_order() {
    let dir = temp_dir("modpack-overrides");
    let zip = dir.join("p.zip");
    write_zip(
      &zip,
      &[
        ("overrides/config/a.txt", b"base"),
        ("client-overrides/config/a.txt", b"client"),
        ("overrides/mods/b.jar", b"jar"),
        ("overrides/profile.json", b"{}"),
        ("overrides/proton-content.json", b"{}"),
        ("overrides/instance_logs/a.log", b"x"),
        ("other/x.txt", b"x"),
      ],
    )
    .await;
    let out = dir.join("out");
    let progress: Progress = Arc::new(|_| {});
    let prefixes = ["overrides".to_string(), "client-overrides".into()];
    let content = extract_overrides(&zip, &prefixes, &out, &progress)
      .await
      .unwrap();
    assert_eq!(std::fs::read(out.join("config/a.txt")).unwrap(), b"client");
    assert!(!out.join("x.txt").exists() && !out.join("other").exists());
    for skipped in ["profile.json", MANIFEST, "instance_logs"] {
      assert!(!out.join(skipped).exists(), "{skipped}");
    }
    assert_eq!(content.len(), 1);
    assert_eq!(content[0].0, "mods/b.jar");

    let evil = dir.join("evil.zip");
    write_zip(&evil, &[("overrides/../../evil.txt", b"x")]).await;
    assert!(
      extract_overrides(&evil, &prefixes, &out, &progress)
        .await
        .is_err()
    );
    assert!(!dir.parent().unwrap().join("evil.txt").exists());

    std::fs::remove_dir_all(&dir).unwrap();
  }

  #[tokio::test]
  async fn version_change_replaces_pack_files_only() {
    let dir = temp_dir("modpack-version-change");
    let mods = dir.join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("old.jar.disabled"), b"old").unwrap();
    std::fs::write(mods.join("user.jar"), b"user").unwrap();
    let old = pack_entry(ContentKind::Mod, "old.jar".into(), "s".into(), 0);
    let user = Entry {
      from_pack: false,
      ..pack_entry(ContentKind::Mod, "user.jar".into(), "s".into(), 0)
    };
    store::save(
      &dir,
      &store::Manifest {
        entries: vec![old, user],
        ..Default::default()
      },
    )
    .await
    .unwrap();

    let zip = dir.join("p.zip");
    let index = json!({
      "formatVersion": 1, "game": "minecraft", "versionId": "2", "name": "P",
      "files": [], "dependencies": {"minecraft": "1.21.1"}
    });
    write_zip(
      &zip,
      &[
        (INDEX, index.to_string().as_bytes()),
        ("overrides/mods/new.jar", b"new"),
      ],
    )
    .await;
    let plan = plan(
      &Providers::new(),
      &dir,
      PackSource::Path { path: zip.clone() },
    )
    .await
    .unwrap();
    let progress: Progress = Arc::new(|_| {});
    let done = install_files(
      &Providers::new(),
      &dir,
      &zip,
      &plan,
      0,
      &Notify::new(),
      progress,
    )
    .await
    .unwrap();
    assert!(done);
    assert!(!mods.join("old.jar.disabled").exists());
    assert!(mods.join("user.jar").exists() && mods.join("new.jar").exists());
    let mut names: Vec<_> = store::load(&dir)
      .await
      .entries
      .into_iter()
      .map(|e| (e.file_name, e.from_pack))
      .collect();
    names.sort();
    assert_eq!(
      names,
      [("new.jar".into(), true), ("user.jar".into(), false)]
    );

    std::fs::remove_dir_all(&dir).unwrap();
  }

  fn profile() -> Profile {
    serde_json::from_value(json!({
      "id": "p", "name": "My Pack", "created_at": "2026-01-01T00:00:00Z",
      "last_played": "2026-02-01T00:00:00Z", "last_played_non_quick_play": "2026-02-01T00:00:00Z",
      "version": "1.21.1", "loader": "NeoForge", "loader_version": "77", "downloaded": true,
      "use_local_game": false, "game": null, "use_local_jvm": false, "jvm": null
    }))
    .unwrap()
  }

  #[tokio::test]
  async fn export_then_parse_round_trip() {
    let dir = temp_dir("modpack-export");
    let files: &[(&str, &[u8])] = &[
      ("profile.json", b"{}"),
      ("image.png", b"png"),
      ("logs/latest.log", b"log"),
      ("saves/w/level.dat", b"w"),
      ("mods/a.jar", b"a"),
      ("mods/off.jar.disabled", b"off"),
      ("config/c.toml", b"c"),
      ("options.txt", b"o"),
      ("screenshots/s.png", b"s"),
    ];
    for (rel, data) in files {
      let path = dir.join(rel);
      std::fs::create_dir_all(path.parent().unwrap()).unwrap();
      std::fs::write(path, data).unwrap();
    }

    let out = dir.join("export.mrpack");
    let include = vec![
      "config/".into(),
      "options.txt".into(),
      "logs".into(),
      "../x".into(),
    ];
    // no Modrinth entries in the manifest, nothing is looked up
    export_mrpack(
      &Providers::new(),
      &dir,
      &profile(),
      &out,
      include,
      Arc::new(|_| {}),
    )
    .await
    .unwrap();

    let pack = read_pack(&out).await.unwrap();
    assert_eq!(pack.name, "My Pack");
    assert!(matches!(pack.loader, LoaderType::NeoForge));
    assert_eq!(pack.loader_version.as_deref(), Some("77"));
    assert_eq!(pack.minecraft, "1.21.1");

    let target = dir.join("import");
    let progress: Progress = Arc::new(|_| {});
    let content = extract_overrides(&out, &pack.overrides, &target, &progress)
      .await
      .unwrap();
    let mut got = Vec::new();
    walk(&target, Path::new(""), &mut got).unwrap();
    got.sort();
    assert_eq!(
      got,
      [
        "config/c.toml",
        "mods/a.jar",
        "mods/off.jar.disabled",
        "options.txt"
      ]
    );
    assert_eq!(content.len(), 2);

    std::fs::remove_dir_all(&dir).unwrap();
  }

  #[test]
  fn index_files_round_trip() {
    let file = IndexFile {
      path: "mods/a.jar".into(),
      hashes: Hashes {
        sha1: "1".into(),
        sha512: "5".into(),
      },
      env: env("required".into(), "optional".into()),
      downloads: vec!["https://cdn.modrinth.com/data/a/a.jar".into()],
      file_size: 3,
    };
    let index = Index {
      format_version: 1,
      game: "minecraft".into(),
      version_id: "1".into(),
      name: "P".into(),
      summary: None,
      files: vec![file.clone()],
      dependencies: BTreeMap::from([("minecraft".into(), "1.21.1".into())]),
    };
    let pack = parse_index(&serde_json::to_vec(&index).unwrap()).unwrap();
    assert_eq!(
      pack.files,
      [PackFile {
        path: file.path,
        size: 3,
        sha1: "1".into(),
        sha512: Some("5".into()),
        url: Some(file.downloads[0].clone()),
      }]
    );
    assert!(matches!(pack.loader, LoaderType::Vanilla));
    assert_eq!(env("unknown".into(), "required".into()), None);
  }

  #[tokio::test]
  async fn duplicate_skips_logs() {
    let dir = temp_dir("modpack-duplicate");
    let src = dir.join("src");
    for rel in [
      "profile.json",
      "logs/latest.log",
      "crash-reports/c.txt",
      "instance_logs/i.log",
      ".proton-tmp/1/a.jar",
      "mods/a.jar",
      "saves/w/level.dat",
      "config/logs/keep.txt",
    ] {
      let path = src.join(rel);
      std::fs::create_dir_all(path.parent().unwrap()).unwrap();
      std::fs::write(path, b"x").unwrap();
    }
    let dst = dir.join("dst");
    copy_profile(src, dst.clone()).await.unwrap();
    let mut got = Vec::new();
    walk(&dst, Path::new(""), &mut got).unwrap();
    got.sort();
    assert_eq!(
      got,
      [
        "config/logs/keep.txt",
        "mods/a.jar",
        "profile.json",
        "saves/w/level.dat"
      ]
    );

    let copy = duplicate(&profile(), "Copy".into());
    assert_ne!(copy.id, "p");
    assert_eq!(copy.name, "Copy");
    assert!(copy.last_played.is_none() && copy.last_played_non_quick_play.is_none());
    assert_eq!(copy.loader_version.as_deref(), Some("77"));

    std::fs::remove_dir_all(&dir).unwrap();
  }
}
