use std::{
  collections::HashMap,
  path::{Component, Path, PathBuf},
};

use anyhow::Result;
use async_zip::base::read1::seek::ZipArchiveReader;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;
use tokio::{
  fs,
  io::BufReader,
  sync::{Mutex, MutexGuard},
};
use tokio_util::compat::TokioAsyncReadCompatExt;

use super::{
  ContentError, MANIFEST,
  curseforge::fingerprint,
  model::{Channel, ContentKind, FileHashes, Installed, Source, VersionInfo},
  provider::Providers,
};
use proton_utils::file::bytes_hash;

const VERSION: u32 = 1;
const DISABLED: &str = ".disabled";
const OPTIONS: &str = "options.txt";
const PACKS_KEY: &str = "resourcePacks:";
const SHADER_KEY: &str = "shaderPack=";
const SHADERS_ON_KEY: &str = "enableShaders=";

// ponytail: one lock for every profile manifest, per-profile locks if installs in parallel profiles get slow
static LOCK: Mutex<()> = Mutex::const_new(());

/// Held around every manifest read-modify-write so scans and installs do not overwrite each other
pub async fn lock() -> MutexGuard<'static, ()> {
  LOCK.lock().await
}

#[derive(Serialize, Deserialize)]
pub struct Manifest {
  pub version: u32,
  pub entries: Vec<Entry>,
}

impl Default for Manifest {
  fn default() -> Self {
    Self {
      version: VERSION,
      entries: Vec::new(),
    }
  }
}

/// One content file, also the hash cache. `file_name` never has the `.disabled` suffix.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
  pub kind: ContentKind,
  pub file_name: String,
  pub sha1: String,
  pub fingerprint: u32,
  #[serde(default)]
  pub manual: bool,
  #[serde(default)]
  pub from_pack: bool,
  pub installed_at: Option<String>,
  pub source: Option<Source>,
  pub project_id: Option<String>,
  pub version_id: Option<String>,
  pub name: Option<String>,
  pub icon_url: Option<String>,
  pub version_number: Option<String>,
  pub channel: Option<Channel>,
  #[serde(default)]
  pub dependency_of: Vec<String>,
  /// The provider lookup failed, retried on the next scan
  #[serde(default)]
  pub pending: bool,
}

impl Entry {
  pub fn hashes(&self) -> FileHashes {
    FileHashes {
      sha1: self.sha1.clone(),
      fingerprint: self.fingerprint,
    }
  }

  fn identify(&mut self, version: &VersionInfo, name: Option<&(String, Option<String>)>) {
    self.source = Some(version.source);
    self.project_id = Some(version.project_id.clone());
    self.version_id = Some(version.id.clone());
    self.version_number = Some(version.version_number.clone());
    self.channel = Some(version.channel);
    if let Some((name, icon)) = name {
      self.name = Some(name.clone());
      self.icon_url = icon.clone();
    }
    self.manual = false;
    self.pending = false;
  }

  fn installed(&self, enabled: bool, order: Option<usize>, active: Option<bool>) -> Installed {
    Installed {
      kind: self.kind,
      file_name: self.file_name.clone(),
      enabled,
      manual: self.manual,
      from_pack: self.from_pack,
      installed_at: self.installed_at.clone(),
      source: self.source,
      project_id: self.project_id.clone(),
      version_id: self.version_id.clone(),
      sha1: self.sha1.clone(),
      name: self.name.clone(),
      icon_url: self.icon_url.clone(),
      version_number: self.version_number.clone(),
      dependency_of: self.dependency_of.clone(),
      order,
      active,
    }
  }
}

pub async fn load(dir: &Path) -> Manifest {
  let Ok(data) = fs::read(dir.join(MANIFEST)).await else {
    return Manifest::default();
  };
  serde_json::from_slice(&data).unwrap_or_else(|e| {
    tracing::warn!("Content manifest in {} is invalid: {e}", dir.display());
    Manifest::default()
  })
}

/// Writes a temp file and renames it, a crash never leaves a half written manifest
pub async fn save(dir: &Path, manifest: &Manifest) -> Result<()> {
  let tmp = dir.join(format!("{MANIFEST}.tmp"));
  fs::write(&tmp, serde_json::to_vec_pretty(manifest)?).await?;
  fs::rename(&tmp, dir.join(MANIFEST)).await?;
  Ok(())
}

/// Rejects names that could leave the content folder
pub fn safe_name(name: &str) -> Result<&str> {
  let mut parts = Path::new(name).components();
  match (parts.next(), parts.next()) {
    (Some(Component::Normal(_)), None) if !name.contains(['/', '\\']) => Ok(name),
    _ => Err(ContentError::Unsupported.into()),
  }
}

/// (name without `.disabled`, enabled) for a file in a content folder, None for unrelated files
pub fn parse_name(kind: ContentKind, name: &str) -> Option<(String, bool)> {
  let lower = name.to_ascii_lowercase();
  match kind {
    ContentKind::Mod if lower.ends_with(".jar") => Some((name.to_string(), true)),
    ContentKind::Mod if lower.ends_with(".jar.disabled") => {
      Some((name[..name.len() - DISABLED.len()].to_string(), false))
    }
    ContentKind::ResourcePack | ContentKind::Shader if lower.ends_with(".zip") => {
      Some((name.to_string(), true))
    }
    _ => None,
  }
}

/// Content folder of `kind` in the profile, modpacks have none
pub fn folder(dir: &Path, kind: ContentKind) -> Result<PathBuf> {
  Ok(dir.join(kind.dir().ok_or(ContentError::Unsupported)?))
}

// ponytail: folder packs are not listed, add them when users ask
async fn list(dir: &Path, kind: ContentKind) -> Vec<(String, bool)> {
  let mut files = Vec::new();
  let Ok(folder) = folder(dir, kind) else {
    return files;
  };
  let Ok(mut entries) = fs::read_dir(folder).await else {
    return files;
  };
  while let Ok(Some(entry)) = entries.next_entry().await {
    if !entry.file_type().await.is_ok_and(|t| t.is_file()) {
      continue;
    }
    if let Some(file) = entry.file_name().to_str().and_then(|n| parse_name(kind, n)) {
      files.push(file);
    }
  }
  files.sort();
  files
}

/// Path of a content file on disk, with `.disabled` when the mod is disabled
pub fn disk_path(dir: &Path, kind: ContentKind, name: &str) -> Result<PathBuf> {
  let folder = folder(dir, kind)?;
  let path = folder.join(name);
  let disabled = folder.join(format!("{name}{DISABLED}"));
  Ok(
    if kind == ContentKind::Mod && !path.exists() && disabled.exists() {
      disabled
    } else {
      path
    },
  )
}

fn hash_file(path: PathBuf) -> Result<(String, u32)> {
  let bytes = std::fs::read(path)?;
  Ok((bytes_hash(&bytes)?, fingerprint(&bytes)))
}

/// Reconciles the content folder with the manifest: new files are hashed once and identified on
/// Modrinth, then CurseForge. Files neither knows are `manual`.
pub async fn scan(providers: &Providers, dir: &Path, kind: ContentKind) -> Result<Vec<Installed>> {
  folder(dir, kind)?;
  let guard = lock().await;
  let mut manifest = load(dir).await;
  let files = list(dir, kind).await;
  let before = manifest.entries.clone();

  manifest
    .entries
    .retain(|e| e.kind != kind || files.iter().any(|(n, _)| *n == e.file_name));
  for (name, _) in &files {
    if manifest
      .entries
      .iter()
      .any(|e| e.kind == kind && e.file_name == *name)
    {
      continue;
    }
    let path = disk_path(dir, kind, name)?;
    let Ok((sha1, fingerprint)) = spawn_blocking(move || hash_file(path)).await? else {
      continue;
    };
    manifest.entries.push(Entry {
      kind,
      file_name: name.clone(),
      sha1,
      fingerprint,
      manual: true,
      from_pack: false,
      installed_at: None,
      source: None,
      project_id: None,
      version_id: None,
      name: None,
      icon_url: None,
      version_number: None,
      channel: None,
      dependency_of: Vec::new(),
      pending: true,
    });
  }

  if manifest.entries != before {
    save(dir, &manifest).await?;
  }
  let mut pending: Vec<Entry> = manifest
    .entries
    .into_iter()
    .filter(|e| e.kind == kind && e.pending)
    .collect();
  drop(guard);

  // without the lock, installs may change the manifest during the lookup
  identify(providers, pending.iter_mut().collect()).await;
  let _lock = lock().await;
  let mut manifest = load(dir).await;
  if merge(&mut manifest.entries, pending) {
    save(dir, &manifest).await?;
  }

  let packs = match kind {
    ContentKind::ResourcePack => {
      let options = fs::read_to_string(dir.join(OPTIONS))
        .await
        .unwrap_or_default();
      options_packs(&options)
    }
    _ => Vec::new(),
  };
  let shader = match kind {
    ContentKind::Shader => active_shader(&read_iris(dir).await),
    _ => None,
  };

  Ok(
    files
      .iter()
      .filter_map(|(name, enabled)| {
        let entry = manifest
          .entries
          .iter()
          .find(|e| e.kind == kind && e.file_name == *name)?;
        Some(match kind {
          ContentKind::ResourcePack => {
            let order = pack_order(&packs, name);
            entry.installed(order.is_some(), order, None)
          }
          ContentKind::Shader => entry.installed(true, None, Some(shader.as_ref() == Some(name))),
          _ => entry.installed(*enabled, None, None),
        })
      })
      .collect(),
  )
}

/// Looks up pending entries, they stay pending when a provider is unreachable
pub async fn identify(providers: &Providers, mut entries: Vec<&mut Entry>) {
  if entries.is_empty() {
    return;
  }
  let mut found: HashMap<String, VersionInfo> = HashMap::new();
  let mut failed = false;
  for source in [Source::Modrinth, Source::CurseForge] {
    let hashes: Vec<_> = entries
      .iter()
      .filter(|e| !found.contains_key(&e.sha1))
      .map(|e| e.hashes())
      .collect();
    if hashes.is_empty() {
      break;
    }
    match providers.get(source).by_hashes(&hashes).await {
      Ok(res) => found.extend(res),
      Err(e) => {
        tracing::warn!("Content lookup on {source:?} failed: {e}");
        failed = true;
      }
    }
  }

  // names and icons, one request per source
  let mut names: HashMap<String, (String, Option<String>)> = HashMap::new();
  for source in [Source::Modrinth, Source::CurseForge] {
    let mut ids: Vec<String> = found
      .values()
      .filter(|v| v.source == source)
      .map(|v| v.project_id.clone())
      .collect();
    ids.sort();
    ids.dedup();
    if let Ok(projects) = providers.get(source).summaries(&ids).await {
      names.extend(projects.into_iter().map(|p| (p.id, (p.name, p.icon_url))));
    }
  }

  for entry in entries.iter_mut() {
    if let Some(version) = found.get(&entry.sha1) {
      entry.identify(version, names.get(&version.project_id));
    } else if !failed {
      entry.pending = false;
    }
  }
}

/// Writes looked up entries into a freshly loaded manifest, entries that were replaced or
/// identified meanwhile are left alone. Returns whether anything changed.
fn merge(entries: &mut [Entry], identified: Vec<Entry>) -> bool {
  let mut changed = false;
  for new in identified {
    if let Some(old) = entries.iter_mut().find(|e| {
      e.pending && e.kind == new.kind && e.file_name == new.file_name && e.sha1 == new.sha1
    }) && *old != new
    {
      *old = new;
      changed = true;
    }
  }
  changed
}

/// Mods rename `.jar` <-> `.jar.disabled`, resource packs edit `options.txt`, shaders set the active pack
pub async fn set_enabled(dir: &Path, kind: ContentKind, name: &str, enabled: bool) -> Result<()> {
  safe_name(name)?;
  match kind {
    ContentKind::Mod => {
      let on = folder(dir, kind)?.join(name);
      let off = folder(dir, kind)?.join(format!("{name}{DISABLED}"));
      let (from, to) = if enabled { (off, on) } else { (on, off) };
      if from.exists() {
        fs::rename(from, to).await?;
      } else if !to.exists() {
        return Err(ContentError::NotFound.into());
      }
    }
    ContentKind::ResourcePack => {
      edit_packs(dir, |packs| {
        let file = format!("file/{name}");
        packs.retain(|p| *p != file);
        if enabled {
          // last = highest priority
          packs.push(file);
        }
      })
      .await?;
    }
    ContentKind::Shader => {
      if enabled {
        set_active_shader(dir, Some(name)).await?;
      } else if active_shader(&read_iris(dir).await).as_deref() == Some(name) {
        set_active_shader(dir, None).await?;
      }
    }
    ContentKind::Modpack => return Err(ContentError::Unsupported.into()),
  }
  Ok(())
}

/// Reorders the enabled resource packs, `files` is highest priority first. Other entries keep their place.
pub async fn set_order(dir: &Path, files: &[String]) -> Result<()> {
  edit_packs(dir, |packs| reorder(packs, files)).await
}

fn reorder(packs: &mut [String], files: &[String]) {
  let wanted: Vec<String> = files.iter().rev().map(|f| format!("file/{f}")).collect();
  let slots: Vec<usize> = (0..packs.len())
    .filter(|i| wanted.contains(&packs[*i]))
    .collect();
  let present: Vec<String> = wanted.into_iter().filter(|w| packs.contains(w)).collect();
  for (slot, pack) in slots.into_iter().zip(present) {
    packs[slot] = pack;
  }
}

pub async fn set_active_shader(dir: &Path, name: Option<&str>) -> Result<()> {
  if let Some(name) = name {
    safe_name(name)?;
  }
  let path = dir.join("config").join("iris.properties");
  let text = fs::read_to_string(&path).await.unwrap_or_default();
  let text = set_line(&text, SHADER_KEY, &escape(name.unwrap_or_default()));
  let text = set_line(&text, SHADERS_ON_KEY, &name.is_some().to_string());
  fs::create_dir_all(dir.join("config")).await?;
  fs::write(path, text).await?;
  Ok(())
}

/// Keeps the enabled state of a replaced resource or shader pack, best effort
pub async fn rename_pack(dir: &Path, kind: ContentKind, old: &str, new: &str) -> Result<()> {
  match kind {
    ContentKind::ResourcePack => {
      let (old, new) = (format!("file/{old}"), format!("file/{new}"));
      edit_packs(dir, |packs| {
        packs
          .iter_mut()
          .filter(|p| **p == old)
          .for_each(|p| *p = new.clone())
      })
      .await
    }
    ContentKind::Shader if active_shader(&read_iris(dir).await).as_deref() == Some(old) => {
      set_active_shader(dir, Some(new)).await
    }
    _ => Ok(()),
  }
}

/// Deletes the files and their manifest entries
pub async fn remove(dir: &Path, kind: ContentKind, names: &[String]) -> Result<()> {
  for name in names {
    safe_name(name)?;
  }
  let _lock = lock().await;
  let mut manifest = load(dir).await;
  for name in names {
    let path = disk_path(dir, kind, name)?;
    if path.exists() {
      fs::remove_file(path).await?;
    }
  }
  let removed: Vec<String> = manifest
    .entries
    .iter()
    .filter(|e| e.kind == kind && names.contains(&e.file_name))
    .filter_map(|e| e.project_id.clone())
    .collect();
  manifest
    .entries
    .retain(|e| e.kind != kind || !names.contains(&e.file_name));
  for entry in &mut manifest.entries {
    entry.dependency_of.retain(|p| !removed.contains(p));
  }
  save(dir, &manifest).await
}

/// `.jar` is a mod, a zip with `shaders/` a shader pack, a zip with `pack.mcmeta` a resource pack
pub async fn detect_kind(path: &Path) -> Result<ContentKind> {
  if path
    .extension()
    .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
  {
    return Ok(ContentKind::Mod);
  }
  let file = fs::File::open(path)
    .await
    .map_err(|_| ContentError::NotFound)?;
  let zip = ZipArchiveReader::open(BufReader::new(file).compat())
    .await
    .map_err(|_| ContentError::Unsupported)?;
  let names: Vec<&[u8]> = zip
    .inner()
    .cdrs()
    .iter()
    .map(|c| c.insecure_file_name.as_bytes())
    .collect();
  if names.iter().any(|n| n.starts_with(b"shaders/")) {
    Ok(ContentKind::Shader)
  } else if names.contains(&b"pack.mcmeta".as_slice()) {
    Ok(ContentKind::ResourcePack)
  } else {
    Err(ContentError::Unsupported.into())
  }
}

/// Copies a file into the folder of its detected kind
pub async fn add_file(dir: &Path, path: &Path) -> Result<ContentKind> {
  let kind = detect_kind(path).await?;
  let name = path
    .file_name()
    .and_then(|n| n.to_str())
    .ok_or(ContentError::Unsupported)?;
  place(dir, kind, name, path).await?;
  Ok(kind)
}

/// Copies `path` into the content folder as `name`. Refuses to overwrite another file, picking
/// the file already in place is a no-op.
pub async fn place(dir: &Path, kind: ContentKind, name: &str, path: &Path) -> Result<()> {
  let _lock = lock().await;
  let dest = disk_path(dir, kind, safe_name(name)?)?;
  if dest.exists() {
    if fs::canonicalize(&dest).await? == fs::canonicalize(path).await? {
      return Ok(());
    }
    return Err(ContentError::Unsupported.into());
  }
  fs::create_dir_all(folder(dir, kind)?).await?;
  fs::copy(path, dest).await?;
  Ok(())
}

async fn edit_packs(dir: &Path, edit: impl FnOnce(&mut Vec<String>)) -> Result<()> {
  let path = dir.join(OPTIONS);
  let text = fs::read_to_string(&path).await.unwrap_or_default();
  let mut packs = options_packs(&text);
  edit(&mut packs);
  fs::write(
    path,
    set_line(&text, PACKS_KEY, &serde_json::to_string(&packs)?),
  )
  .await?;
  Ok(())
}

/// The `resourcePacks` list, `["vanilla"]` when missing like the game does
fn options_packs(text: &str) -> Vec<String> {
  text
    .lines()
    .find_map(|l| l.strip_prefix(PACKS_KEY))
    .and_then(|v| serde_json::from_str(v.trim()).ok())
    .unwrap_or_else(|| vec!["vanilla".into()])
}

/// 0 = highest priority = last in the list, None = not enabled
fn pack_order(packs: &[String], name: &str) -> Option<usize> {
  let file = format!("file/{name}");
  packs
    .iter()
    .filter(|p| p.starts_with("file/"))
    .rev()
    .position(|p| *p == file)
}

async fn read_iris(dir: &Path) -> String {
  fs::read_to_string(dir.join("config").join("iris.properties"))
    .await
    .unwrap_or_default()
}

fn active_shader(iris: &str) -> Option<String> {
  let value = |key: &str| iris.lines().find_map(|l| l.strip_prefix(key));
  if value(SHADERS_ON_KEY)?.trim() != "true" {
    return None;
  }
  Some(unescape(value(SHADER_KEY)?.trim())).filter(|s| !s.is_empty())
}

/// Replaces the first line starting with `key`, or appends it. Other lines and line endings stay.
fn set_line(text: &str, key: &str, value: &str) -> String {
  let mut found = false;
  let mut out: String = text
    .split_inclusive('\n')
    .map(|line| {
      if found || !line.starts_with(key) {
        return line.to_string();
      }
      found = true;
      let end = &line[line.trim_end_matches(['\r', '\n']).len()..];
      format!("{key}{value}{end}")
    })
    .collect();
  if !found {
    if !out.is_empty() && !out.ends_with('\n') {
      out.push('\n');
    }
    out.push_str(&format!("{key}{value}\n"));
  }
  out
}

/// Java properties escaping for values
fn escape(value: &str) -> String {
  let mut out = String::new();
  for c in value.chars() {
    if matches!(c, '\\' | ':' | '=' | '#' | '!') {
      out.push('\\');
    }
    out.push(c);
  }
  out
}

fn unescape(value: &str) -> String {
  let mut out = String::new();
  let mut chars = value.chars();
  while let Some(c) = chars.next() {
    out.push(if c == '\\' {
      chars.next().unwrap_or(c)
    } else {
      c
    });
  }
  out
}

#[cfg(test)]
pub mod tests {
  use super::*;

  pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("proton-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
  }

  #[test]
  fn options_rewrite_keeps_other_content() {
    let text = "version:3955\r\nresourcePacks:[\"vanilla\",\"fabric\",\"file/a.zip\"]\r\nincompatibleResourcePacks:[\"file/old.zip\"]\r\nlang:en_us";
    let mut packs = options_packs(text);
    assert_eq!(packs, ["vanilla", "fabric", "file/a.zip"]);
    packs.push("file/b.zip".into());
    let out = set_line(text, PACKS_KEY, &serde_json::to_string(&packs).unwrap());
    assert_eq!(
      out,
      "version:3955\r\nresourcePacks:[\"vanilla\",\"fabric\",\"file/a.zip\",\"file/b.zip\"]\r\nincompatibleResourcePacks:[\"file/old.zip\"]\r\nlang:en_us"
    );
    assert_eq!(pack_order(&packs, "b.zip"), Some(0));
    assert_eq!(pack_order(&packs, "a.zip"), Some(1));
    assert_eq!(pack_order(&packs, "c.zip"), None);

    assert_eq!(options_packs(""), ["vanilla"]);
    assert_eq!(set_line("", PACKS_KEY, "[]"), "resourcePacks:[]\n");
    assert_eq!(set_line("a:1", PACKS_KEY, "[]"), "a:1\nresourcePacks:[]\n");
  }

  #[test]
  fn reorder_keeps_non_file_entries() {
    let mut packs: Vec<String> = [
      "vanilla",
      "file/a.zip",
      "fabric",
      "file/b.zip",
      "file/c.zip",
    ]
    .map(String::from)
    .into();
    // highest priority first, d is not enabled and stays disabled
    reorder(
      &mut packs,
      &[
        "a.zip".into(),
        "c.zip".into(),
        "d.zip".into(),
        "b.zip".into(),
      ],
    );
    assert_eq!(
      packs,
      [
        "vanilla",
        "file/b.zip",
        "fabric",
        "file/c.zip",
        "file/a.zip"
      ]
    );
  }

  #[test]
  fn iris_rewrite_keeps_other_keys() {
    let text = "#Iris settings\ncolorSpace=SRGB\nshaderPack=old.zip\nenableShaders=false\n";
    assert_eq!(active_shader(text), None);
    let out = set_line(text, SHADER_KEY, &escape("New: pack.zip"));
    let out = set_line(&out, SHADERS_ON_KEY, "true");
    assert_eq!(
      out,
      "#Iris settings\ncolorSpace=SRGB\nshaderPack=New\\: pack.zip\nenableShaders=true\n"
    );
    assert_eq!(active_shader(&out).as_deref(), Some("New: pack.zip"));
    assert_eq!(active_shader("enableShaders=true\nshaderPack=\n"), None);
  }

  #[tokio::test]
  async fn enable_disable_renames_and_edits_files() {
    let dir = temp_dir("content-enable");
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    std::fs::write(dir.join("mods/a.jar"), b"a").unwrap();
    std::fs::write(dir.join(OPTIONS), "lang:en_us\n").unwrap();

    set_enabled(&dir, ContentKind::Mod, "a.jar", false)
      .await
      .unwrap();
    assert!(dir.join("mods/a.jar.disabled").exists() && !dir.join("mods/a.jar").exists());
    assert_eq!(
      list(&dir, ContentKind::Mod).await,
      [("a.jar".into(), false)]
    );
    set_enabled(&dir, ContentKind::Mod, "a.jar", true)
      .await
      .unwrap();
    assert!(dir.join("mods/a.jar").exists());
    assert!(
      set_enabled(&dir, ContentKind::Mod, "../a.jar", true)
        .await
        .is_err()
    );
    assert!(
      set_enabled(&dir, ContentKind::Mod, "b.jar", true)
        .await
        .is_err()
    );

    set_enabled(&dir, ContentKind::ResourcePack, "p.zip", true)
      .await
      .unwrap();
    assert_eq!(
      std::fs::read_to_string(dir.join(OPTIONS)).unwrap(),
      "lang:en_us\nresourcePacks:[\"vanilla\",\"file/p.zip\"]\n"
    );

    set_active_shader(&dir, Some("s.zip")).await.unwrap();
    assert_eq!(
      active_shader(&read_iris(&dir).await).as_deref(),
      Some("s.zip")
    );
    set_enabled(&dir, ContentKind::Shader, "s.zip", false)
      .await
      .unwrap();
    assert_eq!(active_shader(&read_iris(&dir).await), None);

    std::fs::remove_dir_all(&dir).unwrap();
  }

  #[test]
  fn parses_content_names() {
    assert_eq!(
      parse_name(ContentKind::Mod, "a.jar"),
      Some(("a.jar".into(), true))
    );
    assert_eq!(
      parse_name(ContentKind::Mod, "a.jar.disabled"),
      Some(("a.jar".into(), false))
    );
    assert_eq!(parse_name(ContentKind::Mod, "a.txt"), None);
    assert_eq!(parse_name(ContentKind::Shader, "s.zip.txt"), None);
    assert!(safe_name("a.jar").is_ok());
    for bad in ["", "..", "a/b.jar", "a\\b.jar", "/a.jar"] {
      assert!(safe_name(bad).is_err(), "{bad}");
    }
  }

  async fn write_zip(path: &Path, names: &[&str]) {
    use async_zip::{Compression, ZipEntryBuilder, base::write::ZipFileWriter};
    use tokio_util::compat::TokioAsyncWriteCompatExt;
    let out = fs::File::create(path).await.unwrap();
    let mut writer = ZipFileWriter::new(out.compat_write());
    for name in names {
      let entry = ZipEntryBuilder::new((*name).into(), Compression::Stored);
      writer.write_entry_whole(entry, b"x").await.unwrap();
    }
    writer.close().await.unwrap();
  }

  #[tokio::test]
  async fn detects_kind_for_added_files() {
    let dir = temp_dir("content-kind");
    let (shader, pack, other) = (dir.join("s.zip"), dir.join("p.zip"), dir.join("o.zip"));
    write_zip(&shader, &["shaders/final.fsh", "pack.mcmeta"]).await;
    write_zip(&pack, &["pack.mcmeta", "assets/x.png"]).await;
    write_zip(&other, &["readme.txt"]).await;
    std::fs::write(dir.join("m.jar"), b"").unwrap();
    std::fs::write(dir.join("t.txt"), b"text").unwrap();

    assert_eq!(detect_kind(&shader).await.unwrap(), ContentKind::Shader);
    assert_eq!(detect_kind(&pack).await.unwrap(), ContentKind::ResourcePack);
    assert_eq!(
      detect_kind(&dir.join("m.jar")).await.unwrap(),
      ContentKind::Mod
    );
    for bad in [other, dir.join("t.txt"), dir.join("missing.zip")] {
      assert!(detect_kind(&bad).await.is_err());
    }

    let profile = dir.join("profile");
    assert_eq!(
      add_file(&profile, &pack).await.unwrap(),
      ContentKind::ResourcePack
    );
    assert!(profile.join("resourcepacks/p.zip").exists());
    // never overwrites, re-adding the file in place is a no-op
    std::fs::write(&pack, b"changed").unwrap();
    assert!(add_file(&profile, &pack).await.is_err());
    let placed = profile.join("resourcepacks/p.zip");
    assert!(add_file(&profile, &placed).await.is_ok());
    assert_ne!(std::fs::read(&placed).unwrap(), b"changed");

    // modpacks have no folder, nothing may touch the profile root
    std::fs::write(profile.join("profile.json"), b"{}").unwrap();
    assert!(
      remove(&profile, ContentKind::Modpack, &["profile.json".into()])
        .await
        .is_err()
    );
    assert!(
      place(&profile, ContentKind::Modpack, "x.zip", &placed)
        .await
        .is_err()
    );
    assert!(profile.join("profile.json").exists());

    std::fs::remove_dir_all(&dir).unwrap();
  }

  #[test]
  fn merge_skips_entries_changed_meanwhile() {
    let pending = |name: &str, sha1: &str| Entry {
      kind: ContentKind::Mod,
      file_name: name.into(),
      sha1: sha1.into(),
      fingerprint: 0,
      manual: true,
      from_pack: false,
      installed_at: None,
      source: None,
      project_id: None,
      version_id: None,
      name: None,
      icon_url: None,
      version_number: None,
      channel: None,
      dependency_of: Vec::new(),
      pending: true,
    };
    let found = |mut e: Entry| {
      e.pending = false;
      e.project_id = Some("p".into());
      e
    };
    // b was replaced by an install, c is gone
    let mut entries = vec![pending("a.jar", "1"), pending("b.jar", "new")];
    let identified = vec![
      found(pending("a.jar", "1")),
      found(pending("b.jar", "old")),
      found(pending("c.jar", "1")),
    ];
    assert!(merge(&mut entries, identified));
    assert_eq!(entries[0].project_id.as_deref(), Some("p"));
    assert!(entries[1].pending && entries[1].project_id.is_none());
    assert_eq!(entries.len(), 2);
    assert!(!merge(&mut entries, vec![pending("b.jar", "new")]));
  }
}
