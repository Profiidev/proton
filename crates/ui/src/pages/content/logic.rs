//! Pure content list logic, ported from `content.ts` and `content-list.ts`

use std::{collections::HashSet, path::Path};

use proton_content::{
  ContentError,
  model::{ContentKind, Installed, Source},
};

use crate::router::Kind;

/// Kinds shown in the profile content tab, in tab order
pub const LIST_KINDS: [ContentKind; 3] = [
  ContentKind::Mod,
  ContentKind::ResourcePack,
  ContentKind::Shader,
];

pub fn content_kind(kind: Kind) -> ContentKind {
  match kind {
    Kind::Mod | Kind::Modpack => ContentKind::Mod,
    Kind::ResourcePack => ContentKind::ResourcePack,
    Kind::Shader => ContentKind::Shader,
  }
}

pub fn route_kind(kind: ContentKind) -> Kind {
  match kind {
    ContentKind::Mod => Kind::Mod,
    ContentKind::Modpack => Kind::Modpack,
    ContentKind::ResourcePack => Kind::ResourcePack,
    ContentKind::Shader => Kind::Shader,
  }
}

/// Singular noun of each kind, pluralize with [`plural`]
pub fn word(kind: ContentKind) -> &'static str {
  match kind {
    ContentKind::ResourcePack => "resource pack",
    ContentKind::Shader => "shader",
    ContentKind::Mod | ContentKind::Modpack => "mod",
  }
}

pub fn plural(n: usize, word: &str) -> String {
  format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

pub fn source_label(source: Option<Source>) -> &'static str {
  match source {
    Some(Source::Modrinth) => "Modrinth",
    Some(Source::CurseForge) => "CurseForge",
    None => "Manual",
  }
}

pub fn extension(kind: ContentKind) -> &'static str {
  if kind == ContentKind::Mod {
    ".jar"
  } else {
    ".zip"
  }
}

/// Whether a dropped file can be added as `kind`
pub fn accepts(kind: ContentKind, path: &Path) -> bool {
  path
    .to_string_lossy()
    .to_lowercase()
    .ends_with(extension(kind))
}

pub fn display_name(row: &Installed) -> &str {
  row.name.as_deref().unwrap_or(&row.file_name)
}

/// Resource packs: enabled ones by priority, then disabled; others by name
pub fn sort_rows(kind: ContentKind, rows: &mut [Installed]) {
  let name = |r: &Installed| display_name(r).to_lowercase();
  rows.sort_by(|a, b| {
    if kind == ContentKind::ResourcePack {
      if a.enabled != b.enabled {
        return b.enabled.cmp(&a.enabled);
      }
      if a.enabled {
        let order = |r: &Installed| r.order.unwrap_or(usize::MAX);
        return order(a).cmp(&order(b));
      }
    }
    name(a).cmp(&name(b))
  });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowStatus {
  #[default]
  All,
  Enabled,
  Disabled,
  Updates,
}

impl RowStatus {
  pub const ALL: [(RowStatus, &'static str); 4] = [
    (RowStatus::All, "All status"),
    (RowStatus::Enabled, "Enabled"),
    (RowStatus::Disabled, "Disabled"),
    (RowStatus::Updates, "Updates"),
  ];
}

/// Rows matching the search text and status, `updates` holds file names
pub fn filter_rows<'a>(
  rows: &'a [Installed],
  text: &str,
  status: RowStatus,
  updates: &HashSet<String>,
) -> Vec<&'a Installed> {
  let needle = text.trim().to_lowercase();
  rows
    .iter()
    .filter(|row| {
      display_name(row).to_lowercase().contains(&needle)
        && match status {
          RowStatus::All => true,
          RowStatus::Enabled => row.enabled,
          RowStatus::Disabled => !row.enabled,
          RowStatus::Updates => updates.contains(&row.file_name),
        }
    })
    .collect()
}

/// Installed entries that need `entry`, so removing it may break them
pub fn blocking_dependents<'a>(entry: &Installed, all: &'a [Installed]) -> Vec<&'a Installed> {
  all
    .iter()
    .filter(|other| {
      other.file_name != entry.file_name
        && entry.dependency_of.iter().any(|dep| {
          Some(dep.as_str()) == other.project_id.as_deref() || *dep == other.file_name
        })
    })
    .collect()
}

/// Copy of `list` with the item at `from` moved to `to`
pub fn move_item<T: Clone>(list: &[T], from: usize, to: usize) -> Vec<T> {
  let mut next = list.to_vec();
  let item = next.remove(from);
  next.insert(to, item);
  next
}

/// Text of the remove confirmation, `None` removes right away
pub fn remove_warning(entries: &[Installed], all: &[Installed]) -> Option<String> {
  let removed: HashSet<&str> = entries.iter().map(|e| e.file_name.as_str()).collect();
  let mut blockers: Vec<&str> = vec![];
  for dep in entries.iter().flat_map(|e| blocking_dependents(e, all)) {
    let name = display_name(dep);
    if !removed.contains(dep.file_name.as_str()) && !blockers.contains(&name) {
      blockers.push(name);
    }
  }
  let manual: Vec<&str> = entries
    .iter()
    .filter(|e| e.manual)
    .map(|e| e.file_name.as_str())
    .collect();
  let mut parts = vec![];
  if !blockers.is_empty() {
    parts.push(format!(
      "Still needed by {}, which may stop working.",
      blockers.join(", ")
    ));
  }
  if !manual.is_empty() {
    let verb = if manual.len() == 1 { "was" } else { "were" };
    parts.push(format!(
      "{} {verb} added manually and cannot be re-downloaded.",
      manual.join(", ")
    ));
  }
  (!parts.is_empty()).then(|| parts.join(" "))
}

/// Toast text for a failed content command
pub fn error_message(error: &anyhow::Error) -> &'static str {
  match error.downcast_ref::<ContentError>() {
    Some(ContentError::Running) => "Stop the game first",
    _ => "Something went wrong, please try again",
  }
}

#[cfg(test)]
pub(crate) fn row(file_name: &str) -> Installed {
  Installed {
    kind: ContentKind::ResourcePack,
    file_name: file_name.into(),
    enabled: true,
    manual: false,
    from_pack: false,
    installed_at: None,
    source: None,
    project_id: None,
    version_id: None,
    sha1: String::new(),
    name: None,
    icon_url: None,
    version_number: None,
    dependency_of: vec![],
    order: None,
    active: None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn names(rows: &[Installed]) -> Vec<&str> {
    rows.iter().map(|r| r.file_name.as_str()).collect()
  }

  #[test]
  fn accepts_by_extension() {
    assert!(accepts(ContentKind::Mod, Path::new("/a/Sodium.JAR")));
    assert!(!accepts(ContentKind::Mod, Path::new("/a/pack.zip")));
    assert!(accepts(ContentKind::ResourcePack, Path::new("/a/pack.zip")));
    assert!(!accepts(ContentKind::Shader, Path::new("/a/mod.jar")));
  }

  #[test]
  fn sorts_packs_by_priority_then_disabled() {
    let mut rows = vec![
      Installed {
        enabled: false,
        ..row("off.zip")
      },
      Installed {
        order: Some(1),
        ..row("low.zip")
      },
      Installed {
        order: Some(0),
        ..row("high.zip")
      },
    ];
    sort_rows(ContentKind::ResourcePack, &mut rows);
    assert_eq!(names(&rows), ["high.zip", "low.zip", "off.zip"]);
  }

  #[test]
  fn sorts_other_kinds_by_name() {
    let mut rows = vec![
      Installed {
        enabled: false,
        ..row("b.jar")
      },
      Installed {
        name: Some("A".into()),
        ..row("x.jar")
      },
    ];
    sort_rows(ContentKind::Mod, &mut rows);
    assert_eq!(names(&rows), ["x.jar", "b.jar"]);
  }

  #[test]
  fn error_messages() {
    let msg = |e: ContentError| error_message(&anyhow::Error::new(e));
    assert_eq!(msg(ContentError::Running), "Stop the game first");
    assert_ne!(msg(ContentError::Network), "Stop the game first");
  }

  #[test]
  fn filters_by_text_and_status() {
    let rows = vec![
      Installed {
        name: Some("Sodium".into()),
        ..row("sodium.jar")
      },
      Installed {
        enabled: false,
        ..row("lithium.jar")
      },
    ];
    let none = HashSet::new();
    let found = |text, status, updates: &HashSet<String>| {
      filter_rows(&rows, text, status, updates)
        .into_iter()
        .map(|r| r.file_name.clone())
        .collect::<Vec<_>>()
    };
    assert_eq!(found(" sod ", RowStatus::All, &none), ["sodium.jar"]);
    assert_eq!(found("lith", RowStatus::All, &none), ["lithium.jar"]);
    assert_eq!(found("", RowStatus::Enabled, &none), ["sodium.jar"]);
    assert_eq!(found("", RowStatus::Disabled, &none), ["lithium.jar"]);
    let updates = HashSet::from(["lithium.jar".to_string()]);
    assert_eq!(found("", RowStatus::Updates, &updates), ["lithium.jar"]);
  }

  #[test]
  fn finds_blocking_dependents() {
    let api = Installed {
      dependency_of: vec!["sodium".into(), "gone".into()],
      ..row("api.jar")
    };
    let sodium = Installed {
      project_id: Some("sodium".into()),
      ..row("sodium.jar")
    };
    let all = vec![api.clone(), sodium, row("other.jar")];
    let found: Vec<_> = blocking_dependents(&api, &all)
      .into_iter()
      .map(|r| r.file_name.as_str())
      .collect();
    assert_eq!(found, ["sodium.jar"]);
  }

  #[test]
  fn moves_without_mutating() {
    let list = ["a", "b", "c"];
    assert_eq!(move_item(&list, 0, 2), ["b", "c", "a"]);
    assert_eq!(move_item(&list, 2, 0), ["c", "a", "b"]);
    assert_eq!(list, ["a", "b", "c"]);
  }

  #[test]
  fn remove_warning_names_dependents_and_manual_files() {
    let api = Installed {
      dependency_of: vec!["sodium".into()],
      ..row("api.jar")
    };
    let sodium = Installed {
      project_id: Some("sodium".into()),
      name: Some("Sodium".into()),
      ..row("sodium.jar")
    };
    let manual = Installed {
      manual: true,
      ..row("local.jar")
    };
    let all = vec![api.clone(), sodium.clone(), manual.clone()];
    assert_eq!(remove_warning(&[sodium.clone()], &all), None);
    // removing the dependent too needs no warning
    assert_eq!(remove_warning(&[api.clone(), sodium], &all), None);
    assert_eq!(
      remove_warning(&[api, manual], &all).unwrap(),
      "Still needed by Sodium, which may stop working. local.jar was added manually and cannot \
       be re-downloaded."
    );
  }
}
