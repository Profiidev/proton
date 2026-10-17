//! Text formatting shared by the widgets and pages (ported from the web app's
//! luxon / Intl usage and `instances.ts`)

use std::fmt::Display;

use chrono::{DateTime, Datelike, Months, TimeZone, Utc};

/// luxon `toRelative()` in English: largest whole unit, truncated, e.g. `15 hours ago`
pub fn relative_time(time: DateTime<Utc>, now: DateTime<Utc>) -> String {
  let past = time <= now;
  let (from, to) = if past { (time, now) } else { (now, time) };
  let months = months_between(from, to);
  let secs = (to - from).num_seconds();
  let units = [
    (months / 12, "year"),
    (months / 3, "quarter"),
    (months, "month"),
    (secs / 604_800, "week"),
    (secs / 86_400, "day"),
    (secs / 3_600, "hour"),
    (secs / 60, "minute"),
  ];
  let (count, unit) = units
    .into_iter()
    .find(|(count, _)| *count >= 1)
    .unwrap_or((secs, "second"));
  let s = if count == 1 { "" } else { "s" };
  if past {
    format!("{count} {unit}{s} ago")
  } else {
    format!("in {count} {unit}{s}")
  }
}

/// Whole calendar months from `from` to `to` (`from <= to`)
fn months_between(from: DateTime<Utc>, to: DateTime<Utc>) -> i64 {
  let months = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32;
  let months = months.max(0) as u32;
  match from.checked_add_months(Months::new(months)) {
    Some(shifted) if shifted > to => months as i64 - 1,
    _ => months as i64,
  }
}

/// luxon `setLocale('de')` + `DATETIME_SHORT`: `16.10.2026, 14:30`.
/// Pass the time in the zone it should be shown in (usually `Local`).
pub fn format_datetime<Tz: TimeZone>(time: &DateTime<Tz>) -> String
where
  Tz::Offset: Display,
{
  time.format("%-d.%-m.%Y, %H:%M").to_string()
}

/// `Intl.NumberFormat('en', { notation: 'compact' })`: `999`, `1.2K`, `15K`, `2.1M`
pub fn format_count(n: u64) -> String {
  const UNITS: [(u64, &str); 4] = [
    (1_000_000_000_000, "T"),
    (1_000_000_000, "B"),
    (1_000_000, "M"),
    (1_000, "K"),
  ];
  let Some(ix) = UNITS.iter().position(|(e, _)| n >= *e) else {
    return n.to_string();
  };
  let (e, suffix) = UNITS[ix];
  if n < 10 * e {
    // two significant digits
    let tenths = (n * 10 + e / 2) / e;
    return if tenths == 100 {
      format!("10{suffix}")
    } else if tenths.is_multiple_of(10) {
      format!("{}{suffix}", tenths / 10)
    } else {
      format!("{}.{}{suffix}", tenths / 10, tenths % 10)
    };
  }
  let whole = (n + e / 2) / e;
  match (whole >= 1_000).then(|| ix.checked_sub(1)).flatten() {
    Some(up) => format!("1{}", UNITS[up].1),
    None => format!("{whole}{suffix}"),
  }
}

/// `Running` or `3 running`
pub fn running_label(count: usize) -> String {
  if count > 1 {
    format!("{count} running")
  } else {
    "Running".into()
  }
}

/// Bytes as GB with one decimal, `3.1`
pub fn format_gb(bytes: u64) -> String {
  format!("{:.1}", bytes as f64 / 1024. / 1024. / 1024.)
}

/// Compact duration like `1h 12m`, `5m` or `<1m`
pub fn format_duration(ms: i64) -> String {
  let minutes = ms / 60_000;
  if minutes < 1 {
    return "<1m".into();
  }
  let hours = minutes / 60;
  if hours > 0 {
    format!("{hours}h {}m", minutes % 60)
  } else {
    format!("{minutes}m")
  }
}

/// Memory label and bar percent (0-100), `None` when the usage is unknown
pub fn memory_usage(used_bytes: Option<u64>, total_mb: Option<u64>) -> Option<(String, f32)> {
  let used = used_bytes?;
  let total = total_mb.unwrap_or(0) * 1024 * 1024;
  Some(if total > 0 {
    (
      format!("{} of {} GB", format_gb(used), format_gb(total)),
      (used as f32 / total as f32 * 100.).round().min(100.),
    )
  } else {
    (format!("{} GB", format_gb(used)), 0.)
  })
}

/// `Playing as X · running for 5m`, the account part omitted when unknown
pub fn instance_subtitle(uptime: &str, account: Option<&str>, playing_as: bool) -> String {
  let running = format!("running for {uptime}");
  match account.filter(|a| !a.is_empty()) {
    Some(a) if playing_as => format!("Playing as {a} · {running}"),
    Some(a) => format!("{a} · {running}"),
    None => running,
  }
}

#[cfg(test)]
mod tests {
  use chrono::{Duration, FixedOffset};

  use super::*;

  fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
  }

  #[test]
  fn relative_matches_luxon() {
    let now = at("2026-10-16T12:00:00Z");
    let ago = |d: Duration| relative_time(now - d, now);
    assert_eq!(ago(Duration::seconds(0)), "0 seconds ago");
    assert_eq!(ago(Duration::seconds(42)), "42 seconds ago");
    assert_eq!(ago(Duration::minutes(1)), "1 minute ago");
    assert_eq!(ago(Duration::minutes(37)), "37 minutes ago");
    assert_eq!(ago(Duration::minutes(15 * 60 + 59)), "15 hours ago");
    assert_eq!(ago(Duration::hours(30)), "1 day ago");
    assert_eq!(ago(Duration::days(13)), "1 week ago");
    assert_eq!(ago(Duration::days(24)), "3 weeks ago");
    assert_eq!(
      relative_time(at("2026-09-16T12:00:00Z"), now),
      "1 month ago"
    );
    assert_eq!(
      relative_time(at("2026-09-16T12:00:01Z"), now),
      "4 weeks ago"
    );
    assert_eq!(
      relative_time(at("2026-04-01T00:00:00Z"), now),
      "2 quarters ago"
    );
    assert_eq!(
      relative_time(at("2024-01-01T00:00:00Z"), now),
      "2 years ago"
    );
    assert_eq!(relative_time(now + Duration::hours(2), now), "in 2 hours");
  }

  #[test]
  fn datetime_uses_the_de_short_pattern() {
    let tz = FixedOffset::east_opt(2 * 3600).unwrap();
    let t = at("2026-10-14T12:00:00Z").with_timezone(&tz);
    assert_eq!(format_datetime(&t), "14.10.2026, 14:00");
    let t = at("2026-02-01T07:05:00Z").with_timezone(&tz);
    assert_eq!(format_datetime(&t), "1.2.2026, 09:05");
  }

  #[test]
  fn counts_are_compact() {
    assert_eq!(format_count(0), "0");
    assert_eq!(format_count(999), "999");
    assert_eq!(format_count(1_000), "1K");
    assert_eq!(format_count(1_234), "1.2K");
    assert_eq!(format_count(9_960), "10K");
    assert_eq!(format_count(15_300), "15K");
    assert_eq!(format_count(153_000), "153K");
    assert_eq!(format_count(999_950), "1M");
    assert_eq!(format_count(2_100_000), "2.1M");
    assert_eq!(format_count(280_000_000), "280M");
    assert_eq!(format_count(3_000_000_000), "3B");
  }

  // instances.test.ts / instance.test.ts
  #[test]
  fn instance_helpers() {
    assert_eq!(running_label(1), "Running");
    assert_eq!(running_label(3), "3 running");
    assert_eq!(format_gb((3.1 * 1024f64.powi(3)) as u64), "3.1");
    assert_eq!(format_duration(30_000), "<1m");
    assert_eq!(format_duration(5 * 60_000), "5m");
    assert_eq!(format_duration(72 * 60_000), "1h 12m");
    assert_eq!(memory_usage(None, Some(8192)), None);
    let gb = 1024 * 1024 * 1024;
    assert_eq!(
      memory_usage(Some(2 * gb), Some(8192)),
      Some(("2.0 of 8.0 GB".into(), 25.))
    );
    assert_eq!(memory_usage(Some(gb), None), Some(("1.0 GB".into(), 0.)));
    assert_eq!(memory_usage(Some(20 * gb), Some(8192)).unwrap().1, 100.);
    assert_eq!(
      instance_subtitle("5m", Some("Steve"), true),
      "Playing as Steve · running for 5m"
    );
    assert_eq!(
      instance_subtitle("5m", Some("Steve"), false),
      "Steve · running for 5m"
    );
    assert_eq!(instance_subtitle("<1m", None, true), "running for <1m");
  }

  #[test]
  fn instance_helper_edges() {
    // an empty account name counts as unknown
    assert_eq!(instance_subtitle("1m", Some(""), true), "running for 1m");
    assert_eq!(format_duration(0), "<1m");
    assert_eq!(format_duration(-5_000), "<1m");
    assert_eq!(format_duration(59_999), "<1m");
    assert_eq!(format_duration(60 * 60_000), "1h 0m");
    assert_eq!(format_gb(0), "0.0");
    assert_eq!(memory_usage(Some(0), Some(0)), Some(("0.0 GB".into(), 0.)));
    assert_eq!(memory_usage(None, None), None);
    // only shown while something runs, so 0 is never pluralized
    assert_eq!(running_label(0), "Running");
  }
}
