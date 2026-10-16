use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use log::trace;
use tauri::{AppHandle, Manager, Result, State};
use tokio::sync::{Mutex, Notify};

use proton_config::offline::OfflineResultExt;
use proton_config::settings::{MaxMem, SettingsExt};
use proton_content::{
  ContentError,
  install::{Progress, adopt_file, check_plan, execute_install, find_manual, plan_install},
  model::{
    Category, ContentKind, InstallPlan, Installed, ManualFile, PackPlan, PackSource, PackUpdate,
    PickRef, ProjectDetail, SearchQuery, SearchResult, Source, UpdateInfo, VersionInfo,
  },
  modpack,
  provider::{Providers, profile_loaders},
  store,
  updates::{self, PackUpdates, UpdatesCache},
};
use proton_profiles::{
  config::{JvmSettings, ModpackRef, Profile},
  store::ProfileStore,
};
use proton_utils::{
  file::write_file,
  log::ResultLogExt,
  updater::{UpdateType, update_data},
};
use proton_versions::{
  event::{DownloadCheckStatus, emit_download_check_status},
  loader::LoaderType,
  store::register_cancel,
};

/// Copies loader and game version out of the profile, the lock is dropped before any network call
async fn profile_target(
  state: &Mutex<ProfileStore>,
  profile: &str,
) -> Result<(LoaderType, String)> {
  let store = state.lock().await;
  let profile = store.profile(profile).await.log()?;
  Ok((profile.loader, profile.version))
}

/// Profile, its directory and whether an instance runs, copied out of the store lock
async fn profile_dir(
  state: &Mutex<ProfileStore>,
  profile: &str,
) -> Result<(Profile, PathBuf, bool)> {
  let store = state.lock().await;
  let info = store.profile(profile).await.log()?;
  let dir = store.get_profile_path(profile).log()?;
  let running = store.is_running(profile).await;
  Ok((info, dir, running))
}

/// `options.txt` and `iris.properties` are rewritten by the game while it runs
async fn stopped_profile_dir(state: &Mutex<ProfileStore>, profile: &str) -> Result<PathBuf> {
  let (_, dir, running) = profile_dir(state, profile).await?;
  if running {
    Err(ContentError::Running).log()?;
  }
  Ok(dir)
}

fn progress(handle: &AppHandle, id: usize) -> Progress {
  let handle = handle.clone();
  Arc::new(move |status| emit_download_check_status(&handle, status, id))
}

#[tauri::command]
pub async fn content_sources() -> Result<Vec<Source>> {
  trace!("Command content_sources called");
  Ok(vec![Source::Modrinth, Source::CurseForge])
}

#[tauri::command]
pub async fn content_categories(
  providers: State<'_, Providers>,
  handle: AppHandle,
  kind: ContentKind,
) -> Result<Vec<Category>> {
  trace!("Command content_categories called with kind {kind:?}");
  Ok(
    providers
      .modrinth()
      .categories(kind)
      .await
      .check_online_state(&handle)
      .await?,
  )
}

#[tauri::command]
pub async fn content_search(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  mut query: SearchQuery,
) -> Result<SearchResult> {
  trace!("Command content_search called with query {query:?}");
  if let Some(profile) = &query.compatible_with {
    let (loader, version) = profile_target(&profiles, profile).await?;
    // CurseForge only applies mod loader names, so the Modrinth names work for both
    query.loaders = profile_loaders(Source::Modrinth, query.kind, loader);
    query.game_version = Some(version);
  }
  Ok(
    providers
      .search_all(&query)
      .await
      .check_online_state(&handle)
      .await?,
  )
}

#[tauri::command]
pub async fn content_project(
  providers: State<'_, Providers>,
  handle: AppHandle,
  source: Source,
  id: &str,
) -> Result<ProjectDetail> {
  trace!("Command content_project called with source {source:?} id {id}");
  Ok(
    providers
      .get(source)
      .project(id)
      .await
      .check_online_state(&handle)
      .await?,
  )
}

#[tauri::command]
pub async fn content_versions(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  source: Source,
  id: &str,
  profile: Option<&str>,
) -> Result<Vec<VersionInfo>> {
  trace!("Command content_versions called with source {source:?} id {id} profile {profile:?}");
  let provider = providers.get(source);
  let (loaders, game) = match profile {
    Some(profile) => {
      let (loader, version) = profile_target(&profiles, profile).await?;
      // the loader filter depends on the kind, the project is usually cached already
      let kind = provider
        .project(id)
        .await
        .check_online_state(&handle)
        .await?
        .summary
        .kind;
      (profile_loaders(source, kind, loader), Some(version))
    }
    None => (Vec::new(), None),
  };
  Ok(
    provider
      .versions(id, &loaders, game.as_deref())
      .await
      .check_online_state(&handle)
      .await?,
  )
}

#[tauri::command]
pub async fn content_installed(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  kind: ContentKind,
) -> Result<Vec<Installed>> {
  trace!("Command content_installed called with profile {profile} kind {kind:?}");
  let (_, dir, _) = profile_dir(&profiles, profile).await?;
  Ok(store::scan(&providers, &dir, kind).await.log()?)
}

#[tauri::command]
pub async fn content_set_enabled(
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  profile: &str,
  kind: ContentKind,
  file_name: &str,
  enabled: bool,
) -> Result<()> {
  trace!(
    "Command content_set_enabled called with profile {profile} file {file_name} enabled {enabled}"
  );
  let dir = if kind == ContentKind::Mod {
    profile_dir(&profiles, profile).await?.1
  } else {
    stopped_profile_dir(&profiles, profile).await?
  };
  store::set_enabled(&dir, kind, file_name, enabled)
    .await
    .log()?;
  update_data(&handle, UpdateType::ProfileContent);
  Ok(())
}

#[tauri::command]
pub async fn content_set_order(
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  profile: &str,
  files: Vec<String>,
) -> Result<()> {
  trace!("Command content_set_order called with profile {profile} files {files:?}");
  let dir = stopped_profile_dir(&profiles, profile).await?;
  store::set_order(&dir, &files).await.log()?;
  update_data(&handle, UpdateType::ProfileContent);
  Ok(())
}

#[tauri::command]
pub async fn content_set_active_shader(
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  profile: &str,
  file_name: Option<&str>,
) -> Result<()> {
  trace!("Command content_set_active_shader called with profile {profile} file {file_name:?}");
  let dir = stopped_profile_dir(&profiles, profile).await?;
  store::set_active_shader(&dir, file_name).await.log()?;
  update_data(&handle, UpdateType::ProfileContent);
  Ok(())
}

#[tauri::command]
pub async fn content_remove(
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  profile: &str,
  kind: ContentKind,
  file_names: Vec<String>,
) -> Result<()> {
  trace!("Command content_remove called with profile {profile} kind {kind:?} files {file_names:?}");
  let (_, dir, _) = profile_dir(&profiles, profile).await?;
  store::remove(&dir, kind, &file_names).await.log()?;
  updates::forget(&handle, profile, &file_names);
  Ok(())
}

#[tauri::command]
pub async fn content_add_file(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  path: PathBuf,
) -> Result<()> {
  trace!("Command content_add_file called with profile {profile} path {path:?}");
  let (_, dir, _) = profile_dir(&profiles, profile).await?;
  let kind = store::add_file(&dir, &path).await.log()?;
  store::scan(&providers, &dir, kind).await.log()?;
  Ok(())
}

#[tauri::command]
pub async fn content_adopt_file(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  file: ManualFile,
  path: PathBuf,
) -> Result<()> {
  trace!("Command content_adopt_file called with profile {profile} file {file:?} path {path:?}");
  let (_, dir, _) = profile_dir(&profiles, profile).await?;
  let kind = adopt_file(&dir, &file, &path).await.log()?;
  store::scan(&providers, &dir, kind).await.log()?;
  Ok(())
}

#[tauri::command]
pub async fn content_find_manual(
  handle: AppHandle,
  files: Vec<ManualFile>,
) -> Result<HashMap<String, PathBuf>> {
  let dirs = handle.app_settings()?.download_dirs(&handle);
  Ok(find_manual(dirs, files).await)
}

#[tauri::command]
pub async fn content_plan_install(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  profile: &str,
  picks: Vec<PickRef>,
) -> Result<InstallPlan> {
  trace!("Command content_plan_install called with profile {profile} picks {picks:?}");
  let (profile, dir, _) = profile_dir(&profiles, profile).await?;
  let entries = updates::entries(&dir).await;
  Ok(
    plan_install(
      |s| providers.get(s),
      &profile,
      &entries,
      &picks,
      &HashMap::new(),
    )
    .await
    .check_online_state(&handle)
    .await?,
  )
}

/// Returns Ok(false) when canceled
async fn install(
  providers: &Providers,
  handle: &AppHandle,
  dir: &Path,
  plan: &InstallPlan,
  id: usize,
  cancel: &Notify,
) -> Result<bool> {
  Ok(
    execute_install(providers, dir, plan, id, cancel, progress(handle, id))
      .await
      .check_online_state(handle)
      .await?,
  )
}

#[tauri::command]
pub async fn content_install(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  plan: InstallPlan,
  id: usize,
) -> Result<()> {
  trace!(
    "Command content_install called with profile {} id {id}",
    plan.profile
  );
  let guard = register_cancel(id);
  check_plan(&plan).log()?;
  let (_, dir, _) = profile_dir(&profiles, &plan.profile).await?;
  install(&providers, &handle, &dir, &plan, id, &guard.notify).await?;
  Ok(())
}

#[tauri::command]
pub async fn content_updates(
  cache: State<'_, UpdatesCache>,
) -> Result<HashMap<String, Vec<UpdateInfo>>> {
  trace!("Command content_updates called");
  Ok(cache.0.lock().unwrap().clone())
}

#[tauri::command]
pub async fn modpack_updates(packs: State<'_, PackUpdates>) -> Result<HashMap<String, PackUpdate>> {
  trace!("Command modpack_updates called");
  Ok(packs.0.lock().unwrap().clone())
}

#[tauri::command]
pub async fn content_check_updates(
  handle: AppHandle,
  profile: Option<&str>,
) -> Result<HashMap<String, Vec<UpdateInfo>>> {
  trace!("Command content_check_updates called with profile {profile:?}");
  Ok(updates::check_profiles(&handle, profile).await.log()?)
}

#[tauri::command]
pub async fn content_update(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  profile: &str,
  file_names: Option<Vec<String>>,
  id: usize,
) -> Result<()> {
  trace!("Command content_update called with profile {profile} files {file_names:?} id {id}");
  // registered for the whole job so the update loop waits
  let guard = register_cancel(id);
  emit_download_check_status(&handle, DownloadCheckStatus::ContentResolve, id);
  let (info, dir, _) = profile_dir(&profiles, profile).await?;
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
    .check_online_state(&handle)
    .await?;

  if install(&providers, &handle, &dir, &plan, id, &guard.notify).await? {
    let updated: Vec<String> = available.into_iter().map(|u| u.file_name).collect();
    updates::forget(&handle, profile, &updated);
  }
  Ok(())
}

#[tauri::command]
pub async fn modpack_plan(
  providers: State<'_, Providers>,
  handle: AppHandle,
  source: PackSource,
) -> Result<PackPlan> {
  trace!("Command modpack_plan called with source {source:?}");
  let cache = handle.path().app_cache_dir()?;
  Ok(
    modpack::plan(&providers, &cache, source)
      .await
      .check_online_state(&handle)
      .await?,
  )
}

/// Creates a profile from the plan and returns its id, empty when canceled. The profile is deleted
/// again on error or cancel.
#[tauri::command]
pub async fn modpack_install(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  plan: PackPlan,
  name: String,
  manual: HashMap<String, PathBuf>,
  id: usize,
) -> Result<String> {
  trace!("Command modpack_install called with name {name} id {id}");
  let guard = register_cancel(id);
  let progress = progress(&handle, id);
  progress(DownloadCheckStatus::ContentResolve);
  let cache = handle.path().app_cache_dir()?;
  let archive = modpack::archive(&providers, &cache, &plan.source)
    .await
    .check_online_state(&handle)
    .await?;
  let icon = modpack::icon(&providers, &plan).await;

  let profile = profiles
    .lock()
    .await
    .create_profile(
      name,
      icon.as_deref(),
      plan.minecraft.clone(),
      plan.loader,
      plan.loader_version.clone(),
    )
    .await
    .log()?;

  // every error from here on deletes the profile again
  let res = async {
    let store = profiles.lock().await;
    let mut info = store.profile(&profile).await?;
    info.modpack = pack_ref(&plan.source);
    // only when the system has room for it, otherwise the global setting stays in use
    if let Some(mb) = plan.recommended_memory_mb
      && handle.try_state::<MaxMem>().is_some_and(|max| max.fits(mb))
    {
      info.use_local_jvm = true;
      info.jvm = Some(JvmSettings {
        mem_max: mb as usize,
        ..handle.app_settings()?.minecraft.jvm_settings
      });
    }
    info.update(store.data_dir()).await?;
    store.update_data(UpdateType::Profiles);
    let dir = store.get_profile_path(&profile)?;
    drop(store);
    let done = modpack::install_files(
      &providers,
      &dir,
      &archive,
      &plan,
      id,
      &guard.notify,
      progress,
    )
    .await?;
    // restricted files picked up from the download folders
    for file in plan.manual.iter().filter(|_| done) {
      let path = manual
        .get(&file.file_id)
        .ok_or(ContentError::NeedsManualDownload)?;
      adopt_file(&dir, file, path).await?;
    }
    if !plan.manual.is_empty() {
      update_data(&handle, UpdateType::ProfileContent);
    }
    anyhow::Ok(done)
  }
  .await;
  // ponytail: the cached archive stays, other installs of the same version may still read it
  if let Ok(true) = res {
    return Ok(profile);
  }

  let mut store = profiles.lock().await;
  let _ = store.remove_profile(&profile).await.log();
  store.update_data(UpdateType::Profiles);
  drop(store);
  res.check_online_state(&handle).await?;
  Ok(String::new())
}

/// Switches a modpack profile to the plan's version. Pack files are replaced, content the user
/// added stays. Nothing is deleted when the download fails or is canceled. False when canceled.
#[tauri::command]
pub async fn modpack_update(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  profile: &str,
  plan: PackPlan,
  manual: HashMap<String, PathBuf>,
  id: usize,
) -> Result<bool> {
  trace!("Command modpack_update called with profile {profile} id {id}");
  let guard = register_cancel(id);
  let progress = progress(&handle, id);
  progress(DownloadCheckStatus::ContentResolve);
  let (_, dir, running) = profile_dir(&profiles, profile).await?;
  if running {
    Err(ContentError::Running).log()?;
  }
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
    .collect::<std::result::Result<_, ContentError>>()
    .log()?;
  let cache = handle.path().app_cache_dir()?;
  let res = async {
    let archive = modpack::archive(&providers, &cache, &plan.source).await?;
    let done = modpack::install_files(
      &providers,
      &dir,
      &archive,
      &plan,
      id,
      &guard.notify,
      progress,
    )
    .await?;
    for (file, path) in manual.iter().filter(|_| done) {
      adopt_file(&dir, file, path).await?;
    }
    anyhow::Ok(done)
  }
  .await;
  update_data(&handle, UpdateType::ProfileContent);
  if !res.check_online_state(&handle).await? {
    return Ok(false);
  }

  let store = profiles.lock().await;
  let mut info = store.profile(profile).await.log()?;
  if info.version != plan.minecraft || info.loader_version != plan.loader_version {
    info.downloaded = false;
  }
  info.version = plan.minecraft;
  info.loader = plan.loader;
  info.loader_version = plan.loader_version;
  info.modpack = pack_ref(&plan.source);
  info.update(store.data_dir()).await.log()?;
  store.update_data(UpdateType::Profiles);
  drop(store);
  handle
    .state::<PackUpdates>()
    .0
    .lock()
    .unwrap()
    .remove(profile);
  update_data(&handle, UpdateType::ContentUpdates);
  Ok(true)
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

#[tauri::command]
pub async fn profile_export_mrpack(
  providers: State<'_, Providers>,
  profiles: State<'_, Mutex<ProfileStore>>,
  handle: AppHandle,
  profile: &str,
  path: PathBuf,
  include: Vec<String>,
  id: usize,
) -> Result<()> {
  trace!("Command profile_export_mrpack called with profile {profile} path {path:?} id {id}");
  let guard = register_cancel(id);
  let (info, dir, _) = profile_dir(&profiles, profile).await?;
  tokio::select! {
    res = modpack::export_mrpack(&providers, &dir, &info, &path, include, progress(&handle, id)) => {
      res.check_online_state(&handle).await?;
    }
    _ = guard.notify.notified() => {
      let _ = tokio::fs::remove_file(&path).await;
    }
  }
  Ok(())
}

#[tauri::command]
pub async fn profile_duplicate(
  profiles: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  name: String,
) -> Result<String> {
  trace!("Command profile_duplicate called with profile {profile} name {name}");
  let (info, src, running) = profile_dir(&profiles, profile).await?;
  // the game writes to the folder while it runs
  if running {
    Err(ContentError::Running).log()?;
  }
  let copy = modpack::duplicate(&info, name);
  let data_dir = profiles.lock().await.data_dir().clone();
  let dst = data_dir.join(copy.relative_to_data());
  modpack::copy_profile(src, dst.clone()).await.log()?;
  let mut store = profiles.lock().await;
  let res = async {
    write_file(&dst.join("profile.json"), &copy).await?;
    store.insert_profile(&copy)
  }
  .await;
  if res.is_err() {
    let _ = tokio::fs::remove_dir_all(&dst).await;
  }
  res.log()?;
  store.update_data(UpdateType::Profiles);
  Ok(copy.id)
}
