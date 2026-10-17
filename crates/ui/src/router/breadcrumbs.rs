use std::collections::HashMap;

use super::Kind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
  pub label: String,
  /// Old-style URL, resolve with [`super::Route::from_url`]
  pub href: String,
}

/// Display names the breadcrumbs look up by id
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Names {
  /// profile id → profile name
  pub profiles: HashMap<String, String>,
  /// instance id → its profile's name
  pub instances: HashMap<String, String>,
  /// project id → project title
  pub projects: HashMap<String, String>,
}

/// `quick_play` → "Quick Play", `resource-packs` → "Resource Packs"
fn title(part: &str) -> String {
  part
    .split(['_', '-'])
    .map(|word| {
      let mut chars = word.chars();
      chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
    })
    .collect::<Vec<String>>()
    .join(" ")
}

/// Breadcrumb trail for a URL (port of `nav/breadcrumbs.ts`): section, then
/// profile name (for `info` pages), then subpages. Links below an `info`
/// segment keep the query (`?id=...`) so they resolve to the same entity.
pub fn breadcrumbs(url: &str, names: &Names) -> Vec<Crumb> {
  let (pathname, query) = url.split_once('?').unwrap_or((url, ""));
  let search = if query.is_empty() {
    String::new()
  } else {
    format!("?{query}")
  };
  let param = |key: &str| {
    query
      .split('&')
      .filter_map(|pair| pair.split_once('='))
      .find(|(k, _)| *k == key)
      .map(|(_, v)| v)
  };
  let parts: Vec<&str> = pathname.split('/').filter(|p| !p.is_empty()).collect();
  let id = param("id");

  if parts.first() == Some(&"discover") && parts.get(1) == Some(&"project") {
    let mut crumbs = vec![Crumb {
      href: "/discover".into(),
      label: "Discover".into(),
    }];
    if let Some(kind) = param("kind").and_then(Kind::from_query) {
      crumbs.push(Crumb {
        href: format!("/discover/{}", kind.segment()),
        label: title(kind.segment()),
      });
    }
    crumbs.push(Crumb {
      href: format!("{pathname}{search}"),
      label: id
        .and_then(|id| names.projects.get(id))
        .cloned()
        .unwrap_or_else(|| "Project".into()),
    });
    return crumbs;
  }

  let is_info = parts.get(1) == Some(&"info");
  parts
    .iter()
    .enumerate()
    .map(|(i, part)| {
      let mut label = title(part);
      if *part == "info" && i == 1 {
        let lookup = |map: &HashMap<String, String>, fallback: &str| {
          id.and_then(|id| map.get(id))
            .cloned()
            .unwrap_or_else(|| fallback.into())
        };
        match parts[0] {
          "profiles" => label = lookup(&names.profiles, "Profile"),
          "instances" => label = lookup(&names.instances, "Instance"),
          _ => {}
        }
      }
      let path = format!("/{}", parts[..=i].join("/"));
      Crumb {
        href: if i >= 1 && is_info {
          path + &search
        } else {
          path
        },
        label,
      }
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn names() -> Names {
    Names {
      profiles: [("x".into(), "Survival".into())].into(),
      instances: [("i1".into(), "Modded".into())].into(),
      projects: [("abc".into(), "Faithful".into())].into(),
    }
  }

  fn crumb(href: &str, label: &str) -> Crumb {
    Crumb {
      href: href.into(),
      label: label.into(),
    }
  }

  fn labels(url: &str) -> Vec<String> {
    breadcrumbs(url, &names())
      .into_iter()
      .map(|c| c.label)
      .collect()
  }

  #[test]
  fn shows_profile_name_for_profile_subpages() {
    assert_eq!(
      breadcrumbs("/profiles/info/settings/java?id=x", &names()),
      [
        crumb("/profiles", "Profiles"),
        crumb("/profiles/info?id=x", "Survival"),
        crumb("/profiles/info/settings?id=x", "Settings"),
        crumb("/profiles/info/settings/java?id=x", "Java"),
      ]
    );
  }

  #[test]
  fn title_cases_underscored_segments() {
    assert_eq!(
      breadcrumbs("/profiles/info/quick_play?id=x", &names()).last(),
      Some(&crumb("/profiles/info/quick_play?id=x", "Quick Play"))
    );
  }

  #[test]
  fn shows_the_instances_profile_name() {
    assert_eq!(
      labels("/instances/info/logs?id=i1"),
      ["Instances", "Modded", "Logs"]
    );
  }

  #[test]
  fn falls_back_when_the_entity_is_unknown() {
    assert_eq!(labels("/profiles/info?id=y")[1], "Profile");
    assert_eq!(labels("/instances/info/logs?id=y")[1], "Instance");
  }

  #[test]
  fn handles_top_level_and_root_pages() {
    assert_eq!(
      breadcrumbs("/settings/java", &Names::default()),
      [
        crumb("/settings", "Settings"),
        crumb("/settings/java", "Java")
      ]
    );
    assert_eq!(breadcrumbs("/", &Names::default()), []);
  }

  #[test]
  fn title_cases_hyphenated_segments() {
    assert_eq!(
      breadcrumbs("/discover/resource-packs", &Names::default()),
      [
        crumb("/discover", "Discover"),
        crumb("/discover/resource-packs", "Resource Packs"),
      ]
    );
  }

  #[test]
  fn shows_kind_and_project_name_on_the_project_page() {
    let url = "/discover/project?kind=resourcepack&source=modrinth&id=abc";
    assert_eq!(
      breadcrumbs(url, &names()),
      [
        crumb("/discover", "Discover"),
        crumb("/discover/resource-packs", "Resource Packs"),
        crumb(url, "Faithful"),
      ]
    );
  }

  #[test]
  fn falls_back_while_the_project_name_loads() {
    assert_eq!(
      labels("/discover/project?kind=mod&id=x"),
      ["Discover", "Mods", "Project"]
    );
    assert_eq!(labels("/discover/project?id=x"), ["Discover", "Project"]);
  }
}
