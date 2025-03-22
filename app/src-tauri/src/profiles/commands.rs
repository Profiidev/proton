use tauri::{Result, State};
use tokio::sync::Mutex;

use super::store::{LoaderType, Profile, ProfileStore};

#[tauri::command]
pub async fn profile_create(
  state: State<'_, Mutex<ProfileStore>>,
  name: String,
  icon: Option<Vec<u8>>,
  version: String,
  loader: LoaderType,
  loader_version: Option<String>,
) -> Result<()> {
  let mut store = state.lock().await;
  store.create_profile(name, icon.as_deref(), version, loader, loader_version)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_update(state: State<'_, Mutex<ProfileStore>>, profile: Profile) -> Result<()> {
  let mut store = state.lock().await;
  store.update_profile(&profile)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_update_icon(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  icon: Vec<u8>,
) -> Result<()> {
  let mut store = state.lock().await;
  store.update_profile_icon(profile, &icon)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_remove(state: State<'_, Mutex<ProfileStore>>, profile: &str) -> Result<()> {
  let mut store = state.lock().await;
  store.remove_profile(profile)?;
  Ok(())
}

#[tauri::command]
pub async fn profile_list(state: State<'_, Mutex<ProfileStore>>) -> Result<Vec<Profile>> {
  let store = state.lock().await;
  Ok(store.list_profiles()?)
}
