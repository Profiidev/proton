//! "Installing to {profile}" popover button (`TargetPicker.svelte`)

use std::rc::Rc;

use gpui_kit::{
  Anchor, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
  StatefulInteractiveElement, Styled, Subscription, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    popover::Popover,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::profile::ProfileIcon;
use proton_profiles::{ProfilesExt, config::Profile};

use super::{logic, profile_icon, profile_icons, set_target, target};

/// Which profiles can be picked
pub type ProfileFilter = Rc<dyn Fn(&Profile, &App) -> bool>;

/// Profiles `filter` accepts, and the target among them (remembered, latest played, first)
pub fn candidates(filter: &ProfileFilter, cx: &mut App) -> (Vec<Profile>, Option<Profile>) {
  let remembered = target(cx).read(cx).clone();
  let list: Vec<Profile> = cx
    .profiles()
    .profiles
    .read(cx)
    .iter()
    .filter(|p| filter(p, cx))
    .cloned()
    .collect();
  let target = logic::pick_target(&list, remembered.as_deref()).cloned();
  (list, target)
}

/// Popover button that sets the remembered content target
pub struct TargetPicker {
  filter: ProfileFilter,
  search: Entity<InputState>,
  _subs: [Subscription; 4],
}

impl TargetPicker {
  pub fn new(filter: ProfileFilter, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search profiles"));
    let profiles = cx.profiles().profiles.clone();
    let target = target(cx);
    let icons = profile_icons(cx);
    Self {
      filter,
      _subs: [
        cx.observe(&profiles, |_, _, cx| cx.notify()),
        cx.observe(&target, |_, _, cx| cx.notify()),
        cx.observe(&icons, |_, _, cx| cx.notify()),
        cx.subscribe(&search, |_, _, e: &InputEvent, cx| {
          if matches!(e, InputEvent::Change) {
            cx.notify()
          }
        }),
      ],
      search,
    }
  }

  /// Changes which profiles can be picked (e.g. once the project loaded)
  pub fn set_filter(&mut self, filter: ProfileFilter, cx: &mut Context<Self>) {
    self.filter = filter;
    cx.notify();
  }
}

impl Render for TargetPicker {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let (list, current) = candidates(&self.filter, cx);
    let muted = cx.theme().muted_foreground;
    let needle = self.search.read(cx).value().trim().to_lowercase();
    let shown: Vec<(Profile, bool, Option<_>)> = list
      .into_iter()
      .filter(|p| p.name.to_lowercase().contains(&needle))
      .map(|p| {
        let selected = current.as_ref().is_some_and(|c| c.id == p.id);
        let icon = profile_icon(&p.id, cx);
        (p, selected, icon)
      })
      .collect();
    let empty = shown.is_empty() && needle.is_empty();

    let trigger = Button::new("target-picker")
      .cursor_pointer()
      .outline()
      .h_10()
      .max_w_full()
      .disabled(empty)
      .when(empty, |b| b.cursor_default())
      .child(
        div()
          .flex()
          .items_center()
          .gap(px(6.))
          .flex_1()
          .min_w_0()
          .map(|el| match &current {
            Some(p) => el
              .child(
                ProfileIcon::new(profile_icon(&p.id, cx))
                  .size(px(24.))
                  .fallback_size(px(16.)),
              )
              .child(div().flex_none().text_color(muted).child("Installing to"))
              // the name gives way first, then the loader (the web app let it overflow)
              .child(div().min_w_0().truncate().child(p.name.clone()))
              .child(
                div()
                  .min_w_0()
                  .flex_shrink(0.01)
                  .truncate()
                  .text_color(muted)
                  .child(format!("{} {}", logic::loader_label(p.loader), p.version)),
              ),
            None => el.child(div().text_color(muted).child("No profile to install to")),
          })
          .child(Icon::new(IconName::ChevronDown).small().flex_none()),
      );

    let search = self.search.clone();
    Popover::new("target-picker-popover")
      .anchor(Anchor::TopRight)
      .trigger(trigger)
      .content(move |_, _, cx| {
        let popover = cx.entity();
        let muted = cx.theme().muted_foreground;
        div()
          .w(px(288.))
          .flex()
          .flex_col()
          .gap_1()
          .child(Input::new(&search).small())
          .child(
            div()
              .id("target-list")
              .flex()
              .flex_col()
              .max_h(px(320.))
              .overflow_y_scroll()
              .when(shown.is_empty(), |el| {
                el.child(
                  div()
                    .p_2()
                    .text_sm()
                    .text_color(muted)
                    .child("No matching profiles."),
                )
              })
              .children(shown.iter().map(|(p, selected, icon)| {
                let id = p.id.clone();
                let popover = popover.clone();
                Button::new(format!("target-{}", p.id))
                  .cursor_pointer()
                  .map(|b| if *selected { b.secondary() } else { b.ghost() })
                  .h_auto()
                  .p_2()
                  .child(
                    // button content centers, so the row fills it to stay left aligned
                    div()
                      .w_full()
                      .flex()
                      .items_center()
                      .gap(px(10.))
                      .child(
                        ProfileIcon::new(icon.clone())
                          .size(px(32.))
                          .fallback_size(px(20.)),
                      )
                      .child(
                        div()
                          .flex()
                          .flex_col()
                          .min_w_0()
                          .child(div().truncate().text_sm().child(p.name.clone()))
                          .child(div().text_xs().text_color(muted).child(format!(
                            "{} {}",
                            logic::loader_label(p.loader),
                            p.version
                          ))),
                      ),
                  )
                  .cursor_pointer()
                  .on_click(move |_, window, cx| {
                    set_target(cx, Some(id.clone()));
                    popover.update(cx, |p, cx| p.dismiss(window, cx));
                  })
              })),
          )
      })
  }
}

/// Every profile can be picked
pub fn any_profile() -> ProfileFilter {
  Rc::new(|_, _| true)
}
