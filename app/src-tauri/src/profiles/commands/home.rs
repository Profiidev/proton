use log::trace;
use tauri::{Result, State};
use tokio::sync::Mutex;

use crate::{
  profiles::{
    config::{PlayHistoryFavoriteInfo, QuickPlayInfo},
    store::ProfileStore,
  },
  utils::{log::ResultLogExt, updater::UpdateType},
};

#[tauri::command]
pub async fn profile_favorites_list(
  state: State<'_, Mutex<ProfileStore>>,
) -> Result<Vec<PlayHistoryFavoriteInfo>> {
  trace!("Command profile_favorites_list called");
  let mut store = state.lock().await;
  Ok(store.list_favorites().await.log()?)
}

#[tauri::command]
pub async fn profile_favorites_set(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  quick_play: Option<QuickPlayInfo>,
  favorite: bool,
) -> Result<()> {
  trace!(
    "Command profile_favorites_set called with profile {profile} quick_play {quick_play:?} favorite {favorite}"
  );
  let store = state.lock().await;

  let mut profile = store.profile(profile).await.log()?;
  profile.set_favorite(quick_play, favorite).await.log()?;
  profile.update(store.data_dir()).await.log()?;
  store.update_data(UpdateType::Profiles);
  store.update_data(UpdateType::ProfileQuickPlay);

  Ok(())
}

#[tauri::command]
pub async fn profile_history_list(
  state: State<'_, Mutex<ProfileStore>>,
) -> Result<Vec<PlayHistoryFavoriteInfo>> {
  trace!("Command profile_history_list called");
  let mut store = state.lock().await;
  Ok(store.list_history().await.log()?)
}
