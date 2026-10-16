use gpui_kit::{
  Anchor, App, AppContext, Context, Entity, Global, InteractiveElement, IntoElement, ParentElement,
  Render, SharedString, StatefulInteractiveElement, Styled, Window,
  assets::IconName,
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

/// One job in the downloads panel. The downloads service owns the list
/// ([`DownloadsExt::downloads`]) and the panel only renders it.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadRow {
  pub id: u64,
  pub title: SharedString,
  pub state: DownloadState,
  /// 0..=100, `None` while unknown
  pub percent: Option<f32>,
  /// Stage text, e.g. "Downloading assets · 12 of 300 MiB"
  pub detail: SharedString,
  /// e.g. "4.2 MB/s"
  pub speed: Option<SharedString>,
  /// Set by the panel's cancel button, the service observes the list and acts on it
  pub cancel_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
  Running,
  Done,
  Failed,
  Canceled,
}

struct GlobalDownloads(Entity<Vec<DownloadRow>>);

impl Global for GlobalDownloads {}

pub(crate) fn init(cx: &mut App) {
  let rows = cx.new(|_| vec![]);
  cx.set_global(GlobalDownloads(rows));
}

pub trait DownloadsExt {
  fn downloads(&self) -> Entity<Vec<DownloadRow>>;
}

impl DownloadsExt for App {
  fn downloads(&self) -> Entity<Vec<DownloadRow>> {
    self.global::<GlobalDownloads>().0.clone()
  }
}

fn chip_label(running: usize) -> String {
  match running {
    0 => "Downloads".into(),
    1 => "1 download".into(),
    n => format!("{n} downloads"),
  }
}

fn percent_label(row: &DownloadRow) -> String {
  match row.state {
    DownloadState::Done => "Done".into(),
    DownloadState::Failed => "Failed".into(),
    DownloadState::Canceled => "Canceled".into(),
    DownloadState::Running => row
      .percent
      .map(|p| format!("{}%", p.round()))
      .unwrap_or_default(),
  }
}

/// Header chip + popover; hidden while there are no jobs
pub struct DownloadsPanel {
  rows: Entity<Vec<DownloadRow>>,
}

impl DownloadsPanel {
  pub fn new(cx: &mut Context<Self>) -> Self {
    let rows = cx.downloads();
    cx.observe(&rows, |_, _, cx| cx.notify()).detach();
    Self { rows }
  }
}

impl Render for DownloadsPanel {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let rows = self.rows.read(cx);
    if rows.is_empty() {
      return div().into_any_element();
    }
    let running: Vec<_> = rows
      .iter()
      .filter(|r| r.state == DownloadState::Running)
      .collect();
    let average =
      running.iter().map(|r| r.percent.unwrap_or(0.)).sum::<f32>() / running.len().max(1) as f32;
    let label = chip_label(running.len());
    let any_running = !running.is_empty();
    let theme = cx.theme();

    let trigger = Button::new("downloads")
      .ghost()
      .h_7()
      .px(px(10.))
      .rounded_full()
      .bg(theme.muted)
      .text_xs()
      .child(
        div()
          .flex()
          .items_center()
          .gap_1p5()
          .child(Icon::new(IconName::Download).size_3p5())
          .child(label)
          .when(any_running, |el| {
            el.child(
              Progress::new("downloads-progress")
                .w(px(40.))
                .value(average),
            )
          }),
      );

    let list = self.rows.clone();
    Popover::new("downloads-popover")
      .anchor(Anchor::TopRight)
      .trigger(trigger)
      .content(move |_, _, cx| panel(&list, cx))
      .into_any_element()
  }
}

fn panel(list: &Entity<Vec<DownloadRow>>, cx: &mut App) -> impl IntoElement + use<> {
  let theme = cx.theme();
  let rows = list.read(cx).clone();
  let all_running = rows.iter().all(|r| r.state == DownloadState::Running);
  let clear = list.clone();
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
            .ghost()
            .xsmall()
            .label("Clear finished")
            .disabled(all_running)
            .on_click(move |_, _, cx| {
              clear.update(cx, |rows, cx| {
                rows.retain(|r| r.state == DownloadState::Running);
                cx.notify();
              })
            }),
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
        .children(rows.into_iter().map(|row| {
          let cancel = list.clone();
          let id = row.id;
          div()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(
              div()
                .flex()
                .items_center()
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
                    .child(percent_label(&row)),
                )
                .when(row.state == DownloadState::Running, |el| {
                  el.child(
                    Button::new(("download-cancel", id))
                      .ghost()
                      .xsmall()
                      .icon(IconName::X)
                      .tooltip("Cancel download")
                      .disabled(row.cancel_requested)
                      .on_click(move |_, _, cx| {
                        cancel.update(cx, |rows, cx| {
                          if let Some(r) = rows.iter_mut().find(|r| r.id == id) {
                            r.cancel_requested = true;
                            cx.notify();
                          }
                        })
                      }),
                  )
                }),
            )
            .child(
              Progress::new(("download-progress", id)).value(match row.state {
                DownloadState::Done => 100.,
                _ => row.percent.unwrap_or(0.),
              }),
            )
            .when(row.state == DownloadState::Running, |el| {
              el.child(
                div()
                  .flex()
                  .justify_between()
                  .text_xs()
                  .text_color(theme.muted_foreground)
                  .child(row.detail.clone())
                  .children(row.speed.clone()),
              )
            })
        })),
    )
    .child(
      div().text_xs().text_color(theme.muted_foreground).child(
        "Downloads continue while you browse. Games start as soon as their files are ready.",
      ),
    )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn labels() {
    assert_eq!(chip_label(0), "Downloads");
    assert_eq!(chip_label(1), "1 download");
    assert_eq!(chip_label(3), "3 downloads");
    let mut row = DownloadRow {
      id: 1,
      title: "x".into(),
      state: DownloadState::Running,
      percent: None,
      detail: "".into(),
      speed: None,
      cancel_requested: false,
    };
    assert_eq!(percent_label(&row), "");
    row.percent = Some(41.6);
    assert_eq!(percent_label(&row), "42%");
    row.state = DownloadState::Failed;
    assert_eq!(percent_label(&row), "Failed");
  }
}
