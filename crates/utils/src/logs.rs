//! Minecraft log parsing (`logs.ts`) and the batched [`LogBuffer`] model shown by the
//! components `LogWindow`; the profiles service appends live instance output to it.

use std::sync::LazyLock;

use gpui_kit::{Context, SharedString};
use regex::Regex;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogEntry {
  pub time: SharedString,
  pub level: SharedString,
  pub location: SharedString,
  pub message: SharedString,
  pub sub_location: Option<SharedString>,
  /// Unparsable line (e.g. stack trace) belonging to the previous entry, `level` is inherited
  pub continuation: bool,
}

pub const LOG_LEVELS: [&str; 4] = ["INFO", "WARN", "ERROR", "DEBUG"];

/// Console toggle modes
pub const CONSOLE_ALL: &[&str] = &[];
pub const CONSOLE_WARN: &[&str] = &["WARN", "ERROR"];
pub const CONSOLE_ERR: &[&str] = &["ERROR"];

static LINE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"^\[(?<time>.+?)\] \[(?<location>.+?)/(?<level>.+?)\](?: \[(?<sub>.+?)\])?: (?<message>.+)$",
  )
  .unwrap()
});

/// ANSI color codes from log4j patterns some modpacks set for the console
static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]").unwrap());

/// Parses `[time] [location/LEVEL] ([subLocation]): message`, unparsable lines become a bare message
pub fn parse_line(line: &str) -> LogEntry {
  let line = &*ANSI.replace_all(line, "");
  let Some(c) = LINE.captures(line) else {
    return LogEntry {
      message: line.to_string().into(),
      ..Default::default()
    };
  };
  let s = |name: &str| SharedString::from(c[name].to_string());
  LogEntry {
    time: s("time"),
    level: s("level"),
    location: s("location"),
    message: s("message"),
    sub_location: c.name("sub").map(|m| m.as_str().to_string().into()),
    continuation: false,
  }
}

/// Parses all lines, continuation lines inherit the level of the previous parsed line
pub fn parse_logs<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<LogEntry> {
  let mut level = SharedString::default();
  lines
    .into_iter()
    .map(|line| parse_continued(line, &mut level))
    .collect()
}

fn parse_continued(line: &str, level: &mut SharedString) -> LogEntry {
  let mut entry = parse_line(line);
  if entry.level.is_empty() {
    entry.continuation = true;
    entry.level = level.clone();
  } else {
    *level = entry.level.clone();
  }
  entry
}

/// Lowercased, trimmed search query plus the allowed levels (empty = all)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogFilter {
  query: String,
  levels: Vec<SharedString>,
}

impl LogFilter {
  pub fn new(search: &str, levels: &[impl AsRef<str>]) -> Self {
    Self {
      query: search.trim().to_lowercase(),
      levels: levels
        .iter()
        .map(|l| SharedString::from(l.as_ref().to_string()))
        .collect(),
    }
  }

  /// Level in the set AND location, sub location or message contains the query
  pub fn matches(&self, e: &LogEntry) -> bool {
    (self.levels.is_empty() || self.levels.contains(&e.level))
      && (self.query.is_empty()
        || [Some(&e.location), e.sub_location.as_ref(), Some(&e.message)]
          .into_iter()
          .flatten()
          .any(|s| s.to_lowercase().contains(&self.query)))
  }
}

pub fn filter_logs<'a>(entries: &'a [LogEntry], filter: &LogFilter) -> Vec<&'a LogEntry> {
  entries.iter().filter(|e| filter.matches(e)).collect()
}

/// Parsed log lines; every mutation notifies observers once
#[derive(Default)]
pub struct LogBuffer {
  entries: Vec<LogEntry>,
  /// Level continuation lines inherit
  level: SharedString,
  /// Bumped when entries are replaced instead of appended
  generation: u64,
}

impl LogBuffer {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn entries(&self) -> &[LogEntry] {
    &self.entries
  }

  /// Changes whenever the entries were replaced instead of appended
  pub fn generation(&self) -> u64 {
    self.generation
  }

  pub fn push(&mut self, line: &str, cx: &mut Context<Self>) {
    self.extend([line], cx);
  }

  /// Appends a batch with a single notify
  pub fn extend<'a>(&mut self, lines: impl IntoIterator<Item = &'a str>, cx: &mut Context<Self>) {
    let before = self.entries.len();
    for line in lines {
      let entry = parse_continued(line, &mut self.level);
      self.entries.push(entry);
    }
    if self.entries.len() != before {
      cx.notify();
    }
  }

  /// Replaces all lines (e.g. another run was selected)
  pub fn set<'a>(&mut self, lines: impl IntoIterator<Item = &'a str>, cx: &mut Context<Self>) {
    self.entries.clear();
    self.level = SharedString::default();
    self.generation += 1;
    self.extend(lines, cx);
    cx.notify();
  }

  pub fn clear(&mut self, cx: &mut Context<Self>) {
    self.set([], cx);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn times(entries: &[LogEntry], levels: &[&str], search: &str) -> Vec<String> {
    filter_logs(entries, &LogFilter::new(search, levels))
      .into_iter()
      .map(|e| {
        if e.time.is_empty() {
          e.message.to_string()
        } else {
          e.time.to_string()
        }
      })
      .collect()
  }

  #[test]
  fn parses_a_line_without_sub_location() {
    assert_eq!(
      parse_line("[17:48:10] [Render thread/INFO]: Setting user: ProfiDev"),
      LogEntry {
        time: "17:48:10".into(),
        level: "INFO".into(),
        location: "Render thread".into(),
        message: "Setting user: ProfiDev".into(),
        sub_location: None,
        continuation: false,
      }
    );
  }

  #[test]
  fn parses_a_line_with_sub_location() {
    let e = parse_line("[17:48:02] [main/WARN] [minecraft/Main]: Slow start");
    assert_eq!(
      (
        &*e.level,
        &*e.location,
        &*e.message,
        e.sub_location.as_deref()
      ),
      ("WARN", "main", "Slow start", Some("minecraft/Main"))
    );
  }

  #[test]
  fn keeps_an_unparsable_line_as_message() {
    assert_eq!(
      parse_line("\tat java.lang.Thread.run"),
      LogEntry {
        message: "\tat java.lang.Thread.run".into(),
        ..Default::default()
      }
    );
  }

  #[test]
  fn filters_by_level_and_search() {
    let entries: Vec<_> = [
      "[1] [Server thread/INFO]: Saving the game",
      "[2] [Server thread/WARN]: Can't keep up!",
      "[3] [Render thread/ERROR]: Failed to load sound",
      "[4] [Render thread/DEBUG] [Iris]: Loaded shader pack",
      "stack trace line",
    ]
    .into_iter()
    .map(parse_line)
    .collect();
    assert_eq!(times(&entries, &[], "").len(), 5);
    assert_eq!(times(&entries, CONSOLE_ALL, "").len(), 5);
    assert_eq!(times(&entries, CONSOLE_WARN, ""), ["2", "3"]);
    assert_eq!(times(&entries, CONSOLE_ERR, ""), ["3"]);
    assert_eq!(times(&entries, &["INFO", "DEBUG"], ""), ["1", "4"]);
    assert_eq!(times(&entries, &[], "SAVING"), ["1"]);
    assert_eq!(times(&entries, &[], "render"), ["3", "4"]);
    assert_eq!(times(&entries, &[], "iris"), ["4"]);
    assert_eq!(times(&entries, &[], " trace "), ["stack trace line"]);
    assert_eq!(times(&entries, &["ERROR"], "render"), ["3"]);
  }

  #[test]
  fn continuation_lines_inherit_the_level() {
    let entries = parse_logs([
      "orphan before any level",
      "[1] [Render thread/ERROR]: Crash",
      "java.lang.RuntimeException: boom",
      "\tat Foo.bar",
      "[2] [Render thread/INFO]: Recovered",
    ]);
    let levels: Vec<_> = entries
      .iter()
      .map(|e| (e.level.to_string(), e.continuation))
      .collect();
    assert_eq!(
      levels,
      [
        ("".into(), true),
        ("ERROR".into(), false),
        ("ERROR".into(), true),
        ("ERROR".into(), true),
        ("INFO".into(), false),
      ]
    );
    assert_eq!(entries[2].time, "");
    assert_eq!(
      times(&entries, CONSOLE_ERR, ""),
      ["1", "java.lang.RuntimeException: boom", "\tat Foo.bar"]
    );
  }

  #[test]
  fn strips_ansi_colors() {
    let e = parse_line(
      "\x1b[m\x1b[32m[08:54:13] [Worker-16/INFO] [colorwheel_patcher/]: Found patch\x1b[m",
    );
    assert_eq!(e.level, "INFO");
    assert_eq!(e.time, "08:54:13");
    assert_eq!(e.message, "Found patch");
  }
}
