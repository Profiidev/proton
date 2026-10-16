use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, Watcher};
use tauri::{
  AppHandle, Manager,
  async_runtime::{Receiver, block_on, channel, spawn},
};
use tokio::{
  select,
  sync::{Mutex, Notify},
};

use crate::MANIFEST;
use crate::store::ProfileStore;
use proton_utils::{
  future::UpdateLimiter,
  log::ErrorLogExt,
  tauri_compat::{UpdateType, update_data},
};
use proton_versions::paths::QUICK_PLAY;

fn async_watcher(config: Config) -> notify::Result<(RecommendedWatcher, Receiver<Event>)> {
  let (tx, rx) = channel(10);

  let watcher = RecommendedWatcher::new(
    move |res| {
      block_on(async {
        if let Ok(event) = res {
          // fails once the watch loop stopped, the remaining events are not needed
          let _ = tx.send(event).await;
        }
      })
    },
    config,
  )?;

  Ok((watcher, rx))
}

pub fn watch_profile(path: PathBuf, profile: String, app: AppHandle) -> Result<Arc<Notify>> {
  let config = Config::default();
  let (mut watcher, mut rx) = async_watcher(config)?;

  watcher.watch(&path, notify::RecursiveMode::Recursive)?;

  let stop = Arc::new(Notify::new());
  let stop_clone = stop.clone();
  let data_dir = app.path().app_data_dir()?;
  let handle = app.clone();
  let content = UpdateLimiter::new(Duration::from_millis(500), move |()| {
    update_data(&handle, UpdateType::ProfileContent)
  });

  spawn(async move {
    loop {
      let event = select! {
        event = rx.recv() => {
          match event {
            Some(event) => event,
            None => {
              println!("Watcher channel closed, stopping watcher.");
              break;
            },
          }
        },
        _ = stop_clone.notified() => break,
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

      if event.paths.iter().any(|p| p.ends_with(QUICK_PLAY)) {
        let store = app.state::<Mutex<ProfileStore>>();
        let store = store.lock().await;
        if let Ok(mut info) = store.profile(&profile).await.log_err() {
          let _ = info.update_quick_play(&data_dir).await.log_err();
          let _ = info.update(&data_dir).await.log_err();
          store.update_data(UpdateType::ProfileQuickPlay);
        }
      }
    }
    // keep the watcher alive until here
    drop(watcher);
  });

  Ok(stop)
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
  use crate::TMP_DIR;

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
