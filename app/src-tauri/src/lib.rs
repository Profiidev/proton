use anyhow::Result;
use chrono::Local;
use content::{
  commands::{
    content_add_file, content_adopt_file, content_categories, content_check_updates,
    content_find_manual, content_install, content_installed, content_plan_install, content_project,
    content_remove, content_search, content_set_active_shader, content_set_enabled,
    content_set_order, content_sources, content_update, content_updates, content_versions,
    modpack_install, modpack_plan, modpack_update, modpack_updates, profile_duplicate,
    profile_export_mrpack,
  },
  provider::Providers,
  updates::{PackUpdates, UpdatesCache, run_loop},
};
use profiles::store::ProfileStore;
use store::TauriAppStoreExt;

use account::{
  commands::{
    account_add_skin, account_change_cape, account_change_skin, account_get_active,
    account_get_cape, account_get_skin, account_list, account_list_skins, account_login,
    account_refresh, account_refresh_one, account_remove, account_remove_skin, account_set_active,
  },
  skin_store::SkinStore,
  store::AccountStore,
};
use profiles::commands::{
  instance_list, instance_logs, instance_stop, instance_stop_all, profile_cancel_download,
  profile_clear_logs, profile_create, profile_favorites_list, profile_favorites_set,
  profile_get_icon, profile_history_list, profile_launch, profile_list, profile_logs,
  profile_mods_count, profile_open_path, profile_quick_play_icon, profile_quick_play_list,
  profile_quick_play_remove, profile_remove, profile_repair, profile_runs_list, profile_update,
  profile_update_icon, quick_play_server_status,
};
use settings::{settings_get, settings_set};
use tauri::{AppHandle, Emitter, Manager, Url, webview::PageLoadEvent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
use tokio::sync::Mutex;
use versions::{
  commands::{loader_version_list, version_list},
  store::McVersionStore,
};

use crate::{
  offline::{MANIFEST_REFRESH_ERROR, OfflineState, is_offline, try_reconnect},
  settings::{MaxMem, SETTINGS_KEY, SettingsExt},
  utils::{log::ResultLogExt, updater::default_client},
  versions::{event::UpdateLimiterStore, loader::LoaderType, paths::MCVersionPath},
};

mod account;
mod content;
mod offline;
mod profiles;
mod settings;
mod store;
mod utils;
mod versions;

const CLIENT_ID: &str = "dd35660a-6381-41f8-bb34-2a36669581d0";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  migrate_data_dir();

  tauri::Builder::default()
    .plugin(tauri_plugin_process::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin({
      let builder = tauri_plugin_window_state::Builder::new();
      // tiling compositors (e.g. Hyprland) report tiled windows as maximized,
      // restoring that state would force the window into a maximized layout
      #[cfg(target_os = "linux")]
      let builder = builder.with_state_flags(
        tauri_plugin_window_state::StateFlags::all()
          - tauri_plugin_window_state::StateFlags::MAXIMIZED,
      );
      builder.build()
    })
    .plugin(
      tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::Stdout))
        .target(Target::new(TargetKind::LogDir {
          file_name: Some(Local::now().to_rfc3339().replace(":", "-")),
        }))
        .filter(|metadata| {
          !metadata.target().starts_with("notify::")
            && !metadata.target().starts_with("serde_xml_rs::")
        })
        .rotation_strategy(RotationStrategy::KeepAll)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build(),
    )
    .plugin(tauri_plugin_single_instance::init(|app, _, _| {
      let _ = app
        .get_webview_window("main")
        .expect("No main window")
        .set_focus();
    }))
    .plugin(tauri_plugin_store::Builder::new().build())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
      //accounts
      account_login,
      account_refresh,
      account_refresh_one,
      account_list,
      account_get_skin,
      account_get_active,
      account_set_active,
      account_remove,
      account_add_skin,
      account_get_cape,
      account_list_skins,
      account_remove_skin,
      account_change_skin,
      account_change_cape,
      //versions
      version_list,
      loader_version_list,
      //profiles
      profile_create,
      profile_remove,
      profile_update,
      profile_get_icon,
      profile_open_path,
      profile_update_icon,
      profile_list,
      profile_mods_count,
      profile_launch,
      profile_repair,
      profile_cancel_download,
      profile_runs_list,
      profile_clear_logs,
      profile_logs,
      profile_quick_play_list,
      profile_quick_play_remove,
      profile_quick_play_icon,
      quick_play_server_status,
      //home
      profile_favorites_set,
      profile_favorites_list,
      profile_history_list,
      //instances
      instance_list,
      instance_logs,
      instance_stop,
      instance_stop_all,
      //settings
      settings_get,
      settings_set,
      //offline
      is_offline,
      try_reconnect,
      //content
      content_sources,
      content_categories,
      content_search,
      content_project,
      content_versions,
      content_plan_install,
      content_install,
      content_installed,
      content_set_enabled,
      content_set_order,
      content_set_active_shader,
      content_remove,
      content_add_file,
      content_adopt_file,
      content_find_manual,
      content_updates,
      modpack_updates,
      content_check_updates,
      content_update,
      modpack_plan,
      modpack_install,
      modpack_update,
      profile_duplicate,
      profile_export_mrpack,
    ])
    .setup(|app| {
      let _ = app.handle().app_store()?;

      app.manage(Mutex::new(SkinStore::new(app.handle().clone())?));
      app.manage(Mutex::new(AccountStore::new(app.handle().clone())?));
      app.manage(Mutex::new(ProfileStore::new(app.handle().clone())?));
      app.manage(std::sync::Mutex::new(UpdateLimiterStore::default()));
      app.manage(OfflineState::new(app.handle().clone()));
      app.manage(Providers::new());
      app.manage(UpdatesCache::default());
      app.manage(PackUpdates::default());

      app.manage(Mutex::new(tauri::async_runtime::block_on(
        McVersionStore::new(app.handle().clone()),
      )?));

      let handle = app.handle().clone();
      app.manage(tauri::async_runtime::spawn(async move {
        handle.manage(MaxMem::new());
        if let Err(err) = async_online_check(&handle).await.log() {
          log::error!("Error: {err}");
          let _ = handle.emit(MANIFEST_REFRESH_ERROR, ()).log();
        }
        tauri::async_runtime::spawn(run_loop(handle));
      }));

      Ok(())
    })
    .on_page_load(|view, payload| {
      if view.label() == "main"
        && payload.event() == PageLoadEvent::Started
        && let Ok(current_url) = view.url()
        && let Ok(mut settings) = view.app_handle().app_settings()
        && let Some(url) = settings.url
        && url != current_url
      {
        #[cfg(debug_assertions)]
        if url.scheme() == "tauri" {
          let url = Url::parse("http://localhost:1420").unwrap();
          settings.url = Some(url.clone());
          let _ = view.navigate(url);

          if let Ok(store) = view.app_handle().app_store() {
            let _ = store.set(SETTINGS_KEY, &settings);
          }
          return;
        }
        #[cfg(not(debug_assertions))]
        if url.scheme() == "http" || url.scheme() == "https" {
          let url = Url::parse("tauri://localhost/home").unwrap();
          settings.url = Some(url.clone());
          let _ = view.navigate(url);

          if let Ok(store) = view.app_handle().app_store() {
            let _ = store.set(SETTINGS_KEY, &settings);
          }
          return;
        }
        let _ = view.navigate(url);
      }
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

async fn async_online_check(handle: &AppHandle) -> Result<()> {
  let state = handle.state::<OfflineState>();
  if !state.check_online_state().await {
    return Err(anyhow::anyhow!("Offline state detected"));
  }

  async_setup_refresh(handle).await?;

  state.state_init();

  Ok(())
}

/// Moves app data from the old identifier folder. Runs before plugins create the new folder.
fn migrate_data_dir() {
  let Some(base) = dirs::data_dir() else {
    return;
  };
  let old = base.join("de.proton.mc");
  let new = base.join("io.profidev.proton");
  if old.exists()
    && !new.exists()
    && let Err(e) = std::fs::rename(&old, &new)
  {
    eprintln!("Failed to migrate data dir: {e}");
  }
}

async fn async_setup_refresh(handle: &AppHandle) -> Result<()> {
  let client = default_client();
  // download first to not lock the store while downloading the actual files
  let (mc_manifest, java_manifest) = McVersionStore::download_manifests(handle, &client).await?;

  let version_state = handle.state::<Mutex<McVersionStore>>();
  let mut version_store = version_state.lock().await;
  version_store.update_manifests(mc_manifest, java_manifest)?;
  drop(version_store);

  for loader in LoaderType::mod_loaders() {
    let data_dir = handle.path().app_data_dir()?;
    let version_path = MCVersionPath::new(&data_dir, "");
    loader.download_metadata(&client, &version_path).await?;
  }

  Ok(())
}
