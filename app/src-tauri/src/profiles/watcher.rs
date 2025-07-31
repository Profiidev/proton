use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, Watcher};
use tauri::{
  async_runtime::{block_on, channel, spawn, Receiver},
  AppHandle, Manager,
};
use tokio::{
  select,
  sync::{Mutex, Notify},
};

use crate::{profiles::store::ProfileStore, utils::log::ResultLogExt, versions::QUICK_PLAY};

fn async_watcher(config: Config) -> notify::Result<(RecommendedWatcher, Receiver<Event>)> {
  let (tx, rx) = channel(10);

  let watcher = RecommendedWatcher::new(
    move |res| {
      block_on(async {
        if let Ok(event) = res {
          let _ = tx.send(event).await.log();
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

      if let EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) = event.kind {
        if event.paths.iter().any(|p| p.ends_with(QUICK_PLAY)) {
          let store = app.state::<Mutex<ProfileStore>>();
          let store = store.lock().await;
          if let Ok(mut info) = store.profile(&profile).await.log() {
            let _ = info.update_quick_play(&data_dir, &app).await.log();
          }
        }
      }
    }
    // keep the watcher alive until here
    drop(watcher);
  });

  Ok(stop)
}
