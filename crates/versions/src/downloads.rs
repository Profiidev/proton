//! One row of the downloads panel, port of `app/src/lib/downloads.ts`. No gpui.

use std::time::Instant;

use crate::event::DownloadCheckStatus;

/// Weight of the newest sample in the speed moving average
const SMOOTHING: f64 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
  Running,
  Done,
  Error,
  Canceled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadProgress {
  pub id: usize,
  pub title: String,
  pub status: DownloadStatus,
  pub started_at: Instant,
  /// Current stage text, e.g. `Downloading assets`
  pub stage: &'static str,
  pub value: Option<usize>,
  pub total: Option<usize>,
  /// Stage counts bytes (shown in MiB, has a speed)
  pub bytes: bool,
  /// Counted thing of a count stage, e.g. `files`
  pub unit: Option<&'static str>,
  /// Smoothed bytes/s of the current byte stage
  pub speed: Option<f64>,
  /// Time of the last byte-stage value, for speed deltas
  pub updated_at: Option<Instant>,
  pub cancel_requested: bool,
}

struct Stage {
  stage: &'static str,
  progress: Option<(usize, usize)>,
  bytes: bool,
  unit: Option<&'static str>,
}

fn parse_stage(status: &DownloadCheckStatus) -> Stage {
  use DownloadCheckStatus::*;
  let simple = |stage| Stage {
    stage,
    progress: None,
    bytes: false,
    unit: None,
  };
  let count = |stage, d, t| Stage {
    stage,
    progress: Some((d, t)),
    bytes: false,
    unit: None,
  };
  let bytes = |stage, d, t| Stage {
    stage,
    progress: Some((d, t)),
    bytes: true,
    unit: None,
  };
  match *status {
    AssetsManifestCheck => simple("Checking assets manifest"),
    AssetsManifestDownload => simple("Downloading assets manifest"),
    ClientCheck => simple("Checking client jar"),
    ContentResolve => simple("Resolving content"),
    Done => simple("Starting"),
    JavaManifestCheck => simple("Checking java manifest"),
    JavaManifestDownload => simple("Downloading java manifest"),
    ModLoaderFilesDownloadInfo => simple("Downloading mod loader file information"),
    ModLoaderMeta => simple("Downloading mod loader version meta"),
    ModLoaderPreprocess => simple("Running mod loader preprocessing"),
    ModLoaderPreprocessDone => simple("Mod loader preprocessing done"),
    VersionManifestCheck => simple("Checking version manifest"),
    VersionManifestDownload => simple("Downloading version manifest"),
    AssetsCheck(d, t) => count("Checking assets", d, t),
    AssetsDownload(d, t) => bytes("Downloading assets", d, t),
    ClientDownload(d, t) => bytes("Downloading client", d, t),
    ContentDownload(d, t) => bytes("Downloading content", d, t),
    ContentFiles(d, t) => Stage {
      unit: Some("files"),
      ..count("Installing content", d, t)
    },
    JavaCheck(d, t) => count("Checking java files", d, t),
    JavaDownload(d, t) => bytes("Downloading java files", d, t),
    LibraryCheck(d, t) => count("Checking libraries", d, t),
    LibraryDownload(d, t) => bytes("Downloading libraries", d, t),
    ModLoaderFilesCheck(d, t) => count("Checking mod loader files", d, t),
    ModLoaderFilesDownload(d, t) => bytes("Downloading mod loader files", d, t),
    ModpackOverrides(d, t) => count("Applying modpack overrides", d, t),
    NativeLibraryCheck(d, t) => count("Checking native libraries", d, t),
    NativeLibraryDownload(d, t) => bytes("Downloading native libraries", d, t),
  }
}

impl DownloadProgress {
  pub fn new(id: usize, title: String, now: Instant) -> Self {
    Self {
      id,
      title,
      status: DownloadStatus::Running,
      started_at: now,
      stage: "Checking version manifest",
      value: None,
      total: None,
      bytes: false,
      unit: None,
      speed: None,
      updated_at: None,
      cancel_requested: false,
    }
  }

  /// Applies a progress tick, ignored once the download finished
  pub fn apply_status(&mut self, status: &DownloadCheckStatus, now: Instant) {
    if self.status != DownloadStatus::Running {
      return;
    }
    let Stage {
      stage,
      progress,
      bytes,
      unit,
    } = parse_stage(status);
    let same_stage = bytes && self.bytes && self.stage == stage;
    let (prev_value, prev_speed, prev_updated) = (self.value, self.speed, self.updated_at);

    self.bytes = bytes;
    self.speed = None;
    self.stage = stage;
    self.value = progress.map(|p| p.0);
    self.total = progress.map(|p| p.1);
    self.unit = unit;
    self.updated_at = bytes.then_some(now);

    if *status == DownloadCheckStatus::Done {
      (self.value, self.total) = (Some(1), Some(1));
      return;
    }
    let Some(value) = self.value.filter(|_| same_stage) else {
      return;
    };
    let dt = prev_updated.map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f64());
    let prev = prev_value.unwrap_or(0);
    if dt <= 0.0 || value < prev {
      (self.speed, self.updated_at) = (prev_speed, prev_updated);
      return;
    }
    let sample = (value - prev) as f64 / dt;
    self.speed = Some(prev_speed.map_or(sample, |s| s * (1.0 - SMOOTHING) + sample * SMOOTHING));
  }

  /// Final status, canceled wins when a cancel was requested
  pub fn finish(&mut self, ok: bool) {
    self.status = match (self.cancel_requested, ok) {
      (true, _) => DownloadStatus::Canceled,
      (_, true) => DownloadStatus::Done,
      _ => DownloadStatus::Error,
    };
  }

  pub fn is_running(&self) -> bool {
    self.status == DownloadStatus::Running
  }

  pub fn percent(&self) -> u32 {
    if self.status == DownloadStatus::Done {
      return 100;
    }
    match (self.value, self.total) {
      (Some(v), Some(t)) if t > 0 => ((v as f64 / t as f64 * 100.0).round() as u32).min(100),
      _ => 0,
    }
  }

  pub fn percent_label(&self) -> String {
    match self.status {
      DownloadStatus::Done => "Done".into(),
      DownloadStatus::Error => "Failed".into(),
      DownloadStatus::Canceled => "Canceled".into(),
      DownloadStatus::Running if self.value.is_none() => String::new(),
      DownloadStatus::Running => format!("{}%", self.percent()),
    }
  }

  pub fn detail(&self) -> String {
    if !self.is_running() {
      return String::new();
    }
    let (Some(value), Some(total)) = (self.value, self.total) else {
      return self.stage.into();
    };
    let amount = if self.bytes {
      format!("{} of {} MiB", mib(value), mib(total))
    } else {
      let unit = self.unit.map(|u| format!(" {u}")).unwrap_or_default();
      format!("{} of {}{unit}", thousands(value), thousands(total))
    };
    format!("{} · {amount}", self.stage)
  }

  pub fn speed_label(&self) -> String {
    match self.speed {
      Some(speed) if self.is_running() && self.bytes && speed > 0.0 => {
        format!("{:.1} MB/s", speed / 1_000_000.0)
      }
      _ => String::new(),
    }
  }
}

fn mib(bytes: usize) -> String {
  let value = bytes as f64 / 1024.0 / 1024.0;
  if value < 10.0 {
    format!("{value:.1}")
  } else {
    format!("{}", value.round())
  }
}

fn thousands(n: usize) -> String {
  let s = n.to_string();
  let mut out = String::new();
  for (i, c) in s.chars().enumerate() {
    if i > 0 && (s.len() - i).is_multiple_of(3) {
      out.push(',');
    }
    out.push(c);
  }
  out
}

pub fn chip_label(running: usize) -> String {
  match running {
    0 => "Downloads".into(),
    1 => "1 download".into(),
    n => format!("{n} downloads"),
  }
}

/// Average percent of the running downloads, for the chip bar
pub fn running_percent<'a>(downloads: impl IntoIterator<Item = &'a DownloadProgress>) -> f64 {
  let percents: Vec<_> = downloads
    .into_iter()
    .filter(|d| d.is_running())
    .map(|d| d.percent() as f64)
    .collect();
  if percents.is_empty() {
    100.0
  } else {
    percents.iter().sum::<f64>() / percents.len() as f64
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use DownloadCheckStatus::*;

  use super::*;

  const MIB: usize = 1024 * 1024;

  fn base() -> DownloadProgress {
    DownloadProgress::new(1, "Survival: launch".into(), Instant::now())
  }

  fn at(d: &DownloadProgress, ms: u64) -> Instant {
    d.started_at + Duration::from_millis(ms)
  }

  fn applied(status: DownloadCheckStatus) -> DownloadProgress {
    let mut d = base();
    d.apply_status(&status, d.started_at);
    d
  }

  fn with(f: impl FnOnce(&mut DownloadProgress)) -> DownloadProgress {
    let mut d = base();
    f(&mut d);
    d
  }

  #[test]
  fn updates_stage_and_counts() {
    let d = applied(LibraryCheck(120, 300));
    assert_eq!(
      (d.bytes, d.stage, d.total, d.value, d.speed),
      (false, "Checking libraries", Some(300), Some(120), None)
    );
    assert!(applied(AssetsDownload(1, 2)).bytes);
  }

  #[test]
  fn computes_speed_from_deltas_within_a_byte_stage() {
    let mut d = base();
    d.apply_status(&AssetsDownload(0, 100 * MIB), at(&d, 1000));
    assert_eq!(d.speed, None);
    d.apply_status(&AssetsDownload(2_000_000, 100 * MIB), at(&d, 2000));
    assert_eq!(d.speed, Some(2_000_000.0));
    d.apply_status(&AssetsDownload(6_000_000, 100 * MIB), at(&d, 3000));
    // Smoothed: 0.7 * 2e6 + 0.3 * 4e6
    assert!((d.speed.unwrap() - 2_600_000.0).abs() < 1.0);
  }

  #[test]
  fn resets_speed_when_the_stage_changes() {
    let mut d = base();
    d.apply_status(&AssetsDownload(0, 10), at(&d, 0));
    d.apply_status(&AssetsDownload(5, 10), at(&d, 1000));
    assert_eq!(d.speed, Some(5.0));
    d.apply_status(&LibraryDownload(1, 10), at(&d, 2000));
    assert_eq!(d.speed, None);
  }

  #[test]
  fn keeps_speed_on_zero_time_delta() {
    let mut d = base();
    d.apply_status(&ClientDownload(0, 10), at(&d, 0));
    d.apply_status(&ClientDownload(5, 10), at(&d, 1000));
    d.apply_status(&ClientDownload(6, 10), at(&d, 1000));
    assert_eq!((d.speed, d.value), (Some(5.0), Some(6)));
  }

  #[test]
  fn marks_done_as_full_progress() {
    let d = applied(Done);
    assert_eq!((d.percent(), d.status), (100, DownloadStatus::Running));
  }

  #[test]
  fn ignores_events_for_finished_downloads() {
    let mut d = with(|d| d.finish(true));
    let before = d.clone();
    d.apply_status(&ClientCheck, Instant::now());
    assert_eq!(d, before);
  }

  #[test]
  fn finish_prefers_canceled() {
    assert_eq!(with(|d| d.finish(false)).status, DownloadStatus::Error);
    let d = with(|d| {
      d.cancel_requested = true;
      d.finish(true)
    });
    assert_eq!(d.status, DownloadStatus::Canceled);
  }

  #[test]
  fn percent_and_label() {
    assert_eq!(base().percent(), 0);
    let progress = |v, t| {
      with(|d| {
        d.value = Some(v);
        d.total = Some(t)
      })
    };
    assert_eq!(progress(1, 3).percent(), 33);
    assert_eq!(progress(0, 0).percent(), 0);
    assert_eq!(with(|d| d.status = DownloadStatus::Done).percent(), 100);

    assert_eq!(base().percent_label(), "");
    assert_eq!(progress(1, 4).percent_label(), "25%");
    let label = |s| with(|d| d.status = s).percent_label();
    assert_eq!(label(DownloadStatus::Done), "Done");
    assert_eq!(label(DownloadStatus::Error), "Failed");
    assert_eq!(label(DownloadStatus::Canceled), "Canceled");
  }

  #[test]
  fn detail() {
    assert_eq!(base().detail(), "Checking version manifest");
    assert_eq!(applied(ClientCheck).detail(), "Checking client jar");
    assert!(!applied(LibraryCheck(3, 4)).bytes);
    let detail = |s| applied(s).detail();
    assert_eq!(
      detail(AssetsDownload(41 * MIB, 96 * MIB)),
      "Downloading assets · 41 of 96 MiB"
    );
    assert_eq!(
      detail(ClientDownload(MIB / 2, 2 * MIB)),
      "Downloading client · 0.5 of 2.0 MiB"
    );
    assert_eq!(
      detail(AssetsCheck(2904, 3580)),
      "Checking assets · 2,904 of 3,580"
    );
    assert_eq!(with(|d| d.status = DownloadStatus::Done).detail(), "");
    assert_eq!(detail(ContentResolve), "Resolving content");
    assert_eq!(
      detail(ContentDownload(41 * MIB, 96 * MIB)),
      "Downloading content · 41 of 96 MiB"
    );
    assert_eq!(
      detail(ContentFiles(41, 96)),
      "Installing content · 41 of 96 files"
    );
    assert_eq!(
      detail(ModpackOverrides(3, 10)),
      "Applying modpack overrides · 3 of 10"
    );
    assert_eq!(thousands(1_234_567), "1,234,567");
  }

  #[test]
  fn drops_the_unit_when_the_stage_changes() {
    let mut d = applied(ContentFiles(1, 2));
    assert_eq!(d.unit, Some("files"));
    d.apply_status(&ModpackOverrides(1, 2), Instant::now());
    assert_eq!(d.unit, None);
  }

  #[test]
  fn speed_label_only_for_running_byte_stages() {
    let d = with(|d| {
      d.bytes = true;
      d.speed = Some(3_200_000.0)
    });
    assert_eq!(d.speed_label(), "3.2 MB/s");
    let mut other = d.clone();
    other.bytes = false;
    assert_eq!(other.speed_label(), "");
    let mut done = d.clone();
    done.status = DownloadStatus::Done;
    assert_eq!(done.speed_label(), "");
  }

  #[test]
  fn list_helpers() {
    assert_eq!(chip_label(0), "Downloads");
    assert_eq!(chip_label(1), "1 download");
    assert_eq!(chip_label(2), "2 downloads");

    let running = |v| {
      with(|d| {
        d.value = Some(v);
        d.total = Some(4)
      })
    };
    let list = [
      running(1),
      with(|d| d.status = DownloadStatus::Done),
      with(|d| d.status = DownloadStatus::Error),
      running(3),
    ];
    assert_eq!(running_percent(&list), 50.0);
    assert_eq!(running_percent(&[]), 100.0);
  }
}
