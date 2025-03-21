use std::sync::Arc;

use log::trace;
use reqwest::Client;
use tauri::{AppHandle, Manager, Result, State};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{account::store::AccountStore, utils::log::ResultLogExt};

use super::{
  launch::{launch_minecraft_version, LaunchArgs},
  store::McVersionStore,
};

#[derive(Error, Debug)]
enum LaunchError {
  #[error("No Account found")]
  NoAccountFound,
}

#[tauri::command]
pub async fn versions_launch(
  state: State<'_, Mutex<McVersionStore>>,
  auth: State<'_, Mutex<AccountStore>>,
  client: State<'_, Arc<Client>>,
  handle: AppHandle,
  version: &str,
  account: &str,
) -> Result<()> {
  trace!("Command version_launch called");
  let store = state.lock().await;
  let auth_store = auth.lock().await;

  store
    .check_or_download(version, client.inner().clone(), &handle)
    .await
    .log()?;

  let Some(info) = auth_store.launch_info(account) else {
    let err: anyhow::Result<()> = Err(LaunchError::NoAccountFound.into()).log();
    return Ok(err?);
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
  })
  .log()?;

  Ok(())
}
