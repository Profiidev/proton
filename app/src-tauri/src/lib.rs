use tauri::{AppHandle, Url, WebviewWindowBuilder};

#[tauri::command]
fn auth(handle: AppHandle) {
  WebviewWindowBuilder::new(
    &handle,
    "auth",
    tauri::WebviewUrl::External(
      Url::parse_with_params(
        "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize",
        vec![
          ("client_id", "dd35660a-6381-41f8-bb34-2a36669581d0"),
          ("response_type", "code"),
          ("scope", "XboxLive.signin offline_access"),
        ],
      )
      .unwrap(),
    ),
  )
  .inner_size(600.0, 600.0)
  .build()
  .unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![auth])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
