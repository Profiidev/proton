use meta::{Action, Arch, Os, OsName, Rule};

pub mod commands;
mod download;
mod event;
mod launch;
mod meta;
pub mod store;

const JAVA_DIR: &str = "java";

const MC_DIR: &str = "minecraft";
const VERSION_DIR: &str = "versions";
const LIBRARY_DIR: &str = "libraries";
const NATIVE_DIR: &str = "bin";
const ASSETS_DIR: &str = "assets";
const ASSETS_INDEX_DIR: &str = "indices";

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
