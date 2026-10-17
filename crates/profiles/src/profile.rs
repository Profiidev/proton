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
  download::default_client,
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

  pub fn set_favorite(&mut self, quick_play: Option<QuickPlayInfo>, favorite: bool) {
    if let Some(quick_play) = quick_play {
      if let Some(item) = self.quick_play.iter_mut().find(|q| *q == &quick_play) {
        item.favorite = favorite;
      }
    } else {
      self.favorite = favorite;
    }
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

  pub fn remove_quick_play(&mut self, quick_play: QuickPlayInfo) {
    if let Some(index) = self.quick_play.iter().position(|q| q == &quick_play) {
      self.quick_play.remove(index);
    }
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
        let icon_hash = bytes_hash(quick_play.id.as_bytes());
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
        if let Ok(resp) = default_client()
          .get(&*proton_utils::http::url(&url))
          .send()
          .await
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
      if let Ok(date) = DateTime::parse_from_str(&name, "%Y:%m:%dT%H:%M:%S%.f%:z") {
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
  use std::{
    sync::Arc,
    time::{Duration as StdDuration, SystemTime},
  };

  use image::{GenericImageView, RgbaImage};
  use proton_utils::{rt, test_http::FakeServer};
  use tokio::sync::Notify;

  use super::*;
  use crate::tests::{profile, quick_play};

  const ICON_PATH: &str = "/api.mcstatus.io/v2/icon";

  fn png(w: u32, h: u32) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    RgbaImage::new(w, h)
      .write_to(&mut cursor, ImageFormat::Png)
      .unwrap();
    cursor.into_inner()
  }

  /// A profile written to `<dir>/profiles/<id>`
  async fn written(dir: &Path, id: &str) -> (Profile, ProfileInfo) {
    let profile = profile(id);
    let data_dir = dir.to_path_buf();
    fs::create_dir_all(path!(&data_dir, profile.relative_to_data()))
      .await
      .unwrap();
    profile.update(&data_dir).await.unwrap();
    let info = ProfileInfo {
      path: profile.relative_to_data(),
      watcher: Arc::new(Notify::new()),
    };
    (profile, info)
  }

  fn server_icon_path(dir: &Path, profile: &Profile, server: &str) -> PathBuf {
    path!(
      dir,
      profile.relative_to_data(),
      SERVER_ICON_DIR,
      format!("{}.png", bytes_hash(server.as_bytes()))
    )
  }

  #[test]
  fn create_writes_config_and_a_square_icon() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let (tx, _rx) = flume::unbounded();
    let (id, info) = rt::block_on(Profile::create(
      &data_dir,
      &tx,
      "Ünïcode".into(),
      Some(&png(10, 30)),
      "1.21".into(),
      LoaderType::Vanilla,
      // vanilla has no loader, the version is dropped
      Some("0.1".into()),
    ))
    .unwrap();

    assert_eq!(info.path, path!(PROFILE_DIR, &id));
    let profile = rt::block_on(info.profile(&data_dir)).unwrap();
    assert_eq!(
      (profile.id.as_str(), profile.name.as_str()),
      (id.as_str(), "Ünïcode")
    );
    assert_eq!(profile.loader_version, None);
    assert!(!profile.downloaded && !profile.favorite && profile.quick_play.is_empty());

    let icon = rt::block_on(info.get_icon(&data_dir)).unwrap().unwrap();
    assert_eq!(
      image::load_from_memory(&icon).unwrap().dimensions(),
      (256, 256)
    );
  }

  #[test]
  fn create_rejects_a_broken_icon_before_touching_disk() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let (tx, _rx) = flume::unbounded();
    let Err(err) = rt::block_on(Profile::create(
      &data_dir,
      &tx,
      "x".into(),
      Some(b"not a png"),
      "1.21".into(),
      LoaderType::Vanilla,
      None,
    )) else {
      panic!("a broken icon was accepted");
    };
    assert!(matches!(
      err.downcast_ref(),
      Some(ProfileError::InvalidImage)
    ));
    assert!(!data_dir.join(PROFILE_DIR).exists());
  }

  #[test]
  fn favorites_target_the_profile_or_one_quick_play_entry() {
    let mut p = profile("p");
    p.quick_play = vec![
      quick_play("w", QuickPlayType::Singleplayer),
      quick_play("w", QuickPlayType::Multiplayer),
    ];
    p.set_favorite(Some(quick_play("w", QuickPlayType::Multiplayer)), true);
    assert!(!p.favorite && !p.quick_play[0].favorite && p.quick_play[1].favorite);

    // an unknown entry changes nothing
    p.set_favorite(Some(quick_play("gone", QuickPlayType::Realms)), true);
    p.set_favorite(None, true);
    assert!(p.favorite);
    assert_eq!(
      p.quick_play.iter().map(|q| q.favorite).collect::<Vec<_>>(),
      [false, true]
    );
  }

  #[test]
  fn merge_quick_play_updates_known_entries_and_appends_new_ones() {
    let mut p = profile("p");
    let mut known = quick_play("w", QuickPlayType::Singleplayer);
    known.favorite = true;
    p.quick_play = vec![known];

    let mut renamed = quick_play("w", QuickPlayType::Singleplayer);
    renamed.name = "Renamed".into();
    let time = renamed.last_played_time;
    p.merge_quick_play(vec![renamed, quick_play("s", QuickPlayType::Multiplayer)]);

    assert_eq!(p.quick_play.len(), 2);
    // the user's flags survive, the game's name and time win
    assert!(p.quick_play[0].favorite);
    assert_eq!(p.quick_play[0].name, "Renamed");
    assert_eq!(p.quick_play[0].last_played_time, time);
    assert_eq!(p.quick_play[1].id, "s");

    p.remove_quick_play(quick_play("w", QuickPlayType::Singleplayer));
    p.remove_quick_play(quick_play("missing", QuickPlayType::Singleplayer));
    assert_eq!(p.quick_play, [quick_play("s", QuickPlayType::Multiplayer)]);
  }

  #[test]
  fn list_quick_play_drops_only_deleted_worlds() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let mut p = profile("p");
    p.quick_play = vec![
      quick_play("kept", QuickPlayType::Singleplayer),
      quick_play("deleted", QuickPlayType::Singleplayer),
      quick_play("server", QuickPlayType::Multiplayer),
      quick_play("realm", QuickPlayType::Realms),
    ];

    // no saves dir at all: every world counts as deleted
    let (list, pruned) = rt::block_on(p.clone().list_quick_play(&data_dir)).unwrap();
    assert!(pruned);
    assert_eq!(list.len(), 2);

    std::fs::create_dir_all(path!(&data_dir, p.relative_to_data(), SAVES_DIR, "kept")).unwrap();
    // a file named like a world is not a world
    std::fs::write(
      path!(&data_dir, p.relative_to_data(), SAVES_DIR, "deleted"),
      "",
    )
    .unwrap();
    let (list, pruned) = rt::block_on(p.list_quick_play(&data_dir)).unwrap();
    assert!(pruned);
    let ids: Vec<_> = list.iter().map(|q| q.id.as_str()).collect();
    assert_eq!(ids, ["kept", "server", "realm"]);

    // nothing left to prune
    assert!(!rt::block_on(p.list_quick_play(&data_dir)).unwrap().1);
  }

  #[test]
  fn singleplayer_icon_comes_from_the_world_folder() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let p = profile("p");
    let world = quick_play("w", QuickPlayType::Singleplayer);
    assert_eq!(
      rt::block_on(p.quick_play_icon(&data_dir, &world)).unwrap(),
      None
    );

    let world_dir = path!(&data_dir, p.relative_to_data(), SAVES_DIR, "w");
    std::fs::create_dir_all(&world_dir).unwrap();
    std::fs::write(world_dir.join(ICON_FILE), b"icon").unwrap();
    assert_eq!(
      rt::block_on(p.quick_play_icon(&data_dir, &world)).unwrap(),
      Some(b"icon".to_vec())
    );
    // realms have no icon source
    let realm = quick_play("w", QuickPlayType::Realms);
    assert_eq!(
      rt::block_on(p.quick_play_icon(&data_dir, &realm)).unwrap(),
      None
    );
  }

  #[test]
  fn server_icon_is_fetched_once_then_cached_for_an_hour() {
    let server = FakeServer::start();
    server.route(
      "GET",
      &format!("{ICON_PATH}/mc.example.org:25565"),
      200,
      "png",
    );
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let p = profile("p");
    let entry = quick_play("mc.example.org:25565", QuickPlayType::Multiplayer);

    for _ in 0..2 {
      assert_eq!(
        rt::block_on(p.quick_play_icon(&data_dir, &entry)).unwrap(),
        Some(b"png".to_vec())
      );
    }
    assert_eq!(server.calls().len(), 1);

    // stale cache is fetched again, the new answer replaces it
    let cached = server_icon_path(dir.path(), &p, &entry.id);
    std::fs::File::options()
      .write(true)
      .open(&cached)
      .unwrap()
      .set_modified(SystemTime::now() - StdDuration::from_secs(2 * 3600))
      .unwrap();
    server.route(
      "GET",
      &format!("{ICON_PATH}/mc.example.org:25565"),
      200,
      "new",
    );
    assert_eq!(
      rt::block_on(p.quick_play_icon(&data_dir, &entry)).unwrap(),
      Some(b"new".to_vec())
    );
    assert_eq!(std::fs::read(&cached).unwrap(), b"new");
    assert_eq!(server.calls().len(), 2);
  }

  #[test]
  fn server_icon_is_none_for_the_default_icon_or_a_failed_fetch() {
    let server = FakeServer::start();
    server.route("GET", &format!("{ICON_PATH}/default"), 200, DEFAULT_ICON);
    server.route("GET", &format!("{ICON_PATH}/broken"), 500, "png");
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let p = profile("p");

    let default = quick_play("default", QuickPlayType::Multiplayer);
    for _ in 0..2 {
      assert_eq!(
        rt::block_on(p.quick_play_icon(&data_dir, &default)).unwrap(),
        None
      );
    }
    // a cached default icon is fetched again, the server may have set one since
    assert_eq!(server.calls().len(), 2);
    assert_eq!(
      std::fs::read(server_icon_path(dir.path(), &p, "default")).unwrap(),
      DEFAULT_ICON
    );

    // an error body is neither returned nor cached
    let broken = quick_play("broken", QuickPlayType::Multiplayer);
    assert_eq!(
      rt::block_on(p.quick_play_icon(&data_dir, &broken)).unwrap(),
      None
    );
    assert!(!server_icon_path(dir.path(), &p, "broken").exists());
  }

  #[test]
  fn icons_are_validated_and_missing_ones_are_none() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    rt::block_on(async {
      let (_, info) = written(dir.path(), "p").await;
      assert_eq!(info.get_icon(&data_dir).await.unwrap(), None);

      let err = info.update_icon(b"nope", &data_dir).await.unwrap_err();
      assert!(matches!(
        err.downcast_ref(),
        Some(ProfileError::InvalidImage)
      ));
      assert_eq!(info.get_icon(&data_dir).await.unwrap(), None);

      let icon = png(4, 4);
      info.update_icon(&icon, &data_dir).await.unwrap();
      // stored as given, only create scales
      assert_eq!(info.get_icon(&data_dir).await.unwrap(), Some(icon));
    });
  }

  #[test]
  fn logs_read_clear_and_survive_missing_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    rt::block_on(async {
      let (p, info) = written(dir.path(), "p").await;
      let now = Utc::now();
      // no log dir yet
      assert!(info.logs(&data_dir, now).await.unwrap().is_empty());
      assert!(info.list_runs(&data_dir).await.unwrap().is_empty());
      info.clear_logs(&data_dir).await.unwrap();

      let log_dir = ProfileInfo::log_dir(dir.path(), &p.id);
      fs::create_dir_all(&log_dir).await.unwrap();
      fs::write(run_log_file(&log_dir, now), "a\nb\n")
        .await
        .unwrap();
      assert_eq!(info.logs(&data_dir, now).await.unwrap(), ["a", "b"]);
      // a run without a file
      let other = now - Duration::seconds(1);
      assert!(info.logs(&data_dir, other).await.unwrap().is_empty());
      assert_eq!(info.list_runs(&data_dir).await.unwrap(), [now]);

      info.clear_logs(&data_dir).await.unwrap();
      assert!(log_dir.is_dir());
      assert!(info.list_runs(&data_dir).await.unwrap().is_empty());
    });
  }

  #[test]
  fn remove_profile_stops_the_watcher_and_deletes_the_dir() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    rt::block_on(async {
      let (p, info) = written(dir.path(), "p").await;
      info.remove_profile(&data_dir).await.unwrap();
      assert!(!path!(&data_dir, p.relative_to_data()).exists());
      // the stop is kept as a permit for the watch loop
      tokio::time::timeout(StdDuration::from_secs(1), info.watcher.notified())
        .await
        .unwrap();
      // removing twice fails, the dir is gone
      assert!(info.remove_profile(&data_dir).await.is_err());
    });
  }

  #[test]
  fn list_saves_lists_only_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let p = profile("p");
    assert!(rt::block_on(p.list_saves(&data_dir)).unwrap().is_empty());
    let saves = path!(&data_dir, p.relative_to_data(), SAVES_DIR);
    std::fs::create_dir_all(saves.join("Wörld 1")).unwrap();
    std::fs::write(saves.join("file"), "").unwrap();
    assert_eq!(rt::block_on(p.list_saves(&data_dir)).unwrap(), ["Wörld 1"]);
  }

  #[tokio::test]
  async fn runs_roundtrip_through_file_names() {
    let dir = tempfile::tempdir().unwrap();
    // rfc3339 shortens the fraction to 0, 3, 6 or 9 digits, each must parse back the same
    let at = |nanos| DateTime::from_timestamp(1_800_000_000, nanos).unwrap();
    let times = [at(0), at(120_000_000), at(123_456_000), at(123_456_789)];
    for t in times {
      fs::write(run_log_file(dir.path(), t), "x").await.unwrap();
    }
    fs::write(dir.path().join("garbage.log"), "x")
      .await
      .unwrap();
    let mut newest_first = times;
    newest_first.reverse();
    assert_eq!(list_runs(dir.path()).await.unwrap(), newest_first);
    assert!(
      list_runs(&dir.path().join("missing"))
        .await
        .unwrap()
        .is_empty()
    );
  }
}
