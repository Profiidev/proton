use std::sync::Arc;

use reqwest::Client;
use tauri::{AppHandle, Manager, Result, State};
use tokio::sync::Mutex;

use crate::account::store::AccountStore;

use super::{
  launch::{launch_minecraft_version, LaunchArgs},
  store::McVersionStore,
};

#[tauri::command]
pub async fn versions_launch(
  state: State<'_, Mutex<McVersionStore>>,
  auth: State<'_, Mutex<AccountStore>>,
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  version: &str,
  account: &str,
) -> Result<()> {
  let store = state.lock().await;
  let auth_store = auth.lock().await;

  store
    .check_or_download(version, client.inner().clone(), &handle)
    .await?;

  let Some(info) = auth_store.launch_info(account) else {
    panic!("Error")
  };

  launch_minecraft_version(&LaunchArgs {
    access_token: info.access_token,
    launcher_name: handle.package_info().name.clone(),
    launcher_version: handle.package_info().version.to_string(),
    player_name: info.name,
    player_uuid: info.id,
    user_type: "msa".into(),
    data_dir: handle.path().app_data_dir()?,
    version: version.into(),
  })?;

  Ok(())
}
