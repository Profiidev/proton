use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, Watcher};
use tokio::{select, sync::Notify};

use crate::{MANIFEST, PROFILE_CONFIG, config::Profile, store::ProfilesMsg};
use proton_utils::{
  file::{read_parse_file, write_file},
  future::UpdateLimiter,
  log::ErrorLogExt,
  path, rt,
};
use proton_versions::paths::QUICK_PLAY;

/// Watches a profile dir until the returned `Notify` is notified. Content changes become
/// [`ProfilesMsg::ContentChanged`], a changed quick play file is merged into the profile and
/// reported as [`ProfilesMsg::QuickPlayFileChanged`].
pub fn watch_profile(
  path: PathBuf,
  profile: String,
  tx: flume::Sender<ProfilesMsg>,
) -> Result<Arc<Notify>> {
  let (event_tx, event_rx) = flume::unbounded::<Event>();
  let mut watcher = RecommendedWatcher::new(
    move |res: notify::Result<Event>| {
      if let Ok(event) = res {
        // fails once the watch loop stopped, the remaining events are not needed
        let _ = event_tx.send(event);
      }
    },
    Config::default(),
  )?;
  watcher.watch(&path, notify::RecursiveMode::Recursive)?;

  let stop = Arc::new(Notify::new());
  let stop_ = stop.clone();

  rt::spawn(async move {
    let content_tx = tx.clone();
    let id = profile.clone();
    let content = UpdateLimiter::new(Duration::from_millis(500), move |()| {
      let _ = content_tx.send(ProfilesMsg::ContentChanged(id.clone()));
    });

    loop {
      let event = select! {
        event = event_rx.recv_async() => match event {
          Ok(event) => event,
          Err(_) => break,
        },
        _ = stop_.notified() => break,
      };

      if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
      ) {
        continue;
      }

      if event.paths.iter().any(|p| is_content(&path, p)) {
        content.call(());
      }

      if event.paths.iter().any(|p| p.ends_with(QUICK_PLAY))
        && merge_quick_play(&path).await.log_err().is_ok()
      {
        let _ = tx.send(ProfilesMsg::QuickPlayFileChanged(profile.clone()));
      }
    }
    // keep the watcher alive until here
    drop(watcher);
  });

  Ok(stop)
}

/// Merges the game's quick play file into the profile config inside `dir`
async fn merge_quick_play(dir: &Path) -> Result<()> {
  let config = path!(dir, PROFILE_CONFIG);
  let mut profile: Profile = read_parse_file(&config).await?;
  profile.merge_quick_play(read_parse_file(&path!(dir, QUICK_PLAY)).await?);
  write_file(&config, &profile).await
}

/// Changes in a content folder or the content manifest, temp downloads excluded
fn is_content(profile: &Path, path: &Path) -> bool {
  let Ok(rel) = path.strip_prefix(profile) else {
    return false;
  };
  let mut parts = rel.components().map(|c| c.as_os_str());
  match parts.next().and_then(|c| c.to_str()) {
    Some("mods" | "resourcepacks" | "shaderpacks") => true,
    Some(name) => name == MANIFEST,
    None => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    TMP_DIR,
    config::QuickPlayType,
    tests::{profile, quick_play},
  };

  async fn next(rx: &flume::Receiver<ProfilesMsg>) -> Option<ProfilesMsg> {
    tokio::time::timeout(Duration::from_secs(5), rx.recv_async())
      .await
      .ok()?
      .ok()
  }

  #[test]
  fn quick_play_file_is_merged_into_the_profile() {
    let dir = tempfile::tempdir().unwrap().keep();
    let config = path!(&dir, PROFILE_CONFIG);
    let (tx, rx) = flume::unbounded();
    rt::block_on(async {
      write_file(&config, &profile("p")).await.unwrap();
      let stop = watch_profile(dir.clone(), "p".into(), tx).unwrap();
      let entries = vec![quick_play("s", QuickPlayType::Multiplayer)];
      write_file(&path!(&dir, QUICK_PLAY), &entries)
        .await
        .unwrap();

      let msg = next(&rx).await;
      assert!(
        matches!(&msg, Some(ProfilesMsg::QuickPlayFileChanged(id)) if id == "p"),
        "{msg:?}"
      );
      let merged: Profile = read_parse_file(&config).await.unwrap();
      assert_eq!(merged.quick_play, entries);
      stop.notify_one();
    });
  }

  #[test]
  fn content_changes_are_throttled_and_stop_ends_the_watch() {
    let dir = tempfile::tempdir().unwrap().keep();
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    let (tx, rx) = flume::unbounded();
    rt::block_on(async {
      let stop = watch_profile(dir.clone(), "p".into(), tx).unwrap();
      // a burst of writes, the limiter folds it into few messages
      for i in 0..20 {
        tokio::fs::write(dir.join(format!("mods/{i}.jar")), "x")
          .await
          .unwrap();
      }
      let msg = next(&rx).await;
      assert!(
        matches!(&msg, Some(ProfilesMsg::ContentChanged(id)) if id == "p"),
        "{msg:?}"
      );
      tokio::time::sleep(Duration::from_millis(700)).await;
      assert!(rx.len() <= 1, "{} extra messages", rx.len());

      stop.notify_one();
      // once stopped the loop drops the sender
      while next(&rx).await.is_some() {}
      assert!(rx.is_disconnected());
    });
  }

  #[test]
  fn a_broken_quick_play_file_is_ignored() {
    let dir = tempfile::tempdir().unwrap().keep();
    let (tx, rx) = flume::unbounded();
    rt::block_on(async {
      write_file(&path!(&dir, PROFILE_CONFIG), &profile("p"))
        .await
        .unwrap();
      let _stop = watch_profile(dir.clone(), "p".into(), tx).unwrap();
      tokio::fs::write(dir.join(QUICK_PLAY), "{").await.unwrap();
      tokio::time::sleep(Duration::from_millis(300)).await;
      assert!(rx.is_empty());
    });
  }

  #[test]
  fn watching_a_missing_dir_fails() {
    let (tx, _rx) = flume::unbounded();
    let dir = tempfile::tempdir().unwrap();
    assert!(watch_profile(dir.path().join("missing"), "p".into(), tx).is_err());
  }

  #[test]
  fn detects_content_paths() {
    let p = Path::new("/data/profiles/a");
    assert!(is_content(p, &p.join("mods/x.jar")));
    assert!(is_content(p, &p.join("shaderpacks")));
    assert!(is_content(p, &p.join(MANIFEST)));
    assert!(!is_content(p, &p.join(TMP_DIR).join("1/x.jar")));
    assert!(!is_content(p, &p.join("config/iris.properties")));
    assert!(!is_content(Path::new("/other"), &p.join("mods/x.jar")));
  }
}
