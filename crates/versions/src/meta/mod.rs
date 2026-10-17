use serde::Deserialize;

pub mod java;
pub mod minecraft;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
  pub action: Action,
  pub features: Option<Features>,
  pub os: Option<Os>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Features {
  pub is_demo_user: Option<bool>,
  pub has_custom_resolution: Option<bool>,
  pub is_quick_play_singleplayer: Option<bool>,
  pub is_quick_play_multiplayer: Option<bool>,
  pub has_quick_plays_support: Option<bool>,
  pub is_quick_play_realms: Option<bool>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Os {
  pub name: Option<OsName>,
  pub arch: Option<Arch>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Arch {
  X86,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OsName {
  Windows,
  Osx,
  Linux,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
  Allow,
  Disallow,
}

impl Features {
  pub fn is_superset_of(&self, other: &Features) -> bool {
    (other.is_demo_user.is_none() || self.is_demo_user == other.is_demo_user)
      && (other.has_custom_resolution.is_none()
        || self.has_custom_resolution == other.has_custom_resolution)
      && (other.is_quick_play_singleplayer.is_none()
        || self.is_quick_play_singleplayer == other.is_quick_play_singleplayer)
      && (other.is_quick_play_multiplayer.is_none()
        || self.is_quick_play_multiplayer == other.is_quick_play_multiplayer)
      && (other.has_quick_plays_support.is_none()
        || self.has_quick_plays_support == other.has_quick_plays_support)
      && (other.is_quick_play_realms.is_none()
        || self.is_quick_play_realms == other.is_quick_play_realms)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn superset_ignores_unset_requirements() {
    let all = |v| Features {
      is_demo_user: Some(v),
      has_custom_resolution: Some(v),
      is_quick_play_singleplayer: Some(v),
      is_quick_play_multiplayer: Some(v),
      has_quick_plays_support: Some(v),
      is_quick_play_realms: Some(v),
    };
    let none = Features::default();
    assert!(none.is_superset_of(&none));
    assert!(all(true).is_superset_of(&none));
    assert!(all(false).is_superset_of(&none));
    assert!(all(true).is_superset_of(&all(true)));
    assert!(!all(true).is_superset_of(&all(false)));
    // a missing feature is not the same as false
    assert!(!none.is_superset_of(&all(false)));

    // every single field is checked
    let setters: [fn(&mut Features); 6] = [
      |f| f.is_demo_user = Some(true),
      |f| f.has_custom_resolution = Some(true),
      |f| f.is_quick_play_singleplayer = Some(true),
      |f| f.is_quick_play_multiplayer = Some(true),
      |f| f.has_quick_plays_support = Some(true),
      |f| f.is_quick_play_realms = Some(true),
    ];
    for set in setters {
      let mut required = Features::default();
      set(&mut required);
      assert!(all(true).is_superset_of(&required));
      assert!(!all(false).is_superset_of(&required));
      assert!(!none.is_superset_of(&required));
    }
  }

  #[test]
  fn rules_parse_like_mojang_writes_them() {
    let rule: Rule = serde_json::from_str(
      r#"{"action":"allow","features":{"is_demo_user":true},"os":{"name":"osx","arch":"x86"}}"#,
    )
    .unwrap();
    assert_eq!(rule.action, Action::Allow);
    assert_eq!(rule.features.unwrap().is_demo_user, Some(true));
    let os = rule.os.unwrap();
    assert_eq!((os.name, os.arch), (Some(OsName::Osx), Some(Arch::X86)));
    // an os nobody knows is an error, not a silent match
    assert!(serde_json::from_str::<Rule>(r#"{"action":"allow","os":{"name":"beos"}}"#).is_err());
    assert!(serde_json::from_str::<Rule>(r#"{"action":"maybe"}"#).is_err());
  }
}
