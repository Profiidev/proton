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

use super::{heading, offline_guard, page, ring_card, toast};

/// Looks for a newer release, `Some(version)` when there is one.
// ponytail: stub until Phase 4 wires cargo-packager-updater (signed latest.json on GitHub
// releases); the UI states below already follow the old updater flow
pub async fn check_update() -> anyhow::Result<Option<String>> {
  Ok(None)
}

/// Downloads and installs `version`, then restarts.
// ponytail: stub, see `check_update`
pub async fn install_update(version: String) -> anyhow::Result<()> {
  anyhow::bail!("Updater not available yet ({version})")
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
  Idle,
  Checking,
  UpToDate,
  Available(String),
  Installing(String),
}

pub struct AboutPage {
  pub(super) update: UpdateState,
}

impl AboutPage {
  pub fn new(cx: &mut Context<Self>) -> Self {
    cx.observe(&cx.settings().settings.clone(), |_, _, cx| cx.notify())
      .detach();
    Self {
      update: UpdateState::Idle,
    }
  }

  fn check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.update == UpdateState::Checking || offline_guard("check for updates", window, cx) {
      return;
    }
    self.update = UpdateState::Checking;
    cx.notify();
    let task = check_update();
    cx.spawn_in(window, async move |this, cx| {
      let res = task.await;
      this
        .update_in(cx, |this, window, cx| {
          this.update = match res {
            Ok(Some(v)) => UpdateState::Available(v),
            Ok(None) => {
              toast(window, cx, NotificationType::Info, "No updates available");
              UpdateState::UpToDate
            }
            Err(e) => {
              tracing::error!("Update check failed: {e:#}");
              toast(
                window,
                cx,
                NotificationType::Error,
                "Failed to check for updates",
              );
              UpdateState::Idle
            }
          };
          cx.notify();
        })
        .ok();
    })
    .detach();
  }

  fn install(&mut self, version: String, window: &mut Window, cx: &mut Context<Self>) {
    self.update = UpdateState::Installing(version.clone());
    cx.notify();
    let task = install_update(version.clone());
    cx.spawn_in(window, async move |this, cx| {
      let res = task.await;
      this
        .update_in(cx, |this, window, cx| {
          if let Err(e) = res {
            tracing::error!("Update failed: {e:#}");
            toast(
              window,
              cx,
              NotificationType::Error,
              "Failed to download update",
            );
          }
          this.update = UpdateState::Available(version);
          cx.notify();
        })
        .ok();
    })
    .detach();
  }
}

fn version_line(state: &UpdateState) -> SharedString {
  let version = env!("CARGO_PKG_VERSION");
  match state {
    UpdateState::Available(v) | UpdateState::Installing(v) => {
      format!("Version {version} · {v} available")
    }
    UpdateState::UpToDate => format!("Version {version} · up to date"),
    _ => format!("Version {version}"),
  }
  .into()
}

impl Render for AboutPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let settings = cx.settings().get(cx);
    let (auto_update, hide_controls) = (settings.auto_update, settings.hide_window_controls);
    let button = match &self.update {
      UpdateState::Available(v) | UpdateState::Installing(v) => {
        let version = v.clone();
        Button::new("install-update")
          .primary()
          .label(format!("Install {v}"))
          .loading(matches!(self.update, UpdateState::Installing(_)))
          .disabled(matches!(self.update, UpdateState::Installing(_)))
          .on_click(
            cx.listener(move |this, _, window, cx| this.install(version.clone(), window, cx)),
          )
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
                  .child(version_line(&self.update)),
              ),
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
