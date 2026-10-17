use std::collections::HashMap;

use futures_util::future::join_all;
use reqwest::{
  Url,
  header::{HeaderMap, HeaderValue},
  redirect::{Attempt, Policy},
};
use serde::Deserialize;
use serde_json::json;

use super::{
  CF_KEY, Result,
  http::{Http, api},
  model::{
    Channel, ContentKind, Dependency, DependencyKind, FileHashes, FileInfo, GalleryImage,
    ProjectDetail, ProjectLinks, ProjectSummary, SearchQuery, SearchSort, Source, VersionInfo,
  },
  provider::{ContentProvider, SearchPage},
  render,
};

const API: &str = "https://api.curseforge.com/v1";
const GAME: &str = "432";
const PAGE: u32 = 50;
/// CurseForge rejects `index + pageSize` above this
const MAX_RESULTS: u32 = 10_000;
// ponytail: versions without a profile filter stop after this many files, page further if users miss old ones
const MAX_FILE_PAGES: u32 = 10;

/// Whether the url is on the CurseForge CDN, the only place the API key may go besides the API
pub fn cdn_url(url: &Url) -> bool {
  let host = url.host_str().unwrap_or_default();
  url.scheme() == "https" && (host == "forgecdn.net" || host.ends_with(".forgecdn.net"))
}

/// Downloads carry the API key, so they only follow redirects within the CurseForge CDN
fn cdn_redirect(attempt: Attempt) -> reqwest::redirect::Action {
  if attempt.previous().len() >= 10 {
    attempt.error("too many redirects")
  } else if cdn_url(attempt.url()) {
    attempt.follow()
  } else {
    let blocked = format!("redirect to {} blocked", attempt.url());
    attempt.error(blocked)
  }
}

/// Modrinth category names that differ from the CurseForge ones (normalized)
const CATEGORY_ALIASES: &[(&str, &str)] = &[
  ("optimization", "performance"),
  ("library", "apiandlibrary"),
  ("utility", "utilityqol"),
  ("adventure", "adventureandrpg"),
  ("equipment", "armortoolsandweapons"),
];

pub struct CurseForge {
  http: Http,
}

#[allow(clippy::new_without_default)]
impl CurseForge {
  pub fn download_client(&self) -> &reqwest::Client {
    &self.http.download
  }

  pub fn new() -> Self {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_static(CF_KEY));
    Self {
      http: Http::new(headers, Policy::custom(cdn_redirect)),
    }
  }

  /// Maps Modrinth category names to CurseForge category ids, ignoring unknown names
  async fn category_ids(&self, kind: ContentKind, names: &[String]) -> Result<Vec<u64>> {
    if names.is_empty() {
      return Ok(Vec::new());
    }
    let class = class_id(kind).to_string();
    let url = api(
      API,
      &["categories"],
      &[("gameId", GAME), ("classId", &class)],
    );
    let categories: Data<Vec<CfCategory>> = self.http.get_cached(url).await?;
    Ok(
      names
        .iter()
        .filter_map(|name| {
          let name = normalize(name);
          let alias = CATEGORY_ALIASES
            .iter()
            .find(|(m, _)| *m == name)
            .map(|(_, c)| *c);
          categories
            .data
            .iter()
            .find(|c| {
              let (n, s) = (normalize(&c.name), normalize(&c.slug));
              n == name || s == name || alias.is_some_and(|a| a == n || a == s)
            })
            .map(|c| c.id)
        })
        .collect(),
    )
  }

  async fn files_page(
    &self,
    id: &str,
    query: &[(&str, &str)],
    index: u32,
  ) -> Result<Paged<CfFile>> {
    let index = index.to_string();
    let page = PAGE.to_string();
    let mut query = query.to_vec();
    query.extend([("index", index.as_str()), ("pageSize", page.as_str())]);
    self
      .http
      .get(api(API, &["mods", id, "files"], &query))
      .await
  }

  /// Files by id in one request, order not kept
  pub async fn files(&self, ids: &[u64]) -> Result<Vec<VersionInfo>> {
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    let files: Data<Vec<CfFile>> = self
      .http
      .post(
        api(API, &["mods", "files"], &[]),
        &json!({ "fileIds": ids }),
      )
      .await?;
    Ok(files.data.into_iter().map(CfFile::into_info).collect())
  }

  /// Exact fingerprint matches as (sha1, file)
  async fn matches(&self, files: &[FileHashes]) -> Result<Vec<(String, CfFile)>> {
    if files.is_empty() {
      return Ok(Vec::new());
    }
    let fingerprints: Vec<_> = files.iter().map(|f| f.fingerprint).collect();
    let res: Data<Fingerprints> = self
      .http
      .post(
        api(API, &["fingerprints", GAME], &[]),
        &json!({ "fingerprints": fingerprints }),
      )
      .await?;
    Ok(
      res
        .data
        .exact_matches
        .into_iter()
        .filter_map(|m| {
          let sha1 = files
            .iter()
            .find(|f| f.fingerprint == m.file.file_fingerprint)?
            .sha1
            .clone();
          Some((sha1, m.file))
        })
        .collect(),
    )
  }
}

#[async_trait::async_trait]
impl ContentProvider for CurseForge {
  async fn search(&self, query: &SearchQuery) -> Result<SearchPage> {
    let class = class_id(query.kind).to_string();
    let offset = query.offset.min(MAX_RESULTS - 1);
    let size = query.limit.min(PAGE).min(MAX_RESULTS - offset).to_string();
    let offset = offset.to_string();
    let mut params = vec![
      ("gameId", GAME.to_string()),
      ("classId", class),
      ("searchFilter", query.query.clone()),
      ("sortField", sort_field(query.sort).to_string()),
      ("sortOrder", "desc".into()),
      ("index", offset),
      ("pageSize", size),
    ];
    if let Some(version) = &query.game_version {
      params.push(("gameVersion", version.clone()));
    }
    if matches!(query.kind, ContentKind::Mod | ContentKind::Modpack) {
      let loaders: Vec<_> = query.loaders.iter().filter_map(|l| loader_id(l)).collect();
      if !loaders.is_empty() {
        params.push(("modLoaderTypes", json!(loaders).to_string()));
      }
    }
    let categories = self.category_ids(query.kind, &query.categories).await?;
    if !categories.is_empty() {
      // CurseForge accepts at most 10 category ids
      params.push((
        "categoryIds",
        json!(categories[..categories.len().min(10)]).to_string(),
      ));
    }

    let params: Vec<_> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let res: Paged<Mod> = self
      .http
      .get_cached(api(API, &["mods", "search"], &params))
      .await?;
    Ok(SearchPage {
      total: res.pagination.total_count.min(MAX_RESULTS as u64),
      hits: res.data.into_iter().map(Mod::into_summary).collect(),
    })
  }

  async fn project(&self, id: &str) -> Result<ProjectDetail> {
    let (project, body) = tokio::join!(
      self
        .http
        .get_cached::<Data<Mod>>(api(API, &["mods", id], &[])),
      self
        .http
        .get_cached::<Data<String>>(api(API, &["mods", id, "description"], &[])),
    );
    let project = project?.data;
    let blank = |s: &Option<String>| s.clone().filter(|s| !s.is_empty());
    Ok(ProjectDetail {
      body: render::html(&body?.data),
      gallery: project
        .screenshots
        .iter()
        .map(|s| GalleryImage {
          url: s.url.clone(),
          title: Some(s.title.clone()).filter(|t| !t.is_empty()),
        })
        .collect(),
      links: ProjectLinks {
        source: blank(&project.links.source_url),
        issues: blank(&project.links.issues_url),
        wiki: blank(&project.links.wiki_url),
        discord: None,
      },
      summary: project.into_summary(),
    })
  }

  async fn versions(
    &self,
    id: &str,
    loaders: &[String],
    game: Option<&str>,
  ) -> Result<Vec<VersionInfo>> {
    let loader = match loaders {
      [l] => loader_id(l).map(|l| l.to_string()),
      _ => None,
    };
    let mut query = Vec::new();
    if let Some(game) = game {
      query.push(("gameVersion", game));
    }
    if let Some(loader) = &loader {
      query.push(("modLoaderType", loader.as_str()));
    }

    let first = self.files_page(id, &query, 0).await?;
    let pages = first
      .pagination
      .total_count
      .div_ceil(PAGE as u64)
      .min(MAX_FILE_PAGES as u64) as u32;
    let rest = join_all((1..pages).map(|p| self.files_page(id, &query, p * PAGE))).await;

    let mut files = first.data;
    for page in rest {
      files.extend(page?.data);
    }
    let mut versions: Vec<_> = files
      .into_iter()
      .map(CfFile::into_info)
      // files without loader tags (old uploads, packs) are kept
      .filter(|v| {
        v.loaders.is_empty() || loaders.is_empty() || v.loaders.iter().any(|l| loaders.contains(l))
      })
      .collect();
    versions.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    Ok(versions)
  }

  async fn summaries(&self, ids: &[String]) -> Result<Vec<ProjectSummary>> {
    let ids: Vec<u64> = ids.iter().filter_map(|id| id.parse().ok()).collect();
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    let mods: Data<Vec<Mod>> = self
      .http
      .post(
        api(API, &["mods"], &[]),
        &json!({ "modIds": ids, "filterPcOnly": true }),
      )
      .await?;
    Ok(mods.data.into_iter().map(Mod::into_summary).collect())
  }

  async fn by_hashes(&self, files: &[FileHashes]) -> Result<HashMap<String, VersionInfo>> {
    Ok(
      self
        .matches(files)
        .await?
        .into_iter()
        .map(|(sha1, file)| (sha1, file.into_info()))
        .collect(),
    )
  }

  async fn latest_for(
    &self,
    files: &[FileHashes],
    loaders: &[String],
    game: &str,
    release_only: bool,
  ) -> Result<HashMap<String, VersionInfo>> {
    let matches = self.matches(files).await?;
    if matches.is_empty() {
      return Ok(HashMap::new());
    }
    let mod_ids: Vec<_> = matches.iter().map(|(_, f)| f.mod_id).collect();
    let mods: Data<Vec<Mod>> = self
      .http
      .post(
        api(API, &["mods"], &[]),
        &json!({ "modIds": mod_ids, "filterPcOnly": true }),
      )
      .await?;

    let file_ids: Vec<u64> = mods
      .data
      .iter()
      .filter_map(|m| {
        m.latest_files_indexes
          .iter()
          .filter(|i| i.game_version == game && (!release_only || i.release_type == 1))
          .filter(|i| {
            loaders.is_empty()
              || i
                .mod_loader
                .and_then(loader_name)
                .is_none_or(|l| loaders.iter().any(|x| x == l))
          })
          .map(|i| i.file_id)
          .max()
      })
      .collect();
    let by_mod: HashMap<String, VersionInfo> = self
      .files(&file_ids)
      .await?
      .into_iter()
      .map(|v| (v.project_id.clone(), v))
      .collect();
    Ok(
      matches
        .into_iter()
        .filter_map(|(sha1, f)| Some((sha1, by_mod.get(&f.mod_id.to_string())?.clone())))
        .collect(),
    )
  }
}

/// CurseForge file fingerprint: Murmur2 (seed 1) over the file without whitespace bytes
pub fn fingerprint(data: &[u8]) -> u32 {
  const M: u32 = 0x5bd1_e995;
  let bytes: Vec<u8> = data
    .iter()
    .copied()
    .filter(|b| !matches!(b, 9 | 10 | 13 | 32))
    .collect();

  let mut h = 1 ^ bytes.len() as u32;
  let (chunks, tail) = bytes.as_chunks::<4>();
  for chunk in chunks {
    let mut k = u32::from_le_bytes(*chunk);
    k = k.wrapping_mul(M);
    k ^= k >> 24;
    k = k.wrapping_mul(M);
    h = h.wrapping_mul(M) ^ k;
  }
  if !tail.is_empty() {
    for (i, b) in tail.iter().enumerate() {
      h ^= (*b as u32) << (8 * i);
    }
    h = h.wrapping_mul(M);
  }

  h ^= h >> 13;
  h = h.wrapping_mul(M);
  h ^ (h >> 15)
}

fn class_id(kind: ContentKind) -> u32 {
  match kind {
    ContentKind::Mod => 6,
    ContentKind::Modpack => 4471,
    ContentKind::ResourcePack => 12,
    ContentKind::Shader => 6552,
  }
}

fn kind(class_id: Option<u32>) -> ContentKind {
  match class_id {
    Some(4471) => ContentKind::Modpack,
    Some(12) => ContentKind::ResourcePack,
    Some(6552) => ContentKind::Shader,
    _ => ContentKind::Mod,
  }
}

fn sort_field(sort: SearchSort) -> u8 {
  match sort {
    SearchSort::Relevance => 2, // Popularity
    SearchSort::Downloads => 6, // TotalDownloads
    SearchSort::Updated => 3,   // LastUpdated
    SearchSort::Newest => 11,   // ReleasedDate
  }
}

fn loader_id(name: &str) -> Option<u8> {
  match name {
    "forge" => Some(1),
    "fabric" => Some(4),
    "quilt" => Some(5),
    "neoforge" => Some(6),
    _ => None,
  }
}

fn loader_name(id: u8) -> Option<&'static str> {
  match id {
    1 => Some("forge"),
    4 => Some("fabric"),
    5 => Some("quilt"),
    6 => Some("neoforge"),
    _ => None,
  }
}

fn channel(release_type: u8) -> Channel {
  match release_type {
    2 => Channel::Beta,
    3 => Channel::Alpha,
    _ => Channel::Release,
  }
}

fn relation(relation_type: u8) -> Option<DependencyKind> {
  match relation_type {
    1 | 6 => Some(DependencyKind::Embedded),
    2 => Some(DependencyKind::Optional),
    3 => Some(DependencyKind::Required),
    5 => Some(DependencyKind::Incompatible),
    _ => None, // 4 = tool
  }
}

/// CurseForge mixes loaders and environments into `gameVersions`: returns (loaders, minecraft versions)
fn split_game_versions(values: &[String]) -> (Vec<String>, Vec<String>) {
  let mut loaders = Vec::new();
  let mut versions = Vec::new();
  for v in values {
    if v.starts_with(|c: char| c.is_ascii_digit()) {
      versions.push(v.clone());
    } else if v != "Client" && v != "Server" {
      loaders.push(v.to_lowercase());
    }
  }
  (loaders, versions)
}

fn normalize(s: &str) -> String {
  s.chars()
    .filter(char::is_ascii_alphanumeric)
    .map(|c| c.to_ascii_lowercase())
    .collect()
}

fn push_unique(list: &mut Vec<String>, value: String) {
  if !list.contains(&value) {
    list.push(value);
  }
}

#[derive(Deserialize)]
struct Data<T> {
  data: T,
}

#[derive(Deserialize)]
struct Paged<T> {
  data: Vec<T>,
  pagination: Pagination,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
  total_count: u64,
}

#[derive(Deserialize)]
struct CfCategory {
  id: u64,
  name: String,
  slug: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fingerprints {
  exact_matches: Vec<FingerprintMatch>,
}

#[derive(Deserialize)]
struct FingerprintMatch {
  file: CfFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mod {
  id: u64,
  name: String,
  summary: String,
  links: Links,
  download_count: f64,
  categories: Vec<Named>,
  class_id: Option<u32>,
  authors: Vec<Named>,
  logo: Option<Image>,
  date_modified: String,
  #[serde(default)]
  latest_files_indexes: Vec<FileIndex>,
  #[serde(default)]
  screenshots: Vec<Image>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Links {
  website_url: String,
  wiki_url: Option<String>,
  issues_url: Option<String>,
  source_url: Option<String>,
}

#[derive(Deserialize)]
struct Named {
  name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Image {
  #[serde(default)]
  title: String,
  thumbnail_url: Option<String>,
  url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileIndex {
  game_version: String,
  file_id: u64,
  release_type: u8,
  mod_loader: Option<u8>,
}

impl Mod {
  fn into_summary(self) -> ProjectSummary {
    let mut loaders = Vec::new();
    let mut game_versions = Vec::new();
    for index in &self.latest_files_indexes {
      if let Some(loader) = index.mod_loader.and_then(loader_name) {
        push_unique(&mut loaders, loader.to_string());
      }
      push_unique(&mut game_versions, index.game_version.clone());
    }
    ProjectSummary {
      source: Source::CurseForge,
      id: self.id.to_string(),
      kind: kind(self.class_id),
      name: self.name,
      author: self
        .authors
        .into_iter()
        .next()
        .map(|a| a.name)
        .unwrap_or_default(),
      description: self.summary,
      // some logos come with an empty thumbnail but a valid full url
      icon_url: self.logo.and_then(|l| {
        [l.thumbnail_url, Some(l.url)]
          .into_iter()
          .flatten()
          .find(|s| !s.is_empty())
      }),
      categories: self.categories.into_iter().map(|c| c.name).collect(),
      loaders,
      game_versions,
      downloads: self.download_count as u64,
      updated_at: self.date_modified,
      client_side: None,
      server_side: None,
      url: self.links.website_url,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
  id: u64,
  mod_id: u64,
  display_name: String,
  file_name: String,
  release_type: u8,
  hashes: Vec<CfHash>,
  file_date: String,
  file_length: u64,
  download_url: Option<String>,
  game_versions: Vec<String>,
  dependencies: Vec<CfDependency>,
  file_fingerprint: u32,
}

#[derive(Deserialize)]
struct CfHash {
  value: String,
  algo: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfDependency {
  mod_id: u64,
  relation_type: u8,
}

impl CfFile {
  fn into_info(self) -> VersionInfo {
    let (loaders, game_versions) = split_game_versions(&self.game_versions);
    VersionInfo {
      source: Source::CurseForge,
      project_id: self.mod_id.to_string(),
      id: self.id.to_string(),
      name: self.display_name.clone(),
      version_number: self.display_name,
      channel: channel(self.release_type),
      game_versions,
      loaders,
      published_at: self.file_date,
      files: vec![FileInfo {
        file_name: self.file_name,
        url: self.download_url,
        sha1: self
          .hashes
          .into_iter()
          .find(|h| h.algo == 1)
          .map(|h| h.value)
          .unwrap_or_default(),
        sha512: None,
        size: self.file_length,
        primary: true,
      }],
      dependencies: self
        .dependencies
        .into_iter()
        .filter_map(|d| {
          Some(Dependency {
            project_id: d.mod_id.to_string(),
            version_id: None,
            kind: relation(d.relation_type)?,
          })
        })
        .collect(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fingerprint_known_vectors() {
    // this implementation matched the fileFingerprint (752734651) of CurseForge file 3437116
    assert_eq!(fingerprint(b""), 1540447798);
    assert_eq!(fingerprint(b"a"), 626045324);
    assert_eq!(fingerprint(b"abc"), 1621425345);
    assert_eq!(fingerprint(b"abcd"), 3376380438);
    assert_eq!(fingerprint(b"proton"), 2410239631);
  }

  #[test]
  fn fingerprint_ignores_whitespace() {
    assert_eq!(fingerprint(b"a b\tc\r\nd"), fingerprint(b"abcd"),);
    assert_ne!(fingerprint(b"abcd"), fingerprint(b"abce"));
  }

  #[test]
  fn splits_game_versions() {
    let values: Vec<String> = ["Client", "Fabric", "1.21.1", "Server", "NeoForge", "26.1"]
      .map(String::from)
      .into();
    let (loaders, versions) = split_game_versions(&values);
    assert_eq!(loaders, ["fabric", "neoforge"]);
    assert_eq!(versions, ["1.21.1", "26.1"]);
  }

  #[test]
  fn maps_enums() {
    assert_eq!(channel(1), Channel::Release);
    assert_eq!(channel(2), Channel::Beta);
    assert_eq!(channel(3), Channel::Alpha);
    assert_eq!(relation(3), Some(DependencyKind::Required));
    assert_eq!(relation(2), Some(DependencyKind::Optional));
    assert_eq!(relation(5), Some(DependencyKind::Incompatible));
    assert_eq!(relation(1), Some(DependencyKind::Embedded));
    assert_eq!(relation(4), None);
    assert_eq!(kind(Some(6552)), ContentKind::Shader);
    assert_eq!(class_id(ContentKind::ResourcePack), 12);
    for name in ["forge", "fabric", "quilt", "neoforge"] {
      assert_eq!(loader_id(name).and_then(loader_name), Some(name));
    }
    assert_eq!(loader_id("iris"), None);
  }

  #[test]
  fn restricted_file_has_no_url() {
    let file: CfFile = serde_json::from_value(json!({
      "id": 1, "modId": 2, "displayName": "Pack 1.0", "fileName": "pack.zip", "releaseType": 1,
      "hashes": [{"value": "md5", "algo": 2}, {"value": "sha", "algo": 1}],
      "fileDate": "d", "fileLength": 3, "downloadUrl": null, "gameVersions": ["1.21.1"],
      "dependencies": [{"modId": 9, "relationType": 3}], "fileFingerprint": 4
    }))
    .unwrap();
    let info = file.into_info();
    assert_eq!(info.files[0].url, None);
    assert_eq!(info.files[0].sha1, "sha");
    assert_eq!(info.dependencies[0].project_id, "9");
  }

  #[test]
  fn empty_logo_thumbnail_falls_back() {
    let logo = |thumb: &str, url: &str| {
      let m: Mod = serde_json::from_value(json!({
        "id": 1, "slug": "s", "name": "n", "summary": "", "links": {"websiteUrl": ""},
        "downloadCount": 0.0, "categories": [], "classId": 4471, "authors": [],
        "logo": {"thumbnailUrl": thumb, "url": url}, "dateModified": ""
      }))
      .unwrap();
      m.into_summary().icon_url
    };
    assert_eq!(logo("", "full"), Some("full".into()));
    assert_eq!(logo("thumb", "full"), Some("thumb".into()));
    assert_eq!(logo("", ""), None);
  }
}
