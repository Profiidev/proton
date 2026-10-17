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
  use proton_utils::{
    rt,
    test_http::{Call, FakeServer},
  };
  use reqwest::Url;

  use super::*;
  use crate::ContentError;

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

  /// Query pairs of a recorded call
  fn query_of(call: &Call) -> HashMap<String, String> {
    Url::parse(&format!("http://x{}", call.path))
      .unwrap()
      .query_pairs()
      .into_owned()
      .collect()
  }

  fn version_json(id: &str, published: &str) -> serde_json::Value {
    json!({
      "id": id, "project_id": "p", "name": id, "version_number": id, "version_type": "release",
      "game_versions": ["1.21.1"], "loaders": ["fabric"], "date_published": published,
      "files": [{"url": "https://cdn.modrinth.com/f.jar", "filename": "f.jar", "primary": true,
        "size": 1, "hashes": {"sha1": format!("sha-{id}"), "sha512": null}}],
      "dependencies": []
    })
  }

  fn project_json(organization: Option<&str>) -> serde_json::Value {
    json!({
      "id": "AANobbMI", "slug": null, "project_type": "shader", "team": "t1",
      "organization": organization, "title": "Sodium", "description": "fast", "body": "# Hi",
      "icon_url": "https://cdn/icon.png", "categories": ["optimization"],
      "additional_categories": ["utility"], "loaders": ["iris"], "game_versions": ["1.21.1"],
      "downloads": 7, "updated": "u", "client_side": "required", "server_side": null,
      "gallery": [{"url": "https://cdn/g.png", "title": null}],
      "source_url": "https://src", "issues_url": "", "wiki_url": null, "discord_url": "https://dc"
    })
  }

  #[test]
  fn search_sends_facets_sort_and_caps_the_limit() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/api.modrinth.com/v2/search",
      200,
      json!({"total_hits": 42, "hits": [{
        "project_id": "a", "project_type": "modpack", "slug": null, "author": "me", "title": "A",
        "description": "", "categories": ["fabric"], "versions": [], "downloads": 1,
        "icon_url": null, "date_modified": "d", "client_side": null, "server_side": null
      }]})
      .to_string(),
    );
    let q = SearchQuery {
      query: "sod ium".into(),
      sort: SearchSort::Downloads,
      offset: 40,
      limit: 500,
      ..query()
    };
    let modrinth = Modrinth::new();
    let page = rt::block_on(modrinth.search(&q)).unwrap();
    assert_eq!(page.total, 42);
    let hit = &page.hits[0];
    assert_eq!(
      (hit.kind, hit.loaders.clone()),
      (ContentKind::Modpack, vec!["fabric".into()])
    );
    // no slug links by id
    assert_eq!(hit.url, "https://modrinth.com/modpack/a");

    let calls = server.calls();
    let q = query_of(&calls[0]);
    assert_eq!(q["query"], "sod ium");
    assert_eq!(q["index"], "downloads");
    assert_eq!(q["offset"], "40");
    assert_eq!(q["limit"], "100");
    assert_eq!(q["facets"], facets(&query()));

    // the same page again comes from the cache
    rt::block_on(modrinth.search(&SearchQuery {
      query: "sod ium".into(),
      sort: SearchSort::Downloads,
      offset: 40,
      limit: 500,
      ..query()
    }))
    .unwrap();
    assert_eq!(server.calls().len(), 1);
  }

  #[test]
  fn sort_and_kind_names() {
    let sorts = [
      (SearchSort::Relevance, "relevance"),
      (SearchSort::Downloads, "downloads"),
      (SearchSort::Updated, "updated"),
      (SearchSort::Newest, "newest"),
    ];
    for (sort, name) in sorts {
      assert_eq!(sort_str(sort), name);
    }
    for k in [
      ContentKind::Mod,
      ContentKind::Modpack,
      ContentKind::ResourcePack,
      ContentKind::Shader,
    ] {
      assert_eq!(kind(kind_str(k)), k);
    }
    // datapacks and plugins show up as mods
    assert_eq!(kind("datapack"), ContentKind::Mod);
  }

  #[test]
  fn facets_edge_cases() {
    let bare = SearchQuery {
      loaders: vec![],
      categories: vec![],
      game_version: None,
      ..query()
    };
    assert_eq!(facets(&bare), r#"[["project_type:mod"]]"#);
    // resource packs have no loaders on Modrinth, a loader facet would hide all of them
    let packs = SearchQuery {
      kind: ContentKind::ResourcePack,
      ..bare.clone()
    };
    assert_eq!(facets(&packs), r#"[["project_type:resourcepack"]]"#);
    let client = SearchQuery {
      environment: Some(Environment::Client),
      ..bare
    };
    let f: Vec<Vec<String>> = serde_json::from_str(&facets(&client)).unwrap();
    assert_eq!(f[1].len(), environments(Environment::Client).len());
    assert!(f[1].contains(&"environment:singleplayer_only".to_string()));
  }

  #[test]
  fn project_with_organization_uses_its_name() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/api.modrinth.com/v2/project/AANobbMI",
      200,
      project_json(Some("org1")).to_string(),
    );
    server.route(
      "GET",
      "/api.modrinth.com/v3/organization/org1",
      200,
      r#"{"name":"CaffeineMC"}"#,
    );
    let detail = rt::block_on(Modrinth::new().project("AANobbMI")).unwrap();
    assert_eq!(detail.summary.author, "CaffeineMC");
    assert_eq!(detail.summary.kind, ContentKind::Shader);
    assert_eq!(detail.summary.categories, ["optimization", "utility"]);
    assert_eq!(detail.summary.url, "https://modrinth.com/shader/AANobbMI");
    assert!(detail.body.contains("<h1>"), "{}", detail.body);
    assert_eq!(detail.gallery[0].url, "https://cdn/g.png");
    // empty links are dropped like missing ones
    assert_eq!(detail.links.source.as_deref(), Some("https://src"));
    assert_eq!(detail.links.issues, None);
    assert_eq!(detail.links.wiki, None);
    assert_eq!(detail.links.discord.as_deref(), Some("https://dc"));
  }

  #[test]
  fn project_author_comes_from_the_team() {
    let cases = [
      // the owner, wherever it is listed
      (
        json!([{"role": "Dev", "user": {"username": "b"}}, {"role": "Owner", "user": {"username": "a"}}]),
        "a",
      ),
      // no owner falls back to the first member
      (json!([{"role": "Dev", "user": {"username": "b"}}]), "b"),
      (json!([]), ""),
    ];
    for (members, author) in cases {
      let server = FakeServer::start();
      server.route(
        "GET",
        "/api.modrinth.com/v2/project/AANobbMI",
        200,
        project_json(None).to_string(),
      );
      server.route(
        "GET",
        "/api.modrinth.com/v2/team/t1/members",
        200,
        members.to_string(),
      );
      let detail = rt::block_on(Modrinth::new().project("AANobbMI")).unwrap();
      assert_eq!(detail.summary.author, author);
    }
  }

  #[test]
  fn project_fails_when_the_author_lookup_fails() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/api.modrinth.com/v2/project/x",
      200,
      project_json(None).to_string(),
    );
    let res = rt::block_on(Modrinth::new().project("x"));
    assert!(matches!(res, Err(ContentError::NotFound)));
  }

  #[test]
  fn versions_filter_only_when_given_and_sort_newest_first() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/api.modrinth.com/v2/project/p/version",
      200,
      json!([
        version_json("old", "2024-01-01"),
        version_json("new", "2026-01-01"),
        version_json("mid", "2025-01-01")
      ])
      .to_string(),
    );
    let modrinth = Modrinth::new();
    let ids = |v: Vec<VersionInfo>| v.into_iter().map(|v| v.id).collect::<Vec<_>>();
    let all = rt::block_on(modrinth.versions("p", &[], None)).unwrap();
    assert_eq!(ids(all), ["new", "mid", "old"]);
    rt::block_on(modrinth.versions("p", &["fabric".into(), "quilt".into()], Some("1.21.1")))
      .unwrap();

    let calls = server.calls();
    let unfiltered = query_of(&calls[0]);
    assert_eq!(unfiltered.len(), 1);
    assert_eq!(unfiltered["include_changelog"], "false");
    let filtered = query_of(&calls[1]);
    assert_eq!(filtered["loaders"], r#"["fabric","quilt"]"#);
    assert_eq!(filtered["game_versions"], r#"["1.21.1"]"#);
  }

  #[test]
  fn empty_id_lists_skip_the_request() {
    let server = FakeServer::start();
    let modrinth = Modrinth::new();
    rt::block_on(async {
      assert!(modrinth.versions_by_id(&[]).await.unwrap().is_empty());
      assert!(modrinth.summaries(&[]).await.unwrap().is_empty());
      assert!(modrinth.by_hashes(&[]).await.unwrap().is_empty());
      let latest = modrinth.latest_for(&[], &[], "1.21.1", true).await;
      assert!(latest.unwrap().is_empty());
    });
    assert!(server.calls().is_empty());
  }

  #[test]
  fn bulk_lookups_send_ids_as_json() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/api.modrinth.com/v2/versions",
      200,
      json!([version_json("a", "d")]).to_string(),
    );
    server.route(
      "GET",
      "/api.modrinth.com/v2/projects",
      200,
      json!([project_json(Some("org"))]).to_string(),
    );
    let modrinth = Modrinth::new();
    let ids = vec!["a".to_string(), "b\"c".to_string()];
    let versions = rt::block_on(modrinth.versions_by_id(&ids)).unwrap();
    assert_eq!(versions[0].files[0].sha1, "sha-a");
    let summaries = rt::block_on(modrinth.summaries(&ids)).unwrap();
    // summaries never look up the author
    assert_eq!(summaries[0].author, "");

    let calls = server.calls();
    assert_eq!(calls.len(), 2);
    for call in &calls {
      assert_eq!(query_of(call)["ids"], r#"["a","b\"c"]"#);
    }
  }

  fn hashes(sha1: &str) -> FileHashes {
    FileHashes {
      sha1: sha1.into(),
      fingerprint: 0,
    }
  }

  #[test]
  fn by_hashes_posts_sha1s() {
    let server = FakeServer::start();
    server.route(
      "POST",
      "/api.modrinth.com/v2/version_files",
      200,
      json!({"h1": version_json("a", "d")}).to_string(),
    );
    let res = rt::block_on(Modrinth::new().by_hashes(&[hashes("h1"), hashes("h2")])).unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res["h1"].id, "a");
    let body: serde_json::Value = serde_json::from_str(server.calls()[0].body_str()).unwrap();
    assert_eq!(body, json!({"hashes": ["h1", "h2"], "algorithm": "sha1"}));
  }

  #[test]
  fn latest_for_adds_loaders_and_release_filter_only_when_set() {
    let server = FakeServer::start();
    server.route(
      "POST",
      "/api.modrinth.com/v2/version_files/update",
      200,
      json!({"h1": version_json("a", "d")}).to_string(),
    );
    let modrinth = Modrinth::new();
    let files = [hashes("h1")];
    rt::block_on(async {
      modrinth
        .latest_for(&files, &[], "1.21.1", false)
        .await
        .unwrap();
      let res = modrinth
        .latest_for(&files, &["fabric".into()], "1.21.1", true)
        .await
        .unwrap();
      assert_eq!(res["h1"].id, "a");
    });
    let bodies: Vec<serde_json::Value> = server
      .calls()
      .iter()
      .map(|c| serde_json::from_str(c.body_str()).unwrap())
      .collect();
    assert_eq!(
      bodies,
      [
        json!({"hashes": ["h1"], "algorithm": "sha1", "game_versions": ["1.21.1"]}),
        json!({"hashes": ["h1"], "algorithm": "sha1", "game_versions": ["1.21.1"],
          "loaders": ["fabric"], "version_types": ["release"]}),
      ]
    );
  }

  #[test]
  fn categories_filter_by_kind_from_one_cached_request() {
    let server = FakeServer::start();
    server.route(
      "GET",
      "/api.modrinth.com/v2/tag/category",
      200,
      json!([
        {"name": "magic", "project_type": "mod", "header": "categories"},
        {"name": "realistic", "project_type": "shader", "header": "style"},
        {"name": "fabric", "project_type": "mod", "header": "loaders"}
      ])
      .to_string(),
    );
    let modrinth = Modrinth::new();
    let names = |kind| {
      rt::block_on(modrinth.categories(kind))
        .unwrap()
        .into_iter()
        .map(|c| (c.name, c.header))
        .collect::<Vec<_>>()
    };
    assert_eq!(
      names(ContentKind::Mod),
      [
        ("magic".into(), "categories".into()),
        ("fabric".into(), "loaders".into())
      ]
    );
    assert_eq!(
      names(ContentKind::Shader),
      [("realistic".to_string(), "style".to_string())]
    );
    assert!(names(ContentKind::Modpack).is_empty());
    assert_eq!(server.calls().len(), 1);
  }
}
