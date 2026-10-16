//! Running instance row (instances page) / card (profile instances tab)

use std::rc::Rc;

use gpui_kit::{
  App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, RenderOnce,
  SharedString, Styled, Window,
  component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    progress::Progress,
  },
  div,
  prelude::FluentBuilder,
  px,
};

use super::{
  Callback, colors,
  format::{instance_subtitle, memory_usage},
};

#[derive(IntoElement)]
pub struct InstanceRow {
  id: ElementId,
  number: usize,
  /// [`super::format::format_duration`] of the uptime
  uptime: SharedString,
  account: Option<SharedString>,
  memory_used: Option<u64>,
  /// `settings.system_max_mem` in MB
  memory_total_mb: Option<u64>,
  card: bool,
  on_console: Option<Callback>,
  on_stop: Option<Callback>,
}

impl InstanceRow {
  pub fn new(id: impl Into<ElementId>, number: usize, uptime: impl Into<SharedString>) -> Self {
    Self {
      id: id.into(),
      number,
      uptime: uptime.into(),
      account: None,
      memory_used: None,
      memory_total_mb: None,
      card: false,
      on_console: None,
      on_stop: None,
    }
  }

  pub fn account(mut self, account: Option<SharedString>) -> Self {
    self.account = account;
    self
  }

  pub fn memory(mut self, used_bytes: Option<u64>, total_mb: Option<u64>) -> Self {
    self.memory_used = used_bytes;
    self.memory_total_mb = total_mb;
    self
  }

  /// Standalone ring card instead of a list row
  pub fn card(mut self, card: bool) -> Self {
    self.card = card;
    self
  }

  pub fn on_console(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_console = Some(Rc::new(f));
    self
  }

  pub fn on_stop(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_stop = Some(Rc::new(f));
    self
  }
}

impl RenderOnce for InstanceRow {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let subtitle = instance_subtitle(&self.uptime, self.account.as_deref(), self.card);
    let memory = memory_usage(self.memory_used, self.memory_total_mb);
    let (console, stop) = (self.on_console, self.on_stop);
    let accent = theme.accent.opacity(0.4);

    div()
      .id(self.id)
      .flex()
      .flex_wrap()
      .items_center()
      .gap_2()
      .map(|this| {
        if self.card {
          // -1px: a `ring-1` takes no space
          this
            .m(px(-1.))
            .px_4()
            .py(px(14.))
            .rounded(px(14.))
            .border_1()
            .border_color(theme.foreground.opacity(0.1))
            .bg(theme.popover)
        } else {
          this
            .px(px(14.))
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .hover(move |s| s.bg(accent))
        }
      })
      .child(
        div()
          .size_2()
          .flex_none()
          .rounded_full()
          .bg(colors::green_500()),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .flex_1()
          .min_w(px(180.))
          .child(
            div()
              .font_weight(FontWeight::SEMIBOLD)
              .when(self.card, |d| d.text_size(px(15.)))
              .child(format!("Instance {}", self.number)),
          )
          .child(
            div()
              .text_xs()
              .text_color(theme.muted_foreground)
              .child(subtitle),
          ),
      )
      .when_some(memory, |this, (label, percent)| {
        this.child(
          div()
            .mr_1()
            .flex()
            .flex_col()
            .w(px(180.))
            .gap(px(6.))
            .child(
              div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .text_xs()
                .child(div().text_color(theme.muted_foreground).child("Memory"))
                .child(label),
            )
            .child(Progress::new("memory").value(percent)),
        )
      })
      .child(
        Button::new("console")
          .outline()
          .label("Console")
          .when_some(console, |b, f| b.on_click(move |_, w, cx| f(w, cx))),
      )
      .child(
        Button::new("stop")
          .danger()
          .label("Stop")
          .when_some(stop, |b, f| b.on_click(move |_, w, cx| f(w, cx))),
      )
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use gpui_kit::{self as gpui, AppContext, TestAppContext, test::TestWindowExt};

  use super::*;
  use crate::test_view;

  #[gpui::test]
  fn console_and_stop(cx: &mut TestAppContext) {
    let hits = Rc::new(Cell::new((0, 0)));
    let (handle, _) = test_view::open(cx, {
      let hits = hits.clone();
      move |_, _| {
        let (a, b) = (hits.clone(), hits.clone());
        div()
          .w(px(700.))
          .child(
            InstanceRow::new("one", 1, "1h 12m")
              .account(Some("Steve".into()))
              .memory(Some(2 << 30), Some(8192))
              .card(true)
              .on_console(move |_, _| a.set((a.get().0 + 1, a.get().1)))
              .on_stop(move |_, _| b.set((b.get().0, b.get().1 + 1))),
          )
          .child(InstanceRow::new("two", 2, "<1m"))
          .into_any_element()
      }
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.within("one").click("console", cx);
      window.within("one").click("stop", cx);
    })
    .unwrap();
    assert_eq!(hits.get(), (1, 1));
  }
}
