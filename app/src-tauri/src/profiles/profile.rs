use std::{io::Cursor, path::PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use image::{imageops::FilterType, ImageFormat};
use tauri::{AppHandle, Manager};
use tokio::fs;
use uuid::Uuid;

use crate::{
  path,
  profiles::{
    config::{LoaderType, Profile, ProfileError, ProfileInfo, QuickPlayInfo, QuickPlayType},
    watcher::watch_profile,
    PROFILE_CONFIG, PROFILE_DIR, PROFILE_IMAGE, PROFILE_LOGS, SAVES_DIR,
  },
  utils::{
    dir::list_dirs_in_dir,
    file::{read_parse_file, write_file},
    updater::{update_data, UpdateType},
  },
  versions::QUICK_PLAY,
};

impl Profile {
  pub fn relative_to_data(&self) -> PathBuf {
    path!(PROFILE_DIR, &self.id)
  }

  pub async fn create(
    data_dir: &PathBuf,
    app: &AppHandle,
    name: String,
    icon: Option<&[u8]>,
    version: String,
    loader: LoaderType,
    loader_version: Option<String>,
  ) -> Result<(String, ProfileInfo)> {
    let id = Uuid::new_v4().to_string();
    let relative_path = path!(PROFILE_DIR, &id);
    let path = path!(data_dir, &relative_path);

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
      favorite: false,
      history: false,
      quick_play: Vec::new(),
      version,
      loader,
      loader_version,
      downloaded: false,
      use_local_dev: false,
      use_local_game: false,
      use_local_jvm: false,
      game: None,
      jvm: None,
      dev: None,
    };

    fs::create_dir_all(&path).await?;

    let stop = watch_profile(path.clone(), id.clone(), app.clone())?;

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

  pub async fn update(&self, data_dir: &PathBuf, app: &AppHandle) -> Result<()> {
    write_file(
      &path!(data_dir, self.relative_to_data(), PROFILE_CONFIG),
      self,
    )
    .await?;

    update_data(app, UpdateType::Profiles);
    Ok(())
  }

  pub async fn set_favorite(
    &mut self,
    quick_play: Option<QuickPlayInfo>,
    favorite: bool,
    data_dir: &PathBuf,
    app: &AppHandle,
  ) -> Result<()> {
    if let Some(quick_play) = quick_play {
      if let Some(item) = self.quick_play.iter_mut().find(|q| *q == &quick_play) {
        item.favorite = favorite;
      }
    } else {
      self.favorite = favorite;
    }

    self.update(data_dir, app).await?;
    Ok(())
  }

  pub async fn list_saves(&self, data_dir: &PathBuf) -> Result<Vec<String>> {
    let saves_path = path!(data_dir, PROFILE_DIR, &self.id, SAVES_DIR);
    if !saves_path.exists() {
      return Ok(Vec::new());
    }

    Ok(list_dirs_in_dir(saves_path).await?)
  }

  pub async fn update_quick_play(&mut self, data_dir: &PathBuf, app: &AppHandle) -> Result<()> {
    let quick_play_path = path!(data_dir, self.relative_to_data(), QUICK_PLAY);

    let quick_plays: Vec<QuickPlayInfo> = read_parse_file(&quick_play_path).await?;

    for quick_play in quick_plays {
      let index = self.quick_play.iter().position(|q| q == &quick_play);

      if let Some(index) = index {
        self.quick_play[index].last_played_time = quick_play.last_played_time;
        self.quick_play[index].name = quick_play.name;
      } else {
        self.quick_play.push(quick_play);
      }
    }

    self.update(data_dir, app).await?;
    update_data(app, UpdateType::ProfileQuickPlay);

    Ok(())
  }

  pub async fn list_quick_play(
    &mut self,
    data_dir: &PathBuf,
    app: &AppHandle,
  ) -> Result<Vec<QuickPlayInfo>> {
    let saves = self.list_saves(data_dir).await?;

    let prev_len = self.quick_play.len();
    self
      .quick_play
      .retain(|q| saves.contains(&q.id) || q.r#type != QuickPlayType::Singleplayer);

    if self.quick_play.len() < prev_len {
      self.update(data_dir, app).await?;
      update_data(app, UpdateType::ProfileQuickPlay);
    }

    Ok(self.quick_play.clone())
  }

  pub async fn remove_quick_play(
    &mut self,
    quick_play: QuickPlayInfo,
    data_dir: &PathBuf,
    app: &AppHandle,
  ) -> Result<()> {
    let index = self.quick_play.iter().position(|q| q == &quick_play);

    if let Some(index) = index {
      let _ = self.quick_play.remove(index);
      self.update(data_dir, app).await?;
      update_data(app, UpdateType::ProfileQuickPlay);
    }

    Ok(())
  }
}

impl ProfileInfo {
  pub fn log_dir(handle: &AppHandle, profile: &str) -> Result<PathBuf> {
    Ok(path!(
      handle.path().app_data_dir()?,
      PROFILE_DIR,
      profile,
      PROFILE_LOGS
    ))
  }

  pub async fn get_icon(&self, data_dir: &PathBuf) -> Result<Option<Vec<u8>>> {
    let icon_path = path!(data_dir, &self.path, PROFILE_IMAGE);
    if !icon_path.exists() {
      return Ok(None);
    }
    let icon = fs::read(icon_path).await?;
    Ok(Some(icon))
  }

  pub async fn update_icon(&self, icon: &[u8], data_dir: &PathBuf, app: &AppHandle) -> Result<()> {
    if image::load_from_memory(icon).is_err() {
      return Err(ProfileError::InvalidImage.into());
    }

    fs::write(&path!(data_dir, &self.path, PROFILE_IMAGE), icon).await?;

    update_data(app, UpdateType::Profiles);

    Ok(())
  }

  pub async fn remove_profile(&self, data_dir: &PathBuf, app: &AppHandle) -> Result<()> {
    self.watcher.notify_waiters();
    fs::remove_dir_all(path!(data_dir, &self.path)).await?;
    update_data(app, UpdateType::Profiles);

    Ok(())
  }

  pub async fn profile(&self, data_dir: &PathBuf) -> Result<Profile> {
    read_parse_file(&path!(data_dir, &self.path, PROFILE_CONFIG)).await
  }

  pub async fn list_runs(&self, data_dir: &PathBuf) -> Result<Vec<DateTime<Utc>>> {
    let log_dir = path!(data_dir, &self.path, PROFILE_LOGS);
    if !log_dir.exists() {
      return Ok(Vec::new());
    }

    let mut res = Vec::new();
    let mut stream = fs::read_dir(log_dir).await?;
    while let Some(entry) = stream.next_entry().await? {
      if entry.file_type().await?.is_file() {
        if let Some(name) = entry.file_name().to_str() {
          // replace the last 3 dashes with colons but leave the rest of the name intact
          let name = name.trim_end_matches(".log").replace("-", ":");
          if let Ok(date) = DateTime::parse_from_str(&name, "%Y:%m:%dT%H:%M:%S.%f%:z") {
            res.push(date.to_utc());
          }
        }
      }
    }

    Ok(res)
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

    let log_file = log_dir.join(format!("{}.log", timestamp.to_rfc3339().replace(":", "-")));
    println!("Log file path: {:?}", log_file.to_str());
    if !log_file.exists() {
      return Ok(Vec::new());
    }

    let content = fs::read_to_string(log_file).await?;
    Ok(content.lines().map(String::from).collect())
  }
}
