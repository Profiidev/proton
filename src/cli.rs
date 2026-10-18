//! Command line: lists profiles, quick play entries and accounts straight from disk, `launch`
//! hands the profile to the running app (or this process becomes the app).

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use proton_account::store::AccountStore;
use proton_profiles::{
  config::{Profile, QuickPlayInfo},
  store::read_profiles,
};
use proton_ui::actions::LaunchRequest;
use proton_utils::{dir, rt};

/// Minecraft launcher. Without a command it opens the launcher. Profiles, quick play entries
/// and accounts match by id or name.
#[derive(Parser)]
#[command(version)]
struct Cli {
  #[command(subcommand)]
  command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
  /// List profiles
  Profiles,
  /// List the worlds, servers and realms of a profile
  QuickPlay { profile: String },
  /// List accounts, * marks the active one
  Accounts,
  /// Start a profile in the launcher
  Launch {
    profile: String,
    /// Join this world, server or realm
    #[arg(long)]
    quick_play: Option<String>,
    /// Play with this account instead of the active one
    #[arg(long)]
    account: Option<String>,
  },
}

pub enum Start {
  App,
  Launch(LaunchRequest),
  /// A list command already printed its output
  Done,
}

/// Parses the process args, invalid ones and `--help` print and exit
pub fn run() -> Result<Start> {
  match Cli::parse().command {
    None => return Ok(Start::App),
    Some(Command::Profiles) => {
      for p in profiles()? {
        let loader = match &p.loader_version {
          Some(v) => format!("{:?} {v}", p.loader),
          None => format!("{:?}", p.loader),
        };
        println!("{}\t{}\t{}\t{loader}", p.id, p.name, p.version);
      }
    }
    Some(Command::QuickPlay { profile }) => {
      for q in find_profile(&profiles()?, &profile)?.quick_play {
        println!("{}\t{}\t{:?}", q.id, q.name, q.r#type);
      }
    }
    Some(Command::Accounts) => {
      let store = AccountStore::new()?;
      let active = store.active_id();
      for (id, name) in accounts(&store) {
        let mark = if active.as_ref() == Some(&id) {
          "*"
        } else {
          " "
        };
        println!("{mark} {id}\t{}", name.unwrap_or("(login expired)".into()));
      }
    }
    Some(Command::Launch {
      profile,
      quick_play,
      account,
    }) => {
      let profile = find_profile(&profiles()?, &profile)?;
      let quick_play = quick_play
        .map(|q| find_quick_play(&profile.quick_play, &q))
        .transpose()?;
      let account = account
        .map(|a| find_account(&accounts(&AccountStore::new()?), &a))
        .transpose()?;
      return Ok(Start::Launch(LaunchRequest {
        profile: profile.id,
        quick_play,
        account,
      }));
    }
  }
  Ok(Start::Done)
}

fn profiles() -> Result<Vec<Profile>> {
  let data_dir = dir::data_dir()?;
  Ok(rt::block_on(read_profiles(&data_dir)))
}

/// Account id and name (`None` when the login expired), sorted by name
fn accounts(store: &AccountStore) -> Vec<(String, Option<String>)> {
  let mut list: Vec<_> = store
    .list_profiles()
    .into_iter()
    .map(|(id, info)| (id, info.map(|i| i.name)))
    .collect();
  list.sort_by(|a, b| a.1.cmp(&b.1));
  list
}

/// The one item whose id or name is `query`; an exact id wins over names
fn find<'a, T>(items: &'a [T], query: &str, key: impl Fn(&T) -> (&str, &str)) -> Option<&'a T> {
  if let Some(item) = items.iter().find(|i| key(i).0 == query) {
    return Some(item);
  }
  let mut named = items
    .iter()
    .filter(|i| key(i).1.eq_ignore_ascii_case(query));
  named.next().filter(|_| named.next().is_none())
}

fn find_profile(profiles: &[Profile], query: &str) -> Result<Profile> {
  match find(profiles, query, |p| (&p.id, &p.name)) {
    Some(p) => Ok(p.clone()),
    None => bail!("No single profile named {query:?}, see `proton profiles`"),
  }
}

fn find_quick_play(entries: &[QuickPlayInfo], query: &str) -> Result<QuickPlayInfo> {
  match find(entries, query, |q| (&q.id, &q.name)) {
    Some(q) => Ok(q.clone()),
    None => bail!("No single quick play entry named {query:?}, see `proton quick-play`"),
  }
}

fn find_account(accounts: &[(String, Option<String>)], query: &str) -> Result<String> {
  match find(accounts, query, |(id, name)| {
    (id, name.as_deref().unwrap_or_default())
  }) {
    Some((id, _)) => Ok(id.clone()),
    None => bail!("No single account named {query:?}, see `proton accounts`"),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn find_prefers_ids_and_needs_a_unique_name() {
    let items = [
      ("a", "Survival"),
      ("b", "survival"),
      ("c", "Creative"),
      ("d", "c"),
    ];
    fn key<'a>(i: &'a (&str, &str)) -> (&'a str, &'a str) {
      *i
    }
    assert_eq!(find(&items, "a", key), Some(&items[0]));
    // an id beats another item's name
    assert_eq!(find(&items, "c", key), Some(&items[2]));
    assert_eq!(find(&items, "creative", key), Some(&items[2]));
    assert_eq!(find(&items, "Survival", key), None);
    assert_eq!(find(&items, "missing", key), None);
  }

  #[test]
  fn cli_is_valid() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
  }
}
