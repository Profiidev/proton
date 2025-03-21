use std::sync::Arc;

use anyhow::Result;
use chrono::Local;
use reqwest::Client;
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
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
use tokio::{join, sync::Mutex};
use versions::{commands::versions_launch, store::McVersionStore};

mod account;
mod store;
mod utils;
mod versions;

const CLIENT_ID: &str = "dd35660a-6381-41f8-bb34-2a36669581d0";

const ASYNC_STATE_LOADED_EVENT: &str = "async-state-loaded";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(
      tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::Stdout))
        .target(Target::new(TargetKind::LogDir {
          file_name: Some(Local::now().to_rfc3339()),
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
      versions_launch,
    ])
    .setup(|app| {
      let _ = app.handle().app_store()?;

      app.manage(Mutex::new(SkinStore::new(app.handle())?));
      app.manage(Mutex::new(AccountStore::new(app.handle())?));
      app.manage(Arc::new(Client::new()));

      let handle = app.handle().clone();
      let handle = tauri::async_runtime::spawn(async move {
        if async_setup(handle).await.is_err() {
          std::process::exit(0)
        }
      });
      app.manage(handle);

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

async fn async_setup(handle: AppHandle) -> Result<()> {
  let client = Client::new();
  let (mc_version_store,) = join!(McVersionStore::new(&client));

  handle.manage(Mutex::new(mc_version_store?));
  handle.emit(ASYNC_STATE_LOADED_EVENT, ())?;

  Ok(())
}
