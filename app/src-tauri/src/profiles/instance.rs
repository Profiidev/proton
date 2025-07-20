use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use log::debug;
use serde::Serialize;
use tauri::{async_runtime::spawn, AppHandle};
use thiserror::Error;
use tokio::{
  io::{AsyncBufReadExt, BufReader},
  process::Child,
  select,
  sync::Mutex,
};
use uuid::Uuid;

use crate::utils::updater::{update_data, UpdateType};

pub struct Instance {
  id: String,
  lines: Arc<Mutex<Vec<String>>>,
}

#[derive(Serialize)]
pub struct InstanceInfo {
  pub id: String,
}

#[derive(Error, Debug)]
pub enum InstanceError {
  #[error("Invalid Process State")]
  InvalidProcessState,
  #[error("Not Found")]
  NotFound,
}

impl Instance {
  pub async fn create(
    mut child: Child,
    app_handle: &AppHandle,
    profile: String,
    instances: &Arc<Mutex<HashMap<String, Vec<Instance>>>>,
  ) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    debug!(
      "Adding child instance with pid {} and id {}",
      child.id().ok_or(InstanceError::InvalidProcessState)?,
      id
    );

    let stdout = child
      .stdout
      .take()
      .ok_or(InstanceError::InvalidProcessState)?;
    let stderr = child
      .stderr
      .take()
      .ok_or(InstanceError::InvalidProcessState)?;
    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = BufReader::new(stderr).lines();

    let lines = Arc::new(Mutex::new(Vec::new()));

    let id_ = id.clone();
    let profile_ = profile.clone();
    let lines_ = lines.clone();
    let instances_ = instances.clone();
    let handle = app_handle.clone();
    spawn(async move {
      loop {
        let line = select! {
          Ok(Some(line)) = stdout.next_line() => line,
          Ok(Some(line)) = stderr.next_line() => line,
          _ = child.wait() => {
            debug!("Child with profile {profile_} and id {id_} exited");
            let mut instances = instances_.lock().await;
            let entry = instances.entry(profile_).or_default();
            if let Some(i) = entry.iter().position(|i| i.id == id_) {
              let _ = entry.swap_remove(i);
            }
            update_data(&handle, UpdateType::Instances);
            break;
          }
          else => break
        };
        debug!("Profile: {}, id: {}, {}", profile_, id_, &line);
        lines_.lock().await.push(line);
        update_data(&handle, UpdateType::InstanceLogs);
      }
    });

    let instance = Instance { id, lines };
    let mut instances = instances.lock().await;
    instances.entry(profile).or_default().push(instance);
    update_data(app_handle, UpdateType::Instances);

    Ok(())
  }

  pub async fn lines(&self) -> Vec<String> {
    self.lines.lock().await.clone()
  }

  pub fn id(&self) -> &str {
    &self.id
  }
}
