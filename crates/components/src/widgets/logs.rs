//! Minecraft log parsing (`logs.ts`), a batched log buffer and the virtualized LogWindow

use std::{ops::Range, sync::LazyLock};

use gpui_kit::{
  App, Context, Entity, EventEmitter, FontWeight, Hsla, InteractiveElement, IntoElement,
  ListHorizontalSizingBehavior, MouseButton, ParentElement, Render, SharedString, Styled,
  Subscription, TestSupportExt, UniformListScrollHandle, Window,
  component::{ActiveTheme, scroll::Scrollbar},
  div, px, uniform_list,
};
use regex::Regex;

use super::colors;

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

/// Tailwind `text-{green,yellow,red,blue}-500` per level, `None` = default text color
pub fn level_color(level: &str) -> Option<Hsla> {
  match level {
    "INFO" => Some(colors::green_500()),
    "WARN" => Some(colors::yellow_500()),
    "ERROR" => Some(colors::red_500()),
    "DEBUG" => Some(colors::blue_500()),
    _ => None,
  }
}

static LINE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"^\[(?<time>.+?)\] \[(?<location>.+?)/(?<level>.+?)\](?: \[(?<sub>.+?)\])?: (?<message>.+)$",
  )
  .unwrap()
});

/// Parses `[time] [location/LEVEL] ([subLocation]): message`, unparsable lines become a bare message
pub fn parse_line(line: &str) -> LogEntry {
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

/// Emitted when following the tail turns on or off (user scrolled away / back)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowChanged(pub bool);

/// Virtualized log view over a [`LogBuffer`]; appends re-render only this view
pub struct LogWindow {
  buffer: Entity<LogBuffer>,
  /// Show the source column, otherwise the source prefixes the message
  source: bool,
  filter: LogFilter,
  /// Buffer indices passing the filter
  visible: Vec<usize>,
  /// Index into `visible` of the longest line, the list measures its width from it
  widest: (usize, usize),
  seen: (u64, usize),
  follow: bool,
  scroll: UniformListScrollHandle,
  _observe: Subscription,
}

impl EventEmitter<FollowChanged> for LogWindow {}

impl LogWindow {
  pub fn new(buffer: Entity<LogBuffer>, cx: &mut Context<Self>) -> Self {
    let observe = cx.observe(&buffer, |this, _, cx| {
      this.sync(cx);
      cx.notify();
    });
    let mut this = Self {
      buffer,
      source: false,
      filter: LogFilter::default(),
      visible: Vec::new(),
      widest: (0, 0),
      seen: (u64::MAX, 0),
      follow: true,
      scroll: UniformListScrollHandle::new(),
      _observe: observe,
    };
    this.sync(cx);
    this
  }

  pub fn source(mut self, source: bool) -> Self {
    self.source = source;
    self
  }

  pub fn is_following(&self) -> bool {
    self.follow
  }

  /// Number of lines passing the filter
  pub fn visible_len(&self) -> usize {
    self.visible.len()
  }

  pub fn set_filter(&mut self, filter: LogFilter, cx: &mut Context<Self>) {
    if self.filter != filter {
      self.filter = filter;
      self.seen = (u64::MAX, 0);
      self.sync(cx);
      cx.notify();
    }
  }

  pub fn set_follow(&mut self, follow: bool, cx: &mut Context<Self>) {
    if follow {
      self.scroll.scroll_to_bottom();
    }
    if self.follow != follow {
      self.follow = follow;
      cx.emit(FollowChanged(follow));
    }
    cx.notify();
  }

  /// Filters only the lines appended since the last sync
  fn sync(&mut self, cx: &mut Context<Self>) {
    let buffer = self.buffer.read(cx);
    let entries = buffer.entries();
    let (generation, len) = self.seen;
    let start = if generation == buffer.generation && len <= entries.len() {
      len
    } else {
      self.visible.clear();
      self.widest = (0, 0);
      0
    };
    for (ix, entry) in entries.iter().enumerate().skip(start) {
      if self.filter.matches(entry) {
        let width = line_width(entry);
        if width > self.widest.1 {
          self.widest = (self.visible.len(), width);
        }
        self.visible.push(ix);
      }
    }
    self.seen = (buffer.generation, entries.len());
    if self.follow {
      self.scroll.scroll_to_bottom();
    }
  }

  /// Wheel / scrollbar drag: follow only while scrolled to the bottom
  fn sync_follow(&mut self, cx: &mut Context<Self>) {
    let at_end = self.scroll.is_scrolled_to_end().unwrap_or(true);
    if at_end != self.follow {
      self.follow = at_end;
      cx.emit(FollowChanged(at_end));
      cx.notify();
    }
  }

  fn render_rows(&self, range: Range<usize>, cx: &App) -> Vec<gpui_kit::AnyElement> {
    let entries = self.buffer.read(cx).entries();
    let muted = cx.theme().muted_foreground;
    range
      .filter_map(|ix| self.visible.get(ix).map(|&e| &entries[e]))
      .map(|e| {
        let row = div().h(px(20.)).flex().gap(px(10.)).whitespace_nowrap();
        if e.continuation {
          return row
            .child(SharedString::from(e.message.replace('\t', "    ")))
            .into_any_element();
        }
        let level = div()
          .w(px(42.))
          .flex_none()
          .font_weight(FontWeight::MEDIUM)
          .child(e.level.clone());
        let level = match level_color(&e.level) {
          Some(color) => level.text_color(color),
          None => level,
        };
        let row = row
          .child(div().text_color(muted).child(e.time.clone()))
          .child(level);
        if self.source {
          let source = match &e.sub_location {
            Some(sub) => format!("{} ({sub})", e.location).into(),
            None => e.location.clone(),
          };
          row
            .child(div().text_color(muted).child(source))
            .child(e.message.clone())
        } else if e.location.is_empty() {
          row.child(e.message.clone())
        } else {
          row.child(SharedString::from(format!("{}: {}", e.location, e.message)))
        }
        .into_any_element()
      })
      .collect()
  }
}

/// Approximate monospace width in chars, to pick the widest row
fn line_width(e: &LogEntry) -> usize {
  if e.continuation {
    return e.message.chars().count() + e.message.matches('\t').count() * 3;
  }
  e.time.len()
    + 12
    + e.location.len()
    + e.sub_location.as_ref().map_or(0, |s| s.len() + 3)
    + e.message.chars().count()
}

impl Render for LogWindow {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let frame = div()
      .id("log-window")
      .relative()
      .flex()
      .flex_col()
      .min_h(px(420.))
      .size_full()
      .overflow_hidden()
      .rounded_xl()
      .border_1()
      .border_color(theme.foreground.opacity(0.1))
      .bg(colors::log_background(theme.background))
      .font_family(theme.mono_font_family.clone())
      .text_size(px(12.5))
      .line_height(px(20.));

    if self.visible.is_empty() {
      return frame
        .px(px(14.))
        .py_3()
        .child(
          div()
            .font_family(theme.font_family.clone())
            .text_sm()
            .text_color(theme.muted_foreground)
            .child("No logs found for the current filter settings"),
        )
        .test_support()
        .into_any_element();
    }

    let list = uniform_list(
      "log-lines",
      self.visible.len(),
      cx.processor(|this, range, _, cx| this.render_rows(range, cx)),
    )
    .track_scroll(&self.scroll)
    .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
    .with_width_from_item(Some(self.widest.0))
    .size_full()
    .px(px(14.))
    .py_3()
    .on_scroll_wheel(cx.listener(|_, _, window, cx| {
      // after the list applied the wheel delta
      cx.defer_in(window, |this, _, cx| this.sync_follow(cx));
    }));

    frame
      .on_mouse_up(
        MouseButton::Left,
        cx.listener(|this, _, _, cx| this.sync_follow(cx)),
      )
      .child(list)
      .child(Scrollbar::new(&self.scroll))
      .test_support()
      .into_any_element()
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{
    self as gpui, AppContext, ScrollDelta, StyleRefinement, TestAppContext, point,
    test::TestWindowExt,
  };
  use std::{cell::Cell, rc::Rc};

  use super::*;
  use crate::test_view;

  struct Counter(Rc<Cell<usize>>);

  impl Render for Counter {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
      self.0.set(self.0.get() + 1);
      div().size_full()
    }
  }

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

  #[gpui::test]
  fn follows_the_tail_until_scrolled_up(cx: &mut TestAppContext) {
    let buffer = cx.new(|_| LogBuffer::new());
    let lines: Vec<String> = (0..10_000)
      .map(|i| format!("[12:00:00] [main/INFO]: line {i}"))
      .collect();
    buffer.update(cx, |b, cx| b.extend(lines.iter().map(String::as_str), cx));

    let log = cx.new(|cx| LogWindow::new(buffer.clone(), cx));
    let renders = Rc::new(Cell::new(0));
    let sibling = cx.new(|_| Counter(renders.clone()));
    let (handle, _) = test_view::open(cx, {
      let log = log.clone();
      move |_, _| {
        div()
          .size(px(600.))
          .child(
            div().h(px(20.)).child(
              sibling
                .clone()
                .cached(StyleRefinement::default().size_full()),
            ),
          )
          .child(log.clone())
          .into_any_element()
      }
    });
    test_view::draw(handle, cx);
    test_view::draw(handle, cx);
    let end = |cx: &mut TestAppContext| log.read_with(cx, |l, _| l.scroll.is_scrolled_to_end());
    assert_eq!(end(cx), Some(true));
    assert_eq!(log.read_with(cx, |l, _| l.visible_len()), 10_000);

    // appending re-renders the list, not the views around it
    let parent_renders = renders.get();
    buffer.update(cx, |b, cx| {
      b.extend(["[12:00:01] [main/WARN]: more"; 100], cx)
    });
    // a normal (not forced) frame, so clean cached views are replayed
    cx.update_window(handle, |_, window, cx| window.draw(cx).clear(cx))
      .unwrap();
    assert_eq!(renders.get(), parent_renders);
    assert_eq!(end(cx), Some(true));

    cx.update_window(handle, |_, window, cx| {
      window.scroll(
        "log-window",
        ScrollDelta::Pixels(point(px(0.), px(200.))),
        cx,
      );
    })
    .unwrap();
    cx.run_until_parked();
    test_view::draw(handle, cx);
    assert!(!log.read_with(cx, |l, _| l.is_following()));

    buffer.update(cx, |b, cx| b.push("[12:00:02] [main/INFO]: unseen", cx));
    test_view::draw(handle, cx);
    test_view::draw(handle, cx);
    assert_eq!(end(cx), Some(false));

    log.update(cx, |l, cx| l.set_follow(true, cx));
    test_view::draw(handle, cx);
    test_view::draw(handle, cx);
    assert_eq!(end(cx), Some(true));

    log.update(cx, |l, cx| {
      l.set_filter(LogFilter::new("", CONSOLE_WARN), cx)
    });
    assert_eq!(log.read_with(cx, |l, _| l.visible_len()), 100);
    buffer.update(cx, |b, cx| b.clear(cx));
    assert_eq!(log.read_with(cx, |l, _| l.visible_len()), 0);
    test_view::draw(handle, cx);
  }
}
