//! Versions service: manifests, Java/Minecraft/loader downloads, game launch and the downloads
//! panel model (`cx.versions()`).

use meta::{Action, Arch, Os, OsName, Rule};

use crate::meta::Features;

pub use service::*;

pub mod download;
pub mod downloads;
pub mod event;
pub mod launch;
pub mod loader;
pub mod maven;
pub mod meta;
pub mod paths;
pub mod store;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;

mod listener;
mod service;

#[cfg(target_os = "linux")]
const OS_NAME: Option<OsName> = Some(OsName::Linux);
#[cfg(target_os = "windows")]
const OS_NAME: Option<OsName> = Some(OsName::Windows);
#[cfg(target_os = "macos")]
const OS_NAME: Option<OsName> = Some(OsName::Osx);
#[cfg(target_arch = "x86")]
const ARCH: Option<Arch> = Some(Arch::X86);
#[cfg(not(target_arch = "x86"))]
const ARCH: Option<Arch> = None;
#[cfg(target_family = "unix")]
const SEPARATOR: &str = ":";
#[cfg(target_family = "windows")]
const SEPARATOR: &str = ";";
#[cfg(all(not(debug_assertions), target_os = "windows"))]
const DETACHED_PROCESS: u32 = 0x00000008;

fn check_rule(rule: &Rule) -> bool {
  let Rule { action, os, .. } = rule;

  match (os, action) {
    (
      Some(Os {
        name: OS_NAME,
        arch: ARCH,
      }),
      Action::Allow,
    ) => true,
    (
      Some(Os {
        name: OS_NAME,
        arch: ARCH,
      }),
      Action::Disallow,
    ) => false,
    (None, Action::Allow) => true,
    (None, Action::Disallow) => false,
    (_, Action::Disallow) => true,
    (_, Action::Allow) => false,
  }
}

fn check_feature(rule: &Rule, features: &Features) -> bool {
  let Rule {
    action,
    features: required_features,
    ..
  } = rule;

  match (required_features, action) {
    (Some(required_features), Action::Allow) => features.is_superset_of(required_features),
    (Some(required_features), Action::Disallow) => !features.is_superset_of(required_features),
    (None, Action::Allow) => true,
    (None, Action::Disallow) => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn rule(json: &str) -> Rule {
    serde_json::from_str(json).unwrap()
  }

  #[cfg(all(target_os = "linux", not(target_arch = "x86")))]
  #[test]
  fn os_rules_match_only_this_os() {
    let cases = [
      (r#"{"action":"allow"}"#, true),
      (r#"{"action":"disallow"}"#, false),
      (r#"{"action":"allow","os":{"name":"linux"}}"#, true),
      (r#"{"action":"disallow","os":{"name":"linux"}}"#, false),
      (r#"{"action":"allow","os":{"name":"osx"}}"#, false),
      // disallowing another os allows this one
      (r#"{"action":"disallow","os":{"name":"osx"}}"#, true),
      (r#"{"action":"allow","os":{"name":"windows"}}"#, false),
      // x86 only libs are skipped on 64 bit
      (r#"{"action":"allow","os":{"arch":"x86"}}"#, false),
      (
        r#"{"action":"allow","os":{"name":"linux","arch":"x86"}}"#,
        false,
      ),
      // an os object without a name never matches a named os
      (r#"{"action":"allow","os":{}}"#, false),
      // unknown fields like the old osx version regex are ignored
      (
        r#"{"action":"disallow","os":{"name":"osx","version":"^10\\.5\\.\\d$"}}"#,
        true,
      ),
    ];
    for (json, expected) in cases {
      assert_eq!(check_rule(&rule(json)), expected, "{json}");
    }
  }

  #[test]
  fn feature_rules_need_every_required_feature() {
    let features = Features {
      has_custom_resolution: Some(true),
      has_quick_plays_support: Some(true),
      ..Default::default()
    };
    let cases = [
      (r#"{"action":"allow"}"#, true),
      (r#"{"action":"disallow"}"#, false),
      (
        r#"{"action":"allow","features":{"has_custom_resolution":true}}"#,
        true,
      ),
      (
        r#"{"action":"allow","features":{"has_custom_resolution":true,"has_quick_plays_support":true}}"#,
        true,
      ),
      (
        r#"{"action":"allow","features":{"is_demo_user":true}}"#,
        false,
      ),
      (
        r#"{"action":"allow","features":{"has_custom_resolution":false}}"#,
        false,
      ),
      (
        r#"{"action":"disallow","features":{"is_demo_user":true}}"#,
        true,
      ),
      (
        r#"{"action":"disallow","features":{"has_custom_resolution":true}}"#,
        false,
      ),
      // an empty feature set is a subset of everything
      (r#"{"action":"allow","features":{}}"#, true),
    ];
    for (json, expected) in cases {
      assert_eq!(check_feature(&rule(json), &features), expected, "{json}");
    }
  }
}
