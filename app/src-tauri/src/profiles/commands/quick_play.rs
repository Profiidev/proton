use log::trace;
use tauri::{Result, State};
use tokio::sync::Mutex;

use crate::{
  profiles::{config::QuickPlayInfo, store::ProfileStore},
  utils::{log::ResultLogExt, updater::UpdateType},
};

#[tauri::command]
pub async fn profile_quick_play_list(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
) -> Result<Vec<QuickPlayInfo>> {
  trace!("Command profile_quick_play_list called with profile {profile}");
  let store = state.lock().await;

  let mut profile = store.profile(profile).await.log()?;
  let (quick_play, updated) = profile.list_quick_play(store.data_dir()).await.log()?;

  if updated {
    profile.update(store.data_dir()).await.log()?;
    store.update_data(UpdateType::ProfileQuickPlay);
  }

  Ok(quick_play)
}

#[tauri::command]
pub async fn profile_quick_play_remove(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  quick_play: QuickPlayInfo,
) -> Result<()> {
  trace!(
    "Command profile_quick_play_remove called with profile {profile} quick_play {quick_play:?}"
  );
  let store = state.lock().await;

  let mut profile = store.profile(profile).await.log()?;
  profile.remove_quick_play(quick_play).await.log()?;
  profile.update(store.data_dir()).await.log()?;
  store.update_data(UpdateType::ProfileQuickPlay);

  Ok(())
}
