use std::time::Duration;

use log::trace;
use serde::Serialize;
use tauri::{Result, State, Url};
use tokio::sync::Mutex;

use crate::{
  offline::OfflineResultExt,
  profiles::{config::QuickPlayInfo, store::ProfileStore},
  utils::{
    log::ResultLogExt,
    updater::{UpdateType, default_client},
  },
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

#[tauri::command]
pub async fn profile_quick_play_icon(
  state: State<'_, Mutex<ProfileStore>>,
  profile: &str,
  quick_play: QuickPlayInfo,
) -> Result<Option<String>> {
  trace!("Command profile_quick_play_icon called with profile {profile} quick_play {quick_play:?}");
  let store = state.lock().await;

  let profile = store.profile(profile).await.log()?;
  let handle = store.handle().clone();
  let data_dir = store.data_dir().clone();

  drop(store);

  let icon = profile
    .quick_play_icon(&data_dir, &quick_play)
    .await
    .check_online_state(&handle)
    .await?;

  Ok(icon)
}

const STATUS_BASE_URL: &str = "https://api.mcstatus.io/v2/status/java";

#[derive(Serialize, Debug, PartialEq)]
pub struct ServerStatus {
  online: bool,
  players_online: u32,
  players_max: u32,
}

#[tauri::command]
pub async fn quick_play_server_status(address: &str) -> Result<Option<ServerStatus>> {
  trace!("Command quick_play_server_status called with address {address}");
  let Ok(mut url) = Url::parse(STATUS_BASE_URL) else {
    return Ok(None);
  };
  // push encodes the address, so `/`, `?` or `#` can not change the request target
  if let Ok(mut segments) = url.path_segments_mut() {
    segments.push(address);
  }
  let Ok(resp) = default_client()
    .get(url)
    .timeout(Duration::from_secs(5))
    .send()
    .await
  else {
    return Ok(None);
  };
  Ok(resp.text().await.ok().and_then(|t| parse_server_status(&t)))
}

fn parse_server_status(json: &str) -> Option<ServerStatus> {
  let value: serde_json::Value = serde_json::from_str(json).ok()?;
  let count = |key: &str| value["players"][key].as_u64().unwrap_or(0) as u32;
  Some(ServerStatus {
    online: value["online"].as_bool()?,
    players_online: count("online"),
    players_max: count("max"),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_server_status_handles_online_offline_and_garbage() {
    assert_eq!(
      parse_server_status(r#"{"online":true,"players":{"online":5,"max":20,"list":[]}}"#),
      Some(ServerStatus {
        online: true,
        players_online: 5,
        players_max: 20
      })
    );
    assert_eq!(
      parse_server_status(r#"{"online":false,"host":"x"}"#),
      Some(ServerStatus {
        online: false,
        players_online: 0,
        players_max: 0
      })
    );
    assert_eq!(parse_server_status("not json"), None);
    assert_eq!(parse_server_status(r#"{"error":"bad"}"#), None);
  }
}
