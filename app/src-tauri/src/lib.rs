use mc_auth::get_minecraft_token;
use reqwest::Client;
use store::TauriAppStoreExt;
use tauri::AppHandle;

mod mc_auth;
mod store;

#[tauri::command]
async fn auth(handle: AppHandle) {
  let token = get_minecraft_token(handle, &Client::new()).await;
  let _ = dbg!(token);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_store::Builder::new().build())
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![auth])
    .setup(|app| {
      let _ = app.handle().app_store()?;

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
