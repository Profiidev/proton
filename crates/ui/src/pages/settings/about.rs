//! `/settings/about`: version, update check, app switches

use gpui_kit::{
  Context, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled, Window,
  component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
    notification::NotificationType,
  },
  div, px,
};
use proton_components::widgets::form::SwitchTile;
use proton_config::SettingsExt;

use crate::updater::{UpdateState, UpdaterExt};

use super::{heading, offline_guard, page, ring_card, toast};

pub struct AboutPage;

impl AboutPage {
  pub fn new(cx: &mut Context<Self>) -> Self {
    cx.observe(&cx.settings().settings.clone(), |_, _, cx| cx.notify())
      .detach();
    cx.observe(&cx.updater(), |_, _, cx| cx.notify()).detach();
    Self
  }

  fn check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if offline_guard("check for updates", window, cx) {
      return;
    }
    let task = cx.updater().update(cx, |u, cx| u.check(cx));
    cx.spawn_in(window, async move |_, cx| {
      let res = task.await;
      cx.update(|window, cx| match res {
        Ok(Some(_)) => {}
        Ok(None) => toast(window, cx, NotificationType::Info, "No updates available"),
        Err(e) => {
          tracing::error!("Update check failed: {e:#}");
          toast(
            window,
            cx,
            NotificationType::Error,
            "Failed to check for updates",
          );
        }
      })
      .ok();
    })
    .detach();
  }
}

fn version_line(state: &UpdateState) -> SharedString {
  let version = env!("CARGO_PKG_VERSION");
  match state {
    UpdateState::Available { version: v, .. } => format!("Version {version} · {v} available"),
    UpdateState::Downloading {
      version: v,
      percent: Some(p),
    } => format!("Version {version} · downloading {v} ({p}%)"),
    UpdateState::Downloading { version: v, .. } => format!("Version {version} · downloading {v}"),
    UpdateState::Installing { version: v } => format!("Version {version} · installing {v}"),
    UpdateState::UpToDate => format!("Version {version} · up to date"),
    UpdateState::Error(_) => format!("Version {version} · update check failed"),
    UpdateState::Idle | UpdateState::Checking => format!("Version {version}"),
  }
  .into()
}

impl Render for AboutPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let settings = cx.settings().get(cx);
    let (auto_update, hide_controls) = (settings.auto_update, settings.hide_window_controls);
    let state = cx.updater().read(cx).state.clone();
    let notes = match &state {
      UpdateState::Available { notes, .. } => notes.clone(),
      _ => None,
    };
    let button = match &state {
      UpdateState::Available { version: v, .. }
      | UpdateState::Downloading { version: v, .. }
      | UpdateState::Installing { version: v } => {
        let busy = !matches!(state, UpdateState::Available { .. });
        Button::new("install-update")
          .primary()
          .label(format!("Install {v}"))
          .loading(busy)
          .disabled(busy)
          .on_click(|_, _, cx| crate::updater::install(cx))
      }
      state => Button::new("check-updates")
        .outline()
        .label("Check for updates")
        .loading(*state == UpdateState::Checking)
        .disabled(*state == UpdateState::Checking)
        .on_click(cx.listener(|this, _, window, cx| this.check(window, cx))),
    };

    page(
      "about-page",
      px(16.),
      [
        heading("About", None, cx).into_any_element(),
        ring_card(cx)
          .flex()
          .flex_wrap()
          .items_center()
          .gap_4()
          .p_4()
          .child(
            div()
              .flex()
              .flex_col()
              .flex_1()
              .min_w(px(200.))
              .child(
                div()
                  .line_height(px(24.))
                  .font_weight(FontWeight::MEDIUM)
                  .child("Proton"),
              )
              .child(
                div()
                  .text_xs()
                  .line_height(px(16.))
                  .text_color(cx.theme().muted_foreground)
                  .child(version_line(&state)),
              )
              .children(notes.map(|n| {
                div()
                  .pt_1()
                  .text_xs()
                  .line_height(px(16.))
                  .text_color(cx.theme().muted_foreground)
                  .child(n)
              })),
          )
          .child(button)
          .into_any_element(),
        SwitchTile::new(
          "auto-update",
          "Update automatically",
          "Install new versions when the launcher starts.",
        )
        .checked(auto_update)
        .on_change(|v, _, cx| {
          let v = *v;
          proton_config::update(cx, |s| s.auto_update = v)
        })
        .into_any_element(),
        SwitchTile::new(
          "hide-window-controls",
          "Hide window controls",
          "Hide minimize, maximize and close in the title bar, e.g. for tiling window managers.",
        )
        .checked(hide_controls)
        .on_change(|v, _, cx| {
          let v = *v;
          proton_config::update(cx, |s| s.hide_window_controls = v)
        })
        .into_any_element(),
      ],
    )
  }
}
