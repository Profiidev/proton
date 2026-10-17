//! Pure helpers ported from `lib/content.ts`, `content/discover.ts` and `content/install.ts`

use std::collections::{BTreeSet, HashSet};

use proton_content::model::{
  Category, ContentKind, Environment, Installed, ProjectSummary, SearchQuery, SearchSort, Source,
  UpdateInfo, VersionInfo,
};
use proton_profiles::config::Profile;
use proton_versions::loader::LoaderType;

use crate::router;

pub const PAGE_SIZE: u32 = 20;

/// Whether something can be installed into a profile, with an optional note
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Compat {
  pub ok: bool,
  pub note: Option<String>,
}

impl Compat {
  fn ok() -> Self {
    Self {
      ok: true,
      note: None,
    }
  }

  fn no(note: String) -> Self {
    Self {
      ok: false,
      note: Some(note),
    }
  }
}

pub fn loader_label(loader: LoaderType) -> String {
  format!("{loader:?}")
}

/// Whether a mod for `loaders` (lowercase ids) runs on the profile's loader
pub fn loader_compatible(loader: LoaderType, loaders: &[String]) -> Compat {
  let name = loader_label(loader);
  if loaders.iter().any(|l| *l == name.to_lowercase()) {
    return Compat::ok();
  }
  if loader == LoaderType::Quilt && loaders.iter().any(|l| l == "fabric") {
    return Compat {
      ok: true,
      note: Some("Fabric version, runs on Quilt".into()),
    };
  }
  Compat::no(format!("Not available for {name}"))
}

/// Whether a project or one of its versions (`game_versions`, `loaders`) fits `profile`
pub fn profile_compat(
  profile: &Profile,
  kind: ContentKind,
  game_versions: &[String],
  loaders: &[String],
) -> Compat {
  if kind == ContentKind::Modpack {
    return Compat::ok();
  }
  if !game_versions.is_empty() && !game_versions.contains(&profile.version) {
    return Compat::no(format!("Not available for {}", profile.version));
  }
  match kind {
    ContentKind::Mod => loader_compatible(profile.loader, loaders),
    // shaders need Iris
    ContentKind::Shader
      if !matches!(
        profile.loader,
        LoaderType::Fabric | LoaderType::Quilt | LoaderType::NeoForge
      ) =>
    {
      Compat::no("Shaders need Fabric, Quilt or NeoForge".into())
    }
    _ => Compat::ok(),
  }
}

pub fn summary_compat(profile: &Profile, s: &ProjectSummary) -> Compat {
  profile_compat(profile, s.kind, &s.game_versions, &s.loaders)
}

/// Remembered target, else the latest played, else the first profile
pub fn pick_target<'a>(profiles: &'a [Profile], remembered: Option<&str>) -> Option<&'a Profile> {
  profiles
    .iter()
    .find(|p| Some(p.id.as_str()) == remembered)
    .or_else(|| {
      profiles
        .iter()
        .filter(|p| p.last_played.is_some())
        .max_by_key(|p| p.last_played)
    })
    .or(profiles.first())
}

/// Discover filters
#[derive(Debug, Clone, PartialEq)]
pub struct Filters {
  pub query: String,
  pub sort: SearchSort,
  pub sources: Vec<Source>,
  /// Only show content for the target profile
  pub compatible: bool,
  pub loaders: Vec<String>,
  pub game_version: Option<String>,
  pub categories: Vec<String>,
  pub environment: Option<Environment>,
}

impl Filters {
  pub fn new(kind: ContentKind) -> Self {
    Self {
      query: String::new(),
      sort: SearchSort::Relevance,
      sources: vec![Source::Modrinth, Source::CurseForge],
      compatible: matches!(kind, ContentKind::Mod | ContentKind::ResourcePack),
      loaders: vec![],
      game_version: None,
      categories: vec![],
      environment: None,
    }
  }
}

/// Search query for `kind`, sending only the filters that apply to it
pub fn build_query(
  kind: ContentKind,
  filters: &Filters,
  target: Option<&str>,
  offset: u32,
) -> SearchQuery {
  let compatible = filters
    .compatible
    .then_some(target)
    .flatten()
    .filter(|_| kind != ContentKind::Modpack);
  let has_loaders = matches!(kind, ContentKind::Mod | ContentKind::Modpack);
  SearchQuery {
    kind,
    query: filters.query.trim().to_string(),
    sources: filters.sources.clone(),
    loaders: if has_loaders && compatible.is_none() {
      filters.loaders.clone()
    } else {
      vec![]
    },
    game_version: if compatible.is_some() {
      None
    } else {
      filters.game_version.clone()
    },
    categories: filters.categories.clone(),
    environment: if has_loaders {
      filters.environment
    } else {
      None
    },
    sort: filters.sort,
    offset,
    limit: PAGE_SIZE,
    compatible_with: compatible.map(str::to_string),
  }
}

/// Install button state of a project card against the target profile
pub fn card_action(
  id: &str,
  installed: &HashSet<String>,
  updates: &[UpdateInfo],
) -> proton_components::widgets::content::CardAction {
  use proton_components::widgets::content::CardAction;
  if updates.iter().any(|u| u.project_id == id) {
    CardAction::Update
  } else if installed.contains(id) {
    CardAction::Installed
  } else {
    CardAction::Install
  }
}

/// `world-generation` -> `World generation`
pub fn tag_label(tag: &str) -> String {
  let text = tag.replace('-', " ");
  let mut chars = text.chars();
  match chars.next() {
    Some(c) => c.to_uppercase().chain(chars).collect(),
    None => text,
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CategoryGroup {
  pub header: String,
  pub label: String,
  pub names: Vec<String>,
}

/// Filter checkbox groups, `Category` first (`Look` on shaders)
pub fn category_groups(kind: ContentKind, categories: &[Category]) -> Vec<CategoryGroup> {
  let mut groups: Vec<CategoryGroup> = vec![];
  for c in categories {
    match groups.iter_mut().find(|g| g.header == c.header) {
      Some(g) => g.names.push(c.name.clone()),
      None => groups.push(CategoryGroup {
        header: c.header.clone(),
        label: match c.header.as_str() {
          "categories" if kind == ContentKind::Shader => "Look".into(),
          "categories" => "Category".into(),
          "features" => "Features".into(),
          "performance impact" => "Performance cost".into(),
          "resolutions" => "Resolution".into(),
          h => tag_label(h),
        },
        names: vec![c.name.clone()],
      }),
    }
  }
  // stable, keeps the rest in order
  groups.sort_by_key(|g| g.header != "categories");
  groups
}

/// `Client`, `Server`, `Client and server`, None when unknown
pub fn runs_on(client: Option<&str>, server: Option<&str>) -> Option<&'static str> {
  let supported = |s: Option<&str>| s.is_some_and(|s| s != "unsupported");
  match (supported(client), supported(server)) {
    (true, true) => Some("Client and server"),
    (true, false) => Some("Client"),
    (false, true) => Some("Server"),
    _ => None,
  }
}

/// Card tags: categories, loaders (mods and packs), runs on; deduplicated in order
pub fn card_tags(p: &ProjectSummary) -> Vec<String> {
  let loaders = matches!(p.kind, ContentKind::Mod | ContentKind::Modpack)
    .then_some(&p.loaders)
    .into_iter()
    .flatten();
  let runs = runs_on(p.client_side.as_deref(), p.server_side.as_deref()).map(str::to_string);
  dedup(
    p.categories
      .iter()
      .chain(loaders)
      .map(|t| tag_label(t))
      .chain(runs),
  )
}

pub fn dedup(items: impl IntoIterator<Item = String>) -> Vec<String> {
  let mut seen = BTreeSet::new();
  items
    .into_iter()
    .filter(|t| seen.insert(t.clone()))
    .collect()
}

const IRIS: [&str; 2] = ["YL57xq9U", "455508"];
const SODIUM: [&str; 2] = ["AANobbMI", "394468"];

/// Whether the installed mods already include Iris and Sodium
pub fn shader_ready(mods: &[Installed]) -> bool {
  let has = |ids: [&str; 2]| {
    mods
      .iter()
      .any(|m| m.project_id.as_deref().is_some_and(|p| ids.contains(&p)))
  };
  has(IRIS) && has(SODIUM)
}

/// Game versions newest first by the known version order, unknown ones last
pub fn sort_versions(versions: &[String], known: &[String]) -> Vec<String> {
  let mut sorted: Vec<String> = known
    .iter()
    .filter(|v| versions.contains(v))
    .cloned()
    .collect();
  let rest: Vec<String> = versions
    .iter()
    .filter(|v| !sorted.contains(v))
    .cloned()
    .collect();
  sorted.extend(rest);
  sorted
}

/// `a, b, c, +2 more`
pub fn short_list(list: &[String], n: usize) -> String {
  if list.len() > n {
    format!("{}, +{} more", list[..n].join(", "), list.len() - n)
  } else {
    list.join(", ")
  }
}

/// `0.5.8 (release)`
pub fn version_label(v: &VersionInfo) -> String {
  format!("{} ({})", v.version_number, v.channel.as_str())
}

/// `id` when listed, else the newest release, else the newest version (lists are newest first)
pub fn default_version<'a>(
  versions: &'a [VersionInfo],
  id: Option<&str>,
) -> Option<&'a VersionInfo> {
  use proton_content::model::Channel;
  versions
    .iter()
    .find(|v| Some(v.id.as_str()) == id)
    .or_else(|| versions.iter().find(|v| v.channel == Channel::Release))
    .or(versions.first())
}

/// Profile tile note, `Fabric 1.21 · modpack, edits may be replaced on update`
pub fn profile_note(profile: &Profile, compat: &Compat) -> String {
  let mut parts = vec![format!(
    "{} {}",
    loader_label(profile.loader),
    profile.version
  )];
  if profile.modpack.is_some() {
    parts.push("modpack, edits may be replaced on update".into());
  }
  parts.extend(compat.note.clone());
  parts.join(" · ")
}

/// `1 mod`, `2 mods`
pub fn plural(n: usize, word: &str) -> String {
  if n == 1 {
    format!("{n} {word}")
  } else {
    format!("{n} {word}s")
  }
}

pub fn content_kind(kind: router::Kind) -> ContentKind {
  match kind {
    router::Kind::Mod => ContentKind::Mod,
    router::Kind::Modpack => ContentKind::Modpack,
    router::Kind::ResourcePack => ContentKind::ResourcePack,
    router::Kind::Shader => ContentKind::Shader,
  }
}

pub fn route_kind(kind: ContentKind) -> router::Kind {
  match kind {
    ContentKind::Mod => router::Kind::Mod,
    ContentKind::Modpack => router::Kind::Modpack,
    ContentKind::ResourcePack => router::Kind::ResourcePack,
    ContentKind::Shader => router::Kind::Shader,
  }
}

pub fn content_source(source: router::Source) -> Source {
  match source {
    router::Source::Modrinth => Source::Modrinth,
    router::Source::CurseForge => Source::CurseForge,
  }
}

pub fn route_source(source: Source) -> router::Source {
  match source {
    Source::Modrinth => router::Source::Modrinth,
    Source::CurseForge => router::Source::CurseForge,
  }
}

#[cfg(test)]
pub(crate) mod tests {
  use proton_components::widgets::content::CardAction;
  use proton_content::model::Channel;
  use proton_profiles::config::ModpackRef;

  use super::*;

  pub fn profile(id: &str, loader: LoaderType, last_played: Option<&str>) -> Profile {
    serde_json::from_value(serde_json::json!({
      "id": id, "name": format!("P {id}"), "created_at": "2024-01-01T00:00:00Z",
      "last_played": last_played, "version": "1.21", "loader": loader,
      "loader_version": null, "downloaded": false, "use_local_game": false, "game": null,
      "use_local_jvm": false, "jvm": null
    }))
    .unwrap()
  }

  pub fn summary(id: &str) -> ProjectSummary {
    ProjectSummary {
      source: Source::Modrinth,
      id: id.into(),
      kind: ContentKind::Mod,
      name: id.into(),
      author: "a".into(),
      description: String::new(),
      icon_url: None,
      categories: vec![],
      loaders: vec!["fabric".into()],
      game_versions: vec!["1.21".into()],
      downloads: 0,
      updated_at: "2024-01-01T00:00:00Z".into(),
      client_side: None,
      server_side: None,
      url: String::new(),
    }
  }

  pub fn version(id: &str, channel: Channel) -> VersionInfo {
    VersionInfo {
      source: Source::Modrinth,
      project_id: "p".into(),
      id: id.into(),
      name: id.into(),
      version_number: format!("v{id}"),
      channel,
      game_versions: vec![],
      loaders: vec![],
      published_at: "2024-01-01T00:00:00Z".into(),
      files: vec![],
      dependencies: vec![],
    }
  }

  fn installed(project: &str) -> Installed {
    Installed {
      kind: ContentKind::Mod,
      file_name: format!("{project}.jar"),
      enabled: true,
      manual: false,
      from_pack: false,
      source: None,
      project_id: Some(project.into()),
      version_id: None,
      name: None,
      icon_url: None,
      version_number: None,
      dependency_of: vec![],
      order: None,
      active: None,
    }
  }

  fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
  }

  // content.test.ts
  #[test]
  fn loader_compat() {
    assert!(loader_compatible(LoaderType::Fabric, &s(&["fabric"])).ok);
    assert!(loader_compatible(LoaderType::NeoForge, &s(&["neoforge"])).ok);
    assert_eq!(
      loader_compatible(LoaderType::Quilt, &s(&["fabric"])),
      Compat {
        ok: true,
        note: Some("Fabric version, runs on Quilt".into())
      }
    );
    assert!(!loader_compatible(LoaderType::Forge, &s(&["fabric"])).ok);
    assert!(!loader_compatible(LoaderType::Fabric, &s(&["quilt"])).ok);
  }

  #[test]
  fn profile_compatibility() {
    let p = profile("x", LoaderType::Fabric, None);
    assert_eq!(
      profile_compat(&p, ContentKind::ResourcePack, &s(&["1.20"]), &[]),
      Compat::no("Not available for 1.21".into())
    );
    assert!(profile_compat(&p, ContentKind::ResourcePack, &[], &[]).ok);
    assert!(summary_compat(&p, &summary("a")).ok);
    let vanilla = profile("v", LoaderType::Vanilla, None);
    assert!(!summary_compat(&vanilla, &summary("a")).ok);
    let shader = (s(&["1.21"]), s(&["iris"]));
    assert!(profile_compat(&p, ContentKind::Shader, &shader.0, &shader.1).ok);
    let forge = profile("f", LoaderType::Forge, None);
    assert!(!profile_compat(&forge, ContentKind::Shader, &shader.0, &shader.1).ok);
    assert!(profile_compat(&p, ContentKind::Modpack, &s(&["1.8"]), &[]).ok);
  }

  #[test]
  fn picks_target() {
    let list = [
      profile("a", LoaderType::Fabric, Some("2024-01-01T00:00:00Z")),
      profile("b", LoaderType::Fabric, Some("2024-02-01T00:00:00Z")),
      profile("c", LoaderType::Fabric, None),
    ];
    assert_eq!(pick_target(&list, Some("c")).unwrap().id, "c");
    assert_eq!(pick_target(&list, Some("gone")).unwrap().id, "b");
    let single = [profile("z", LoaderType::Fabric, None)];
    assert_eq!(pick_target(&single, None).unwrap().id, "z");
    assert!(pick_target(&[], None).is_none());
  }

  #[test]
  fn builds_queries() {
    let filters = Filters {
      categories: s(&["tech"]),
      compatible: false,
      environment: Some(Environment::Client),
      game_version: Some("1.20".into()),
      loaders: s(&["forge"]),
      query: " sodium ".into(),
      sort: SearchSort::Downloads,
      sources: vec![Source::Modrinth],
    };
    let q = build_query(ContentKind::Mod, &filters, Some("x"), 40);
    assert_eq!(
      (
        q.query.as_str(),
        q.offset,
        q.limit,
        &q.loaders,
        q.game_version.as_deref()
      ),
      ("sodium", 40, 20, &s(&["forge"]), Some("1.20"))
    );
    assert_eq!(q.environment, Some(Environment::Client));
    assert_eq!(q.compatible_with, None);
    assert_eq!(
      (q.categories, q.sort),
      (s(&["tech"]), SearchSort::Downloads)
    );

    let compatible = Filters {
      compatible: true,
      ..filters.clone()
    };
    let q = build_query(ContentKind::Mod, &compatible, Some("x"), 0);
    assert_eq!(q.compatible_with.as_deref(), Some("x"));
    assert!(q.loaders.is_empty() && q.game_version.is_none());

    let q = build_query(ContentKind::ResourcePack, &filters, None, 0);
    assert!(q.loaders.is_empty() && q.environment.is_none());

    let q = build_query(ContentKind::Modpack, &compatible, Some("x"), 0);
    assert_eq!(q.compatible_with, None);
    assert_eq!(q.loaders, s(&["forge"]));
  }

  #[test]
  fn card_actions() {
    let installed: HashSet<String> = ["p".to_string()].into();
    assert_eq!(card_action("n", &installed, &[]), CardAction::Install);
    assert_eq!(card_action("p", &installed, &[]), CardAction::Installed);
    let update = UpdateInfo {
      profile: "x".into(),
      kind: ContentKind::Mod,
      file_name: "f".into(),
      source: Source::Modrinth,
      project_id: "p".into(),
      name: "P".into(),
      current: "1".into(),
      latest: version("2", Channel::Release),
    };
    assert_eq!(card_action("p", &installed, &[update]), CardAction::Update);
  }

  // discover.test.ts
  #[test]
  fn discover_helpers() {
    assert_eq!(tag_label("world-generation"), "World generation");
    let cat = |header: &str, name: &str| Category {
      name: name.into(),
      header: header.into(),
      kind: ContentKind::Shader,
    };
    let groups = category_groups(
      ContentKind::Shader,
      &[
        cat("performance impact", "low"),
        cat("categories", "cartoon"),
        cat("performance impact", "high"),
      ],
    );
    assert_eq!(
      groups,
      [
        CategoryGroup {
          header: "categories".into(),
          label: "Look".into(),
          names: s(&["cartoon"])
        },
        CategoryGroup {
          header: "performance impact".into(),
          label: "Performance cost".into(),
          names: s(&["low", "high"])
        },
      ]
    );
    assert_eq!(
      category_groups(ContentKind::Mod, &[cat("categories", "x")])[0].label,
      "Category"
    );

    assert_eq!(
      runs_on(Some("required"), Some("optional")),
      Some("Client and server")
    );
    assert_eq!(
      runs_on(Some("required"), Some("unsupported")),
      Some("Client")
    );
    assert_eq!(runs_on(None, None), None);

    assert!(shader_ready(&[
      installed("YL57xq9U"),
      installed("AANobbMI")
    ]));
    assert!(!shader_ready(&[installed("455508")]));

    assert_eq!(
      sort_versions(
        &s(&["1.20", "snap", "1.21"]),
        &s(&["1.21", "1.20.1", "1.20"])
      ),
      s(&["1.21", "1.20", "snap"])
    );
    assert_eq!(
      short_list(&s(&["a", "b", "c", "d", "e"]), 3),
      "a, b, c, +2 more"
    );
    assert_eq!(short_list(&s(&["a", "b"]), 3), "a, b");

    let mut p = summary("x");
    p.categories = s(&["optimization", "fabric"]);
    p.client_side = Some("required".into());
    assert_eq!(card_tags(&p), s(&["Optimization", "Fabric", "Client"]));
  }

  // install.test.ts
  #[test]
  fn install_helpers() {
    assert_eq!(version_label(&version("1", Channel::Beta)), "v1 (beta)");
    let list = [version("3", Channel::Beta), version("2", Channel::Release)];
    assert_eq!(default_version(&list, None).unwrap().id, "2");
    assert_eq!(default_version(&list, Some("3")).unwrap().id, "3");
    assert_eq!(default_version(&list, Some("missing")).unwrap().id, "2");
    assert_eq!(
      default_version(&[version("1", Channel::Alpha)], None)
        .unwrap()
        .id,
      "1"
    );
    assert!(default_version(&[], None).is_none());

    let mut p = profile("x", LoaderType::Fabric, None);
    assert_eq!(profile_note(&p, &Compat::ok()), "Fabric 1.21");
    p.modpack = Some(ModpackRef {
      source: Source::Modrinth,
      project_id: "p".into(),
      version_id: "v".into(),
    });
    assert_eq!(
      profile_note(&p, &Compat::no("Not here".into())),
      "Fabric 1.21 · modpack, edits may be replaced on update · Not here"
    );
  }
}
