use std::{
  io::Cursor,
  path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use image::{ImageFormat, imageops::FilterType};
use tokio::fs;
use tracing::debug;
use uuid::Uuid;

use crate::{
  PROFILE_CONFIG, PROFILE_DIR, PROFILE_IMAGE, PROFILE_LOGS, SAVES_DIR,
  config::{Profile, ProfileError, ProfileInfo, QuickPlayInfo, QuickPlayType},
  store::ProfilesMsg,
  watcher::watch_profile,
};
use proton_utils::path;
use proton_utils::{
  dir::list_dirs_in_dir,
  file::{bytes_hash, last_modified_ago, read_parse_file, write_file},
};
use proton_versions::{loader::LoaderType, paths::MCVersionPath};

const DEFAULT_ICON: &[u8] = include_bytes!("../assets/default_icon.png");
const ICON_FILE: &str = "icon.png";
const ICON_BASE_URL: &str = "https://api.mcstatus.io/v2/icon";
const SERVER_ICON_DIR: &str = "server_icons";

impl Profile {
  pub fn relative_to_data(&self) -> PathBuf {
    path!(PROFILE_DIR, &self.id)
  }

  pub async fn create(
    data_dir: &PathBuf,
    tx: &flume::Sender<ProfilesMsg>,
    name: String,
    icon: Option<&[u8]>,
    version: String,
    loader: LoaderType,
    loader_version: Option<String>,
  ) -> Result<(String, ProfileInfo)> {
    let id = Uuid::new_v4().to_string();
    let relative_path = path!(PROFILE_DIR, &id);
    let path = path!(data_dir, &relative_path);

    let loader_version = match loader.loader() {
      None => None,
      Some(_) if loader_version.is_some() => loader_version,
      Some(loader) => {
        let version_path = MCVersionPath::new(data_dir, &version);
        Some(
          loader
            .newest_loader_version_for_mc_version(&version, &version_path)
            .await?,
        )
      }
    };

    let icon = match icon {
      Some(icon) => {
        let Some(icon) = image::load_from_memory(icon).ok() else {
          return Err(ProfileError::InvalidImage.into());
        };

        let scaled = icon.resize_to_fill(256, 256, FilterType::Lanczos3);
        let mut cursor = Cursor::new(Vec::new());
        scaled.write_to(&mut cursor, ImageFormat::Png)?;
        Some(cursor.into_inner())
      }
      None => None,
    };

    let profile = Profile {
      id: id.clone(),
      name,
      created_at: Utc::now(),
      last_played: None,
      last_played_non_quick_play: None,
      favorite: false,
      quick_play: Vec::new(),
      version,
      loader,
      loader_version,
      downloaded: false,
      use_local_game: false,
      use_local_jvm: false,
      game: None,
      jvm: None,
      modpack: None,
    };

    fs::create_dir_all(&path).await?;

    let stop = watch_profile(path.clone(), id.clone(), tx.clone())?;

    write_file(&path!(&path, PROFILE_CONFIG), &profile).await?;
    if let Some(icon) = icon {
      fs::write(&path!(&path, PROFILE_IMAGE), icon).await?;
    }

    Ok((
      id,
      ProfileInfo {
        path: relative_path,
        watcher: stop,
      },
    ))
  }

  pub async fn update(&self, data_dir: &PathBuf) -> Result<()> {
    write_file(
      &path!(data_dir, self.relative_to_data(), PROFILE_CONFIG),
      self,
    )
    .await?;

    Ok(())
  }

  pub async fn set_favorite(
    &mut self,
    quick_play: Option<QuickPlayInfo>,
    favorite: bool,
  ) -> Result<()> {
    if let Some(quick_play) = quick_play {
      if let Some(item) = self.quick_play.iter_mut().find(|q| *q == &quick_play) {
        item.favorite = favorite;
      }
    } else {
      self.favorite = favorite;
    }

    Ok(())
  }

  pub async fn list_saves(&self, data_dir: &PathBuf) -> Result<Vec<String>> {
    let saves_path = path!(data_dir, PROFILE_DIR, &self.id, SAVES_DIR);
    if !saves_path.exists() {
      return Ok(Vec::new());
    }

    Ok(list_dirs_in_dir(saves_path).await?)
  }

  /// Takes names and play times from the game's quick play file, adds new entries
  pub fn merge_quick_play(&mut self, quick_plays: Vec<QuickPlayInfo>) {
    for quick_play in quick_plays {
      if let Some(item) = self.quick_play.iter_mut().find(|q| *q == &quick_play) {
        item.last_played_time = quick_play.last_played_time;
        item.name = quick_play.name;
      } else {
        self.quick_play.push(quick_play);
      }
    }
  }

  pub async fn list_quick_play(
    &mut self,
    data_dir: &PathBuf,
  ) -> Result<(Vec<QuickPlayInfo>, bool)> {
    let saves = self.list_saves(data_dir).await?;

    let prev_len = self.quick_play.len();
    self
      .quick_play
      .retain(|q| saves.contains(&q.id) || q.r#type != QuickPlayType::Singleplayer);

    Ok((self.quick_play.clone(), self.quick_play.len() < prev_len))
  }

  pub async fn remove_quick_play(&mut self, quick_play: QuickPlayInfo) -> Result<()> {
    let index = self.quick_play.iter().position(|q| q == &quick_play);

    if let Some(index) = index {
      let _ = self.quick_play.remove(index);
    }

    Ok(())
  }

  pub async fn quick_play_icon(
    &self,
    data_dir: &PathBuf,
    quick_play: &QuickPlayInfo,
  ) -> Result<Option<Vec<u8>>> {
    match quick_play.r#type {
      QuickPlayType::Singleplayer => {
        let icon_path = path!(
          data_dir,
          self.relative_to_data(),
          SAVES_DIR,
          &quick_play.id,
          ICON_FILE
        );

        if icon_path.exists() {
          return Ok(Some(fs::read(icon_path).await?));
        }
      }
      QuickPlayType::Multiplayer => {
        let dir = path!(data_dir, self.relative_to_data(), SERVER_ICON_DIR);
        if !dir.exists() {
          fs::create_dir_all(&dir).await?;
        }
        let icon_hash = bytes_hash(quick_play.id.as_bytes())?;
        let icon_path = path!(dir, format!("{}.png", icon_hash));

        // if the icon was fetched less than an hour ago, use the cached version
        if let Ok(Some(duration)) = last_modified_ago(&icon_path).await
          && duration < Duration::hours(1)
        {
          debug!("Using cached server icon for {}", quick_play.id);
          let icon = fs::read(&icon_path).await?;
          if icon != DEFAULT_ICON {
            return Ok(Some(icon));
          }
        }

        debug!("Fetching server icon for {}", quick_play.id);
        let url = format!("{}/{}", ICON_BASE_URL, quick_play.id);
        if let Ok(resp) = reqwest::get(&url).await
          && resp.status().is_success()
          && let Ok(bytes) = resp.bytes().await
        {
          let icon = bytes.to_vec();
          fs::write(&icon_path, &icon).await?;
          if icon != DEFAULT_ICON {
            return Ok(Some(icon));
          }
        }
      }
      _ => (),
    }

    debug!(
      "No icon found for {} type {:?}",
      quick_play.id, quick_play.r#type
    );

    Ok(None)
  }
}

impl ProfileInfo {
  pub fn log_dir(data_dir: &Path, profile: &str) -> PathBuf {
    path!(data_dir, PROFILE_DIR, profile, PROFILE_LOGS)
  }

  pub async fn get_icon(&self, data_dir: &PathBuf) -> Result<Option<Vec<u8>>> {
    let icon_path = path!(data_dir, &self.path, PROFILE_IMAGE);
    if !icon_path.exists() {
      return Ok(None);
    }
    let icon = fs::read(icon_path).await?;
    Ok(Some(icon))
  }

  pub async fn update_icon(&self, icon: &[u8], data_dir: &PathBuf) -> Result<()> {
    if image::load_from_memory(icon).is_err() {
      return Err(ProfileError::InvalidImage.into());
    }

    fs::write(&path!(data_dir, &self.path, PROFILE_IMAGE), icon).await?;

    Ok(())
  }

  pub async fn remove_profile(&self, data_dir: &PathBuf) -> Result<()> {
    // notify_one keeps a permit, so the stop is not lost while the watch loop is busy
    self.watcher.notify_one();
    fs::remove_dir_all(path!(data_dir, &self.path)).await?;

    Ok(())
  }

  pub async fn profile(&self, data_dir: &PathBuf) -> Result<Profile> {
    read_parse_file(&path!(data_dir, &self.path, PROFILE_CONFIG)).await
  }

  pub async fn list_runs(&self, data_dir: &PathBuf) -> Result<Vec<DateTime<Utc>>> {
    list_runs(&path!(data_dir, &self.path, PROFILE_LOGS)).await
  }

  pub async fn clear_logs(&self, data_dir: &PathBuf) -> Result<()> {
    let log_dir = path!(data_dir, &self.path, PROFILE_LOGS);
    if !log_dir.exists() {
      return Ok(());
    }

    fs::remove_dir_all(log_dir.clone()).await?;
    fs::create_dir_all(log_dir).await?;

    Ok(())
  }

  pub async fn logs(&self, data_dir: &PathBuf, timestamp: DateTime<Utc>) -> Result<Vec<String>> {
    let log_dir = path!(data_dir, &self.path, PROFILE_LOGS);
    if !log_dir.exists() {
      return Ok(Vec::new());
    }

    let log_file = run_log_file(&log_dir, timestamp);
    if !log_file.exists() {
      return Ok(Vec::new());
    }

    let content = fs::read_to_string(log_file).await?;
    Ok(content.lines().map(String::from).collect())
  }
}

/// Start times of the runs whose logs are stored in `log_dir`, newest first
pub async fn list_runs(log_dir: &Path) -> Result<Vec<DateTime<Utc>>> {
  if !log_dir.exists() {
    return Ok(Vec::new());
  }

  let mut res = Vec::new();
  let mut stream = fs::read_dir(log_dir).await?;
  while let Some(entry) = stream.next_entry().await? {
    if entry.file_type().await?.is_file()
      && let Some(name) = entry.file_name().to_str()
    {
      // the file name is the rfc3339 start time with `:` replaced by `-`
      let name = name.trim_end_matches(".log").replace("-", ":");
      if let Ok(date) = DateTime::parse_from_str(&name, "%Y:%m:%dT%H:%M:%S.%f%:z") {
        res.push(date.to_utc());
      }
    }
  }
  res.sort_unstable_by(|a, b| b.cmp(a));

  Ok(res)
}

/// Log file of the run started at `launched_at`
pub fn run_log_file(log_dir: &Path, launched_at: DateTime<Utc>) -> PathBuf {
  log_dir.join(format!(
    "{}.log",
    launched_at.to_rfc3339().replace(":", "-")
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn runs_roundtrip_through_file_names() {
    let dir = tempfile::tempdir().unwrap();
    let older = Utc::now() - Duration::hours(1);
    let newer = Utc::now();
    for t in [older, newer] {
      fs::write(run_log_file(dir.path(), t), "x").await.unwrap();
    }
    fs::write(dir.path().join("garbage.log"), "x")
      .await
      .unwrap();
    assert_eq!(list_runs(dir.path()).await.unwrap(), [newer, older]);
    assert!(
      list_runs(&dir.path().join("missing"))
        .await
        .unwrap()
        .is_empty()
    );
  }
}
