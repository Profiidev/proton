use serde::{Deserialize, Serialize};

/// Every page of the app. [`Route::to_url`] / [`Route::from_url`] speak the
/// old SvelteKit URLs, so a URL saved by the Tauri app still restores.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Route {
  Home,
  Profiles,
  Profile {
    id: String,
    tab: ProfileTab,
  },
  Instances,
  InstanceLogs {
    id: String,
  },
  Discover(Kind),
  Project {
    id: String,
    kind: Kind,
    source: Source,
  },
  Settings(SettingsTab),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProfileTab {
  QuickPlay(QuickPlayTab),
  /// Never [`Kind::Modpack`]
  Content(Kind),
  Instances,
  Logs,
  Settings(ProfileSettingsTab),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuickPlayTab {
  Singleplayer,
  Multiplayer,
  Realms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProfileSettingsTab {
  General,
  Minecraft,
  Java,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettingsTab {
  Accounts,
  Skins,
  Capes,
  Minecraft,
  Java,
  About,
}

/// Content kind as it appears in URLs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
  Mod,
  Modpack,
  ResourcePack,
  Shader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
  Modrinth,
  CurseForge,
}

/// Generates `segment()` / `from_segment()` for an enum of unit variants
macro_rules! segments {
  ($ty:ty, $get:ident, $parse:ident, { $($variant:ident => $seg:literal),* $(,)? }) => {
    impl $ty {
      pub const ALL: &'static [Self] = &[$(Self::$variant),*];

      pub fn $get(self) -> &'static str {
        match self { $(Self::$variant => $seg),* }
      }

      pub fn $parse(segment: &str) -> Option<Self> {
        match segment { $($seg => Some(Self::$variant),)* _ => None }
      }
    }
  };
}

segments!(QuickPlayTab, segment, from_segment, {
  Singleplayer => "singleplayer", Multiplayer => "multiplayer", Realms => "realms",
});
segments!(ProfileSettingsTab, segment, from_segment, {
  General => "general", Minecraft => "minecraft", Java => "java",
});
segments!(SettingsTab, segment, from_segment, {
  Accounts => "accounts", Skins => "skins", Capes => "capes",
  Minecraft => "minecraft", Java => "java", About => "about",
});
// `KIND_ROUTE` of the old app
segments!(Kind, segment, from_segment, {
  Mod => "mods", Modpack => "modpacks", ResourcePack => "resource-packs", Shader => "shaders",
});
segments!(Source, query, from_query, { Modrinth => "modrinth", CurseForge => "curseforge" });

impl Kind {
  /// `ContentKind` as serialized by the backend (`?kind=` of the project page)
  pub fn query(self) -> &'static str {
    match self {
      Kind::Mod => "mod",
      Kind::Modpack => "modpack",
      Kind::ResourcePack => "resourcepack",
      Kind::Shader => "shader",
    }
  }

  pub fn from_query(query: &str) -> Option<Self> {
    Self::ALL.iter().copied().find(|k| k.query() == query)
  }
}

impl ProfileTab {
  fn path(self) -> String {
    match self {
      ProfileTab::QuickPlay(t) => format!("quick_play/{}", t.segment()),
      ProfileTab::Content(k) => format!("content/{}", k.segment()),
      ProfileTab::Instances => "instances".into(),
      ProfileTab::Logs => "logs".into(),
      ProfileTab::Settings(t) => format!("settings/{}", t.segment()),
    }
  }

  /// Applies the old `+page.ts` redirects (`content` → `content/mods`, ...)
  fn parse(segments: &[&str]) -> Option<Self> {
    Some(match segments {
      [] | ["quick_play"] => ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
      ["quick_play", t] => ProfileTab::QuickPlay(QuickPlayTab::from_segment(t)?),
      ["content"] => ProfileTab::Content(Kind::Mod),
      ["content", k] => match Kind::from_segment(k)? {
        Kind::Modpack => return None,
        k => ProfileTab::Content(k),
      },
      ["instances"] => ProfileTab::Instances,
      ["logs"] => ProfileTab::Logs,
      ["settings"] => ProfileTab::Settings(ProfileSettingsTab::General),
      ["settings", t] => ProfileTab::Settings(ProfileSettingsTab::from_segment(t)?),
      _ => return None,
    })
  }
}

impl Route {
  /// Path and query, e.g. `/profiles/info/logs?id=x`
  pub fn to_url(&self) -> String {
    match self {
      Route::Home => "/home".into(),
      Route::Profiles => "/profiles".into(),
      Route::Profile { id, tab } => format!("/profiles/info/{}?id={}", tab.path(), encode(id)),
      Route::Instances => "/instances".into(),
      Route::InstanceLogs { id } => format!("/instances/info/logs?id={}", encode(id)),
      Route::Discover(kind) => format!("/discover/{}", kind.segment()),
      Route::Project { id, kind, source } => format!(
        "/discover/project?kind={}&source={}&id={}",
        kind.query(),
        source.query(),
        encode(id)
      ),
      Route::Settings(tab) => format!("/settings/{}", tab.segment()),
    }
  }

  /// Parses a URL saved by the old app (`tauri://localhost/home`,
  /// `http://localhost:1420/profiles/info?id=x`) or a bare path. Redirects
  /// of the old routes apply; `None` for anything unknown.
  pub fn from_url(url: &str) -> Option<Self> {
    let url = url.split('#').next().unwrap_or_default();
    // drop `scheme://host`
    let url = match url.split_once("://") {
      Some((_, rest)) => rest.find('/').map_or("", |i| &rest[i..]),
      None => url,
    };
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    let param = |key: &str| {
      query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| decode(v))
        .filter(|v| !v.is_empty() && v != "null" && v != "undefined")
    };
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    Some(match segments.as_slice() {
      [] | ["home"] => Route::Home,
      ["profiles"] => Route::Profiles,
      ["profiles", "info", rest @ ..] => {
        let tab = ProfileTab::parse(rest)?;
        match param("id") {
          Some(id) => Route::Profile { id, tab },
          None => Route::Profiles,
        }
      }
      ["instances"] => Route::Instances,
      ["instances", "info"] | ["instances", "info", "logs"] => match param("id") {
        Some(id) => Route::InstanceLogs { id },
        None => Route::Instances,
      },
      ["discover"] => Route::Discover(Kind::Mod),
      ["discover", "project"] => match param("id") {
        // the project page defaulted a missing kind / source like this
        Some(id) => Route::Project {
          id,
          kind: param("kind")
            .and_then(|k| Kind::from_query(&k))
            .unwrap_or(Kind::Mod),
          source: param("source")
            .and_then(|s| Source::from_query(&s))
            .unwrap_or(Source::Modrinth),
        },
        None => Route::Discover(Kind::Mod),
      },
      ["discover", kind] => Route::Discover(Kind::from_segment(kind)?),
      ["settings"] => Route::Settings(SettingsTab::Accounts),
      ["settings", tab] => Route::Settings(SettingsTab::from_segment(tab)?),
      _ => return None,
    })
  }

  /// The profile id this route belongs to
  pub fn profile_id(&self) -> Option<&str> {
    match self {
      Route::Profile { id, .. } => Some(id),
      _ => None,
    }
  }
}

/// `encodeURIComponent`
fn encode(value: &str) -> String {
  let mut out = String::with_capacity(value.len());
  for b in value.bytes() {
    if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
      out.push(b as char);
    } else {
      out.push_str(&format!("%{b:02X}"));
    }
  }
  out
}

/// Percent-decoding; malformed escapes stay as they are
fn decode(value: &str) -> String {
  let bytes = value.as_bytes();
  let mut out = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    let hex = bytes
      .get(i + 1..i + 3)
      .and_then(|h| std::str::from_utf8(h).ok())
      .and_then(|h| u8::from_str_radix(h, 16).ok());
    match (bytes[i], hex) {
      (b'%', Some(b)) => {
        out.push(b);
        i += 3;
      }
      (b'+', _) => {
        out.push(b' ');
        i += 1;
      }
      (b, _) => {
        out.push(b);
        i += 1;
      }
    }
  }
  String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn all_routes() -> Vec<Route> {
    let id = || "8f1c2d3e-aaaa".to_string();
    let mut routes = vec![Route::Home, Route::Profiles, Route::Instances];
    let mut tabs = vec![ProfileTab::Instances, ProfileTab::Logs];
    tabs.extend(QuickPlayTab::ALL.iter().map(|t| ProfileTab::QuickPlay(*t)));
    tabs.extend(
      [Kind::Mod, Kind::ResourcePack, Kind::Shader]
        .map(ProfileTab::Content)
        .iter(),
    );
    tabs.extend(
      ProfileSettingsTab::ALL
        .iter()
        .map(|t| ProfileTab::Settings(*t)),
    );
    routes.extend(tabs.into_iter().map(|tab| Route::Profile { id: id(), tab }));
    routes.push(Route::InstanceLogs { id: id() });
    routes.extend(Kind::ALL.iter().map(|k| Route::Discover(*k)));
    for kind in Kind::ALL {
      for source in Source::ALL {
        routes.push(Route::Project {
          id: "AANobbMI".into(),
          kind: *kind,
          source: *source,
        });
      }
    }
    routes.extend(SettingsTab::ALL.iter().map(|t| Route::Settings(*t)));
    routes
  }

  #[test]
  fn every_route_round_trips() {
    let routes = all_routes();
    assert_eq!(routes.len(), 3 + 11 + 1 + 4 + 8 + 6);
    for route in routes {
      let url = route.to_url();
      assert_eq!(Route::from_url(&url), Some(route.clone()), "{url}");
      let json = serde_json::to_string(&route).unwrap();
      assert_eq!(serde_json::from_str::<Route>(&json).unwrap(), route);
    }
  }

  #[test]
  fn urls_match_the_old_app() {
    let id = || "x".to_string();
    let cases = [
      (Route::Home, "/home"),
      (
        Route::Profile {
          id: id(),
          tab: ProfileTab::QuickPlay(QuickPlayTab::Singleplayer),
        },
        "/profiles/info/quick_play/singleplayer?id=x",
      ),
      (
        Route::Profile {
          id: id(),
          tab: ProfileTab::Content(Kind::ResourcePack),
        },
        "/profiles/info/content/resource-packs?id=x",
      ),
      (
        Route::Profile {
          id: id(),
          tab: ProfileTab::Settings(ProfileSettingsTab::Java),
        },
        "/profiles/info/settings/java?id=x",
      ),
      (
        Route::InstanceLogs { id: id() },
        "/instances/info/logs?id=x",
      ),
      (
        Route::Discover(Kind::ResourcePack),
        "/discover/resource-packs",
      ),
      (
        Route::Project {
          id: "a b".into(),
          kind: Kind::ResourcePack,
          source: Source::CurseForge,
        },
        "/discover/project?kind=resourcepack&source=curseforge&id=a%20b",
      ),
      (Route::Settings(SettingsTab::About), "/settings/about"),
    ];
    for (route, url) in cases {
      assert_eq!(route.to_url(), url);
    }
  }

  #[test]
  fn saved_urls_of_the_tauri_app_restore() {
    let profile = |tab| Route::Profile {
      id: "x".into(),
      tab,
    };
    let cases = [
      ("tauri://localhost/home", Route::Home),
      ("http://localhost:1420/", Route::Home),
      ("http://tauri.localhost", Route::Home),
      ("/", Route::Home),
      ("", Route::Home),
      (
        "tauri://localhost/profiles/info?id=x",
        profile(ProfileTab::QuickPlay(QuickPlayTab::Singleplayer)),
      ),
      (
        "/profiles/info/quick_play/realms?id=x#top",
        profile(ProfileTab::QuickPlay(QuickPlayTab::Realms)),
      ),
      (
        "/profiles/info/content?id=x",
        profile(ProfileTab::Content(Kind::Mod)),
      ),
      (
        "/profiles/info/settings?id=x",
        profile(ProfileTab::Settings(ProfileSettingsTab::General)),
      ),
      ("/profiles/info/logs", Route::Profiles),
      ("/profiles/info/logs?id=null", Route::Profiles),
      (
        "/instances/info?id=i",
        Route::InstanceLogs { id: "i".into() },
      ),
      ("/instances/info/logs", Route::Instances),
      ("/discover", Route::Discover(Kind::Mod)),
      (
        "/discover/project?id=P",
        Route::Project {
          id: "P".into(),
          kind: Kind::Mod,
          source: Source::Modrinth,
        },
      ),
      ("/settings", Route::Settings(SettingsTab::Accounts)),
    ];
    for (url, route) in cases {
      assert_eq!(Route::from_url(url), Some(route), "{url}");
    }
  }

  #[test]
  fn unknown_urls_are_rejected() {
    for url in [
      "/nope",
      "/settings/nope",
      "/discover/nope",
      "/profiles/info/content/modpacks?id=x",
      "/profiles/info/quick_play/nope?id=x",
      "/home/extra",
    ] {
      assert_eq!(Route::from_url(url), None, "{url}");
    }
  }

  #[test]
  fn percent_coding() {
    assert_eq!(encode("a b/ü"), "a%20b%2F%C3%BC");
    assert_eq!(decode("a%20b%2F%C3%BC"), "a b/ü");
    assert_eq!(decode("100%"), "100%");
  }
}
