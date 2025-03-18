use reqwest::Client;
use store::TauriAppStoreExt;

use account::commands::{account_list, account_login, account_refresh, account_refresh_one};

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
    ])
    .manage(Client::new())
    .setup(|app| {
      let _ = app.handle().app_store()?;

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
