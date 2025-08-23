use anyhow::Result;
use chrono::Local;
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
  instance_list, instance_logs, instance_stop, profile_clear_logs, profile_create,
  profile_favorites_list, profile_favorites_set, profile_get_icon, profile_history_list,
  profile_launch, profile_list, profile_logs, profile_open_path, profile_quick_play_icon,
  profile_quick_play_list, profile_quick_play_remove, profile_remove, profile_repair,
  profile_runs_list, profile_update, profile_update_icon,
};
use settings::{settings_get, settings_set};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
use tokio::sync::Mutex;
use versions::{
  commands::{loader_version_list, version_list},
  store::McVersionStore,
};

use crate::{
  offline::{MANIFEST_REFRESH_ERROR, OfflineState, is_offline, try_reconnect},
  settings::MaxMem,
  utils::{log::ResultLogExt, updater::default_client},
  versions::{loader::LoaderType, paths::MCVersionPath},
};

mod account;
mod offline;
mod profiles;
mod settings;
mod store;
mod utils;
mod versions;

const CLIENT_ID: &str = "dd35660a-6381-41f8-bb34-2a36669581d0";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_process::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_window_state::Builder::new().build())
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
      profile_launch,
      profile_repair,
      profile_runs_list,
      profile_clear_logs,
      profile_logs,
      profile_quick_play_list,
      profile_quick_play_remove,
      profile_quick_play_icon,
      //home
      profile_favorites_set,
      profile_favorites_list,
      profile_history_list,
      //instances
      instance_list,
      instance_logs,
      instance_stop,
      //settings
      settings_get,
      settings_set,
      //offline
      is_offline,
      try_reconnect,
    ])
    .setup(|app| {
      let _ = app.handle().app_store()?;

      app.manage(Mutex::new(SkinStore::new(app.handle().clone())?));
      app.manage(Mutex::new(AccountStore::new(app.handle().clone())?));
      app.manage(Mutex::new(ProfileStore::new(app.handle().clone())?));
      app.manage(Mutex::new(OfflineState::new(app.handle().clone())));

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
      }));

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

async fn async_online_check(handle: &AppHandle) -> Result<()> {
  let offline_state = handle.state::<Mutex<OfflineState>>();
  let mut state = offline_state.lock().await;
  if !state.check_online_state().await {
    return Err(anyhow::anyhow!("Offline state detected"));
  }
  drop(state);

  async_setup_refresh(handle).await?;

  let mut state = offline_state.lock().await;
  state.state_init();

  Ok(())
}

async fn async_setup_refresh(handle: &AppHandle) -> Result<()> {
  let version_state = handle.state::<Mutex<McVersionStore>>();
  let mut version_store = version_state.lock().await;
  version_store.refresh_manifests().await?;
  drop(version_store);

  let client = default_client();
  for loader in LoaderType::mod_loaders() {
    let data_dir = handle.path().app_data_dir()?;
    let version_path = MCVersionPath::new(&data_dir, "");
    loader.download_metadata(&client, &version_path).await?;
  }

  Ok(())
}
