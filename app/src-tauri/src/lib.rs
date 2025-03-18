use reqwest::Client;
use store::TauriAppStoreExt;

use account::{
  commands::{
    account_clear_skins, account_get_skin, account_list, account_login, account_refresh,
    account_refresh_one,
  },
  skin_store::SkinStore,
};
use tauri::Manager;
use tokio::sync::Mutex;

mod account;
mod store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_store::Builder::new().build())
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![
      account_login,
      account_refresh,
      account_refresh_one,
      account_list,
      account_clear_skins,
      account_get_skin
    ])
    .setup(|app| {
      let _ = app.handle().app_store()?;

      app.manage(Mutex::new(SkinStore::new(app.handle())));
      app.manage(Client::new());

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
