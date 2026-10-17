use gpui_kit::{
  Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
  StatefulInteractiveElement, Styled, Subscription, Window,
  assets::IconName,
  component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{
  colors, form::open_destroy_dialog, format::running_label, profile::ProfileExternalLink,
};
use proton_profiles::{ProfilesExt, instance::InstanceInfo};

use super::{RowView, empty_state, group_by_profile, rows, running_summary, spawn_toast};
use crate::{
  actions::launch,
  router::{ProfileTab, Route, RouterExt},
};

/// `/instances`: running games grouped by profile
pub struct InstancesPage {
  groups: Vec<(InstanceInfo, Vec<Entity<RowView>>)>,
  count: usize,
  _instances: Subscription,
}

impl InstancesPage {
  pub fn new(cx: &mut Context<Self>) -> Self {
    let instances = cx.profiles().instances.clone();
    let mut this = Self {
      groups: Vec::new(),
      count: 0,
      _instances: cx.observe(&instances, |this: &mut Self, _, cx| {
        this.sync(cx);
        cx.notify();
      }),
    };
    this.sync(cx);
    this
  }

  fn sync(&mut self, cx: &mut Context<Self>) {
    let list = cx.profiles().instances.read(cx).clone();
    self.count = list.len();
    self.groups = group_by_profile(&list)
      .into_iter()
      .map(|group| {
        let views = rows(
          &group,
          false,
          |i| {
            format!(
              "Are you sure you want to stop this instance of profile \"{}\"?",
              i.profile_name
            )
          },
          cx,
        );
        (group[0].clone(), views)
      })
      .collect();
  }

  #[cfg(test)]
  pub(crate) fn row_count(&self) -> usize {
    self.groups.iter().map(|(_, r)| r.len()).sum()
  }
}

impl Render for InstancesPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let running = self.count > 0;

    let header = div()
      .flex()
      .items_center()
      .gap_2()
      .child(
        div()
          .flex_1()
          .child(
            div()
              .text_xl()
              .font_weight(FontWeight::SEMIBOLD)
              .child("Instances"),
          )
          .when(running, |el| {
            el.child(
              div()
                .mt_0p5()
                .text_color(theme.muted_foreground)
                .child(running_summary(self.count, self.groups.len())),
            )
          }),
      )
      .when(running, |el| {
        el.child(
          Button::new("stop-all")
            .cursor_pointer()
            .danger()
            .label("Stop all")
            .on_click(|_, window, cx| {
              open_destroy_dialog(
                window,
                cx,
                "Stop all instances",
                "Are you sure you want to stop every running instance?",
                "Stop all",
                |_, cx| {
                  let task = cx.profiles().stop_all(None);
                  spawn_toast("Failed to stop the instances", task, cx);
                },
              )
            }),
        )
      });

    let groups = self.groups.iter().map(|(first, views)| {
      let profile = first.profile_id.clone();
      let launch_id = profile.clone();
      div()
        .id(gpui_kit::SharedString::from(profile.clone()))
        .flex()
        .flex_col()
        .gap_2()
        .child(
          div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
              div()
                .id("profile-link")
                .cursor_pointer()
                .text_base()
                .font_weight(FontWeight::MEDIUM)
                .child(first.profile_name.clone())
                .on_click(move |_, _, cx| {
                  cx.navigate(Route::Profile {
                    id: profile.clone(),
                    tab: ProfileTab::Instances,
                  })
                }),
            )
            .child(
              div()
                .text_color(theme.muted_foreground)
                .child(format!("{:?} {}", first.loader, first.version)),
            )
            .child(
              div()
                .flex()
                .items_center()
                .h_5()
                .px_2()
                .rounded_full()
                .bg(colors::green_500().opacity(0.15))
                .text_color(colors::green_500())
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .child(running_label(views.len())),
            )
            .child(div().flex_1())
            .child(
              Button::new("launch-another")
                .cursor_pointer()
                .outline()
                .icon(Icon::new(IconName::Plus))
                .label("Launch another")
                .on_click(move |_, _, cx| launch(launch_id.clone(), None, cx)),
            ),
        )
        .child(
          div()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(14.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover.opacity(0.5))
            .children(views.iter().cloned()),
        )
    });

    div()
      .id("instances-page")
      .size_full()
      .overflow_y_scroll()
      .flex()
      .flex_col()
      .gap_4()
      .px_5()
      .pt_4()
      .pb_6()
      .child(header)
      .children(groups)
      .when(!running, |el| {
        el.child(
          empty_state(
            "No games running",
            "Launch a profile to see its instances here.",
          )
          .flex_none()
          .child(ProfileExternalLink::new(|_, cx| {
            cx.navigate(Route::Profiles)
          })),
        )
      })
  }
}
