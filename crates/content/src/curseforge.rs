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
  use proton_utils::{
    rt,
    test_http::{Call, FakeServer, Response},
  };

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

  fn query_of(call: &Call) -> HashMap<String, String> {
    Url::parse(&format!("http://x{}", call.path))
      .unwrap()
      .query_pairs()
      .into_owned()
      .collect()
  }

  fn json_of(call: &Call) -> serde_json::Value {
    serde_json::from_str(call.body_str()).unwrap()
  }

  fn file_json(id: u64, mod_id: u64, date: &str, versions: &[&str]) -> serde_json::Value {
    json!({
      "id": id, "modId": mod_id, "displayName": format!("v{id}"), "fileName": format!("{id}.jar"),
      "releaseType": 1, "hashes": [{"value": format!("sha{id}"), "algo": 1}], "fileDate": date,
      "fileLength": 1, "downloadUrl": format!("https://edge.forgecdn.net/{id}.jar"),
      "gameVersions": versions, "dependencies": [], "fileFingerprint": id
    })
  }

  fn mod_json(id: u64, indexes: serde_json::Value) -> serde_json::Value {
    json!({
      "id": id, "name": format!("m{id}"), "summary": "s", "downloadCount": 12.9,
      "links": {"websiteUrl": format!("https://www.curseforge.com/m{id}"), "wikiUrl": "",
        "issuesUrl": "https://issues", "sourceUrl": null},
      "categories": [{"name": "Magic"}], "classId": 6, "authors": [{"name": "a"}, {"name": "b"}],
      "logo": null, "dateModified": "d", "latestFilesIndexes": indexes,
      "screenshots": [{"title": "", "thumbnailUrl": null, "url": "https://s/1.png"},
        {"title": "Two", "url": "https://s/2.png"}]
    })
  }

  fn query(kind: ContentKind) -> SearchQuery {
    SearchQuery {
      kind,
      query: "jei".into(),
      sources: vec![Source::CurseForge],
      loaders: vec![],
      game_version: None,
      categories: vec![],
      environment: None,
      sort: SearchSort::Updated,
      offset: 0,
      limit: 20,
      compatible_with: None,
    }
  }

  fn search_route(server: &FakeServer, total: u64) {
    server.route(
      "GET",
      "/api.curseforge.com/v1/mods/search",
      200,
      json!({"data": [mod_json(1, json!([]))], "pagination": {"totalCount": total}}).to_string(),
    );
  }

  #[test]
  fn requests_carry_the_api_key() {
    let server = FakeServer::start();
    search_route(&server, 1);
    rt::block_on(CurseForge::new().search(&query(ContentKind::Mod))).unwrap();
    assert_eq!(server.calls()[0].header("x-api-key"), Some(CF_KEY));
  }

  #[test]
  fn search_sends_filters_and_caps_the_total() {
    let server = FakeServer::start();
    search_route(&server, 50_000);
    let q = SearchQuery {
      loaders: vec!["iris".into(), "fabric".into(), "quilt".into()],
      game_version: Some("1.21.1".into()),
      ..query(ContentKind::Mod)
    };
    let page = rt::block_on(CurseForge::new().search(&q)).unwrap();
    assert_eq!(page.total, 10_000);
    let hit = &page.hits[0];
    assert_eq!((hit.id.as_str(), hit.author.as_str()), ("1", "a"));
    assert_eq!(hit.downloads, 12);

    let q = query_of(&server.calls()[0]);
    let expected = [
      ("gameId", "432"),
      ("classId", "6"),
      ("searchFilter", "jei"),
      ("sortField", "3"),
      ("sortOrder", "desc"),
      ("index", "0"),
      ("pageSize", "20"),
      ("gameVersion", "1.21.1"),
      // unknown loaders are dropped
      ("modLoaderTypes", "[4,5]"),
    ];
    assert_eq!(q.len(), expected.len());
    for (k, v) in expected {
      assert_eq!(q[k], v, "{k}");
    }
  }

  #[test]
  fn search_window_stays_inside_the_10k_limit() {
    // (offset, limit) -> (index, pageSize)
    let cases = [
      ((0, 100), ("0", "50")),
      ((9_990, 50), ("9990", "10")),
      ((9_999, 50), ("9999", "1")),
      ((20_000, 20), ("9999", "1")),
    ];
    for ((offset, limit), (index, size)) in cases {
      let server = FakeServer::start();
      search_route(&server, 1);
      let q = SearchQuery {
        offset,
        limit,
        ..query(ContentKind::Mod)
      };
      rt::block_on(CurseForge::new().search(&q)).unwrap();
      let q = query_of(&server.calls()[0]);
      assert_eq!((q["index"].as_str(), q["pageSize"].as_str()), (index, size));
    }
  }

  #[test]
  fn loader_filter_only_applies_to_mods_and_modpacks() {
    for (kind, filtered) in [
      (ContentKind::Mod, true),
      (ContentKind::Modpack, true),
      (ContentKind::ResourcePack, false),
      (ContentKind::Shader, false),
    ] {
      let server = FakeServer::start();
      search_route(&server, 1);
      let q = SearchQuery {
        loaders: vec!["forge".into()],
        ..query(kind)
      };
      rt::block_on(CurseForge::new().search(&q)).unwrap();
      let q = query_of(&server.calls()[0]);
      assert_eq!(q.contains_key("modLoaderTypes"), filtered, "{kind:?}");
      assert_eq!(q["classId"], class_id(kind).to_string());
    }
  }

  #[test]
  fn categories_map_by_name_slug_or_alias() {
    let server = FakeServer::start();
    search_route(&server, 1);
    server.route(
      "GET",
      "/api.curseforge.com/v1/categories",
      200,
      json!({"data": [
        {"id": 1, "name": "Performance", "slug": "performance"},
        {"id": 2, "name": "API and Library", "slug": "library-api"},
        {"id": 3, "name": "Magic", "slug": "magic"},
        {"id": 4, "name": "Tech", "slug": "technology"}
      ]})
      .to_string(),
    );
    let names = ["optimization", "library", "Magic", "technology", "unknown"];
    let q = SearchQuery {
      categories: names.map(String::from).into(),
      ..query(ContentKind::Mod)
    };
    rt::block_on(CurseForge::new().search(&q)).unwrap();
    let calls = server.calls();
    let categories = calls
      .iter()
      .find(|c| c.path.contains("categories?"))
      .unwrap();
    let cq = query_of(categories);
    assert_eq!(
      (cq["gameId"].as_str(), cq["classId"].as_str()),
      ("432", "6")
    );
    let search = calls.iter().find(|c| c.path.contains("search")).unwrap();
    assert_eq!(query_of(search)["categoryIds"], "[1,2,3,4]");
  }

  #[test]
  fn at_most_ten_category_ids_are_sent() {
    let server = FakeServer::start();
    search_route(&server, 1);
    let data: Vec<_> = (0..12)
      .map(|i| json!({"id": i, "name": format!("c{i}"), "slug": format!("c{i}")}))
      .collect();
    server.route(
      "GET",
      "/api.curseforge.com/v1/categories",
      200,
      json!({ "data": data }).to_string(),
    );
    let q = SearchQuery {
      categories: (0..12).map(|i| format!("c{i}")).collect(),
      ..query(ContentKind::Mod)
    };
    rt::block_on(CurseForge::new().search(&q)).unwrap();
    let search = server
      .calls()
      .into_iter()
      .find(|c| c.path.contains("search"))
      .unwrap();
    assert_eq!(query_of(&search)["categoryIds"], "[0,1,2,3,4,5,6,7,8,9]");
  }

  #[test]
  fn sort_fields() {
    let cases = [
      (SearchSort::Relevance, 2),
      (SearchSort::Downloads, 6),
      (SearchSort::Updated, 3),
      (SearchSort::Newest, 11),
    ];
    for (sort, field) in cases {
      assert_eq!(sort_field(sort), field);
    }
  }

  #[test]
  fn project_joins_mod_and_description() {
    let server = FakeServer::start();
    let indexes = json!([
      {"gameVersion": "1.21.1", "fileId": 1, "releaseType": 1, "modLoader": 4},
      {"gameVersion": "1.21.1", "fileId": 2, "releaseType": 1, "modLoader": 6},
      {"gameVersion": "1.20.1", "fileId": 3, "releaseType": 1, "modLoader": 4},
      {"gameVersion": "1.20.1", "fileId": 4, "releaseType": 1, "modLoader": null}
    ]);
    server.route(
      "GET",
      "/api.curseforge.com/v1/mods/7",
      200,
      json!({ "data": mod_json(7, indexes) }).to_string(),
    );
    server.route(
      "GET",
      "/api.curseforge.com/v1/mods/7/description",
      200,
      json!({"data": "<p>Hi<script>x()</script></p>"}).to_string(),
    );
    let detail = rt::block_on(CurseForge::new().project("7")).unwrap();
    assert!(!detail.body.contains("script"), "{}", detail.body);
    assert!(detail.body.contains("Hi"));
    // loaders and versions are deduplicated in order
    assert_eq!(detail.summary.loaders, ["fabric", "neoforge"]);
    assert_eq!(detail.summary.game_versions, ["1.21.1", "1.20.1"]);
    assert_eq!(detail.summary.url, "https://www.curseforge.com/m7");
    let titles: Vec<_> = detail.gallery.iter().map(|g| g.title.clone()).collect();
    assert_eq!(titles, [None, Some("Two".into())]);
    assert_eq!(detail.links.source, None);
    assert_eq!(detail.links.wiki, None);
    assert_eq!(detail.links.issues.as_deref(), Some("https://issues"));
    assert_eq!(detail.links.discord, None);
  }

  #[test]
  fn project_fails_without_a_description() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/api.curseforge.com/v1/mods/7",
      200,
      json!({ "data": mod_json(7, json!([])) }).to_string(),
    );
    let res = rt::block_on(CurseForge::new().project("7"));
    assert!(matches!(res, Err(crate::ContentError::NotFound)));
  }

  fn files_route(server: &FakeServer, total: u64, files: Vec<serde_json::Value>) {
    let files = files.clone();
    server.route_fn("GET", "/api.curseforge.com/v1/mods/9/files", move |call| {
      let index: usize = query_of(call)["index"].parse().unwrap();
      let page: Vec<_> = files
        .iter()
        .skip(index)
        .take(PAGE as usize)
        .cloned()
        .collect();
      Response::new(
        200,
        json!({"data": page, "pagination": {"totalCount": total}}).to_string(),
      )
    });
  }

  #[test]
  fn versions_fetch_every_page_up_to_the_cap() {
    // (total files, pages requested)
    for (total, pages) in [(0, 1), (50, 1), (51, 2), (120, 3), (5_000, 10)] {
      let server = FakeServer::start();
      let files = (0..total.min(600))
        .map(|i| file_json(i, 9, &format!("{i:05}"), &[]))
        .collect();
      files_route(&server, total, files);
      let versions = rt::block_on(CurseForge::new().versions("9", &[], None)).unwrap();
      let calls = server.calls();
      assert_eq!(calls.len(), pages, "{total}");
      let mut indexes: Vec<u32> = calls
        .iter()
        .map(|c| query_of(c)["index"].parse().unwrap())
        .collect();
      indexes.sort();
      assert_eq!(
        indexes,
        (0..pages as u32).map(|p| p * PAGE).collect::<Vec<_>>()
      );
      assert_eq!(versions.len() as u64, total.min(pages as u64 * PAGE as u64));
      // newest first across pages
      assert!(
        versions
          .windows(2)
          .all(|w| w[0].published_at >= w[1].published_at)
      );
    }
  }

  #[test]
  fn versions_filter_by_loader() {
    let server = FakeServer::start();
    files_route(
      &server,
      3,
      vec![
        file_json(1, 9, "1", &["Fabric", "1.21.1"]),
        file_json(2, 9, "2", &["Forge", "1.21.1"]),
        // no loader tag, kept for any loader
        file_json(3, 9, "3", &["1.21.1", "Client"]),
      ],
    );
    let cf = CurseForge::new();
    let ids = |loaders: &[&str], game| {
      let loaders: Vec<String> = loaders.iter().map(|s| s.to_string()).collect();
      rt::block_on(cf.versions("9", &loaders, game))
        .unwrap()
        .into_iter()
        .map(|v| v.id)
        .collect::<Vec<_>>()
    };
    assert_eq!(ids(&["fabric"], Some("1.21.1")), ["3", "1"]);
    assert_eq!(ids(&[], None), ["3", "2", "1"]);
    // quilt profiles take fabric mods too, the server filter is skipped for two loaders
    assert_eq!(ids(&["quilt", "fabric"], None), ["3", "1"]);

    let calls = server.calls();
    let q: Vec<_> = calls.iter().map(query_of).collect();
    assert_eq!(q[0]["gameVersion"], "1.21.1");
    assert_eq!(q[0]["modLoaderType"], "4");
    assert!(!q[1].contains_key("gameVersion") && !q[1].contains_key("modLoaderType"));
    assert!(!q[2].contains_key("modLoaderType"));
    assert_eq!(q[0]["pageSize"], "50");
  }

  #[test]
  fn versions_fail_when_a_later_page_fails() {
    let server = FakeServer::start();
    server.route_fn("GET", "/api.curseforge.com/v1/mods/9/files", |call| {
      if query_of(call)["index"] == "0" {
        Response::new(
          200,
          json!({"data": [], "pagination": {"totalCount": 60}}).to_string(),
        )
      } else {
        Response::new(404, "")
      }
    });
    let res = rt::block_on(CurseForge::new().versions("9", &[], None));
    assert!(res.is_err());
  }

  #[test]
  fn summaries_skip_non_numeric_ids() {
    let server = FakeServer::start();
    server.route(
      "POST",
      "/api.curseforge.com/v1/mods",
      200,
      json!({"data": [mod_json(5, json!([]))]}).to_string(),
    );
    let cf = CurseForge::new();
    let none = rt::block_on(cf.summaries(&["AANobbMI".into(), "".into()])).unwrap();
    assert!(none.is_empty());
    assert!(server.calls().is_empty());

    let some = rt::block_on(cf.summaries(&["5".into(), "x".into()])).unwrap();
    assert_eq!(some[0].id, "5");
    assert_eq!(
      json_of(&server.calls()[0]),
      json!({"modIds": [5], "filterPcOnly": true})
    );
  }

  #[test]
  fn files_and_fingerprints_skip_empty_input() {
    let server = FakeServer::start();
    let cf = CurseForge::new();
    rt::block_on(async {
      assert!(cf.files(&[]).await.unwrap().is_empty());
      assert!(cf.by_hashes(&[]).await.unwrap().is_empty());
      assert!(
        cf.latest_for(&[], &[], "1.21.1", false)
          .await
          .unwrap()
          .is_empty()
      );
    });
    assert!(server.calls().is_empty());
  }

  fn fingerprints_route(server: &FakeServer, matched: &[(u64, u64)]) {
    let matches: Vec<_> = matched
      .iter()
      .map(|(fp, mod_id)| json!({"file": file_json(*fp, *mod_id, "d", &["1.21.1"])}))
      .collect();
    server.route(
      "POST",
      "/api.curseforge.com/v1/fingerprints/432",
      200,
      json!({"data": {"exactMatches": matches}}).to_string(),
    );
  }

  fn hashes(fingerprint: u32) -> FileHashes {
    FileHashes {
      sha1: format!("local{fingerprint}"),
      fingerprint,
    }
  }

  #[test]
  fn by_hashes_maps_fingerprints_back_to_sha1() {
    let server = FakeServer::start();
    // 99 matched nothing we sent, it is dropped
    fingerprints_route(&server, &[(11, 1), (99, 2)]);
    let res = rt::block_on(CurseForge::new().by_hashes(&[hashes(11), hashes(12)])).unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res["local11"].project_id, "1");
    assert_eq!(
      json_of(&server.calls()[0]),
      json!({"fingerprints": [11, 12]})
    );
  }

  #[test]
  fn latest_for_picks_the_newest_matching_index() {
    let server = FakeServer::start();
    fingerprints_route(&server, &[(11, 1)]);
    let indexes = json!([
      {"gameVersion": "1.21.1", "fileId": 100, "releaseType": 1, "modLoader": 4},
      {"gameVersion": "1.21.1", "fileId": 300, "releaseType": 2, "modLoader": 4},
      {"gameVersion": "1.21.1", "fileId": 400, "releaseType": 1, "modLoader": 1},
      {"gameVersion": "1.21.1", "fileId": 150, "releaseType": 1, "modLoader": null},
      {"gameVersion": "1.20.1", "fileId": 900, "releaseType": 1, "modLoader": 4}
    ]);
    server.route(
      "POST",
      "/api.curseforge.com/v1/mods",
      200,
      json!({"data": [mod_json(1, indexes)]}).to_string(),
    );
    server.route_fn("POST", "/api.curseforge.com/v1/mods/files", |call| {
      let ids: Vec<u64> = serde_json::from_value(json_of(call)["fileIds"].clone()).unwrap();
      let files: Vec<_> = ids.iter().map(|id| file_json(*id, 1, "d", &[])).collect();
      Response::new(200, json!({ "data": files }).to_string())
    });
    let cf = CurseForge::new();
    let latest = |loaders: &[&str], release_only| {
      let loaders: Vec<String> = loaders.iter().map(|s| s.to_string()).collect();
      rt::block_on(cf.latest_for(&[hashes(11)], &loaders, "1.21.1", release_only))
        .unwrap()
        .get("local11")
        .map(|v| v.id.clone())
    };
    assert_eq!(latest(&["fabric"], true), Some("150".into()));
    assert_eq!(latest(&["fabric"], false), Some("300".into()));
    assert_eq!(latest(&[], true), Some("400".into()));
    assert_eq!(latest(&["neoforge"], true), Some("150".into()));
  }

  #[test]
  fn latest_for_without_matches_stops_early() {
    let server = FakeServer::start();
    fingerprints_route(&server, &[]);
    let res = rt::block_on(CurseForge::new().latest_for(&[hashes(1)], &[], "1.21.1", false));
    assert!(res.unwrap().is_empty());
    assert_eq!(server.calls().len(), 1);
  }

  #[test]
  fn cdn_urls() {
    let cases = [
      ("https://edge.forgecdn.net/files/1/2/a.jar", true),
      ("https://forgecdn.net/a.jar", true),
      ("http://edge.forgecdn.net/a.jar", false),
      ("https://evilforgecdn.net/a.jar", false),
      ("https://forgecdn.net.evil.com/a.jar", false),
      ("https://www.curseforge.com/a.jar", false),
    ];
    for (url, ok) in cases {
      assert_eq!(cdn_url(&Url::parse(url).unwrap()), ok, "{url}");
    }
  }

  #[test]
  fn downloads_do_not_follow_redirects_off_the_cdn() {
    let server = FakeServer::start();
    let target = format!("{}/elsewhere", server.origin());
    server.route_fn("GET", "/start", move |_| {
      Response::new(302, "").header("Location", &target)
    });
    server.route("GET", "/elsewhere", 200, "key leaked");
    let cf = CurseForge::new();
    let url = format!("{}/start", server.origin());
    let res = rt::block_on(cf.download_client().get(url).send());
    assert!(res.unwrap_err().is_redirect());
    let calls = server.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].header("x-api-key"), Some(CF_KEY));
  }

  #[test]
  fn normalizes_category_names() {
    assert_eq!(normalize("API & Library"), "apilibrary");
    assert_eq!(normalize("Adventure and RPG"), "adventureandrpg");
    assert_eq!(normalize("Über-Mod"), "bermod");
  }
}
