//! Header chip + popover over `proton_versions`' downloads. Each row is its own entity: a
//! progress tick re-renders only that row and the chip, never the panel or the header. Rows
//! have a fixed height per state so the open popover draws them `.cached()`.

use gpui_kit::{
  Anchor, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
  StatefulInteractiveElement, StyleRefinement, Styled, Subscription, Window,
  assets::IconName,
  base::TestSupportExt,
  component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    popover::Popover,
    progress::Progress,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_versions::{
  VersionsExt,
  downloads::{DownloadProgress, chip_label, running_percent},
};

/// Chip label and bar, re-rendered on ticks
#[derive(Default, PartialEq)]
struct Chip {
  running: usize,
  percent: f32,
}

impl Render for Chip {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
      .flex()
      .items_center()
      .gap_1p5()
      .child(Icon::new(IconName::Download).size_3p5())
      .child(chip_label(self.running))
      .when(self.running > 0, |el| {
        el.child(
          Progress::new("downloads-progress")
            .w(px(40.))
            .value(self.percent),
        )
      })
  }
}

/// Title line, progress bar and detail line heights; [`row_style`] adds them up
const TITLE_H: f32 = 20.;
const BAR_H: f32 = 8.;
const DETAIL_H: f32 = 16.;
const ROW_GAP: f32 = 6.;

fn row_style(running: bool) -> StyleRefinement {
  let h = TITLE_H + ROW_GAP + BAR_H + if running { ROW_GAP + DETAIL_H } else { 0. };
  StyleRefinement::default().w_full().h(px(h)).flex_none()
}

/// One job in the popover
pub struct DownloadRowView {
  row: Entity<DownloadProgress>,
}

impl DownloadRowView {
  fn new(row: Entity<DownloadProgress>, cx: &mut Context<Self>) -> Self {
    cx.observe(&row, |_, _, cx| cx.notify()).detach();
    Self { row }
  }
}

impl Render for DownloadRowView {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    proton_components::render_count::hit::<Self>();
    let theme = cx.theme();
    let row = self.row.read(cx);
    let id = row.id;
    let running = row.is_running();
    let speed = row.speed_label();
    // fill the cached wrapper, else the bar and the spacers collapse to content width
    div()
      .size_full()
      .flex()
      .flex_col()
      .gap_1p5()
      .child(
        div()
          .flex()
          .items_center()
          .h(px(TITLE_H))
          .gap_2()
          .child(
            div()
              .flex_1()
              .truncate()
              .font_medium()
              .child(row.title.clone()),
          )
          .child(
            div()
              .text_xs()
              .text_color(theme.muted_foreground)
              .child(row.percent_label()),
          )
          .when(running, |el| {
            el.child(
              div()
                .id(format!("download-cancel-{id}"))
                .test_support()
                .child(
                  Button::new(("download-cancel", id))
                    .cursor_pointer()
                    .ghost()
                    .xsmall()
                    .icon(IconName::X)
                    .tooltip("Cancel download")
                    .disabled(row.cancel_requested)
                    .when(row.cancel_requested, |b| b.cursor_default())
                    .on_click(move |_, _, cx| cx.versions().clone().cancel_download(id, cx)),
                ),
            )
          }),
      )
      .child(Progress::new(("download-progress", id)).value(row.percent() as f32))
      .when(running, |el| {
        el.child(
          div()
            .flex()
            .justify_between()
            .h(px(DETAIL_H))
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(row.detail())
            .when(!speed.is_empty(), |el| el.child(speed)),
        )
      })
  }
}

/// Hidden while there are no jobs
pub struct DownloadsPanel {
  rows: Vec<Entity<DownloadRowView>>,
  chip: Entity<Chip>,
  /// per row, picks its cached height; all true disables "Clear finished"
  running: Vec<bool>,
  /// one per row, rebuilt when the list changes
  row_subs: Vec<Subscription>,
}

impl DownloadsPanel {
  pub fn new(cx: &mut Context<Self>) -> Self {
    let list = cx.versions().downloads.clone();
    cx.observe(&list, |this, _, cx| {
      this.sync_rows(cx);
      cx.notify();
    })
    .detach();
    let mut this = Self {
      rows: vec![],
      chip: cx.new(|_| Chip::default()),
      running: vec![],
      row_subs: vec![],
    };
    this.sync_rows(cx);
    this
  }

  fn sync_rows(&mut self, cx: &mut Context<Self>) {
    let rows: Vec<_> = cx.versions().downloads.read(cx).rows().cloned().collect();
    self.row_subs = rows
      .iter()
      .map(|row| cx.observe(row, |this, _, cx| this.sync_summary(cx)))
      .collect();
    self.rows = rows
      .into_iter()
      .map(|row| cx.new(|cx| DownloadRowView::new(row, cx)))
      .collect();
    self.sync_summary(cx);
  }

  /// Updates the chip; the panel itself re-renders only when a row starts or stops running
  fn sync_summary(&mut self, cx: &mut Context<Self>) {
    let list = cx.versions().downloads.read(cx);
    let rows: Vec<&DownloadProgress> = list.rows().map(|r| r.read(cx)).collect();
    let flags: Vec<bool> = rows.iter().map(|r| r.is_running()).collect();
    let chip = Chip {
      running: flags.iter().filter(|r| **r).count(),
      percent: running_percent(rows.iter().copied()) as f32,
    };
    self.chip.update(cx, |c, cx| {
      if *c != chip {
        *c = chip;
        cx.notify();
      }
    });
    if flags != self.running {
      self.running = flags;
      cx.notify();
    }
  }
}

impl Render for DownloadsPanel {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    if self.rows.is_empty() {
      return div().into_any_element();
    }
    let trigger = Button::new("downloads")
      .cursor_pointer()
      .ghost()
      .h_7()
      .px(px(10.))
      .rounded_full()
      .bg(cx.theme().muted)
      .text_xs()
      .child(self.chip.clone());

    let rows: Vec<_> = self
      .rows
      .iter()
      .zip(&self.running)
      .map(|(row, running)| (row.clone(), *running))
      .collect();
    let all_running = self.running.iter().all(|r| *r);
    Popover::new("downloads-popover")
      .p_1()
      .anchor(Anchor::TopRight)
      .trigger(trigger)
      .content(move |_, _, cx| panel(&rows, all_running, cx))
      .into_any_element()
  }
}

fn panel(
  rows: &[(Entity<DownloadRowView>, bool)],
  all_running: bool,
  cx: &mut gpui_kit::App,
) -> impl IntoElement + use<> {
  let theme = cx.theme();
  div()
    .w(px(400.))
    .flex()
    .flex_col()
    .gap_3()
    .child(
      div()
        .flex()
        .items_center()
        .justify_between()
        .child(div().font_medium().child("Downloads"))
        .child(
          Button::new("downloads-clear")
            .cursor_pointer()
            .ghost()
            .xsmall()
            .label("Clear finished")
            .disabled(all_running)
            .when(all_running, |b| b.cursor_default())
            .on_click(|_, _, cx| cx.versions().clone().clear_finished(cx)),
        ),
    )
    .child(
      div()
        .id("downloads-list")
        .max_h(px(384.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_3()
        .children(
          rows
            .iter()
            .map(|(row, running)| row.clone().cached(row_style(*running))),
        ),
    )
    .child(
      div().text_xs().text_color(theme.muted_foreground).child(
        "Downloads continue while you browse. Games start as soon as their files are ready.",
      ),
    )
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use gpui_kit::{self as gpui, TestAppContext, test::TestWindowExt};
  use proton_versions::{downloads::DownloadStatus, event::DownloadCheckStatus};

  use super::*;

  fn count<T: 'static>(cx: &mut TestAppContext, entity: &Entity<T>) -> Rc<Cell<usize>> {
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    cx.update(|cx| {
      cx.observe(entity, move |_, _| seen.set(seen.get() + 1))
        .detach()
    });
    count
  }

  #[gpui::test]
  fn tick_reaches_only_its_row_and_cancel_calls_the_service(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap().keep();
    proton_store::init_at(dir.join("store.json"));
    let (window, panel) = cx.update(|cx| {
      gpui_kit::init(cx);
      proton_components::assets::load(cx).unwrap();
      proton_config::init(cx);
      proton_versions::init_state(cx);
      let (window, panel) =
        gpui_kit::open_window(Default::default(), cx, |_, cx| cx.new(DownloadsPanel::new)).unwrap();
      (window, panel)
    });
    let handle = cx.update(|cx| cx.versions().handle.clone());
    let a = handle.start("A");
    let _b = handle.start("B");
    cx.run_until_parked();

    let views = panel.read_with(cx, |p, _| p.rows.clone());
    let chip = panel.read_with(cx, |p, _| p.chip.clone());
    let (panel_n, a_n, b_n, chip_n) = (
      count(cx, &panel),
      count(cx, &views[0]),
      count(cx, &views[1]),
      count(cx, &chip),
    );
    handle.send(proton_versions::event::DownloadMsg::Status {
      id: a,
      status: DownloadCheckStatus::AssetsCheck(1, 2),
    });
    cx.run_until_parked();
    assert_eq!(
      (panel_n.get(), a_n.get(), b_n.get(), chip_n.get()),
      (0, 1, 0, 1)
    );

    // open the popover, click A's cancel
    cx.update_window(window, |_, window, cx| {
      window.render_frame(cx);
      window.click("downloads".to_string(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
      window.render_frame(cx);
      window.click(format!("download-cancel-{a}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    let row = cx.read(|cx| cx.versions().downloads.read(cx).get(a).unwrap());
    assert!(row.read_with(cx, |r, _| r.cancel_requested));
    let before = panel_n.get();
    handle.finish(a, true);
    cx.run_until_parked();
    assert_eq!(row.read_with(cx, |r, _| r.status), DownloadStatus::Canceled);
    // "Clear finished" got enabled
    assert_eq!(panel_n.get(), before + 1);
    cx.update_window(window, |_, window, cx| {
      window.render_frame(cx);
      window.click("downloads-clear".to_string(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(cx.read(|cx| cx.versions().downloads.read(cx).get(a).is_none()));
    assert_eq!(panel.read_with(cx, |p, _| p.rows.len()), 1);
  }
}
