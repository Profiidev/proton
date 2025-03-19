use reqwest::Client;
use store::TauriAppStoreExt;

use account::{
  commands::{
    account_add_skin, account_get_active, account_get_cape, account_get_skin, account_list,
    account_list_skins, account_login, account_refresh, account_refresh_one, account_remove,
    account_remove_skin, account_set_active,
  },
  skin_store::SkinStore,
};
use tauri::Manager;
use tokio::sync::Mutex;

mod account;
mod macros;
mod store;
mod versions;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
      account_remove_skin
    ])
    .setup(|app| {
      let _ = app.handle().app_store()?;

      app.manage(Mutex::new(SkinStore::new(app.handle())?));
      app.manage(Client::new());

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
