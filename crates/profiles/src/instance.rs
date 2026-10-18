use std::{
  collections::HashMap,
  mem,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use thiserror::Error;
use tokio::{
  fs,
  io::{AsyncBufRead, AsyncBufReadExt, BufReader, Lines},
  process::Child,
  select,
  sync::{Mutex, Notify},
  time::{MissedTickBehavior, interval, timeout},
};
use tracing::{debug, trace};
use uuid::Uuid;

use crate::{
  config::{Profile, ProfileInfo},
  profile::{list_runs, run_log_file},
  store::ProfilesMsg,
};
use proton_utils::{log::ErrorLogExt, rt};
use proton_versions::loader::LoaderType;

/// Log lines reach the UI at most this often, one notify per batch
const LOG_BATCH: Duration = Duration::from_millis(100);

pub type Instances = Arc<Mutex<HashMap<String, Vec<Instance>>>>;

pub struct Instance {
  info: InstanceInfo,
  pid: u32,
  stop_signal: Arc<Notify>,
}

#[derive(Clone, Debug)]
pub struct InstanceInfo {
  pub id: String,
  pub launched_at: DateTime<Utc>,
  pub profile_name: String,
  pub profile_id: String,
  pub version: String,
  pub loader: LoaderType,
  pub account_name: String,
  /// `-Xmx` in MB the game was launched with
  pub mem_max: u64,
}

#[derive(Error, Debug)]
pub enum InstanceError {
  #[error("Game process has no pid or output")]
  InvalidProcessState,
  #[error("Instance not found")]
  NotFound,
}

impl Instance {
  /// Registers a running game and pumps its output to the gpui side until it exits. On exit the
  /// log is saved to the profile's log dir.
  pub async fn create(
    mut child: Child,
    data_dir: &Path,
    profile: &Profile,
    account_name: String,
    mem_max: u64,
    instances: &Instances,
    tx: &flume::Sender<ProfilesMsg>,
  ) -> Result<()> {
    let pid = child.id().ok_or(InstanceError::InvalidProcessState)?;
    let stdout = child
      .stdout
      .take()
      .ok_or(InstanceError::InvalidProcessState)?;
    let stderr = child
      .stderr
      .take()
      .ok_or(InstanceError::InvalidProcessState)?;

    let info = InstanceInfo {
      id: Uuid::new_v4().to_string(),
      launched_at: Utc::now(),
      profile_name: profile.name.clone(),
      profile_id: profile.id.clone(),
      version: profile.version.clone(),
      loader: profile.loader,
      account_name,
      mem_max,
    };
    debug!("Adding child instance with pid {pid} and id {}", info.id);
    let stop_signal = Arc::new(Notify::new());

    // registered before the pump starts, so InstanceAdded always comes before InstanceRemoved
    instances
      .lock()
      .await
      .entry(profile.id.clone())
      .or_default()
      .push(Instance {
        info: info.clone(),
        pid,
        stop_signal: stop_signal.clone(),
      });
    let _ = tx.send(ProfilesMsg::InstanceAdded(info.clone()));

    let pump = Pump {
      info,
      log_dir: ProfileInfo::log_dir(data_dir, &profile.id),
      instances: instances.clone(),
      tx: tx.clone(),
    };
    rt::spawn(pump.run(
      child,
      BufReader::new(stdout).lines(),
      BufReader::new(stderr).lines(),
      stop_signal,
    ));

    Ok(())
  }

  pub fn stop(&self) {
    debug!("Stopping instance with id {}", self.info.id);
    // notify_one stores a permit, so a stop sent while the task is outside select! is not lost
    self.stop_signal.notify_one();
  }

  pub fn info(&self) -> &InstanceInfo {
    &self.info
  }

  pub fn pid(&self) -> u32 {
    self.pid
  }
}

struct Pump {
  info: InstanceInfo,
  log_dir: PathBuf,
  instances: Instances,
  tx: flume::Sender<ProfilesMsg>,
}

impl Pump {
  async fn run<O, E>(
    self,
    mut child: Child,
    mut stdout: Lines<O>,
    mut stderr: Lines<E>,
    stop: Arc<Notify>,
  ) where
    O: AsyncBufRead + Unpin,
    E: AsyncBufRead + Unpin,
  {
    let id = self.info.id.clone();
    let mut all = Vec::new();
    let mut pending = Vec::new();
    let mut tick = interval(LOG_BATCH);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let status = loop {
      select! {
        Ok(Some(line)) = stdout.next_line() => pending.push(line),
        Ok(Some(line)) = stderr.next_line() => pending.push(line),
        _ = tick.tick() => self.flush(&mut all, &mut pending),
        _ = stop.notified() => {
          debug!("Stopping instance {id}");
          let _ = child.kill().await.log_err();
          break None;
        }
        exit = child.wait() => {
          debug!("Instance {id} exited with {exit:?}");
          // the pipes may still hold the last lines
          drain(&mut stdout, &mut pending).await;
          drain(&mut stderr, &mut pending).await;
          break exit.ok();
        }
      }
    };
    self.flush(&mut all, &mut pending);
    self.finish(status.is_some_and(|s| !s.success()), all).await;
  }

  fn flush(&self, all: &mut Vec<String>, pending: &mut Vec<String>) {
    if pending.is_empty() {
      return;
    }
    trace!("Instance {}: {} new lines", self.info.id, pending.len());
    all.extend(pending.iter().cloned());
    let _ = self.tx.send(ProfilesMsg::InstanceLines(
      self.info.id.clone(),
      mem::take(pending),
    ));
  }

  async fn finish(self, crashed: bool, lines: Vec<String>) {
    let Pump {
      info,
      log_dir,
      instances,
      tx,
    } = self;

    if let Some(entry) = instances.lock().await.get_mut(&info.profile_id)
      && let Some(i) = entry.iter().position(|i| i.info.id == info.id)
    {
      entry.swap_remove(i);
    }
    let _ = tx.send(ProfilesMsg::InstanceRemoved(info.id.clone()));

    if crashed {
      let _ = tx.send(ProfilesMsg::InstanceCrashed {
        profile_name: info.profile_name.clone(),
      });
    }

    if fs::create_dir_all(&log_dir).await.log_err().is_ok() {
      let file = run_log_file(&log_dir, info.launched_at);
      let _ = fs::write(file, lines.join("\n")).await.log_err();
      if let Ok(runs) = list_runs(&log_dir).await.log_err() {
        let _ = tx.send(ProfilesMsg::Runs(info.profile_id, runs));
      }
    }
  }
}

/// Reads what is left in a pipe, bounded in case a child process keeps it open
async fn drain<R: AsyncBufRead + Unpin>(lines: &mut Lines<R>, out: &mut Vec<String>) {
  let _ = timeout(Duration::from_secs(1), async {
    while let Ok(Some(line)) = lines.next_line().await {
      out.push(line);
    }
  })
  .await;
}

#[cfg(test)]
mod tests {
  use std::time::Instant;

  use super::*;
  use crate::tests::{profile, sh};

  /// Messages until the run list arrives, which is the last thing an instance sends
  async fn until_runs(rx: &flume::Receiver<ProfilesMsg>) -> Vec<ProfilesMsg> {
    let mut msgs = Vec::new();
    while let Ok(Ok(msg)) = timeout(Duration::from_secs(10), rx.recv_async()).await {
      let done = matches!(msg, ProfilesMsg::Runs(..));
      msgs.push(msg);
      if done {
        return msgs;
      }
    }
    panic!("no runs message, got {msgs:?}");
  }

  fn lines(msgs: &[ProfilesMsg]) -> Vec<String> {
    msgs
      .iter()
      .filter_map(|m| match m {
        ProfilesMsg::InstanceLines(_, l) => Some(l.clone()),
        _ => None,
      })
      .flatten()
      .collect()
  }

  fn crashed(msgs: &[ProfilesMsg]) -> bool {
    msgs
      .iter()
      .any(|m| matches!(m, ProfilesMsg::InstanceCrashed { .. }))
  }

  fn setup() -> (
    PathBuf,
    Instances,
    flume::Sender<ProfilesMsg>,
    flume::Receiver<ProfilesMsg>,
  ) {
    let (tx, rx) = flume::unbounded();
    (
      tempfile::tempdir().unwrap().keep(),
      Default::default(),
      tx,
      rx,
    )
  }

  #[test]
  fn exit_codes_decide_the_crash_event_and_the_log_is_kept() {
    for (script, crash) in [
      ("echo out; echo err >&2; exit 0", false),
      ("echo out; echo err >&2; exit 3", true),
      // killed by a signal has no exit code, still a crash
      ("echo out; echo err >&2; kill -9 $$", true),
    ] {
      let (dir, instances, tx, rx) = setup();
      let p = profile("p");
      let msgs = rt::block_on(async {
        Instance::create(sh(script), &dir, &p, "acc".into(), 2048, &instances, &tx)
          .await
          .unwrap();
        until_runs(&rx).await
      });

      let ProfilesMsg::InstanceAdded(info) = &msgs[0] else {
        panic!("added first, got {msgs:?}");
      };
      assert_eq!(
        (info.profile_id.as_str(), info.account_name.as_str()),
        ("p", "acc")
      );
      let mut got = lines(&msgs);
      // stdout and stderr race, only the set is fixed
      got.sort();
      assert_eq!(got, ["err", "out"], "{script}");
      assert_eq!(crashed(&msgs), crash, "{script}");
      assert!(
        msgs
          .iter()
          .any(|m| matches!(m, ProfilesMsg::InstanceRemoved(id) if *id == info.id))
      );
      assert!(rt::block_on(instances.lock())["p"].is_empty());

      let log = std::fs::read_to_string(run_log_file(
        &ProfileInfo::log_dir(&dir, "p"),
        info.launched_at,
      ))
      .unwrap();
      let mut saved: Vec<_> = log.lines().collect();
      saved.sort();
      assert_eq!(saved, ["err", "out"]);
    }
  }

  #[test]
  fn stop_kills_the_game_without_a_crash() {
    let (dir, instances, tx, rx) = setup();
    let p = profile("p");
    let msgs = rt::block_on(async {
      Instance::create(
        sh("echo up; sleep 30"),
        &dir,
        &p,
        "acc".into(),
        2048,
        &instances,
        &tx,
      )
      .await
      .unwrap();
      let list = instances.lock().await;
      let instance = &list["p"][0];
      assert!(instance.pid() > 0);
      // sent before the pump waits, the permit is kept
      instance.stop();
      drop(list);
      until_runs(&rx).await
    });
    assert!(!crashed(&msgs));
    assert!(
      msgs
        .iter()
        .any(|m| matches!(m, ProfilesMsg::InstanceRemoved(_)))
    );
  }

  #[test]
  fn a_child_holding_the_pipe_open_does_not_block_the_exit() {
    let (dir, instances, tx, rx) = setup();
    let p = profile("p");
    let start = Instant::now();
    let msgs = rt::block_on(async {
      // the background sleep inherits stdout, so the pipe stays open after exit
      Instance::create(
        sh("echo last; sleep 5 & exit 0"),
        &dir,
        &p,
        "acc".into(),
        2048,
        &instances,
        &tx,
      )
      .await
      .unwrap();
      until_runs(&rx).await
    });
    assert!(start.elapsed() < Duration::from_secs(10));
    assert_eq!(lines(&msgs), ["last"]);
  }

  #[test]
  fn a_child_without_piped_output_is_rejected() {
    let (dir, instances, tx, rx) = setup();
    let err = rt::block_on(async {
      let child = tokio::process::Command::new("sh")
        .args(["-c", "exit 0"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
      Instance::create(
        child,
        &dir,
        &profile("p"),
        "acc".into(),
        2048,
        &instances,
        &tx,
      )
      .await
      .unwrap_err()
    });
    assert!(matches!(
      err.downcast_ref(),
      Some(InstanceError::InvalidProcessState)
    ));
    assert!(rt::block_on(instances.lock()).is_empty());
    assert!(rx.is_empty());
  }
}
