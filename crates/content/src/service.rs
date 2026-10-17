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
