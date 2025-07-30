use std::{fs, io::Cursor, path::PathBuf};

use anyhow::Result;
use chrono::Utc;
use image::{imageops::FilterType, ImageFormat};
use tauri::AppHandle;
use uuid::Uuid;

use crate::{
  path,
  profiles::{
    config::{LoaderType, Profile, ProfileError, ProfileInfo},
    watcher::watch_profile,
    PROFILE_CONFIG, PROFILE_DIR, PROFILE_IMAGE,
  },
  utils::file::write_file,
};

pub fn create_profile(
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

  fs::create_dir_all(&path)?;

  let stop = watch_profile(path.clone(), id.clone(), app.clone())?;

  write_file(&path!(&path, PROFILE_CONFIG), &profile)?;
  if let Some(icon) = icon {
    fs::write(&path!(&path, PROFILE_IMAGE), icon)?;
  }

  Ok((
    id,
    ProfileInfo {
      path: relative_path,
      watcher: stop,
    },
  ))
}
