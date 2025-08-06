use meta::{Action, Arch, Os, OsName, Rule};

use crate::versions::meta::Features;

pub mod commands;
mod download;
mod event;
pub mod launch;
pub mod loader;
mod maven;
mod meta;
pub mod store;

const JAVA_DIR: &str = "java";

const MC_DIR: &str = "minecraft";
const VERSION_DIR: &str = "versions";
const LIBRARY_DIR: &str = "lib";
const ASSETS_DIR: &str = "assets";
const ASSETS_INDEX_DIR: &str = "indexes";
pub const QUICK_PLAY: &str = "quick_play.json";

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
