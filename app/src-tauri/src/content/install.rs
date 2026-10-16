use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use anyhow::Result;
use chrono::Utc;
use reqwest::Url;
use sha2::{Digest, Sha512};
use tauri::async_runtime::spawn_blocking;
use tokio::{fs, select, sync::Notify};

use super::{
  ContentError, TMP_DIR,
  curseforge::{cdn_url, fingerprint},
  model::{
    Channel, Conflict, ContentKind, DependencyKind, InstallPlan, ManualFile, OptionalRef, PickRef,
    PlanItem, PlanReason, ProjectRef, ProjectSummary, Source, VersionInfo,
  },
  modpack::mrpack_host,
  provider::{ContentProvider, Providers, profile_loaders},
  store::{self, Entry, safe_name},
};
use crate::{
  profiles::config::Profile,
  utils::{download::download_file, file::FileError, future::FuturePool},
  versions::{event::DownloadCheckStatus, loader::LoaderType},
};

/// Iris on Modrinth, shaders need it
const IRIS: &str = "YL57xq9U";
const IRIS_CURSEFORGE: &str = "455508";

pub type Progress = Arc<dyn Fn(DownloadCheckStatus) + Send + Sync>;

struct Want {
  source: Source,
  project_id: String,
  version_id: Option<String>,
  dependency_of: Option<String>,
}

/// Resolves picks and their required dependencies (newest compatible versions) into an install
/// plan. `replaces` maps project ids to installed file names that get replaced (updates).
pub async fn plan_install<'p>(
  get: impl Fn(Source) -> &'p dyn ContentProvider,
  profile: &Profile,
  entries: &[Entry],
  picks: &[PickRef],
  replaces: &HashMap<String, String>,
) -> Result<InstallPlan> {
  let mut plan = InstallPlan {
    profile: profile.id.clone(),
    items: Vec::new(),
    already_installed: Vec::new(),
    optional: Vec::new(),
    conflicts: Vec::new(),
    manual: Vec::new(),
    total_bytes: 0,
  };
  // picking another version of an installed project switches to it
  let mut replaces = replaces.clone();
  for pick in picks {
    let Some(version) = &pick.version_id else {
      continue;
    };
    if let Some(entry) = entries.iter().find(|e| {
      e.project_id.as_deref() == Some(pick.project_id.as_str())
        && e.version_id.as_ref() != Some(version)
    }) {
      replaces
        .entry(pick.project_id.clone())
        .or_insert(entry.file_name.clone());
    }
  }
  let replaces = &replaces;
  let installed = |id: &str| {
    !replaces.contains_key(id)
      && entries.iter().any(|e| {
        e.project_id
          .as_deref()
          .is_some_and(|p| p == id || (id == IRIS && p == IRIS_CURSEFORGE))
      })
  };

  let mut visited = HashSet::new();
  let mut optional = Vec::new();
  // (project id, name of the project that declares it incompatible)
  let mut incompatible = Vec::new();
  let mut level: Vec<Want> = picks
    .iter()
    .map(|p| Want {
      source: p.source,
      project_id: p.project_id.clone(),
      version_id: p.version_id.clone(),
      dependency_of: None,
    })
    .collect();

  while !level.is_empty() {
    level.retain(|w| visited.insert((w.source, w.project_id.clone())));
    let projects = summaries(&get, level.iter().map(|w| (w.source, &w.project_id))).await?;
    let mut next = Vec::new();

    for want in level {
      let requested = want.dependency_of.is_none();
      let conflict = |reason: &str, name: &str| Conflict {
        project_id: want.project_id.clone(),
        name: name.to_string(),
        reason: reason.to_string(),
      };
      let Some(project) = projects.get(&(want.source, want.project_id.clone())) else {
        if requested {
          return Err(ContentError::NotFound.into());
        }
        plan
          .conflicts
          .push(conflict("Dependency not found", &want.project_id));
        continue;
      };
      if installed(&want.project_id) {
        plan.already_installed.push(ProjectRef {
          project_id: want.project_id.clone(),
          name: project.name.clone(),
          dependency_of: want.dependency_of.clone(),
        });
        continue;
      }

      let version = match supported(project.kind, profile.loader) {
        Ok(()) => {
          let loaders = profile_loaders(want.source, project.kind, profile.loader);
          let versions = get(want.source)
            .versions(&want.project_id, &loaders, Some(&profile.version))
            .await?;
          match &want.version_id {
            Some(id) => versions.into_iter().find(|v| v.id == *id),
            None => newest(versions),
          }
        }
        Err(e) if requested => return Err(e.into()),
        Err(_) => None,
      };
      let Some((version, file)) =
        version.and_then(|v| Some((v.clone(), v.primary_file()?.clone())))
      else {
        if requested {
          return Err(ContentError::Incompatible.into());
        }
        plan
          .conflicts
          .push(conflict("No compatible version", &project.name));
        continue;
      };

      for dep in &version.dependencies {
        match dep.kind {
          DependencyKind::Required => next.push(Want {
            source: want.source,
            project_id: dep.project_id.clone(),
            version_id: None,
            dependency_of: Some(want.project_id.clone()),
          }),
          DependencyKind::Optional => optional.push((want.source, dep.project_id.clone())),
          DependencyKind::Incompatible => {
            incompatible.push((dep.project_id.clone(), project.name.clone()))
          }
          DependencyKind::Embedded => {}
        }
      }
      if project.kind == ContentKind::Shader {
        next.push(Want {
          source: Source::Modrinth,
          project_id: IRIS.into(),
          version_id: None,
          dependency_of: Some(want.project_id.clone()),
        });
      }

      if file.url.is_none() {
        plan.manual.push(ManualFile {
          source: want.source,
          project_id: want.project_id.clone(),
          file_id: version.id.clone(),
          file_name: file.file_name.clone(),
          sha1: file.sha1.clone(),
          size: file.size,
          project_url: project.url.clone(),
          download_url: format!("{}/download/{}", project.url, version.id),
        });
        continue;
      }
      plan.total_bytes += file.size;
      plan.items.push(PlanItem {
        name: project.name.clone(),
        icon_url: project.icon_url.clone(),
        kind: project.kind,
        reason: if requested {
          PlanReason::Requested
        } else {
          PlanReason::Dependency
        },
        dependency_of: want.dependency_of.clone(),
        replaces: replaces.get(&want.project_id).cloned(),
        version,
      });
    }
    level = next;
  }

  for (id, by) in incompatible {
    let name = plan
      .items
      .iter()
      .find(|i| i.version.project_id == id)
      .map(|i| i.name.clone())
      .or_else(|| {
        let entry = entries
          .iter()
          .find(|e| e.project_id.as_deref() == Some(&id))?;
        Some(entry.name.clone().unwrap_or(entry.file_name.clone()))
      });
    if let Some(name) = name {
      plan.conflicts.push(Conflict {
        project_id: id,
        name,
        reason: format!("Incompatible with {by}"),
      });
    }
  }

  let mut seen = HashSet::new();
  optional.retain(|o| !visited.contains(o) && !installed(&o.1) && seen.insert(o.clone()));
  // names only, a failing lookup drops the suggestions
  if let Ok(projects) = summaries(&get, optional.iter().map(|(s, id)| (*s, id))).await {
    plan.optional = optional
      .into_iter()
      .filter_map(|(source, id)| {
        Some(OptionalRef {
          name: projects.get(&(source, id.clone()))?.name.clone(),
          source,
          project_id: id,
        })
      })
      .collect();
  }

  Ok(plan)
}

/// Whether the profile loader can run content of this kind
fn supported(kind: ContentKind, loader: LoaderType) -> std::result::Result<(), ContentError> {
  match (kind, loader) {
    (ContentKind::Modpack, _) => Err(ContentError::Unsupported),
    (ContentKind::Mod, LoaderType::Vanilla) => Err(ContentError::Incompatible),
    // Iris runs on Fabric, Quilt and NeoForge
    (ContentKind::Shader, LoaderType::Vanilla | LoaderType::Forge) => {
      Err(ContentError::Incompatible)
    }
    _ => Ok(()),
  }
}

/// Newest release, or the newest version when there is no release
pub fn newest(versions: Vec<VersionInfo>) -> Option<VersionInfo> {
  let latest = |release: bool| {
    versions
      .iter()
      .filter(|v| !release || v.channel == Channel::Release)
      .max_by(|a, b| a.published_at.cmp(&b.published_at))
      .cloned()
  };
  latest(true).or_else(|| latest(false))
}

async fn summaries<'p, 'a>(
  get: &impl Fn(Source) -> &'p dyn ContentProvider,
  wants: impl Iterator<Item = (Source, &'a String)>,
) -> Result<HashMap<(Source, String), ProjectSummary>> {
  let mut ids: HashMap<Source, Vec<String>> = HashMap::new();
  for (source, id) in wants {
    ids.entry(source).or_default().push(id.clone());
  }
  let mut out = HashMap::new();
  for (source, ids) in ids {
    for project in get(source).summaries(&ids).await? {
      out.insert((source, project.id.clone()), project);
    }
  }
  Ok(out)
}

/// Downloads every plan item into `<profile>/.proton-tmp/<id>/`, then moves them into place once
/// all are verified. Returns Ok(false) when canceled, the folder is unchanged on error or cancel.
pub async fn execute_install(
  providers: &Providers,
  dir: &Path,
  plan: &InstallPlan,
  id: usize,
  cancel: &Notify,
  progress: Progress,
) -> Result<bool> {
  let tmp = dir.join(TMP_DIR).join(id.to_string());
  let downloaded = select! {
    res = download(providers, &tmp, plan, progress.clone()) => Some(res),
    _ = cancel.notified() => None,
  };
  let res = match downloaded {
    Some(Ok(fingerprints)) => commit(dir, &tmp, plan, &fingerprints, &progress)
      .await
      .map(|_| true),
    Some(Err(e)) => Err(e),
    None => Ok(false),
  };
  // the backups of replaced files are all that is left of them
  if !matches!(&res, Err(e) if e.is::<RollbackFailed>()) {
    let _ = fs::remove_dir_all(&tmp).await;
  }
  // only removed when no other job uses it
  let _ = fs::remove_dir(dir.join(TMP_DIR)).await;
  if res.is_ok() {
    progress(DownloadCheckStatus::Done);
  }
  res
}

/// Some replaced files could not be moved back, their backups stay in the temp folder
#[derive(thiserror::Error, Debug)]
#[error("RollbackFailed")]
struct RollbackFailed;

/// Plans come back from the frontend, so they are checked again before anything is downloaded
pub fn check_plan(plan: &InstallPlan) -> Result<()> {
  for item in &plan.items {
    let file = item.version.primary_file().ok_or(ContentError::NotFound)?;
    safe_name(&file.file_name)?;
    if let Some(old) = &item.replaces {
      safe_name(old)?;
    }
    let url = file
      .url
      .as_deref()
      .ok_or(ContentError::NeedsManualDownload)?;
    let source = item.version.source;
    let host_ok = match source {
      Source::Modrinth => mrpack_host(url),
      Source::CurseForge => download_url(source, url).is_ok(),
    };
    if item.kind == ContentKind::Modpack || !host_ok {
      return Err(ContentError::Unsupported.into());
    }
  }
  Ok(())
}

/// CurseForge clients send the API key, so their downloads only go to the CurseForge CDN
pub fn download_url(source: Source, url: &str) -> Result<Url> {
  let url = Url::parse(url).map_err(|_| ContentError::Unsupported)?;
  if source == Source::CurseForge && !cdn_url(&url) {
    return Err(ContentError::Unsupported.into());
  }
  Ok(url)
}

/// Fingerprints of the downloaded files, by item index
async fn download(
  providers: &Providers,
  tmp: &Path,
  plan: &InstallPlan,
  progress: Progress,
) -> Result<Vec<u32>> {
  let mut files = Vec::new();
  for item in &plan.items {
    let file = item.version.primary_file().ok_or(ContentError::NotFound)?;
    files.push(Fetch {
      source: item.version.source,
      url: download_url(
        item.version.source,
        file
          .url
          .as_deref()
          .ok_or(ContentError::NeedsManualDownload)?,
      )?,
      path: tmp.join(safe_name(&file.file_name)?),
      sha1: file.sha1.clone(),
      sha512: file.sha512.clone(),
    });
  }
  fetch_all(providers, files, plan.total_bytes, progress).await
}

/// One file to download and verify
pub struct Fetch {
  pub source: Source,
  pub url: Url,
  pub path: PathBuf,
  pub sha1: String,
  pub sha512: Option<String>,
}

/// Downloads every file to its path after checking sha1 (and sha512 when given). Returns the
/// fingerprints in order, reports `ContentDownload` bytes out of `total`.
pub async fn fetch_all(
  providers: &Providers,
  files: Vec<Fetch>,
  total: u64,
  progress: Progress,
) -> Result<Vec<u32>> {
  let total = total as usize;
  let count = files.len();
  let done = Arc::new(AtomicUsize::new(0));
  progress(DownloadCheckStatus::ContentDownload(0, total));

  let mut futures = Vec::new();
  for (i, file) in files.into_iter().enumerate() {
    let Fetch {
      source,
      url,
      path,
      sha1,
      sha512,
    } = file;
    let client = providers.download_client(source).clone();
    let (done, progress) = (done.clone(), progress.clone());
    futures.push(async move {
      let report = Box::new(move |n| {
        let now = done.fetch_add(n, Ordering::Relaxed) + n;
        progress(DownloadCheckStatus::ContentDownload(now, total));
      });
      // verified before it gets its real name, a failed check never leaves a file at `path`
      let mut part = path.clone().into_os_string();
      part.push(".part");
      let part = PathBuf::from(part);
      let bytes = download_file(&client, &part, url, &sha1, report)
        .await
        .map_err(|e| match e.downcast_ref::<FileError>() {
          Some(_) => ContentError::HashMismatch,
          None => {
            log::warn!("Content download failed: {e}");
            ContentError::Network
          }
        })?;
      let fingerprint = spawn_blocking(move || {
        if let Some(sha512) = sha512
          && hex::encode(Sha512::digest(&bytes)) != sha512
        {
          return Err(ContentError::HashMismatch);
        }
        Ok(fingerprint(&bytes))
      })
      .await?;
      match fingerprint {
        Ok(_) => fs::rename(&part, &path).await?,
        Err(_) => {
          let _ = fs::remove_file(&part).await;
        }
      }
      Ok::<_, anyhow::Error>((i, fingerprint?))
    });
  }

  let mut fingerprints = vec![0; count];
  for res in FuturePool::new(futures).run(None).await {
    let (i, fingerprint) = res??;
    fingerprints[i] = fingerprint;
  }
  Ok(fingerprints)
}

/// Moves verified files into place and writes the manifest, undoing every move on error
async fn commit(
  dir: &Path,
  tmp: &Path,
  plan: &InstallPlan,
  fingerprints: &[u32],
  progress: &Progress,
) -> Result<()> {
  let _lock = store::lock().await;
  let mut manifest = store::load(dir).await;
  let mut moved = Vec::new();
  // (original, backup)
  let mut backups: Vec<(PathBuf, PathBuf)> = Vec::new();
  let backup_dir = tmp.join("replaced");

  let res: Result<()> = async {
    fs::create_dir_all(&backup_dir).await?;
    let total = plan.items.len();
    for (i, item) in plan.items.iter().enumerate() {
      progress(DownloadCheckStatus::ContentFiles(i, total));
      let file = item.version.primary_file().ok_or(ContentError::NotFound)?;
      let name = safe_name(&file.file_name)?;
      let folder = store::folder(dir, item.kind)?;
      fs::create_dir_all(&folder).await?;

      let mut disk_name = name.to_string();
      let mut dependency_of: Vec<String> = item.dependency_of.iter().cloned().collect();
      let mut from_pack = false;
      if let Some(old) = &item.replaces {
        let old_path = store::disk_path(dir, item.kind, safe_name(old)?)?;
        if old_path.extension().is_some_and(|e| e == "disabled") {
          // updates keep the enabled state
          disk_name = format!("{name}.disabled");
        }
        if old_path.exists() {
          let backup = backup_dir.join(backups.len().to_string());
          fs::rename(&old_path, &backup).await?;
          backups.push((old_path, backup));
        }
        if let Some(entry) = manifest
          .entries
          .iter()
          .find(|e| e.kind == item.kind && e.file_name == *old)
        {
          dependency_of.extend(entry.dependency_of.iter().cloned());
          from_pack = entry.from_pack;
        }
        manifest
          .entries
          .retain(|e| e.kind != item.kind || e.file_name != *old);
      }

      let dest = folder.join(&disk_name);
      if dest.exists() {
        let backup = backup_dir.join(backups.len().to_string());
        fs::rename(&dest, &backup).await?;
        backups.push((dest.clone(), backup));
      }
      fs::rename(tmp.join(name), &dest).await?;
      moved.push(dest);

      dependency_of.sort();
      dependency_of.dedup();
      manifest
        .entries
        .retain(|e| e.kind != item.kind || e.file_name != name);
      let version = &item.version;
      manifest.entries.push(Entry {
        kind: item.kind,
        file_name: name.to_string(),
        sha1: file.sha1.clone(),
        fingerprint: fingerprints[i],
        manual: false,
        from_pack,
        installed_at: Some(Utc::now().to_rfc3339()),
        source: Some(version.source),
        project_id: Some(version.project_id.clone()),
        version_id: Some(version.id.clone()),
        name: Some(item.name.clone()),
        icon_url: item.icon_url.clone(),
        version_number: Some(version.version_number.clone()),
        channel: Some(version.channel),
        dependency_of,
        pending: false,
      });
    }
    add_dependents(&mut manifest.entries, plan);
    store::save(dir, &manifest).await
  }
  .await;

  if let Err(e) = res {
    for path in moved {
      let _ = fs::remove_file(path).await;
    }
    let mut restored = true;
    for (original, backup) in backups.into_iter().rev() {
      if let Err(err) = fs::rename(&backup, &original).await {
        log::error!(
          "Failed to restore {} from {}: {err}",
          original.display(),
          backup.display()
        );
        restored = false;
      }
    }
    if !restored {
      log::error!(
        "Install failed with {e}, replaced files are kept in {}",
        backup_dir.display()
      );
      return Err(RollbackFailed.into());
    }
    return Err(e);
  }

  progress(DownloadCheckStatus::ContentFiles(
    plan.items.len(),
    plan.items.len(),
  ));
  for item in &plan.items {
    if let (Some(old), Some(file)) = (&item.replaces, item.version.primary_file()) {
      let _ = store::rename_pack(dir, item.kind, old, &file.file_name).await;
    }
  }
  Ok(())
}

/// Adds installed requesters to the `dependencyOf` of dependencies that were already installed
fn add_dependents(entries: &mut [Entry], plan: &InstallPlan) {
  for dep in &plan.already_installed {
    let Some(by) = &dep.dependency_of else {
      continue;
    };
    if !plan.items.iter().any(|i| i.version.project_id == *by) {
      continue;
    }
    let same = |p: &str| p == dep.project_id || (dep.project_id == IRIS && p == IRIS_CURSEFORGE);
    for entry in entries
      .iter_mut()
      .filter(|e| e.project_id.as_deref().is_some_and(same))
    {
      if !entry.dependency_of.contains(by) {
        entry.dependency_of.push(by.clone());
      }
    }
  }
}

/// Moves a manually downloaded file into place after checking its sha1, the scan identifies it
pub async fn adopt_file(dir: &Path, file: &ManualFile, path: &Path) -> Result<ContentKind> {
  let name = safe_name(&file.file_name)?.to_string();
  let source = path.to_path_buf();
  let sha1 =
    spawn_blocking(move || crate::utils::file::bytes_hash(&std::fs::read(source)?)).await??;
  if sha1 != file.sha1 {
    return Err(ContentError::HashMismatch.into());
  }
  let kind = store::detect_kind(path).await?;
  store::place(dir, kind, &name, path).await?;
  // leaves the download folder clean, a file already inside the profile is the placed one
  if !fs::canonicalize(path)
    .await?
    .starts_with(fs::canonicalize(dir).await?)
    && let Err(e) = fs::remove_file(path).await
  {
    log::warn!("Failed to remove {path:?} after adopting it: {e}");
  }
  Ok(kind)
}

/// Looks through the top level of `dirs` for manually downloaded files, matched by sha1 so
/// renamed duplicates like `mod (1).jar` count too. Returns file id -> path.
pub async fn find_manual(dirs: Vec<PathBuf>, files: Vec<ManualFile>) -> HashMap<String, PathBuf> {
  spawn_blocking(move || {
    let mut found = HashMap::new();
    for entry in dirs
      .iter()
      .filter_map(|d| std::fs::read_dir(d).ok())
      .flatten()
      .flatten()
    {
      let Ok(meta) = entry.metadata() else {
        continue;
      };
      let name = entry.file_name();
      // only hash files that could match, browsers write partial downloads under other names
      let candidates: Vec<_> = files
        .iter()
        .filter(|f| !found.contains_key(&f.file_id))
        .filter(|f| f.size == meta.len() || name.to_str() == Some(&f.file_name))
        .collect();
      if !meta.is_file() || candidates.is_empty() {
        continue;
      }
      let Ok(Ok(sha1)) = std::fs::read(entry.path()).map(|b| crate::utils::file::bytes_hash(&b))
      else {
        continue;
      };
      for file in candidates.into_iter().filter(|f| f.sha1 == sha1) {
        found.insert(file.file_id.clone(), entry.path());
      }
    }
    found
  })
  .await
  .unwrap_or_default()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::content::{
    Result as ContentResult,
    model::{Dependency, FileHashes, FileInfo, ProjectDetail, SearchQuery},
    provider::SearchPage,
    store::tests::temp_dir,
  };

  #[derive(Default)]
  struct Stub {
    projects: Vec<ProjectSummary>,
    versions: Vec<VersionInfo>,
  }

  #[async_trait::async_trait]
  impl ContentProvider for Stub {
    async fn search(&self, _: &SearchQuery) -> ContentResult<SearchPage> {
      unimplemented!()
    }
    async fn project(&self, _: &str) -> ContentResult<ProjectDetail> {
      unimplemented!()
    }
    async fn versions(
      &self,
      id: &str,
      loaders: &[String],
      _: Option<&str>,
    ) -> ContentResult<Vec<VersionInfo>> {
      Ok(
        self
          .versions
          .iter()
          .filter(|v| v.project_id == id)
          .filter(|v| loaders.is_empty() || v.loaders.iter().any(|l| loaders.contains(l)))
          .cloned()
          .collect(),
      )
    }
    async fn summaries(&self, ids: &[String]) -> ContentResult<Vec<ProjectSummary>> {
      Ok(
        self
          .projects
          .iter()
          .filter(|p| ids.contains(&p.id))
          .cloned()
          .collect(),
      )
    }
    async fn by_hashes(&self, _: &[FileHashes]) -> ContentResult<HashMap<String, VersionInfo>> {
      unimplemented!()
    }
    async fn latest_for(
      &self,
      _: &[FileHashes],
      _: &[String],
      _: &str,
      _: bool,
    ) -> ContentResult<HashMap<String, VersionInfo>> {
      unimplemented!()
    }
  }

  fn project(id: &str, kind: ContentKind) -> ProjectSummary {
    ProjectSummary {
      source: Source::Modrinth,
      id: id.into(),
      slug: None,
      kind,
      name: format!("{id} name"),
      author: String::new(),
      description: String::new(),
      icon_url: None,
      categories: vec![],
      loaders: vec![],
      game_versions: vec![],
      downloads: 0,
      updated_at: String::new(),
      client_side: None,
      server_side: None,
      url: format!("https://example.com/{id}"),
    }
  }

  fn version(
    project: &str,
    id: &str,
    loader: &str,
    deps: &[(&str, DependencyKind)],
  ) -> VersionInfo {
    VersionInfo {
      source: Source::Modrinth,
      project_id: project.into(),
      id: id.into(),
      name: id.into(),
      version_number: id.into(),
      channel: Channel::Release,
      game_versions: vec!["1.21.1".into()],
      loaders: vec![loader.into()],
      published_at: format!("2026-01-0{}", id.len()),
      files: vec![FileInfo {
        file_name: format!("{id}.jar"),
        url: Some(format!("https://cdn.example.com/{id}.jar")),
        sha1: format!("sha-{id}"),
        sha512: None,
        size: 10,
        primary: true,
      }],
      dependencies: deps
        .iter()
        .map(|(p, kind)| Dependency {
          project_id: p.to_string(),
          version_id: None,
          kind: *kind,
        })
        .collect(),
    }
  }

  fn profile(loader: LoaderType) -> Profile {
    serde_json::from_value(serde_json::json!({
      "id": "p", "name": "P", "created_at": "2026-01-01T00:00:00Z", "last_played": null,
      "version": "1.21.1", "loader": loader, "loader_version": null, "downloaded": true,
      "use_local_game": false, "game": null, "use_local_jvm": false, "jvm": null
    }))
    .unwrap()
  }

  fn pick(id: &str) -> PickRef {
    PickRef {
      source: Source::Modrinth,
      project_id: id.into(),
      version_id: None,
    }
  }

  fn entry(project: &str) -> Entry {
    Entry {
      kind: ContentKind::Mod,
      file_name: format!("{project}.jar"),
      sha1: format!("sha-{project}"),
      fingerprint: 0,
      manual: false,
      from_pack: false,
      installed_at: None,
      source: Some(Source::Modrinth),
      project_id: Some(project.into()),
      version_id: None,
      name: Some(format!("{project} name")),
      icon_url: None,
      version_number: Some("1".into()),
      channel: Some(Channel::Release),
      dependency_of: vec![],
      pending: false,
    }
  }

  async fn plan(
    stub: &Stub,
    loader: LoaderType,
    entries: &[Entry],
    picks: &[&str],
  ) -> Result<InstallPlan> {
    let picks: Vec<_> = picks.iter().map(|p| pick(p)).collect();
    plan_install(
      |_| stub as &dyn ContentProvider,
      &profile(loader),
      entries,
      &picks,
      &HashMap::new(),
    )
    .await
  }

  fn ids(plan: &InstallPlan) -> Vec<&str> {
    plan
      .items
      .iter()
      .map(|i| i.version.project_id.as_str())
      .collect()
  }

  #[tokio::test]
  async fn resolves_dependency_cycle_once() {
    use DependencyKind::*;
    let stub = Stub {
      projects: vec![
        project("a", ContentKind::Mod),
        project("b", ContentKind::Mod),
      ],
      versions: vec![
        version("a", "a1", "fabric", &[("b", Required)]),
        version("b", "b1", "fabric", &[("a", Required)]),
      ],
    };
    let plan = plan(&stub, LoaderType::Fabric, &[], &["a"]).await.unwrap();
    assert_eq!(ids(&plan), ["a", "b"]);
    assert_eq!(plan.items[1].reason, PlanReason::Dependency);
    assert_eq!(plan.items[1].dependency_of.as_deref(), Some("a"));
    assert_eq!(plan.total_bytes, 20);
  }

  #[tokio::test]
  async fn reports_optional_installed_and_incompatible() {
    use DependencyKind::*;
    let stub = Stub {
      projects: ["a", "lib", "opt", "bad"]
        .map(|id| project(id, ContentKind::Mod))
        .into(),
      versions: vec![
        version(
          "a",
          "a1",
          "fabric",
          &[("lib", Required), ("opt", Optional), ("bad", Incompatible)],
        ),
        version("lib", "l1", "fabric", &[]),
      ],
    };
    let plan = plan(
      &stub,
      LoaderType::Fabric,
      &[entry("lib"), entry("bad")],
      &["a"],
    )
    .await
    .unwrap();
    assert_eq!(ids(&plan), ["a"]);
    assert_eq!(plan.already_installed[0].project_id, "lib");
    assert_eq!(plan.optional[0].name, "opt name");
    assert_eq!(plan.conflicts[0].project_id, "bad");
    assert_eq!(plan.conflicts[0].reason, "Incompatible with a name");
  }

  #[tokio::test]
  async fn picked_version_replaces_installed_one() {
    let stub = Stub {
      projects: vec![project("a", ContentKind::Mod)],
      versions: vec![
        version("a", "a1", "fabric", &[]),
        version("a", "a22", "fabric", &[]),
      ],
    };
    let installed = Entry {
      version_id: Some("a22".into()),
      ..entry("a")
    };
    let run = |version: &str| {
      let picks = [PickRef {
        version_id: Some(version.into()),
        ..pick("a")
      }];
      let entries = [installed.clone()];
      let stub = &stub;
      async move {
        plan_install(
          |_| stub as &dyn ContentProvider,
          &profile(LoaderType::Fabric),
          &entries,
          &picks,
          &HashMap::new(),
        )
        .await
        .unwrap()
      }
    };
    let plan = run("a1").await;
    assert_eq!(plan.items[0].version.id, "a1");
    assert_eq!(plan.items[0].replaces.as_deref(), Some("a.jar"));
    // the installed version stays installed
    let plan = run("a22").await;
    assert!(plan.items.is_empty());
    assert_eq!(plan.already_installed[0].project_id, "a");
  }

  #[tokio::test]
  async fn quilt_accepts_fabric_and_picks_newest_release() {
    let mut beta = version("a", "a333", "fabric", &[]);
    beta.channel = Channel::Beta;
    let stub = Stub {
      projects: vec![project("a", ContentKind::Mod)],
      versions: vec![
        version("a", "a1", "fabric", &[]),
        version("a", "a22", "fabric", &[]),
        beta,
        version("a", "a4444", "forge", &[]),
      ],
    };
    let plan = plan(&stub, LoaderType::Quilt, &[], &["a"]).await.unwrap();
    assert_eq!(plan.items[0].version.id, "a22");
    let err = plan_err(&stub, LoaderType::Vanilla, &["a"]).await;
    assert_eq!(err, "Incompatible");
  }

  async fn plan_err(stub: &Stub, loader: LoaderType, picks: &[&str]) -> String {
    plan(stub, loader, &[], picks)
      .await
      .unwrap_err()
      .to_string()
  }

  #[tokio::test]
  async fn shaders_need_iris() {
    let stub = Stub {
      projects: vec![
        project("s", ContentKind::Shader),
        project(IRIS, ContentKind::Mod),
      ],
      versions: vec![
        version("s", "s1", "iris", &[]),
        version(IRIS, "i1", "neoforge", &[]),
      ],
    };
    assert_eq!(
      plan_err(&stub, LoaderType::Forge, &["s"]).await,
      "Incompatible"
    );
    assert_eq!(
      plan_err(&stub, LoaderType::Vanilla, &["s"]).await,
      "Incompatible"
    );
    let plan = plan(&stub, LoaderType::NeoForge, &[], &["s"])
      .await
      .unwrap();
    assert_eq!(ids(&plan), ["s", IRIS]);

    let mut iris = entry(IRIS_CURSEFORGE);
    iris.source = Some(Source::CurseForge);
    let plan = super::tests::plan(&stub, LoaderType::NeoForge, &[iris], &["s"])
      .await
      .unwrap();
    assert_eq!(ids(&plan), ["s"]);
  }

  #[tokio::test]
  async fn finds_manual_downloads_by_hash() {
    let dir = temp_dir("find-manual");
    std::fs::write(dir.join("a (1).jar"), b"aaa").unwrap();
    std::fs::write(dir.join("b.jar"), b"not b").unwrap();
    let file = |id: &str, bytes: &[u8]| ManualFile {
      source: Source::CurseForge,
      project_id: id.into(),
      file_id: id.into(),
      file_name: format!("{id}.jar"),
      sha1: crate::utils::file::bytes_hash(bytes).unwrap(),
      size: bytes.len() as u64,
      project_url: String::new(),
      download_url: String::new(),
    };
    let found = find_manual(
      vec![dir.clone(), dir.join("missing")],
      vec![file("a", b"aaa"), file("b", b"bbb")],
    )
    .await;
    assert_eq!(found.len(), 1);
    assert_eq!(found["a"], dir.join("a (1).jar"));

    let profile = temp_dir("adopt-manual");
    let downloaded = dir.join("a (1).jar");
    std::fs::write(&downloaded, b"aaa").unwrap();
    adopt_file(&profile, &file("a", b"aaa"), &downloaded)
      .await
      .unwrap();
    assert!(!downloaded.exists());
  }

  #[tokio::test]
  async fn restricted_files_are_manual() {
    let mut v = version("a", "a1", "fabric", &[]);
    v.files[0].url = None;
    let stub = Stub {
      projects: vec![project("a", ContentKind::Mod)],
      versions: vec![v],
    };
    let plan = plan(&stub, LoaderType::Fabric, &[], &["a"]).await.unwrap();
    assert!(plan.items.is_empty());
    assert_eq!(
      plan.manual[0].download_url,
      "https://example.com/a/download/a1"
    );
  }

  #[test]
  fn curseforge_downloads_stay_on_the_cdn() {
    assert!(
      download_url(
        Source::CurseForge,
        "https://edge.forgecdn.net/files/1/2/a.jar"
      )
      .is_ok()
    );
    assert!(download_url(Source::CurseForge, "https://evil.example/forgecdn.net").is_err());
    assert!(download_url(Source::Modrinth, "https://cdn.modrinth.com/a.jar").is_ok());
  }

  #[test]
  fn ipc_plans_are_rechecked() {
    let plan = |url: &str, name: &str, kind: ContentKind| {
      let mut v = version("a", "a1", "fabric", &[]);
      v.files[0].url = Some(url.into());
      v.files[0].file_name = name.into();
      InstallPlan {
        profile: "p".into(),
        items: vec![PlanItem {
          name: "a".into(),
          icon_url: None,
          kind,
          version: v,
          reason: PlanReason::Requested,
          dependency_of: None,
          replaces: None,
        }],
        already_installed: vec![],
        optional: vec![],
        conflicts: vec![],
        manual: vec![],
        total_bytes: 0,
      }
    };
    let ok = "https://cdn.modrinth.com/a.jar";
    assert!(check_plan(&plan(ok, "a.jar", ContentKind::Mod)).is_ok());
    assert!(check_plan(&plan(ok, "../a.jar", ContentKind::Mod)).is_err());
    assert!(check_plan(&plan(ok, "a.jar", ContentKind::Modpack)).is_err());
    assert!(
      check_plan(&plan(
        "https://evil.example/a.jar",
        "a.jar",
        ContentKind::Mod
      ))
      .is_err()
    );
    assert!(
      check_plan(&plan(
        "http://cdn.modrinth.com/a.jar",
        "a.jar",
        ContentKind::Mod
      ))
      .is_err()
    );
    let mut cf = plan("https://edge.forgecdn.net/a.jar", "a.jar", ContentKind::Mod);
    // the CurseForge allowlist does not cover Modrinth hosts and the other way round
    assert!(check_plan(&cf).is_err());
    cf.items[0].version.source = Source::CurseForge;
    assert!(check_plan(&cf).is_ok());
    cf.items[0].version.files[0].url = Some(ok.into());
    assert!(check_plan(&cf).is_err());
  }

  /// Serves `body` for every request on a local port
  async fn serve(body: &'static [u8]) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
      while let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = [0; 1024];
        let _ = socket.read(&mut buf).await;
        let head = format!(
          "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
          body.len()
        );
        let _ = socket.write_all(head.as_bytes()).await;
        let _ = socket.write_all(body).await;
      }
    });
    format!("http://{addr}")
  }

  #[tokio::test]
  async fn rollback_after_hash_mismatch_leaves_folder_unchanged() {
    let dir = temp_dir("content-rollback");
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(dir.join("mods/old.jar.disabled"), b"old").unwrap();
    let base = serve(b"new").await;

    let mut good = version("a", "a1", "fabric", &[]);
    good.files[0].url = Some(format!("{base}/a1.jar"));
    good.files[0].sha1 = crate::utils::file::bytes_hash(b"new").unwrap();
    let mut bad = version("b", "b1", "fabric", &[]);
    bad.files[0].url = Some(format!("{base}/b1.jar"));
    let item = |version: VersionInfo, replaces: Option<&str>| PlanItem {
      name: version.project_id.clone(),
      icon_url: None,
      kind: ContentKind::Mod,
      version,
      reason: PlanReason::Requested,
      dependency_of: None,
      replaces: replaces.map(String::from),
    };
    let mut plan = InstallPlan {
      profile: "p".into(),
      items: vec![item(good, Some("old.jar")), item(bad, None)],
      already_installed: vec![],
      optional: vec![],
      conflicts: vec![],
      manual: vec![],
      total_bytes: 6,
    };
    let providers = Providers::new();
    let notify = Notify::new();
    let progress: Progress = Arc::new(|_| {});

    let err = execute_install(&providers, &dir, &plan, 1, &notify, progress.clone()).await;
    assert_eq!(err.unwrap_err().to_string(), "HashMismatch");
    let names: Vec<_> = std::fs::read_dir(dir.join("mods"))
      .unwrap()
      .map(|e| e.unwrap().file_name())
      .collect();
    assert_eq!(names, ["old.jar.disabled"]);
    assert!(!dir.join(TMP_DIR).exists());
    assert!(!dir.join(crate::content::MANIFEST).exists());

    // the update keeps the disabled state and replaces the old file
    plan.items.pop();
    assert!(
      execute_install(&providers, &dir, &plan, 2, &notify, progress)
        .await
        .unwrap()
    );
    let names: Vec<_> = std::fs::read_dir(dir.join("mods"))
      .unwrap()
      .map(|e| e.unwrap().file_name())
      .collect();
    assert_eq!(names, ["a1.jar.disabled"]);
    let manifest = store::load(&dir).await;
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(manifest.entries[0].file_name, "a1.jar");
    assert_eq!(manifest.entries[0].fingerprint, fingerprint(b"new"));

    std::fs::remove_dir_all(&dir).unwrap();
  }

  #[tokio::test]
  async fn installed_dependency_gets_new_requester() {
    use DependencyKind::*;
    let stub = Stub {
      projects: vec![
        project("a", ContentKind::Mod),
        project("lib", ContentKind::Mod),
      ],
      versions: vec![version("a", "a1", "fabric", &[("lib", Required)])],
    };
    let mut lib = entry("lib");
    lib.dependency_of = vec!["old".into()];
    let mut entries = vec![lib, entry("other")];
    let plan = plan(&stub, LoaderType::Fabric, &entries, &["a"])
      .await
      .unwrap();
    assert_eq!(
      plan.already_installed[0].dependency_of.as_deref(),
      Some("a")
    );

    add_dependents(&mut entries, &plan);
    // twice is a no-op
    add_dependents(&mut entries, &plan);
    assert_eq!(entries[0].dependency_of, ["old", "a"]);
    assert!(entries[1].dependency_of.is_empty());
  }
}
