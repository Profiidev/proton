use std::collections::HashMap;

use reqwest::{header::HeaderMap, redirect::Policy};
use serde::Deserialize;
use serde_json::json;

use super::{
  Result,
  http::{Http, api},
  model::{
    Category, Channel, ContentKind, Dependency, DependencyKind, Environment, FileHashes, FileInfo,
    GalleryImage, ProjectDetail, ProjectLinks, ProjectSummary, SearchQuery, SearchSort, Source,
    VersionInfo,
  },
  provider::{ContentProvider, SearchPage},
  render,
};

const API: &str = "https://api.modrinth.com/v2";
const API_V3: &str = "https://api.modrinth.com/v3";

/// Modrinth lists loaders among the categories of a search hit
const LOADERS: &[&str] = &[
  "fabric",
  "forge",
  "neoforge",
  "quilt",
  "iris",
  "optifine",
  "canvas",
  "vanilla",
  "minecraft",
  "liteloader",
  "modloader",
  "rift",
  "datapack",
];

pub struct Modrinth {
  http: Http,
}

#[allow(clippy::new_without_default)]
impl Modrinth {
  pub fn download_client(&self) -> &reqwest::Client {
    &self.http.download
  }

  pub fn new() -> Self {
    Self {
      http: Http::new(HeaderMap::new(), Policy::default()),
    }
  }

  /// Versions by id in one request, order not kept
  pub async fn versions_by_id(&self, ids: &[String]) -> Result<Vec<VersionInfo>> {
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    // ponytail: one GET for every id, chunk it if huge packs hit the URL length limit
    let ids = json!(ids).to_string();
    let versions: Vec<Version> = self
      .http
      .get(api(API, &["versions"], &[("ids", &ids)]))
      .await?;
    Ok(versions.into_iter().map(Version::into_info).collect())
  }

  /// Filter tags for a kind, grouped by their `header` in the UI
  pub async fn categories(&self, kind: ContentKind) -> Result<Vec<Category>> {
    let tags: Vec<Tag> = self
      .http
      .get_cached(api(API, &["tag", "category"], &[]))
      .await?;
    Ok(
      tags
        .into_iter()
        .filter(|t| t.project_type == kind_str(kind))
        .map(|t| Category {
          name: t.name,
          header: t.header,
          kind,
        })
        .collect(),
    )
  }

  async fn author(&self, project: &Project) -> Result<String> {
    if let Some(org) = &project.organization {
      let org: Organization = self
        .http
        .get_cached(api(API_V3, &["organization", org], &[]))
        .await?;
      return Ok(org.name);
    }
    let members: Vec<Member> = self
      .http
      .get_cached(api(API, &["team", &project.team, "members"], &[]))
      .await?;
    Ok(
      members
        .iter()
        .find(|m| m.role == "Owner")
        .or(members.first())
        .map(|m| m.user.username.clone())
        .unwrap_or_default(),
    )
  }
}

#[async_trait::async_trait]
impl ContentProvider for Modrinth {
  async fn search(&self, query: &SearchQuery) -> Result<SearchPage> {
    let facets = facets(query);
    let offset = query.offset.to_string();
    let limit = query.limit.min(100).to_string();
    let url = api(
      API,
      &["search"],
      &[
        ("query", &query.query),
        ("facets", &facets),
        ("index", sort_str(query.sort)),
        ("offset", &offset),
        ("limit", &limit),
      ],
    );
    let res: SearchResponse = self.http.get_cached(url).await?;
    Ok(SearchPage {
      hits: res.hits.into_iter().map(Hit::into_summary).collect(),
      total: res.total_hits,
    })
  }

  async fn project(&self, id: &str) -> Result<ProjectDetail> {
    let project: Project = self
      .http
      .get_cached(api(API, &["project", id], &[]))
      .await?;
    let author = self.author(&project).await?;

    let blank = |s: Option<String>| s.filter(|s| !s.is_empty());
    Ok(ProjectDetail {
      body: render::markdown(&project.body),
      gallery: project
        .gallery
        .iter()
        .map(|g| GalleryImage {
          url: g.url.clone(),
          title: g.title.clone(),
        })
        .collect(),
      links: ProjectLinks {
        source: blank(project.source_url.clone()),
        issues: blank(project.issues_url.clone()),
        wiki: blank(project.wiki_url.clone()),
        discord: blank(project.discord_url.clone()),
      },
      summary: project.into_summary(author),
    })
  }

  async fn summaries(&self, ids: &[String]) -> Result<Vec<ProjectSummary>> {
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    let ids = json!(ids).to_string();
    let projects: Vec<Project> = self
      .http
      .get_cached(api(API, &["projects"], &[("ids", &ids)]))
      .await?;
    // the author needs one request per project, summaries leave it empty
    Ok(
      projects
        .into_iter()
        .map(|p| p.into_summary(String::new()))
        .collect(),
    )
  }

  async fn versions(
    &self,
    id: &str,
    loaders: &[String],
    game: Option<&str>,
  ) -> Result<Vec<VersionInfo>> {
    let loaders = json!(loaders).to_string();
    let games = json!(game.into_iter().collect::<Vec<_>>()).to_string();
    let mut query = vec![("include_changelog", "false")];
    if loaders != "[]" {
      query.push(("loaders", &loaders));
    }
    if games != "[]" {
      query.push(("game_versions", &games));
    }
    let versions: Vec<Version> = self
      .http
      .get(api(API, &["project", id, "version"], &query))
      .await?;
    Ok(newest_first(versions))
  }

  async fn by_hashes(&self, files: &[FileHashes]) -> Result<HashMap<String, VersionInfo>> {
    if files.is_empty() {
      return Ok(HashMap::new());
    }
    let hashes: Vec<_> = files.iter().map(|f| &f.sha1).collect();
    let res: HashMap<String, Version> = self
      .http
      .post(
        api(API, &["version_files"], &[]),
        &json!({ "hashes": hashes, "algorithm": "sha1" }),
      )
      .await?;
    Ok(res.into_iter().map(|(h, v)| (h, v.into_info())).collect())
  }

  async fn latest_for(
    &self,
    files: &[FileHashes],
    loaders: &[String],
    game: &str,
    release_only: bool,
  ) -> Result<HashMap<String, VersionInfo>> {
    if files.is_empty() {
      return Ok(HashMap::new());
    }
    let hashes: Vec<_> = files.iter().map(|f| &f.sha1).collect();
    let mut body = json!({ "hashes": hashes, "algorithm": "sha1", "game_versions": [game] });
    if !loaders.is_empty() {
      body["loaders"] = json!(loaders);
    }
    if release_only {
      body["version_types"] = json!(["release"]);
    }
    let res: HashMap<String, Version> = self
      .http
      .post(api(API, &["version_files", "update"], &[]), &body)
      .await?;
    Ok(res.into_iter().map(|(h, v)| (h, v.into_info())).collect())
  }
}

fn newest_first(versions: Vec<Version>) -> Vec<VersionInfo> {
  let mut versions: Vec<_> = versions.into_iter().map(Version::into_info).collect();
  versions.sort_by(|a, b| b.published_at.cmp(&a.published_at));
  versions
}

fn facets(query: &SearchQuery) -> String {
  let mut facets = vec![vec![format!("project_type:{}", kind_str(query.kind))]];
  if !query.loaders.is_empty() && query.kind != ContentKind::ResourcePack {
    facets.push(
      query
        .loaders
        .iter()
        .map(|l| format!("categories:{l}"))
        .collect(),
    );
  }
  for category in &query.categories {
    facets.push(vec![format!("categories:{category}")]);
  }
  if let Some(version) = &query.game_version {
    facets.push(vec![format!("versions:{version}")]);
  }
  if let Some(env) = query.environment {
    facets.push(
      environments(env)
        .iter()
        .map(|e| format!("environment:{e}"))
        .collect(),
    );
  }
  json!(facets).to_string()
}

/// `environment` values of projects that run on the given side
fn environments(env: Environment) -> &'static [&'static str] {
  match env {
    Environment::Client => &[
      "client_only",
      "client_only_server_optional",
      "client_or_server",
      "client_or_server_prefers_both",
      "client_and_server",
      "server_only_client_optional",
      "singleplayer_only",
    ],
    Environment::Server => &[
      "server_only",
      "server_only_client_optional",
      "client_only_server_optional",
      "client_or_server",
      "client_or_server_prefers_both",
      "client_and_server",
      "dedicated_server_only",
    ],
  }
}

fn kind_str(kind: ContentKind) -> &'static str {
  match kind {
    ContentKind::Mod => "mod",
    ContentKind::Modpack => "modpack",
    ContentKind::ResourcePack => "resourcepack",
    ContentKind::Shader => "shader",
  }
}

fn kind(project_type: &str) -> ContentKind {
  match project_type {
    "modpack" => ContentKind::Modpack,
    "resourcepack" => ContentKind::ResourcePack,
    "shader" => ContentKind::Shader,
    _ => ContentKind::Mod,
  }
}

fn sort_str(sort: SearchSort) -> &'static str {
  match sort {
    SearchSort::Relevance => "relevance",
    SearchSort::Downloads => "downloads",
    SearchSort::Updated => "updated",
    SearchSort::Newest => "newest",
  }
}

fn web_url(project_type: &str, slug: &str) -> String {
  format!("https://modrinth.com/{project_type}/{slug}")
}

#[derive(Deserialize)]
struct Tag {
  name: String,
  project_type: String,
  header: String,
}

#[derive(Deserialize)]
struct SearchResponse {
  hits: Vec<Hit>,
  total_hits: u64,
}

#[derive(Deserialize)]
struct Hit {
  project_id: String,
  project_type: String,
  slug: Option<String>,
  author: String,
  title: String,
  description: String,
  categories: Vec<String>,
  versions: Vec<String>,
  downloads: u64,
  icon_url: Option<String>,
  date_modified: String,
  client_side: Option<String>,
  server_side: Option<String>,
}

impl Hit {
  fn into_summary(self) -> ProjectSummary {
    let (loaders, categories) = self
      .categories
      .into_iter()
      .partition(|c| LOADERS.contains(&c.as_str()));
    ProjectSummary {
      source: Source::Modrinth,
      url: web_url(
        &self.project_type,
        self.slug.as_deref().unwrap_or(&self.project_id),
      ),
      id: self.project_id,
      kind: kind(&self.project_type),
      name: self.title,
      author: self.author,
      description: self.description,
      icon_url: self.icon_url.filter(|s| !s.is_empty()),
      categories,
      loaders,
      game_versions: self.versions,
      downloads: self.downloads,
      updated_at: self.date_modified,
      client_side: self.client_side,
      server_side: self.server_side,
    }
  }
}

#[derive(Deserialize)]
struct Project {
  id: String,
  slug: Option<String>,
  project_type: String,
  team: String,
  organization: Option<String>,
  title: String,
  description: String,
  body: String,
  icon_url: Option<String>,
  categories: Vec<String>,
  additional_categories: Vec<String>,
  loaders: Vec<String>,
  game_versions: Vec<String>,
  downloads: u64,
  updated: String,
  client_side: Option<String>,
  server_side: Option<String>,
  gallery: Vec<Gallery>,
  source_url: Option<String>,
  issues_url: Option<String>,
  wiki_url: Option<String>,
  discord_url: Option<String>,
}

impl Project {
  fn into_summary(self, author: String) -> ProjectSummary {
    let mut categories = self.categories;
    categories.extend(self.additional_categories);
    ProjectSummary {
      source: Source::Modrinth,
      url: web_url(&self.project_type, self.slug.as_deref().unwrap_or(&self.id)),
      id: self.id,
      kind: kind(&self.project_type),
      name: self.title,
      author,
      description: self.description,
      icon_url: self.icon_url.filter(|s| !s.is_empty()),
      categories,
      loaders: self.loaders,
      game_versions: self.game_versions,
      downloads: self.downloads,
      updated_at: self.updated,
      client_side: self.client_side,
      server_side: self.server_side,
    }
  }
}

#[derive(Deserialize)]
struct Gallery {
  url: String,
  title: Option<String>,
}

#[derive(Deserialize)]
struct Organization {
  name: String,
}

#[derive(Deserialize)]
struct Member {
  role: String,
  user: User,
}

#[derive(Deserialize)]
struct User {
  username: String,
}

#[derive(Deserialize)]
struct Version {
  id: String,
  project_id: String,
  name: String,
  version_number: String,
  version_type: Channel,
  game_versions: Vec<String>,
  loaders: Vec<String>,
  date_published: String,
  files: Vec<File>,
  dependencies: Vec<VersionDependency>,
}

#[derive(Deserialize)]
struct File {
  url: String,
  filename: String,
  primary: bool,
  size: u64,
  hashes: Hashes,
}

#[derive(Deserialize)]
struct Hashes {
  sha1: String,
  sha512: Option<String>,
}

#[derive(Deserialize)]
struct VersionDependency {
  project_id: Option<String>,
  version_id: Option<String>,
  dependency_type: DependencyKind,
}

impl Version {
  fn into_info(self) -> VersionInfo {
    VersionInfo {
      source: Source::Modrinth,
      project_id: self.project_id,
      id: self.id,
      name: self.name,
      version_number: self.version_number,
      channel: self.version_type,
      game_versions: self.game_versions,
      loaders: self.loaders,
      published_at: self.date_published,
      files: self
        .files
        .into_iter()
        .map(|f| FileInfo {
          file_name: f.filename,
          url: Some(f.url),
          sha1: f.hashes.sha1,
          sha512: f.hashes.sha512,
          size: f.size,
          primary: f.primary,
        })
        .collect(),
      // ponytail: dependencies given only by version id or file name are dropped, resolve them via /version/{id} if packs need them
      dependencies: self
        .dependencies
        .into_iter()
        .filter_map(|d| {
          Some(Dependency {
            project_id: d.project_id?,
            version_id: d.version_id,
            kind: d.dependency_type,
          })
        })
        .collect(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn query() -> SearchQuery {
    SearchQuery {
      kind: ContentKind::Mod,
      query: String::new(),
      sources: vec![Source::Modrinth],
      loaders: vec!["quilt".into(), "fabric".into()],
      game_version: Some("1.21.1".into()),
      categories: vec!["optimization".into()],
      environment: None,
      sort: SearchSort::Relevance,
      offset: 0,
      limit: 20,
      compatible_with: None,
    }
  }

  #[test]
  fn builds_facets() {
    assert_eq!(
      facets(&query()),
      r#"[["project_type:mod"],["categories:quilt","categories:fabric"],["categories:optimization"],["versions:1.21.1"]]"#
    );
    let packs = SearchQuery {
      kind: ContentKind::ResourcePack,
      categories: vec![],
      game_version: None,
      environment: Some(Environment::Server),
      ..query()
    };
    let f = facets(&packs);
    assert!(f.starts_with(r#"[["project_type:resourcepack"],["environment:server_only","#));
  }

  #[test]
  fn hit_splits_loaders_from_categories() {
    let hit: Hit = serde_json::from_value(json!({
      "project_id": "AANobbMI", "project_type": "mod", "slug": "sodium", "author": "jellysquid3",
      "title": "Sodium", "description": "fast", "categories": ["fabric", "optimization", "quilt"],
      "versions": ["1.21.1"], "downloads": 5, "icon_url": "", "date_modified": "2026-01-01T00:00:00Z",
      "client_side": "required", "server_side": "unsupported"
    }))
    .unwrap();
    let s = hit.into_summary();
    assert_eq!(s.loaders, ["fabric", "quilt"]);
    assert_eq!(s.categories, ["optimization"]);
    assert_eq!(s.icon_url, None);
    assert_eq!(s.url, "https://modrinth.com/mod/sodium");
  }

  #[test]
  fn version_maps_enums() {
    let v: Version = serde_json::from_value(json!({
      "id": "v", "project_id": "p", "name": "n", "version_number": "1", "version_type": "beta",
      "game_versions": [], "loaders": [], "date_published": "d",
      "files": [{"url": "u", "filename": "f.jar", "primary": true, "size": 1, "hashes": {"sha1": "a", "sha512": "b"}}],
      "dependencies": [
        {"project_id": "x", "version_id": null, "dependency_type": "required"},
        {"project_id": null, "version_id": "y", "dependency_type": "embedded"},
        {"project_id": "z", "version_id": null, "dependency_type": "incompatible"}
      ]
    }))
    .unwrap();
    let info = v.into_info();
    assert_eq!(info.channel, Channel::Beta);
    assert_eq!(info.files[0].url.as_deref(), Some("u"));
    let kinds: Vec<_> = info.dependencies.iter().map(|d| d.kind).collect();
    assert_eq!(
      kinds,
      [DependencyKind::Required, DependencyKind::Incompatible]
    );
  }
}
