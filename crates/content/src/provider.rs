use std::collections::HashMap;

use futures_util::future::join_all;

use super::{
  Result,
  curseforge::CurseForge,
  model::{
    ContentKind, FileHashes, ProjectDetail, ProjectSummary, SearchQuery, SearchResult, SearchSort,
    Source, VersionInfo,
  },
  modrinth::Modrinth,
};
use proton_versions::loader::LoaderType;

pub struct SearchPage {
  pub hits: Vec<ProjectSummary>,
  pub total: u64,
}

#[async_trait::async_trait]
pub trait ContentProvider: Send + Sync {
  async fn search(&self, query: &SearchQuery) -> Result<SearchPage>;
  async fn project(&self, id: &str) -> Result<ProjectDetail>;
  /// Empty `loaders` means no loader filter
  async fn versions(
    &self,
    id: &str,
    loaders: &[String],
    game: Option<&str>,
  ) -> Result<Vec<VersionInfo>>;
  /// Projects by id in one request, `author` may be empty
  async fn summaries(&self, ids: &[String]) -> Result<Vec<ProjectSummary>>;
  /// Identifies local files, keyed by sha1
  async fn by_hashes(&self, files: &[FileHashes]) -> Result<HashMap<String, VersionInfo>>;
  /// Newest version matching loaders and game version for each file, keyed by sha1.
  /// `release_only` skips betas and alphas.
  async fn latest_for(
    &self,
    files: &[FileHashes],
    loaders: &[String],
    game: &str,
    release_only: bool,
  ) -> Result<HashMap<String, VersionInfo>>;
}

pub struct Providers {
  modrinth: Modrinth,
  curseforge: CurseForge,
}

#[allow(clippy::new_without_default)]
impl Providers {
  pub fn new() -> Self {
    Self {
      modrinth: Modrinth::new(),
      curseforge: CurseForge::new(),
    }
  }

  pub fn modrinth(&self) -> &Modrinth {
    &self.modrinth
  }

  pub fn curseforge(&self) -> &CurseForge {
    &self.curseforge
  }

  /// Client for file downloads, CurseForge CDN downloads need the CurseForge key
  pub fn download_client(&self, source: Source) -> &reqwest::Client {
    match source {
      Source::Modrinth => self.modrinth.download_client(),
      Source::CurseForge => self.curseforge.download_client(),
    }
  }

  pub fn get(&self, source: Source) -> &dyn ContentProvider {
    match source {
      Source::Modrinth => &self.modrinth,
      Source::CurseForge => &self.curseforge,
    }
  }

  /// Searches every requested source in parallel. One failing source becomes a warning.
  pub async fn search_all(&self, query: &SearchQuery) -> Result<SearchResult> {
    let n = query.sources.len().max(1) as u32;
    // ponytail: offset and limit split evenly, pages drift when one source runs out of hits
    let split = SearchQuery {
      offset: query.offset / n,
      limit: query.limit.div_ceil(n),
      ..query.clone()
    };
    let results = join_all(query.sources.iter().map(|s| self.get(*s).search(&split))).await;
    merge(
      query.sort,
      query.sources.iter().copied().zip(results).collect(),
    )
  }
}

fn merge(sort: SearchSort, results: Vec<(Source, Result<SearchPage>)>) -> Result<SearchResult> {
  let mut pages = Vec::new();
  let mut warnings = Vec::new();
  let mut error = None;
  for (source, result) in results {
    match result {
      Ok(page) => pages.push(page),
      Err(err) => {
        let name = match source {
          Source::Modrinth => "Modrinth",
          Source::CurseForge => "CurseForge",
        };
        warnings.push(format!("{name} search failed: {err}"));
        error = Some(err);
      }
    }
  }
  if pages.is_empty()
    && let Some(err) = error
  {
    return Err(err);
  }

  let total = pages.iter().map(|p| p.total).sum();
  let mut iters: Vec<_> = pages.into_iter().map(|p| p.hits.into_iter()).collect();
  let mut hits = Vec::new();
  // interleave by rank: a0, b0, a1, b1, ...
  loop {
    let before = hits.len();
    hits.extend(iters.iter_mut().filter_map(Iterator::next));
    if hits.len() == before {
      break;
    }
  }
  if sort == SearchSort::Downloads {
    hits.sort_by_key(|h| std::cmp::Reverse(h.downloads));
  }

  Ok(SearchResult {
    hits,
    total,
    warnings,
  })
}

/// Loader names a profile accepts on a provider for a kind of content
pub fn profile_loaders(source: Source, kind: ContentKind, loader: LoaderType) -> Vec<String> {
  let names: &[&str] = match (kind, source) {
    (ContentKind::ResourcePack, Source::Modrinth) => &["minecraft"],
    (ContentKind::ResourcePack, Source::CurseForge) => &[],
    (ContentKind::Shader, Source::Modrinth) => &["iris", "optifine"],
    (ContentKind::Shader, Source::CurseForge) => &[],
    (ContentKind::Mod | ContentKind::Modpack, _) => match loader {
      LoaderType::Vanilla => &[],
      LoaderType::Fabric => &["fabric"],
      LoaderType::Quilt => &["quilt", "fabric"],
      LoaderType::Forge => &["forge"],
      LoaderType::NeoForge => &["neoforge"],
    },
  };
  names.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ContentError;

  fn hit(source: Source, id: &str, downloads: u64) -> ProjectSummary {
    ProjectSummary {
      source,
      id: id.into(),
      slug: None,
      kind: ContentKind::Mod,
      name: id.into(),
      author: String::new(),
      description: String::new(),
      icon_url: None,
      categories: vec![],
      loaders: vec![],
      game_versions: vec![],
      downloads,
      updated_at: String::new(),
      client_side: None,
      server_side: None,
      url: String::new(),
    }
  }

  fn page(source: Source, hits: &[(&str, u64)]) -> Result<SearchPage> {
    Ok(SearchPage {
      hits: hits.iter().map(|(id, d)| hit(source, id, *d)).collect(),
      total: 100,
    })
  }

  fn ids(r: &SearchResult) -> Vec<&str> {
    r.hits.iter().map(|h| h.id.as_str()).collect()
  }

  #[test]
  fn interleaves_by_rank() {
    let r = merge(
      SearchSort::Relevance,
      vec![
        (
          Source::Modrinth,
          page(Source::Modrinth, &[("m1", 1), ("m2", 1), ("m3", 1)]),
        ),
        (Source::CurseForge, page(Source::CurseForge, &[("c1", 9)])),
      ],
    )
    .unwrap();
    assert_eq!(ids(&r), ["m1", "c1", "m2", "m3"]);
    assert_eq!(r.total, 200);
    assert!(r.warnings.is_empty());
  }

  #[test]
  fn sorts_by_downloads() {
    let r = merge(
      SearchSort::Downloads,
      vec![
        (
          Source::Modrinth,
          page(Source::Modrinth, &[("m1", 50), ("m2", 10)]),
        ),
        (
          Source::CurseForge,
          page(Source::CurseForge, &[("c1", 30), ("c2", 5)]),
        ),
      ],
    )
    .unwrap();
    assert_eq!(ids(&r), ["m1", "c1", "m2", "c2"]);
  }

  #[test]
  fn failing_provider_becomes_warning() {
    let r = merge(
      SearchSort::Relevance,
      vec![
        (Source::Modrinth, page(Source::Modrinth, &[("m1", 1)])),
        (Source::CurseForge, Err(ContentError::RateLimited)),
      ],
    )
    .unwrap();
    assert_eq!(ids(&r), ["m1"]);
    assert_eq!(r.warnings, ["CurseForge search failed: RateLimited"]);
  }

  #[test]
  fn all_providers_failing_is_an_error() {
    let r = merge(
      SearchSort::Relevance,
      vec![(Source::Modrinth, Err(ContentError::Network))],
    );
    assert!(matches!(r, Err(ContentError::Network)));
  }

  #[test]
  fn loader_mapping() {
    use ContentKind::*;
    let l = |s, k, t| profile_loaders(s, k, t);
    assert_eq!(
      l(Source::Modrinth, Mod, LoaderType::Quilt),
      ["quilt", "fabric"]
    );
    assert_eq!(
      l(Source::CurseForge, Mod, LoaderType::NeoForge),
      ["neoforge"]
    );
    assert!(l(Source::Modrinth, Mod, LoaderType::Vanilla).is_empty());
    assert_eq!(
      l(Source::Modrinth, ResourcePack, LoaderType::Fabric),
      ["minecraft"]
    );
    assert_eq!(
      l(Source::Modrinth, Shader, LoaderType::Fabric),
      ["iris", "optifine"]
    );
    assert!(l(Source::CurseForge, Shader, LoaderType::Fabric).is_empty());
  }
}
