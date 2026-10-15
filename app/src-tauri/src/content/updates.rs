use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Mutex,
  time::Duration,
};

use anyhow::Result;
use chrono::Utc;
use tauri::{AppHandle, Manager};

use super::{
  model::{Channel, Source, UpdateInfo, VersionInfo},
  provider::{ContentProvider, Providers, profile_loaders},
  store::{self, Entry},
};
use crate::{
  profiles::{config::Profile, store::ProfileStore},
  store::TauriAppStoreExt,
  utils::{
    log::ResultLogExt,
    updater::{UpdateType, update_data},
  },
  versions::store::is_busy,
};

const INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const LAST_CHECK_KEY: &str = "content_updates_checked";

/// Last update check result per profile id, and per profile a generation that `forget` bumps
#[derive(Default)]
pub struct UpdatesCache(
  pub Mutex<HashMap<String, Vec<UpdateInfo>>>,
  Mutex<HashMap<String, u64>>,
);

impl UpdatesCache {
  fn generations(&self) -> HashMap<String, u64> {
    self.1.lock().unwrap().clone()
  }

  /// Stores check results started at generations `before`, results of profiles that changed
  /// during the check are dropped. Returns the whole cache.
  fn store(
    &self,
    results: HashMap<String, Vec<UpdateInfo>>,
    before: &HashMap<String, u64>,
    clear: bool,
  ) -> HashMap<String, Vec<UpdateInfo>> {
    let mut cache = self.0.lock().unwrap();
    let now = self.1.lock().unwrap();
    if clear {
      // drops removed profiles
      cache.retain(|id, _| now.get(id) != before.get(id));
    }
    cache.extend(
      results
        .into_iter()
        .filter(|(id, _)| now.get(id) == before.get(id)),
    );
    cache.clone()
  }

  fn forget(&self, profile: &str, file_names: &[String]) {
    *self
      .1
      .lock()
      .unwrap()
      .entry(profile.to_string())
      .or_default() += 1;
    if let Some(updates) = self.0.lock().unwrap().get_mut(profile) {
      updates.retain(|u| !file_names.contains(&u.file_name));
    }
  }
}

/// Available updates for the identified, non-pack entries of a profile
pub async fn check<'p>(
  get: impl Fn(Source) -> &'p dyn ContentProvider,
  profile: &Profile,
  entries: &[Entry],
) -> Vec<UpdateInfo> {
  let mut groups: HashMap<_, Vec<&Entry>> = HashMap::new();
  for entry in entries {
    if let (false, false, false, Some(source), Some(_)) = (
      entry.manual,
      entry.from_pack,
      entry.pending,
      entry.source,
      &entry.project_id,
    ) {
      groups
        .entry((source, entry.kind, installed_release(entry)))
        .or_default()
        .push(entry);
    }
  }

  let mut updates = Vec::new();
  for ((source, kind, release_only), entries) in groups {
    let hashes: Vec<_> = entries.iter().map(|e| e.hashes()).collect();
    let loaders = profile_loaders(source, kind, profile.loader);
    // a newer beta must not hide the newest release
    let Ok(latest) = get(source)
      .latest_for(&hashes, &loaders, &profile.version, release_only)
      .await
      .log()
    else {
      continue;
    };
    for entry in entries {
      if let Some(version) = latest.get(&entry.sha1)
        && is_update(entry, version)
      {
        updates.push(UpdateInfo {
          profile: profile.id.clone(),
          kind,
          file_name: entry.file_name.clone(),
          source,
          project_id: version.project_id.clone(),
          name: entry.name.clone().unwrap_or(entry.file_name.clone()),
          current: entry.version_number.clone().unwrap_or_default(),
          latest: version.clone(),
        });
      }
    }
  }
  updates.sort_by(|a, b| a.file_name.cmp(&b.file_name));
  updates
}

/// Same project, different primary file. Pre-releases only replace pre-releases.
fn is_update(entry: &Entry, latest: &VersionInfo) -> bool {
  entry.project_id.as_deref() == Some(latest.project_id.as_str())
    && latest.primary_file().is_some_and(|f| f.sha1 != entry.sha1)
    && (latest.channel == Channel::Release || !installed_release(entry))
}

fn installed_release(entry: &Entry) -> bool {
  entry.channel.is_none_or(|c| c == Channel::Release)
}

/// Manifest entries whose file still exists
pub async fn entries(dir: &Path) -> Vec<Entry> {
  store::load(dir)
    .await
    .entries
    .into_iter()
    .filter(|e| store::disk_path(dir, e.kind, &e.file_name).is_ok_and(|p| p.exists()))
    .collect()
}

/// Checks one or all profiles, stores the result in the cache and returns the whole cache
pub async fn check_profiles(
  handle: &AppHandle,
  profile: Option<&str>,
) -> Result<HashMap<String, Vec<UpdateInfo>>> {
  let targets: Vec<(Profile, PathBuf)> = {
    let state = handle.state::<tokio::sync::Mutex<ProfileStore>>();
    let store = state.lock().await;
    let profiles = match profile {
      Some(id) => vec![store.profile(id).await?],
      None => store.list_profiles().await?,
    };
    profiles
      .into_iter()
      .filter_map(|p| Some((store.get_profile_path(&p.id).ok()?, p)))
      .map(|(dir, p)| (p, dir))
      .collect()
  };

  let cache = handle.state::<UpdatesCache>();
  let before = cache.generations();
  let providers = handle.state::<Providers>();
  let mut results = HashMap::new();
  for (profile, dir) in targets {
    let entries = entries(&dir).await;
    let updates = check(|s| providers.get(s), &profile, &entries).await;
    results.insert(profile.id, updates);
  }

  let all = cache.store(results, &before, profile.is_none());
  update_data(handle, UpdateType::ContentUpdates);
  Ok(all)
}

/// Drops cached updates of files that were just updated
pub fn forget(handle: &AppHandle, profile: &str, file_names: &[String]) {
  handle.state::<UpdatesCache>().forget(profile, file_names);
  update_data(handle, UpdateType::ContentUpdates);
}

/// Checks all profiles now and every 6 hours, waiting while a download or install runs
pub async fn run_loop(handle: AppHandle) {
  loop {
    if is_busy() {
      tokio::time::sleep(Duration::from_secs(60)).await;
      continue;
    }
    let _ = check_profiles(&handle, None).await.log();
    if let Ok(store) = handle.app_store() {
      let _ = store.set(LAST_CHECK_KEY, &Utc::now()).log();
    }
    tokio::time::sleep(INTERVAL).await;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::content::model::{ContentKind, FileInfo};

  fn entry(channel: Channel) -> Entry {
    Entry {
      kind: ContentKind::Mod,
      file_name: "a.jar".into(),
      sha1: "old".into(),
      fingerprint: 0,
      manual: false,
      from_pack: false,
      installed_at: None,
      source: Some(Source::Modrinth),
      project_id: Some("a".into()),
      version_id: Some("1".into()),
      name: Some("A".into()),
      icon_url: None,
      version_number: Some("1".into()),
      channel: Some(channel),
      dependency_of: vec![],
      pending: false,
    }
  }

  fn latest(project: &str, sha1: &str, channel: Channel) -> VersionInfo {
    VersionInfo {
      source: Source::Modrinth,
      project_id: project.into(),
      id: "2".into(),
      name: "2".into(),
      version_number: "2".into(),
      channel,
      game_versions: vec![],
      loaders: vec![],
      published_at: String::new(),
      files: vec![FileInfo {
        file_name: "a2.jar".into(),
        url: Some("u".into()),
        sha1: sha1.into(),
        sha512: None,
        size: 1,
        primary: true,
      }],
      dependencies: vec![],
    }
  }

  #[test]
  fn selects_updates() {
    let release = entry(Channel::Release);
    assert!(is_update(&release, &latest("a", "new", Channel::Release)));
    // equal sha1 is the installed file
    assert!(!is_update(&release, &latest("a", "old", Channel::Release)));
    assert!(!is_update(&release, &latest("b", "new", Channel::Release)));
    // pre-releases only for pre-releases
    assert!(!is_update(&release, &latest("a", "new", Channel::Beta)));
    assert!(is_update(
      &entry(Channel::Beta),
      &latest("a", "new", Channel::Alpha)
    ));
    assert!(is_update(
      &entry(Channel::Alpha),
      &latest("a", "new", Channel::Release)
    ));
  }

  /// Newest version is a beta, the newest release is only returned with `release_only`
  struct Stub;

  #[async_trait::async_trait]
  impl ContentProvider for Stub {
    async fn search(
      &self,
      _: &crate::content::model::SearchQuery,
    ) -> crate::content::Result<crate::content::provider::SearchPage> {
      unimplemented!()
    }
    async fn project(
      &self,
      _: &str,
    ) -> crate::content::Result<crate::content::model::ProjectDetail> {
      unimplemented!()
    }
    async fn versions(
      &self,
      _: &str,
      _: &[String],
      _: Option<&str>,
    ) -> crate::content::Result<Vec<VersionInfo>> {
      unimplemented!()
    }
    async fn summaries(
      &self,
      _: &[String],
    ) -> crate::content::Result<Vec<crate::content::model::ProjectSummary>> {
      unimplemented!()
    }
    async fn by_hashes(
      &self,
      _: &[crate::content::model::FileHashes],
    ) -> crate::content::Result<HashMap<String, VersionInfo>> {
      unimplemented!()
    }
    async fn latest_for(
      &self,
      files: &[crate::content::model::FileHashes],
      _: &[String],
      _: &str,
      release_only: bool,
    ) -> crate::content::Result<HashMap<String, VersionInfo>> {
      let channel = if release_only {
        Channel::Release
      } else {
        Channel::Beta
      };
      Ok(
        files
          .iter()
          .map(|f| (f.sha1.clone(), latest("a", "new", channel)))
          .collect(),
      )
    }
  }

  #[test]
  fn results_of_forgotten_profiles_are_dropped() {
    let cache = UpdatesCache::default();
    let update = |file: &str| UpdateInfo {
      profile: "p".into(),
      kind: ContentKind::Mod,
      file_name: file.into(),
      source: Source::Modrinth,
      project_id: "a".into(),
      name: "A".into(),
      current: "1".into(),
      latest: latest("a", "new", Channel::Release),
    };
    let results = |id: &str| HashMap::from([(id.to_string(), vec![update("a.jar")])]);

    let before = cache.generations();
    cache.store(results("p"), &before, true);
    // an update finished while the next check ran
    let before = cache.generations();
    cache.forget("p", &["a.jar".into()]);
    let all = cache.store(results("p"), &before, true);
    assert_eq!(all["p"].len(), 0);
    let all = cache.store(results("q"), &before, true);
    assert_eq!(all.len(), 2);

    let before = cache.generations();
    assert_eq!(cache.store(results("p"), &before, false)["p"].len(), 1);
  }

  #[tokio::test]
  async fn release_installs_still_get_the_newest_release() {
    let profile: Profile = serde_json::from_value(serde_json::json!({
      "id": "p", "name": "P", "created_at": "2026-01-01T00:00:00Z", "last_played": null,
      "version": "1.21.1", "loader": "Fabric", "loader_version": null, "downloaded": true,
      "use_local_game": false, "game": null, "use_local_jvm": false, "jvm": null
    }))
    .unwrap();
    let mut beta = entry(Channel::Beta);
    beta.file_name = "b.jar".into();
    beta.sha1 = "old-beta".into();
    let updates = check(
      |_| &Stub as &dyn ContentProvider,
      &profile,
      &[entry(Channel::Release), beta],
    )
    .await;
    let channels: Vec<_> = updates
      .iter()
      .map(|u| (u.file_name.as_str(), u.latest.channel))
      .collect();
    assert_eq!(
      channels,
      [("a.jar", Channel::Release), ("b.jar", Channel::Beta)]
    );
  }
}
