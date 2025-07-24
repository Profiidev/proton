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
  instance_list, instance_logs, instance_stop, profile_create, profile_get_icon, profile_launch,
  profile_list, profile_open_path, profile_remove, profile_repair, profile_update,
  profile_update_icon,
};
use settings::{settings_get, settings_set};
use tauri::{AppHandle, Manager};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
use tokio::sync::Mutex;
use versions::{commands::version_list, store::McVersionStore};

mod account;
mod profiles;
mod settings;
mod store;
mod utils;
mod versions;

const CLIENT_ID: &str = "dd35660a-6381-41f8-bb34-2a36669581d0";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(
      tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::Stdout))
        .target(Target::new(TargetKind::LogDir {
          file_name: Some(Local::now().to_rfc3339().replace(":", "-")),
        }))
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
      //instances
      instance_list,
      instance_logs,
      instance_stop,
      //settings
      settings_get,
      settings_set,
    ])
    .setup(|app| {
      let _ = app.handle().app_store()?;

      app.manage(Mutex::new(SkinStore::new(app.handle().clone())?));
      app.manage(Mutex::new(AccountStore::new(app.handle().clone())?));
      app.manage(Mutex::new(ProfileStore::new(app.handle().clone())?));

      app.manage(Mutex::new(tauri::async_runtime::block_on(
        McVersionStore::new(app.handle().clone()),
      )?));

      let handle = app.handle().clone();
      app.manage(tauri::async_runtime::spawn(async move {
        if let Err(err) = async_setup_refresh(handle).await {
          log::error!("Error: {err}");
          std::process::exit(1);
        }
      }));

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

async fn async_setup_refresh(handle: AppHandle) -> Result<()> {
  let version_state = handle.state::<Mutex<McVersionStore>>();
  let mut version_store = version_state.lock().await;
  version_store.refresh_manifests().await?;

  Ok(())
}
