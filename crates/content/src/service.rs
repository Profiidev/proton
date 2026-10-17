//! Service methods. Read-only lookups return a future that runs on
//! the tokio runtime, mutations return a `Task` that refreshes the profile's installed entity.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::Result;
use gpui_kit::{App, AppContext, Entity, Task};
use proton_config::{
  SettingsExt,
  offline::OfflineResultExt,
  settings::{JvmSettings, mem_fits},
};
use proton_profiles::config::{ModpackRef, Profile};
use proton_utils::{dir, entity::WriteChangedExt, file::write_file, log::ErrorLogExt, rt};
use proton_versions::{
  event::DownloadCheckStatus,
  store::{CancelGuard, register_cancel},
};

use tokio::sync::Notify;

use crate::{
  Content, ContentError, ContentExt, InstalledContent,
  install::{Progress, adopt_file, check_plan, execute_install, find_manual, plan_install},
  model::{
    Category, ContentKind, InstallPlan, ManualFile, PackPlan, PackSource, PickRef, ProjectDetail,
    SearchQuery, SearchResult, Source, UpdateInfo, VersionInfo,
  },
  modpack,
  profiles::{self, Target},
  provider::profile_loaders,
  store,
  updates::{self, ContentHandle},
};

/// Kinds with a folder in the profile, the ones `installed` scans
const SCANNED: [ContentKind; 3] = [
  ContentKind::Mod,
  ContentKind::ResourcePack,
  ContentKind::Shader,
];

/// `options.txt` and `iris.properties` are rewritten by the game while it runs
fn not_running(running: bool) -> Result<()> {
  if running {
    Err(ContentError::Running)?;
  }
  Ok(())
}

/// A row in the downloads panel, its id is also the cancel id. Dropped without `finish` (an
/// early `?`) it is marked failed.
struct Job {
  id: usize,
  cancel: Arc<Notify>,
  progress: Progress,
  handle: ContentHandle,
  _guard: CancelGuard,
  finished: bool,
}

impl Job {
  fn start(handle: &ContentHandle, title: String) -> Self {
    let downloads = handle.downloads.clone();
    let id = downloads.start(title);
    let guard = register_cancel(id);
    Self {
      id,
      cancel: guard.notify.clone(),
      progress: Arc::new(move |s| downloads.status(id, s)),
      handle: handle.clone(),
      _guard: guard,
      finished: false,
    }
  }

  /// Ok(false) (canceled) is finished as ok, the row shows canceled since a cancel was requested
  fn finish<T>(mut self, res: Result<T>) -> Result<T> {
    self.finished = true;
    self.handle.downloads.finish(self.id, res.is_ok());
    res
  }
}

impl Drop for Job {
  fn drop(&mut self) {
    if !self.finished {
      self.handle.downloads.finish(self.id, false);
    }
  }
}

/// Runs `fut` on the tokio runtime
async fn tokio<T: Send + 'static>(
  fut: impl Future<Output = Result<T>> + Send + 'static,
) -> Result<T> {
  rt::spawn(fut).await?
}

impl Content {
  pub const SOURCES: [Source; 2] = [Source::Modrinth, Source::CurseForge];

  /// Fetches the categories of `kind` into `categories`
  pub fn load_categories(&self, kind: ContentKind, cx: &mut App) -> Task<Result<Vec<Category>>> {
    let handle = self.handle.clone();
    let entity = self.categories.clone();
    cx.spawn(async move |cx| {
      let list = tokio(async move {
        Ok(
          handle
            .providers
            .modrinth()
            .categories(kind)
            .await
            .check_online_state(&handle.offline)
            .await?,
        )
      })
      .await?;
      entity.update(cx, |map, cx| {
        if map.get(&kind) != Some(&list) {
          map.insert(kind, list.clone());
          cx.notify();
        }
      });
      Ok(list)
    })
  }

  /// Page-local, nothing is stored
  pub fn search(
    &self,
    mut query: SearchQuery,
  ) -> impl Future<Output = Result<SearchResult>> + use<> {
    let handle = self.handle.clone();
    tokio(async move {
      if let Some(profile) = &query.compatible_with {
        let profile = profiles::resolve(&handle.profiles, profile).await?.profile;
        // CurseForge only applies mod loader names, so the Modrinth names work for both
        query.loaders = profile_loaders(Source::Modrinth, query.kind, profile.loader);
        query.game_version = Some(profile.version);
      }
      Ok(
        handle
          .providers
          .search_all(&query)
          .await
          .check_online_state(&handle.offline)
          .await?,
      )
    })
  }

  pub fn project(
    &self,
    source: Source,
    id: String,
  ) -> impl Future<Output = Result<ProjectDetail>> + use<> {
    let handle = self.handle.clone();
    tokio(async move {
      Ok(
        handle
          .providers
          .get(source)
          .project(&id)
          .await
          .check_online_state(&handle.offline)
          .await?,
      )
    })
  }

  /// Versions of a project, filtered to what fits `profile` when given
  pub fn versions(
    &self,
    source: Source,
    id: String,
    profile: Option<String>,
  ) -> impl Future<Output = Result<Vec<VersionInfo>>> + use<> {
    let handle = self.handle.clone();
    tokio(async move {
      let provider = handle.providers.get(source);
      let (loaders, game) = match profile {
        Some(profile) => {
          let profile = profiles::resolve(&handle.profiles, &profile).await?.profile;
          // the loader filter depends on the kind, the project is usually cached already
          let kind = provider
            .project(&id)
            .await
            .check_online_state(&handle.offline)
            .await?
            .summary
            .kind;
          (
            profile_loaders(source, kind, profile.loader),
            Some(profile.version),
          )
        }
        None => (Vec::new(), None),
      };
      Ok(
        provider
          .versions(&id, &loaders, game.as_deref())
          .await
          .check_online_state(&handle.offline)
          .await?,
      )
    })
  }

  /// The installed content of a profile, created and scanned on first use
  pub fn installed(&self, profile: &str, cx: &mut App) -> Entity<InstalledContent> {
    if let Some(entity) = self.installed.read(cx).get(profile) {
      return entity.clone();
    }
    let entity = cx.new(|_| InstalledContent::default());
    self.installed.update(cx, |map, _| {
      map.insert(profile.to_string(), entity.clone());
    });
    self.refresh_installed(profile, cx).detach();
    entity
  }

  /// Rescans the profile, notifies only when something changed. No-op for profiles whose
  /// `installed` entity was never requested.
  // ponytail: concurrent refreshes may finish out of order, add a generation if that shows up
  pub fn refresh_installed(&self, profile: &str, cx: &mut App) -> Task<Result<()>> {
    let Some(entity) = self.installed.read(cx).get(profile).cloned() else {
      return Task::ready(Ok(()));
    };
    let handle = self.handle.clone();
    let profile = profile.to_string();
    cx.spawn(async move |cx| {
      let by_kind = tokio(async move {
        let dir = profiles::resolve(&handle.profiles, &profile).await?.dir;
        let mut by_kind = HashMap::new();
        for kind in SCANNED {
          by_kind.insert(kind, store::scan(&handle.providers, &dir, kind).await?);
        }
        Ok(by_kind)
      })
      .await
      .log_err();
      let by_kind = match by_kind {
        Ok(by_kind) => by_kind,
        Err(e) => {
          // ends the page's spinner, keeps what was listed before
          entity.update(cx, |c, cx| {
            if !c.loaded {
              c.loaded = true;
              cx.notify();
            }
          });
          return Err(e);
        }
      };
      entity.write_changed(
        cx,
        InstalledContent {
          loaded: true,
          by_kind,
        },
      );
      Ok(())
    })
  }

  /// Runs `job` with the profile's directory on tokio, then refreshes its installed entity (also
  /// on error, the job may have changed files before failing)
  fn mutate<T: Send + 'static, F>(
    &self,
    profile: &str,
    cx: &mut App,
    job: impl FnOnce(Target) -> F + Send + 'static,
  ) -> Task<Result<T>>
  where
    F: Future<Output = Result<T>> + Send,
  {
    let id = profile.to_string();
    let profiles = self.handle.profiles.clone();
    let res = tokio(async move { job(profiles::resolve(&profiles, &id).await?).await });
    let profile = profile.to_string();
    cx.spawn(async move |cx| {
      let res = res.await.log_err();
      let refresh = cx.update(|cx| cx.content().clone().refresh_installed(&profile, cx));
      let _ = refresh.await;
      res
    })
  }

  /// Disabling resource packs and shaders edits files the game rewrites, so they need a stopped
  /// profile
  pub fn set_enabled(
    &self,
    profile: &str,
    kind: ContentKind,
    file_name: String,
    enabled: bool,
    cx: &mut App,
  ) -> Task<Result<()>> {
    self.mutate(profile, cx, move |Target { dir, running, .. }| async move {
      if kind != ContentKind::Mod {
        not_running(running)?;
      }
      store::set_enabled(&dir, kind, &file_name, enabled).await
    })
  }

  pub fn set_order(&self, profile: &str, files: Vec<String>, cx: &mut App) -> Task<Result<()>> {
    self.mutate(profile, cx, move |Target { dir, running, .. }| async move {
      not_running(running)?;
      store::set_order(&dir, &files).await
    })
  }

  pub fn set_active_shader(
    &self,
    profile: &str,
    file_name: Option<String>,
    cx: &mut App,
  ) -> Task<Result<()>> {
    self.mutate(profile, cx, move |Target { dir, running, .. }| async move {
      not_running(running)?;
      store::set_active_shader(&dir, file_name.as_deref()).await
    })
  }

  pub fn remove(
    &self,
    profile: &str,
    kind: ContentKind,
    file_names: Vec<String>,
    cx: &mut App,
  ) -> Task<Result<()>> {
    let handle = self.handle.clone();
    self.mutate(
      profile,
      cx,
      move |Target {
              profile: info, dir, ..
            }| async move {
        store::remove(&dir, kind, &file_names).await?;
        handle.forget(&info.id, &file_names);
        Ok(())
      },
    )
  }

  /// A dropped file, its kind is detected from the content
  pub fn add_file(&self, profile: &str, path: PathBuf, cx: &mut App) -> Task<Result<()>> {
    self.mutate(profile, cx, move |Target { dir, .. }| async move {
      store::add_file(&dir, &path).await?;
      Ok(())
    })
  }

  /// A manual download found in the download folders
  pub fn adopt_file(
    &self,
    profile: &str,
    file: ManualFile,
    path: PathBuf,
    cx: &mut App,
  ) -> Task<Result<()>> {
    self.mutate(profile, cx, move |Target { dir, .. }| async move {
      adopt_file(&dir, &file, &path).await?;
      Ok(())
    })
  }

  /// Looks for restricted files in the configured download folders, by file id
  pub fn find_manual(
    &self,
    files: Vec<ManualFile>,
    cx: &App,
  ) -> impl Future<Output = HashMap<String, PathBuf>> + use<> {
    let dirs = cx.settings().get(cx).download_dirs();
    async move {
      rt::spawn(find_manual(dirs, files))
        .await
        .unwrap_or_default()
    }
  }

  pub fn plan_install(
    &self,
    profile: String,
    picks: Vec<PickRef>,
  ) -> impl Future<Output = Result<InstallPlan>> + use<> {
    let handle = self.handle.clone();
    tokio(async move {
      let Target { profile, dir, .. } = profiles::resolve(&handle.profiles, &profile).await?;
      let entries = updates::entries(&dir).await;
      let providers = &handle.providers;

      plan_install(
        |s| providers.get(s),
        &profile,
        &entries,
        &picks,
        &HashMap::new(),
      )
      .await
      .check_online_state(&handle.offline)
      .await
    })
  }

  /// Ok(false) when canceled. Shows up in the downloads panel as `title`, canceled from there.
  pub fn install(&self, plan: InstallPlan, title: String, cx: &mut App) -> Task<Result<bool>> {
    let handle = self.handle.clone();
    let profile = plan.profile.clone();
    self.mutate(&profile, cx, move |Target { dir, .. }| async move {
      let job = Job::start(&handle, title);
      let res = async {
        check_plan(&plan)?;
        execute_install(
          &handle.providers,
          &dir,
          &plan,
          job.id,
          &job.cancel,
          job.progress.clone(),
        )
        .await
        .check_online_state(&handle.offline)
        .await
      }
      .await;
      job.finish(res)
    })
  }

  /// Checks one or all profiles, `updates` and `pack_updates` follow through the listener
  pub fn check_updates(
    &self,
    profile: Option<String>,
  ) -> impl Future<Output = Result<HashMap<String, Vec<UpdateInfo>>>> + use<> {
    let handle = self.handle.clone();
    tokio(async move { handle.check_profiles(profile.as_deref()).await })
  }

  /// Updates the given files, or all with an update
  pub fn update(
    &self,
    profile: &str,
    file_names: Option<Vec<String>>,
    title: String,
    cx: &mut App,
  ) -> Task<Result<()>> {
    let handle = self.handle.clone();
    self.mutate(
      profile,
      cx,
      move |Target {
              profile: info, dir, ..
            }| async move {
        // registered for the whole job so the update loop waits
        let job = Job::start(&handle, title);
        let res = async {
          (job.progress)(DownloadCheckStatus::ContentResolve);
          let providers = &handle.providers;
          let entries = updates::entries(&dir).await;
          let available: Vec<UpdateInfo> = updates::check(|s| providers.get(s), &info, &entries)
            .await
            .into_iter()
            .filter(|u| file_names.as_ref().is_none_or(|f| f.contains(&u.file_name)))
            .collect();

          let picks: Vec<PickRef> = available
            .iter()
            .map(|u| PickRef {
              source: u.source,
              project_id: u.project_id.clone(),
              version_id: Some(u.latest.id.clone()),
            })
            .collect();
          let replaces: HashMap<String, String> = available
            .iter()
            .map(|u| (u.project_id.clone(), u.file_name.clone()))
            .collect();
          let plan = plan_install(|s| providers.get(s), &info, &entries, &picks, &replaces)
            .await
            .check_online_state(&handle.offline)
            .await?;
          check_plan(&plan)?;

          if execute_install(
            providers,
            &dir,
            &plan,
            job.id,
            &job.cancel,
            job.progress.clone(),
          )
          .await
          .check_online_state(&handle.offline)
          .await?
          {
            let updated: Vec<String> = available.into_iter().map(|u| u.file_name).collect();
            handle.forget(&info.id, &updated);
          }
          Ok(())
        }
        .await;
        job.finish(res)
      },
    )
  }

  pub fn modpack_plan(&self, source: PackSource) -> impl Future<Output = Result<PackPlan>> + use<> {
    let handle = self.handle.clone();
    tokio(async move {
      let cache = dir::cache_dir()?;

      modpack::plan(&handle.providers, &cache, source)
        .await
        .check_online_state(&handle.offline)
        .await
    })
  }

  /// Creates a profile from the plan, installs the pack into it and returns its id, empty when
  /// canceled. The profile is deleted again on error or cancel.
  pub fn modpack_install(
    &self,
    plan: PackPlan,
    name: String,
    manual: HashMap<String, PathBuf>,
    title: String,
    cx: &App,
  ) -> impl Future<Output = Result<String>> + use<> {
    let handle = self.handle.clone();
    let jvm = cx.settings().get(cx).minecraft.jvm_settings.clone();
    tokio(async move {
      let job = Job::start(&handle, title);
      (job.progress)(DownloadCheckStatus::ContentResolve);
      let profiles = &handle.profiles;
      let res = async {
        let icon = modpack::icon(&handle.providers, &plan).await;
        let id = profiles
          .store
          .lock()
          .await
          .create_profile(
            name,
            icon.as_deref(),
            plan.minecraft.clone(),
            plan.loader,
            plan.loader_version.clone(),
          )
          .await?;

        // every error from here on deletes the profile again
        let installed = async {
          let store = profiles.store.lock().await;
          let mut info = store.profile(&id).await?;
          info.modpack = pack_ref(&plan.source);
          // only when the system has room for it, otherwise the global setting stays in use
          if let Some(mb) = plan.recommended_memory_mb
            && mem_fits(mb)
          {
            info.use_local_jvm = true;
            info.jvm = Some(JvmSettings {
              mem_max: mb as usize,
              ..jvm
            });
          }
          info.update(store.data_dir()).await?;
          let dir = store.get_profile_path(&id)?;
          drop(store);
          let _ = profiles.refresh().await.log_err();
          let done = modpack::install_files(
            &handle.providers,
            &dir,
            &plan,
            job.id,
            &job.cancel,
            job.progress.clone(),
          )
          .await?;
          // restricted files picked up from the download folders
          for file in plan.manual.iter().filter(|_| done) {
            let path = manual
              .get(&file.file_id)
              .ok_or(ContentError::NeedsManualDownload)?;
            adopt_file(&dir, file, path).await?;
          }
          anyhow::Ok(done)
        }
        .await;
        // ponytail: the cached archive stays, other installs of the same version may still read it
        if let Ok(true) = installed {
          return Ok(id);
        }
        let _ = profiles
          .store
          .lock()
          .await
          .remove_profile(&id)
          .await
          .log_err();
        let _ = profiles.refresh().await.log_err();
        installed.check_online_state(&handle.offline).await?;
        Ok(String::new())
      }
      .await;
      job.finish(res)
    })
  }

  /// Switches a modpack profile to the plan's version. Pack files are replaced, content the
  /// user added stays. Nothing is deleted when the download fails or is canceled. Ok(false) when
  /// canceled.
  pub fn modpack_update(
    &self,
    profile: &str,
    plan: PackPlan,
    manual: HashMap<String, PathBuf>,
    title: String,
    cx: &mut App,
  ) -> Task<Result<bool>> {
    let handle = self.handle.clone();
    self.mutate(profile, cx, move |target| async move {
      let Target {
        profile: mut info,
        dir,
        running,
      } = target;
      let job = Job::start(&handle, title);
      (job.progress)(DownloadCheckStatus::ContentResolve);
      let res = async {
        not_running(running)?;
        // restricted files are checked before anything is replaced
        let manual: Vec<_> = plan
          .manual
          .iter()
          .map(|file| {
            Ok((
              file,
              manual
                .get(&file.file_id)
                .ok_or(ContentError::NeedsManualDownload)?,
            ))
          })
          .collect::<std::result::Result<_, ContentError>>()?;
        let res = async {
          let done = modpack::install_files(
            &handle.providers,
            &dir,
            &plan,
            job.id,
            &job.cancel,
            job.progress.clone(),
          )
          .await?;
          for (file, path) in manual.iter().filter(|_| done) {
            adopt_file(&dir, file, path).await?;
          }
          anyhow::Ok(done)
        }
        .await;
        if !res.check_online_state(&handle.offline).await? {
          return Ok(false);
        }

        if info.version != plan.minecraft || info.loader_version != plan.loader_version {
          info.downloaded = false;
        }
        info.version = plan.minecraft;
        info.loader = plan.loader;
        info.loader_version = plan.loader_version;
        info.modpack = pack_ref(&plan.source);
        write_profile(&dir, &info).await?;
        let _ = handle.profiles.refresh().await.log_err();
        handle.forget_pack(&info.id);
        Ok(true)
      }
      .await;
      job.finish(res)
    })
  }

  /// Writes the profile as `.mrpack` to `path`, the file is removed when canceled
  pub fn export_mrpack(
    &self,
    profile: String,
    path: PathBuf,
    include: Vec<String>,
    title: String,
  ) -> impl Future<Output = Result<()>> + use<> {
    let handle = self.handle.clone();
    tokio(async move {
      let job = Job::start(&handle, title);
      let res = async {
        let Target {
          profile: info, dir, ..
        } = profiles::resolve(&handle.profiles, &profile).await?;
        tokio::select! {
          res = modpack::export_mrpack(&handle.providers, &dir, &info, &path, include, job.progress.clone()) => {
            res.check_online_state(&handle.offline).await?;
          }
          _ = job.cancel.notified() => {
            let _ = tokio::fs::remove_file(&path).await;
          }
        }
        Ok(())
      }
      .await;
      job.finish(res)
    })
  }

  /// Copies a stopped profile under a new name and returns the copy's id
  pub fn duplicate_profile(
    &self,
    profile: String,
    name: String,
  ) -> impl Future<Output = Result<String>> + use<> {
    let profiles = self.handle.profiles.clone();
    tokio(async move {
      let Target {
        profile: info,
        dir: src,
        running,
      } = profiles::resolve(&profiles, &profile).await?;
      // the game writes to the folder while it runs
      not_running(running)?;
      let copy = modpack::duplicate(&info, name);
      let data_dir = profiles.store.lock().await.data_dir().clone();
      let dst = data_dir.join(copy.relative_to_data());
      modpack::copy_profile(src, dst.clone()).await?;
      let res = async {
        write_profile(&dst, &copy).await?;
        profiles.store.lock().await.insert_profile(&copy)
      }
      .await;
      if res.is_err() {
        let _ = tokio::fs::remove_dir_all(&dst).await;
      }
      res?;
      let _ = profiles.refresh().await.log_err();
      Ok(copy.id)
    })
  }

  /// Checks all profiles now and every 6 hours on the tokio runtime
  pub fn start_updates_loop(&self) {
    rt::spawn(self.handle.clone().run_loop());
  }
}

async fn write_profile(dir: &Path, profile: &Profile) -> Result<()> {
  write_file(&dir.join("profile.json"), profile).await
}

/// Remembers remote pack versions so the profile can switch versions later
fn pack_ref(source: &PackSource) -> Option<ModpackRef> {
  match source {
    PackSource::Remote {
      source,
      project_id,
      version_id: Some(version_id),
    } => Some(ModpackRef {
      source: *source,
      project_id: project_id.clone(),
      version_id: version_id.clone(),
    }),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{self as gpui, TestAppContext};
  use proton_profiles::ProfilesExt;
  use proton_utils::{file::bytes_hash, test_http::FakeServer};
  use proton_versions::loader::LoaderType;
  use serde_json::{Value, json};
  use sha2::{Digest, Sha512};

  use super::*;
  use crate::{
    model::SearchSort,
    modpack::tests::write_zip,
    tests::{add_mod, counted, init_test},
  };

  const CDN: &str = "https://cdn.modrinth.com/data";

  /// A fake server, then the services and a fresh profile. Every test starts the server, scans
  /// of unknown files would reach the real providers otherwise.
  async fn setup(cx: &mut TestAppContext, loader: LoaderType) -> (FakeServer, String, PathBuf) {
    cx.executor().allow_parking();
    let server = FakeServer::start();
    init_test(cx);
    let id = create(cx, "P", loader).await;
    let dir = profile_dir(cx, &id).await;
    (server, id, dir)
  }

  async fn create(cx: &mut TestAppContext, name: &str, loader: LoaderType) -> String {
    let task = cx.update(|cx| {
      cx.profiles()
        // a set loader version skips the loader list lookup
        .create(
          name.into(),
          None,
          "1.21.1".into(),
          loader,
          Some("0.16.0".into()),
        )
    });
    task.await.unwrap()
  }

  fn profiles(cx: &mut TestAppContext) -> proton_profiles::ProfilesHandle {
    cx.update(|cx| cx.profiles().handle.clone())
  }

  async fn profile_dir(cx: &mut TestAppContext, id: &str) -> PathBuf {
    let (profiles, id) = (profiles(cx), id.to_string());
    rt::spawn(async move { profiles.store.lock().await.get_profile_path(&id).unwrap() })
      .await
      .unwrap()
  }

  async fn profile(cx: &mut TestAppContext, id: &str) -> Profile {
    let (profiles, id) = (profiles(cx), id.to_string());
    rt::spawn(async move { profiles.store.lock().await.profile(&id).await.unwrap() })
      .await
      .unwrap()
  }

  async fn profile_ids(cx: &mut TestAppContext) -> Vec<String> {
    let profiles = profiles(cx);
    rt::spawn(async move {
      let list = profiles.store.lock().await.list_profiles().await.unwrap();
      let mut ids: Vec<_> = list.into_iter().map(|p| p.id).collect();
      ids.sort();
      ids
    })
    .await
    .unwrap()
  }

  /// Marks the profile running with a sleeping `sh` as its game
  async fn start_game(cx: &mut TestAppContext, id: &str) {
    let profile = profile(cx, id).await;
    let profiles = profiles(cx);
    rt::spawn(async move {
      let child = tokio::process::Command::new("sh")
        .args(["-c", "sleep 30"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
      let store = profiles.store.lock().await;
      store
        .add_instance(child, &profile, "acc".into())
        .await
        .unwrap();
    })
    .await
    .unwrap()
  }

  async fn loaded(cx: &mut TestAppContext, id: &str) -> Entity<InstalledContent> {
    let installed = cx.update(|cx| cx.content().clone().installed(id, cx));
    // the first scan runs on tokio
    while !installed.read_with(cx, |i, _| i.loaded) {
      cx.run_until_parked();
      std::thread::yield_now();
    }
    installed
  }

  fn files_of(installed: &Entity<InstalledContent>, cx: &mut TestAppContext) -> Vec<String> {
    installed.read_with(cx, |i, _| {
      i.get(ContentKind::Mod)
        .iter()
        .map(|m| m.file_name.clone())
        .collect()
    })
  }

  fn err_of<T: std::fmt::Debug>(res: Result<T>) -> String {
    res.unwrap_err().to_string()
  }

  fn project_json(id: &str, kind: &str) -> Value {
    json!({
      "id": id, "slug": id, "project_type": kind, "team": "t", "organization": null,
      "title": format!("{id} name"), "description": "d", "body": "", "icon_url": null,
      "categories": [], "additional_categories": [], "loaders": [], "game_versions": [],
      "downloads": 0, "updated": "u", "client_side": "required", "server_side": "optional",
      "gallery": [], "source_url": null, "issues_url": null, "wiki_url": null, "discord_url": null
    })
  }

  fn version_json(project: &str, id: &str, file: &str, data: &[u8]) -> Value {
    json!({
      "id": id, "project_id": project, "name": id, "version_number": id, "version_type": "release",
      "game_versions": ["1.21.1"], "loaders": ["fabric"], "date_published": format!("2026-01-{id:0>2}"),
      "files": [{"url": format!("{CDN}/{file}"), "filename": file, "primary": true,
        "size": data.len(), "hashes": {"sha1": bytes_hash(data), "sha512": null}}],
      "dependencies": []
    })
  }

  /// Routes the projects lookup, the version list of `project` and its file downloads
  fn serve_project(
    server: &FakeServer,
    project: &str,
    kind: &str,
    versions: &[(&str, &str, &[u8])],
  ) {
    server.route(
      "GET",
      "/api.modrinth.com/v2/projects",
      200,
      json!([project_json(project, kind)]).to_string(),
    );
    let list: Vec<_> = versions
      .iter()
      .map(|(id, file, data)| version_json(project, id, file, data))
      .collect();
    server.route(
      "GET",
      &format!("/api.modrinth.com/v2/project/{project}/version"),
      200,
      Value::from(list).to_string(),
    );
    for (_, file, data) in versions {
      server.route(
        "GET",
        &format!("/cdn.modrinth.com/data/{file}"),
        200,
        data.to_vec(),
      );
    }
  }

  fn pick(project: &str, version: Option<&str>) -> PickRef {
    PickRef {
      source: Source::Modrinth,
      project_id: project.into(),
      version_id: version.map(Into::into),
    }
  }

  #[test]
  fn pack_ref_keeps_only_resolved_remote_versions() {
    let remote = |version_id: Option<&str>| PackSource::Remote {
      source: Source::CurseForge,
      project_id: "p".into(),
      version_id: version_id.map(Into::into),
    };
    let r = pack_ref(&remote(Some("v"))).unwrap();
    assert_eq!(
      (r.source, r.project_id.as_str(), r.version_id.as_str()),
      (Source::CurseForge, "p", "v")
    );
    assert!(pack_ref(&remote(None)).is_none());
    assert!(pack_ref(&PackSource::Path { path: "x".into() }).is_none());
  }

  #[gpui::test]
  async fn load_categories_notifies_only_on_change(cx: &mut TestAppContext) {
    let (server, _, _) = setup(cx, LoaderType::Fabric).await;
    server.route(
      "GET",
      "/api.modrinth.com/v2/tag/category",
      200,
      json!([{"name": "magic", "project_type": "mod", "header": "categories"}]).to_string(),
    );
    let categories = cx.update(|cx| cx.content().categories.clone());
    let count = counted(cx, &categories);
    for _ in 0..2 {
      let task = cx.update(|cx| cx.content().clone().load_categories(ContentKind::Mod, cx));
      assert_eq!(task.await.unwrap()[0].name, "magic");
      cx.run_until_parked();
    }
    assert_eq!(count.get(), 1);
    let stored = categories.read_with(cx, |c, _| c[&ContentKind::Mod].len());
    assert_eq!(stored, 1);
  }

  fn search_query(compatible_with: Option<String>) -> SearchQuery {
    SearchQuery {
      kind: ContentKind::Mod,
      query: String::new(),
      sources: vec![Source::Modrinth],
      loaders: vec!["forge".into()],
      game_version: Some("1.12.2".into()),
      categories: vec![],
      environment: None,
      sort: SearchSort::Relevance,
      offset: 0,
      limit: 10,
      compatible_with,
    }
  }

  #[gpui::test]
  async fn search_for_a_profile_uses_its_loader_and_version(cx: &mut TestAppContext) {
    let (server, id, _) = setup(cx, LoaderType::Quilt).await;
    server.route(
      "GET",
      "/api.modrinth.com/v2/search",
      200,
      r#"{"hits": [], "total_hits": 0}"#,
    );
    let search = cx.update(|cx| cx.content().search(search_query(Some(id))));
    assert_eq!(search.await.unwrap().total, 0);
    let call = &server.calls()[0];
    let facets = reqwest::Url::parse(&format!("http://x{}", call.path))
      .unwrap()
      .query_pairs()
      .find(|(k, _)| k == "facets")
      .unwrap()
      .1
      .into_owned();
    // the profile replaces what the query asked for
    assert_eq!(
      facets,
      r#"[["project_type:mod"],["categories:quilt","categories:fabric"],["versions:1.21.1"]]"#
    );

    let search = cx.update(|cx| cx.content().search(search_query(Some("gone".into()))));
    assert!(search.await.is_err());
    assert_eq!(server.calls().len(), 1);
  }

  #[gpui::test]
  async fn versions_for_a_profile_filter_by_the_project_kind(cx: &mut TestAppContext) {
    let (server, id, _) = setup(cx, LoaderType::Fabric).await;
    server.route(
      "GET",
      "/api.modrinth.com/v2/project/s",
      200,
      project_json("s", "shader").to_string(),
    );
    server.route("GET", "/api.modrinth.com/v2/team/t/members", 200, "[]");
    serve_project(&server, "s", "shader", &[("1", "s.zip", b"s")]);

    let detail = cx.update(|cx| cx.content().project(Source::Modrinth, "s".into()));
    assert_eq!(detail.await.unwrap().summary.kind, ContentKind::Shader);
    let all = cx.update(|cx| cx.content().versions(Source::Modrinth, "s".into(), None));
    assert_eq!(all.await.unwrap().len(), 1);
    let fitting = cx.update(|cx| {
      cx.content()
        .versions(Source::Modrinth, "s".into(), Some(id))
    });
    assert_eq!(fitting.await.unwrap().len(), 1);

    let paths: Vec<_> = server.calls().into_iter().map(|c| c.path).collect();
    let versions: Vec<_> = paths.iter().filter(|p| p.contains("/version?")).collect();
    assert_eq!(versions.len(), 2);
    assert!(!versions[0].contains("loaders"));
    // shaders filter by shader loader, not by the profile's mod loader
    assert!(
      versions[1].contains("loaders=%5B%22iris%22%2C%22optifine%22%5D"),
      "{}",
      versions[1]
    );
    assert!(versions[1].contains("game_versions=%5B%221.21.1%22%5D"));
    // the project was cached by the first lookup
    let projects = paths.iter().filter(|p| p.ends_with("/project/s")).count();
    assert_eq!(projects, 1);
  }

  #[gpui::test]
  async fn plan_and_install_download_into_the_profile(cx: &mut TestAppContext) {
    let (server, id, dir) = setup(cx, LoaderType::Fabric).await;
    serve_project(
      &server,
      "a",
      "mod",
      &[("1", "a-1.jar", b"one"), ("2", "a-2.jar", b"two")],
    );
    let installed = loaded(cx, &id).await;

    let plan = cx.update(|cx| cx.content().plan_install(id.clone(), vec![pick("a", None)]));
    let plan = plan.await.unwrap();
    // the newest release
    assert_eq!(plan.items[0].version.id, "2");
    assert_eq!(plan.total_bytes, 3);

    let task = cx.update(|cx| cx.content().clone().install(plan, "a".into(), cx));
    assert!(task.await.unwrap());
    cx.run_until_parked();
    assert_eq!(std::fs::read(dir.join("mods/a-2.jar")).unwrap(), b"two");
    assert!(!dir.join(crate::TMP_DIR).exists());
    assert_eq!(files_of(&installed, cx), ["a-2.jar"]);

    // picking the older version switches back to it
    let plan = cx.update(|cx| {
      cx.content()
        .plan_install(id.clone(), vec![pick("a", Some("1"))])
    });
    let plan = plan.await.unwrap();
    assert_eq!(plan.items[0].replaces.as_deref(), Some("a-2.jar"));
    let task = cx.update(|cx| cx.content().clone().install(plan, "a".into(), cx));
    assert!(task.await.unwrap());
    cx.run_until_parked();
    assert_eq!(files_of(&installed, cx), ["a-1.jar"]);
    assert!(!dir.join("mods/a-2.jar").exists());
  }

  #[gpui::test]
  async fn install_rejects_untrusted_plans_before_downloading(cx: &mut TestAppContext) {
    let (server, id, dir) = setup(cx, LoaderType::Fabric).await;
    serve_project(&server, "a", "mod", &[("1", "a.jar", b"one")]);
    let plan = cx.update(|cx| cx.content().plan_install(id.clone(), vec![pick("a", None)]));
    let plan = plan.await.unwrap();

    let mut off_host = plan.clone();
    off_host.items[0].version.files[0].url = Some("https://evil.example/a.jar".into());
    let mut escape = plan.clone();
    escape.items[0].version.files[0].file_name = "../a.jar".into();
    for bad in [off_host, escape] {
      let calls = server.calls().len();
      let task = cx.update(|cx| cx.content().clone().install(bad, "a".into(), cx));
      assert!(task.await.is_err());
      assert_eq!(server.calls().len(), calls);
    }
    assert!(!dir.join("mods/a.jar").exists());
  }

  #[gpui::test]
  async fn corrupt_downloads_leave_the_folder_unchanged(cx: &mut TestAppContext) {
    let (server, id, dir) = setup(cx, LoaderType::Fabric).await;
    serve_project(&server, "a", "mod", &[("1", "a.jar", b"one")]);
    // the cdn now serves other bytes than the listed hash
    server.route("GET", "/cdn.modrinth.com/data/a.jar", 200, "tampered");
    let plan = cx.update(|cx| cx.content().plan_install(id.clone(), vec![pick("a", None)]));
    let plan = plan.await.unwrap();
    let task = cx.update(|cx| cx.content().clone().install(plan, "a".into(), cx));
    assert_eq!(err_of(task.await), "Downloaded file is corrupt");
    let left = std::fs::read_dir(dir.join("mods")).map_or(0, |d| d.count());
    assert_eq!(left, 0);
    assert!(!dir.join(crate::TMP_DIR).exists());
  }

  #[gpui::test]
  async fn an_empty_plan_installs_nothing(cx: &mut TestAppContext) {
    let (server, id, _) = setup(cx, LoaderType::Fabric).await;
    let plan = InstallPlan {
      profile: id,
      items: vec![],
      already_installed: vec![],
      optional: vec![],
      conflicts: vec![],
      manual: vec![],
      total_bytes: 0,
    };
    let task = cx.update(|cx| cx.content().clone().install(plan, "none".into(), cx));
    assert!(task.await.unwrap());
    assert!(server.calls().is_empty());
  }

  /// Answers the update check for the `add_mod` entry (sha1 `x`) with version 2 of `a`
  fn serve_update(server: &FakeServer) {
    server.route(
      "POST",
      "/api.modrinth.com/v2/version_files/update",
      200,
      json!({"x": version_json("a", "2", "a-2.jar", b"two")}).to_string(),
    );
    serve_project(server, "a", "mod", &[("2", "a-2.jar", b"two")]);
  }

  #[gpui::test]
  async fn check_then_update_replaces_the_file(cx: &mut TestAppContext) {
    let (server, id, dir) = setup(cx, LoaderType::Fabric).await;
    serve_update(&server);
    let target = dir.clone();
    rt::spawn(add_mod(target)).await.unwrap();

    let check = cx.update(|cx| cx.content().check_updates(Some(id.clone())));
    let all = check.await.unwrap();
    assert_eq!(all[&id][0].file_name, "a.jar");
    cx.run_until_parked();
    let updates = cx.update(|cx| cx.content().updates.clone());
    assert_eq!(updates.read_with(cx, |u, _| u.0[&id].len()), 1);

    // an update for other files only touches nothing
    let task = cx.update(|cx| {
      cx.content()
        .clone()
        .update(&id, Some(vec!["other.jar".into()]), "u".into(), cx)
    });
    task.await.unwrap();
    assert!(dir.join("mods/a.jar").exists());

    let task = cx.update(|cx| cx.content().clone().update(&id, None, "u".into(), cx));
    task.await.unwrap();
    cx.run_until_parked();
    assert!(!dir.join("mods/a.jar").exists());
    assert_eq!(std::fs::read(dir.join("mods/a-2.jar")).unwrap(), b"two");
    assert!(updates.read_with(cx, |u, _| u.0[&id].is_empty()));
  }

  #[gpui::test]
  async fn check_without_a_profile_checks_all(cx: &mut TestAppContext) {
    let (server, first, dir) = setup(cx, LoaderType::Fabric).await;
    serve_update(&server);
    rt::spawn(add_mod(dir)).await.unwrap();
    let second = create(cx, "Q", LoaderType::Fabric).await;
    let check = cx.update(|cx| cx.content().check_updates(None));
    let all = check.await.unwrap();
    assert_eq!(all[&first].len(), 1);
    assert!(all[&second].is_empty());
  }

  #[gpui::test]
  async fn running_profiles_only_take_mod_toggles_and_removals(cx: &mut TestAppContext) {
    let (_server, id, dir) = setup(cx, LoaderType::Fabric).await;
    rt::spawn(add_mod(dir.clone())).await.unwrap();
    start_game(cx, &id).await;

    let content = cx.update(|cx| cx.content().clone());
    let tasks = cx.update(|cx| {
      vec![
        content.set_enabled(&id, ContentKind::ResourcePack, "r.zip".into(), false, cx),
        content.set_enabled(&id, ContentKind::Shader, "s.zip".into(), false, cx),
        content.set_order(&id, vec![], cx),
        content.set_active_shader(&id, None, cx),
      ]
    });
    for task in tasks {
      assert_eq!(err_of(task.await), "Stop the game first");
    }
    let dup = content.duplicate_profile(id.clone(), "Copy".into());
    assert_eq!(err_of(dup.await), "Stop the game first");

    // mods are read once at launch, toggling them while running is fine
    let task =
      cx.update(|cx| content.set_enabled(&id, ContentKind::Mod, "a.jar".into(), false, cx));
    task.await.unwrap();
    assert!(dir.join("mods/a.jar.disabled").exists());
  }

  #[gpui::test]
  async fn remove_deletes_files_and_forgets_their_updates(cx: &mut TestAppContext) {
    let (_server, id, dir) = setup(cx, LoaderType::Fabric).await;
    rt::spawn(add_mod(dir.clone())).await.unwrap();
    let installed = loaded(cx, &id).await;
    let handle = cx.update(|cx| cx.content().handle.clone());
    handle.seed(&id, &["a.jar"]);

    let task = cx.update(|cx| {
      cx.content()
        .clone()
        .remove(&id, ContentKind::Mod, vec!["a.jar".into()], cx)
    });
    task.await.unwrap();
    cx.run_until_parked();
    assert!(!dir.join("mods/a.jar").exists());
    assert!(files_of(&installed, cx).is_empty());
    let updates = cx.update(|cx| cx.content().updates.clone());
    assert!(updates.read_with(cx, |u, _| u.0[&id].is_empty()));
  }

  #[gpui::test]
  async fn added_files_land_in_the_folder_of_their_kind(cx: &mut TestAppContext) {
    let (_server, id, dir) = setup(cx, LoaderType::Fabric).await;
    let installed = loaded(cx, &id).await;
    let drop_dir = tempfile::tempdir().unwrap();
    let jar = drop_dir.path().join("dropped.jar");
    std::fs::write(&jar, b"jar").unwrap();
    let text = drop_dir.path().join("notes.txt");
    std::fs::write(&text, b"hi").unwrap();

    let task = cx.update(|cx| cx.content().clone().add_file(&id, jar.clone(), cx));
    task.await.unwrap();
    cx.run_until_parked();
    assert!(dir.join("mods/dropped.jar").exists());
    // the scan lists it even though no provider knows it
    assert_eq!(files_of(&installed, cx), ["dropped.jar"]);

    let task = cx.update(|cx| cx.content().clone().add_file(&id, text, cx));
    assert_eq!(err_of(task.await), "Not supported");
  }

  fn manual_file(data: &[u8]) -> ManualFile {
    ManualFile {
      source: Source::CurseForge,
      project_id: "1".into(),
      file_id: "10".into(),
      file_name: "restricted.jar".into(),
      sha1: bytes_hash(data),
      size: data.len() as u64,
      download_url: "https://www.curseforge.com/x/download/10".into(),
    }
  }

  #[gpui::test]
  async fn manual_downloads_are_found_and_adopted(cx: &mut TestAppContext) {
    let (_server, id, dir) = setup(cx, LoaderType::Fabric).await;
    let downloads = tempfile::tempdir().unwrap();
    let path = downloads.path().to_path_buf();
    cx.update(|cx| proton_config::update(cx, |s| s.download_dirs = Some(vec![path])));
    // browsers rename duplicates, the hash still matches
    let file = downloads.path().join("restricted (1).jar");
    std::fs::write(&file, b"real").unwrap();
    std::fs::write(downloads.path().join("restricted.jar.part"), b"real").unwrap();

    let wanted = manual_file(b"real");
    let found = cx.update(|cx| cx.content().find_manual(vec![wanted.clone()], cx));
    let found = found.await;
    assert_eq!(found.len(), 1);
    assert_eq!(found["10"], file);

    let wrong = ManualFile {
      sha1: "nope".into(),
      ..wanted.clone()
    };
    let task = cx.update(|cx| {
      cx.content()
        .clone()
        .adopt_file(&id, wrong, file.clone(), cx)
    });
    assert_eq!(err_of(task.await), "Downloaded file is corrupt");
    assert!(file.exists());

    let task = cx.update(|cx| {
      cx.content()
        .clone()
        .adopt_file(&id, wanted, file.clone(), cx)
    });
    task.await.unwrap();
    assert!(dir.join("mods/restricted.jar").exists());
    // the download folder is left clean
    assert!(!file.exists());
  }

  #[gpui::test]
  async fn refresh_of_a_deleted_profile_ends_loading(cx: &mut TestAppContext) {
    let (_server, _, _) = setup(cx, LoaderType::Fabric).await;
    // never requested, nothing to refresh
    let task = cx.update(|cx| cx.content().clone().refresh_installed("gone", cx));
    task.await.unwrap();
    let installed = loaded(cx, "gone").await;
    assert!(installed.read_with(cx, |i, _| i.by_kind.is_empty()));
    let task = cx.update(|cx| cx.content().clone().refresh_installed("gone", cx));
    assert!(task.await.is_err());
  }

  fn sha512(data: &[u8]) -> String {
    hex::encode(Sha512::digest(data))
  }

  /// An mrpack with one downloaded mod and overrides
  fn mrpack(path: &Path, minecraft: &str, file: &str, data: &[u8]) {
    let index = json!({
      "formatVersion": 1, "game": "minecraft", "versionId": minecraft, "name": "Pack",
      "files": [{
        "path": format!("mods/{file}"), "hashes": {"sha1": bytes_hash(data), "sha512": sha512(data)},
        "downloads": [format!("{CDN}/{file}")], "fileSize": data.len()
      }],
      "dependencies": {"minecraft": minecraft, "fabric-loader": "0.16.0"}
    });
    rt::block_on(write_zip(
      path,
      &[
        ("modrinth.index.json", index.to_string().as_bytes()),
        ("overrides/config/x.txt", b"cfg"),
        ("overrides/profile.json", b"never"),
      ],
    ));
  }

  fn png() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::new(1, 1)
      .write_to(&mut out, image::ImageFormat::Png)
      .unwrap();
    out.into_inner()
  }

  /// Keeps the archive cache out of the real cache dir
  fn temp_cache() -> PathBuf {
    let dir = tempfile::tempdir().unwrap().keep();
    unsafe { std::env::set_var("XDG_CACHE_HOME", &dir) };
    dir
  }

  #[gpui::test]
  async fn local_modpacks_install_into_a_new_profile(cx: &mut TestAppContext) {
    let (server, _, _) = setup(cx, LoaderType::Vanilla).await;
    temp_cache();
    server.route("GET", "/cdn.modrinth.com/data/m.jar", 200, "mod");
    let file = tempfile::tempdir().unwrap().keep().join("p.mrpack");
    mrpack(&file, "1.21.4", "m.jar", b"mod");
    let before = profile_ids(cx).await;

    let plan = cx.update(|cx| {
      cx.content()
        .modpack_plan(PackSource::Path { path: file.clone() })
    });
    let plan = plan.await.unwrap();
    assert_eq!((plan.counts.mods, plan.total_bytes), (1, 3));
    let install = cx.update(|cx| {
      cx.content()
        .modpack_install(plan, "Mine".into(), HashMap::new(), "t".into(), cx)
    });
    let id = install.await.unwrap();
    assert!(!before.contains(&id));

    let info = profile(cx, &id).await;
    assert_eq!(info.name, "Mine");
    assert_eq!(info.version, "1.21.4");
    assert_eq!(info.loader_version.as_deref(), Some("0.16.0"));
    // only remote packs can be updated later
    assert!(info.modpack.is_none());
    let dir = profile_dir(cx, &id).await;
    assert_eq!(std::fs::read(dir.join("mods/m.jar")).unwrap(), b"mod");
    assert_eq!(std::fs::read(dir.join("config/x.txt")).unwrap(), b"cfg");
    let json = std::fs::read_to_string(dir.join("profile.json")).unwrap();
    assert_ne!(json, "never");
  }

  #[gpui::test]
  async fn failed_modpack_installs_remove_the_profile(cx: &mut TestAppContext) {
    let (server, _, _) = setup(cx, LoaderType::Vanilla).await;
    temp_cache();
    server.route("GET", "/cdn.modrinth.com/data/m.jar", 200, "mod");
    let file = tempfile::tempdir().unwrap().keep().join("p.mrpack");
    mrpack(&file, "1.21.1", "m.jar", b"mod");
    let plan = cx.update(|cx| cx.content().modpack_plan(PackSource::Path { path: file }));
    let mut plan = plan.await.unwrap();
    let before = profile_ids(cx).await;

    // a restricted file that was never downloaded
    plan.manual.push(manual_file(b"x"));
    let install = cx.update(|cx| {
      cx.content()
        .modpack_install(plan.clone(), "Mine".into(), HashMap::new(), "t".into(), cx)
    });
    assert_eq!(err_of(install.await), "File must be downloaded manually");
    assert_eq!(profile_ids(cx).await, before);

    plan.manual.clear();
    server.route("GET", "/cdn.modrinth.com/data/m.jar", 200, "tampered");
    let install = cx.update(|cx| {
      cx.content()
        .modpack_install(plan, "Mine".into(), HashMap::new(), "t".into(), cx)
    });
    assert_eq!(err_of(install.await), "Downloaded file is corrupt");
    assert_eq!(profile_ids(cx).await, before);
  }

  /// Serves pack versions `v1` (1.21.1, a.jar) and `v2` (1.21.4, b.jar) of project `pack`
  fn serve_pack(server: &FakeServer) {
    let dir = tempfile::tempdir().unwrap();
    let mut versions = Vec::new();
    for (id, minecraft, file) in [("1", "1.21.1", "a.jar"), ("2", "1.21.4", "b.jar")] {
      let path = dir.path().join(format!("{id}.mrpack"));
      mrpack(&path, minecraft, file, file.as_bytes());
      let data = std::fs::read(&path).unwrap();
      versions.push((id, format!("pack-{id}.mrpack"), data));
      server.route("GET", &format!("/cdn.modrinth.com/data/{file}"), 200, file);
    }
    let list: Vec<_> = versions
      .iter()
      .map(|(id, file, data)| version_json("pack", id, file, data))
      .collect();
    server.route(
      "GET",
      "/api.modrinth.com/v2/project/pack/version",
      200,
      Value::from(list).to_string(),
    );
    for (_, file, data) in &versions {
      server.route(
        "GET",
        &format!("/cdn.modrinth.com/data/{file}"),
        200,
        data.clone(),
      );
    }
    let mut project = project_json("pack", "modpack");
    project["icon_url"] = json!("https://cdn.modrinth.com/icon.png");
    server.route(
      "GET",
      "/api.modrinth.com/v2/projects",
      200,
      json!([project]).to_string(),
    );
    server.route("GET", "/cdn.modrinth.com/icon.png", 200, png());
  }

  fn remote(version: Option<&str>) -> PackSource {
    PackSource::Remote {
      source: Source::Modrinth,
      project_id: "pack".into(),
      version_id: version.map(Into::into),
    }
  }

  #[gpui::test]
  async fn remote_modpacks_remember_their_version_and_update(cx: &mut TestAppContext) {
    let (server, _, _) = setup(cx, LoaderType::Vanilla).await;
    let cache = temp_cache();
    serve_pack(&server);

    let plan = cx.update(|cx| cx.content().modpack_plan(remote(Some("1"))));
    let mut plan = plan.await.unwrap();
    assert_eq!(
      plan.icon_url.as_deref(),
      Some("https://cdn.modrinth.com/icon.png")
    );
    assert!(plan.archive.starts_with(&cache));
    plan.recommended_memory_mb = Some(1);
    let install = cx.update(|cx| {
      cx.content()
        .modpack_install(plan, "Pack".into(), HashMap::new(), "t".into(), cx)
    });
    let id = install.await.unwrap();
    let info = profile(cx, &id).await;
    let pack = info.modpack.clone().unwrap();
    assert_eq!(
      (pack.project_id.as_str(), pack.version_id.as_str()),
      ("pack", "1")
    );
    // the pack's memory fits any machine, so the profile uses it
    assert!(info.use_local_jvm);
    assert_eq!(info.jvm.as_ref().unwrap().mem_max, 1);
    let dir = profile_dir(cx, &id).await;
    assert!(dir.join("mods/a.jar").exists());
    assert!(dir.join("image.png").exists());

    // the cached archive is reused while it matches
    let downloads = |server: &FakeServer| {
      server
        .calls()
        .iter()
        .filter(|c| c.path.ends_with(".mrpack"))
        .count()
    };
    let before = downloads(&server);
    let plan = cx.update(|cx| cx.content().modpack_plan(remote(Some("1"))));
    plan.await.unwrap();
    assert_eq!(downloads(&server), before);

    // no version picks the newest
    let plan = cx.update(|cx| cx.content().modpack_plan(remote(None)));
    let plan = plan.await.unwrap();
    assert_eq!(plan.source, remote(Some("2")));
    std::fs::write(dir.join("mods/user.jar"), b"mine").unwrap();
    let update = cx.update(|cx| {
      cx.content()
        .clone()
        .modpack_update(&id, plan, HashMap::new(), "t".into(), cx)
    });
    assert!(update.await.unwrap());
    let info = profile(cx, &id).await;
    assert_eq!(info.version, "1.21.4");
    assert_eq!(info.modpack.unwrap().version_id, "2");
    // a new game version needs a fresh download
    assert!(!info.downloaded);
    assert!(!dir.join("mods/a.jar").exists());
    assert!(dir.join("mods/b.jar").exists());
    assert!(dir.join("mods/user.jar").exists());
  }

  #[gpui::test]
  async fn modpack_update_checks_before_replacing(cx: &mut TestAppContext) {
    let (server, id, dir) = setup(cx, LoaderType::Fabric).await;
    temp_cache();
    serve_pack(&server);
    let plan = cx.update(|cx| cx.content().modpack_plan(remote(Some("2"))));
    let plan = plan.await.unwrap();

    let mut restricted = plan.clone();
    restricted.manual.push(manual_file(b"x"));
    let update = cx.update(|cx| {
      cx.content()
        .clone()
        .modpack_update(&id, restricted, HashMap::new(), "t".into(), cx)
    });
    assert_eq!(err_of(update.await), "File must be downloaded manually");
    assert!(!dir.join("mods/b.jar").exists());

    start_game(cx, &id).await;
    let update = cx.update(|cx| {
      cx.content()
        .clone()
        .modpack_update(&id, plan, HashMap::new(), "t".into(), cx)
    });
    assert_eq!(err_of(update.await), "Stop the game first");
    assert_eq!(profile(cx, &id).await.version, "1.21.1");
  }

  #[gpui::test]
  async fn exported_packs_read_back(cx: &mut TestAppContext) {
    let (server, id, dir) = setup(cx, LoaderType::Fabric).await;
    rt::spawn(add_mod(dir.clone())).await.unwrap();
    // the installed file no longer matches the listed one, it goes into the overrides
    server.route(
      "GET",
      "/api.modrinth.com/v2/versions",
      200,
      json!([version_json("a", "1", "a.jar", b"other")]).to_string(),
    );
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(dir.join("config/c.txt"), b"c").unwrap();
    let out = tempfile::tempdir().unwrap().keep().join("out.mrpack");

    let export = cx.update(|cx| {
      cx.content()
        .export_mrpack(id.clone(), out.clone(), vec!["config".into()], "e".into())
    });
    export.await.unwrap();
    let plan = cx.update(|cx| {
      cx.content()
        .modpack_plan(PackSource::Path { path: out.clone() })
    });
    let plan = plan.await.unwrap();
    assert_eq!(
      (plan.name.as_str(), plan.minecraft.as_str()),
      ("P", "1.21.1")
    );
    assert!(plan.files.is_empty());

    let missing = cx.update(|cx| {
      cx.content()
        .export_mrpack("gone".into(), out.clone(), vec![], "e".into())
    });
    assert!(missing.await.is_err());
  }

  #[gpui::test]
  async fn duplicates_copy_the_folder_under_a_new_id(cx: &mut TestAppContext) {
    let (_server, id, dir) = setup(cx, LoaderType::Fabric).await;
    rt::spawn(add_mod(dir.clone())).await.unwrap();
    std::fs::create_dir_all(dir.join("logs")).unwrap();
    std::fs::write(dir.join("logs/latest.log"), b"log").unwrap();

    let dup = cx.update(|cx| cx.content().duplicate_profile(id.clone(), "Copy".into()));
    let copy = dup.await.unwrap();
    assert_ne!(copy, id);
    let info = profile(cx, &copy).await;
    assert_eq!(info.name, "Copy");
    let copy_dir = profile_dir(cx, &copy).await;
    assert!(copy_dir.join("mods/a.jar").exists());
    assert!(!copy_dir.join("logs").exists());
    assert_eq!(profile_ids(cx).await.len(), 2);

    let dup = cx.update(|cx| cx.content().duplicate_profile("gone".into(), "X".into()));
    assert!(dup.await.is_err());
  }

  #[gpui::test]
  async fn the_updates_loop_checks_right_away(cx: &mut TestAppContext) {
    let (_server, _, _) = setup(cx, LoaderType::Fabric).await;
    let before: Option<chrono::DateTime<chrono::Utc>> = proton_store::store()
      .get("content_updates_checked")
      .unwrap();
    cx.update(|cx| cx.content().start_updates_loop());
    let start = std::time::Instant::now();
    loop {
      let now: Option<chrono::DateTime<chrono::Utc>> = proton_store::store()
        .get("content_updates_checked")
        .unwrap();
      if now.is_some() && now != before {
        break;
      }
      assert!(start.elapsed().as_secs() < 10, "loop never checked");
      std::thread::sleep(std::time::Duration::from_millis(20));
    }
  }

  #[test]
  fn not_running_only_fails_while_running() {
    assert!(not_running(false).is_ok());
    assert_eq!(err_of(not_running(true)), "Stop the game first");
  }
}
