use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};
use log::debug;
use serde::Serialize;
use tauri::{AppHandle, Emitter, async_runtime::spawn};
use thiserror::Error;
use tokio::{
  fs,
  io::{AsyncBufReadExt, BufReader},
  process::Child,
  select,
  sync::{Mutex, Notify},
};
use uuid::Uuid;

use crate::{
  profiles::config::{Profile, ProfileInfo},
  utils::{
    log::ResultLogExt,
    updater::{UpdateType, update_data},
  },
  versions::loader::LoaderType,
};

const CRASH_EVENT: &str = "instance-crash";

pub struct Instance {
  id: String,
  launched_at: DateTime<Utc>,
  profile_name: String,
  profile_id: String,
  version: String,
  loader: LoaderType,
  loader_version: Option<String>,
  stop_signal: Arc<Notify>,
  lines: Arc<Mutex<Vec<String>>>,
}

#[derive(Serialize)]
pub struct InstanceInfo {
  pub id: String,
  pub launched_at: DateTime<Utc>,
  pub profile_name: String,
  pub profile_id: String,
  pub version: String,
  pub loader: LoaderType,
  pub loader_version: Option<String>,
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
    profile: &Profile,
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
    let stop_signal = Arc::new(Notify::new());
    let launched_at = Utc::now();

    let id_ = id.clone();
    let profile_ = profile.id.clone();
    let profile_name = profile.name.clone();
    let lines_ = lines.clone();
    let instances_ = instances.clone();
    let notify = stop_signal.clone();
    let handle = app_handle.clone();

    spawn(async move {
      loop {
        let line = select! {
          Ok(Some(line)) = stdout.next_line() => line,
          Ok(Some(line)) = stderr.next_line() => line,
          _ = notify.notified() => {
            debug!("Stopping instance with profile {profile_} and id {id_}");
            let _ = child.kill().await.log();
            clean_instance(&handle, &instances_, &profile_, &id_, &lines_, launched_at).await;
            break;
          }
          exit = child.wait() => {
            debug!("Child with profile {profile_} and id {id_} exited");
            clean_instance(&handle, &instances_, &profile_, &id_, &lines_, launched_at).await;

            if let Ok(status) = exit && !status.success() {
              debug!("Child with profile {profile_} and id {id_} exited with status: {}", status);
              let _ = handle.emit(CRASH_EVENT, CrashInfo {
                profile_name,
              }).log();
            }
            break;
          }
          else => break
        };
        debug!("Profile: {}, id: {}, {}", profile_, id_, &line);
        lines_.lock().await.push(line);
        update_data(&handle, UpdateType::InstanceLogs);
      }
    });

    let instance = Instance {
      id,
      lines,
      stop_signal,
      launched_at,
      profile_name: profile.name.clone(),
      profile_id: profile.id.clone(),
      version: profile.version.clone(),
      loader: profile.loader,
      loader_version: profile.loader_version.clone(),
    };
    let mut instances = instances.lock().await;
    instances
      .entry(profile.id.clone())
      .or_default()
      .push(instance);
    update_data(app_handle, UpdateType::Instances);

    Ok(())
  }

  pub fn stop(&self) {
    debug!("Stopping instance with id {}", self.id);
    self.stop_signal.notify_waiters();
  }

  pub async fn lines(&self) -> Vec<String> {
    self.lines.lock().await.clone()
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  pub fn profile_name(&self) -> &str {
    &self.profile_name
  }

  pub fn profile_id(&self) -> &str {
    &self.profile_id
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn loader(&self) -> LoaderType {
    self.loader
  }

  pub fn loader_version(&self) -> Option<&String> {
    self.loader_version.as_ref()
  }

  pub fn launched_at(&self) -> DateTime<Utc> {
    self.launched_at
  }
}

async fn clean_instance(
  handle: &AppHandle,
  instances: &Arc<Mutex<HashMap<String, Vec<Instance>>>>,
  profile: &str,
  id: &str,
  lines: &Arc<Mutex<Vec<String>>>,
  launched_at: DateTime<Utc>,
) {
  let mut instances = instances.lock().await;
  if let Some(entry) = instances.get_mut(profile)
    && let Some(i) = entry.iter().position(|i| i.id() == id)
  {
    let _ = entry.swap_remove(i);
  }
  update_data(handle, UpdateType::Instances);

  if let Ok(logs_dir) = ProfileInfo::log_dir(handle, profile)
    && fs::create_dir_all(&logs_dir).await.is_ok()
  {
    let log_file = logs_dir.join(format!(
      "{}.log",
      launched_at.to_rfc3339().replace(":", "-")
    ));

    let lines = lines.lock().await;
    let content = lines.join("\n");
    let _ = fs::write(log_file, content).await.log();

    update_data(handle, UpdateType::ProfileLogs);
  }
}

#[derive(Serialize, Clone, Debug)]
struct CrashInfo {
  profile_name: String,
}
