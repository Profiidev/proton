//! The virtualized LogWindow over a [`LogBuffer`] (parsing and the model live in `proton_utils::logs`)

use std::ops::Range;

use gpui_kit::{
  App, Context, Entity, EventEmitter, FontWeight, Hsla, InteractiveElement, IntoElement,
  ListHorizontalSizingBehavior, MouseButton, ParentElement, Render, SharedString, Styled,
  Subscription, TestSupportExt, UniformListScrollHandle, Window,
  component::{ActiveTheme, scroll::Scrollbar},
  div, px, uniform_list,
};

use super::colors;

pub use proton_utils::logs::*;

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

/// [`crate::render_count`] key of the rendered log rows
pub const ROWS: &str = "LogWindow rows";

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
    let start = if generation == buffer.generation() && len <= entries.len() {
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
    self.seen = (buffer.generation(), entries.len());
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
    crate::render_count::add(ROWS, range.len());
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
    crate::render_count::hit::<Self>();
    let theme = cx.theme();
    // the frame overhangs by 1px on each side: the reference's `ring-1` takes no space
    let frame = div()
      .id("log-window")
      .absolute()
      .top(px(-1.))
      .left(px(-1.))
      .right(px(-1.))
      .bottom(px(-1.))
      .flex()
      .flex_col()
      .overflow_hidden()
      .rounded(px(14.))
      .border_1()
      .border_color(theme.foreground.opacity(0.1))
      .bg(colors::log_background(theme.background))
      .font_family(theme.mono_font_family.clone())
      .text_size(px(12.5))
      .line_height(px(20.));

    if self.visible.is_empty() {
      return outer(
        frame
          .px(px(14.))
          .py_3()
          .child(
            div()
              .font_family(theme.font_family.clone())
              .text_sm()
              .text_color(theme.muted_foreground)
              .child("No logs found for the current filter settings"),
          )
          .test_support(),
      );
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

    outer(
      frame
        .on_mouse_up(
          MouseButton::Left,
          cx.listener(|this, _, _, cx| this.sync_follow(cx)),
        )
        .child(list)
        .child(Scrollbar::new(&self.scroll))
        .test_support(),
    )
  }
}

fn outer(frame: impl IntoElement) -> gpui_kit::AnyElement {
  div()
    .relative()
    .min_h(px(420.))
    .size_full()
    .child(frame)
    .into_any_element()
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
    let rows = crate::render_count::get(ROWS);
    let started = std::time::Instant::now();
    test_view::draw(handle, cx);
    // virtualized: a 600px frame renders a few dozen of the 10k lines
    assert!(crate::render_count::get(ROWS) - rows < 100);
    // ponytail: generous for debug builds and CI; catches an O(n) render, not a slow one
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
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
