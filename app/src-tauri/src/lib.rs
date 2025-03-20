use anyhow::Result;
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
use tauri::{AppHandle, Manager};
use tokio::{join, sync::Mutex};
use versions::{commands::versions_download, store::McVersionStore};

mod account;
mod macros;
mod store;
mod updater;
mod versions;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  env_logger::init();

  tauri::Builder::default()
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
      versions_download,
    ])
    .setup(|app| {
      let _ = app.handle().app_store()?;

      app.manage(Mutex::new(SkinStore::new(app.handle())?));
      app.manage(Mutex::new(AccountStore::new(app.handle())?));
      app.manage(Client::new());

      let handle = app.handle().clone();
      tauri::async_runtime::spawn(async move {
        async_setup(handle)
          .await
          .expect("Failed to init async state")
      });

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

async fn async_setup(handle: AppHandle) -> Result<()> {
  let client = Client::new();
  let (mc_version_store,) = join!(McVersionStore::new(&client));

  let store = mc_version_store?;
  store
    .check_or_download("1.21.4", &client, &handle)
    .await
    .unwrap();

  handle.manage(store);

  Ok(())
}
