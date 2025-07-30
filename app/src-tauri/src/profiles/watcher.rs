use std::{path::PathBuf, sync::Arc};

use notify::{Config, Event, EventKind, RecommendedWatcher, Watcher};
use tauri::{
  async_runtime::{block_on, channel, spawn, Receiver},
  AppHandle,
};
use tokio::{select, sync::Notify};

use crate::{
  utils::updater::{update_data, UpdateType},
  versions::QUICK_PLAY,
};

fn async_watcher(config: Config) -> notify::Result<(RecommendedWatcher, Receiver<Event>)> {
  let (tx, rx) = channel(10);

  let watcher = RecommendedWatcher::new(
    move |res| {
      block_on(async {
        if let Ok(event) = res {
          let _ = tx.send(event).await;
        }
      })
    },
    config,
  )?;

  Ok((watcher, rx))
}

pub fn watch_profile(path: PathBuf, app: AppHandle) -> notify::Result<Arc<Notify>> {
  let config = Config::default();
  let (mut watcher, mut rx) = async_watcher(config)?;

  watcher.watch(&path, notify::RecursiveMode::Recursive)?;

  let stop = Arc::new(Notify::new());
  let stop_clone = stop.clone();

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
          update_data(&app, UpdateType::ProfileQuickPlay);
        }
      }
    }
    // keep the watcher alive until here
    drop(watcher);
  });

  Ok(stop)
}
