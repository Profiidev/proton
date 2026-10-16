//! Sidebar footer: the active account, a popover to switch accounts

use gpui_kit::{
  AnyElement, App, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Render,
  SharedString, Styled, Window,
  assets::IconName,
  base::TestSupportExt,
  component::{
    ActiveTheme, Icon, Sizable, TaskExt,
    button::{Button, ButtonVariants},
    popover::Popover,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_account::{AccountsExt, info::ProfileInfo};
use proton_components::widgets::account::AccountImage;
use proton_utils::log::ErrorLogExt;

use super::textures::{active_skin, head};
use crate::{
  router::{Route, RouterExt, SettingsTab},
  shell::COMPACT_BELOW,
};

pub struct AccountSelector {
  open: bool,
}

impl AccountSelector {
  pub fn new(cx: &mut Context<Self>) -> Self {
    let accounts = cx.accounts().clone();
    // both entities notify only on change
    cx.observe(&accounts.accounts, |_, _, cx| cx.notify()).detach();
    cx.observe(&accounts.active, |_, _, cx| cx.notify()).detach();
    Self { open: false }
  }
}

/// Head + name + id, text hidden when `compact`
fn identity(
  id: impl Into<ElementId>,
  info: Option<&ProfileInfo>,
  fallback_id: &str,
  compact: bool,
  cx: &App,
) -> Vec<AnyElement> {
  let muted = cx.theme().muted_foreground;
  let image = match info {
    Some(info) => AccountImage::new(id, active_skin(info).map(|s| head(s.url.clone())))
      .into_any_element(),
    None => div()
      .flex()
      .flex_none()
      .size_8()
      .items_center()
      .justify_center()
      .child(Icon::new(IconName::CircleQuestionMark))
      .into_any_element(),
  };
  let name: SharedString = info.map_or("Login failed".into(), |i| i.name.clone().into());
  let sub: SharedString = info.map_or(fallback_id.to_string(), |i| i.id.clone()).into();
  let mut out = vec![image];
  if !compact {
    out.push(
      div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .text_left()
        .child(div().truncate().text_sm().child(name))
        .child(div().truncate().text_xs().text_color(muted).child(sub))
        .into_any_element(),
    );
  }
  out
}

impl Render for AccountSelector {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let compact = window.viewport_size().width < COMPACT_BELOW;
    let accounts = cx.accounts();
    let active = accounts.active_id(cx).cloned();
    let info = active
      .as_ref()
      .and_then(|id| accounts.accounts.read(cx).get(id).cloned().flatten());
    let muted = cx.theme().muted_foreground;

    let mut trigger = Button::new("account-selector")
      .ghost()
      .w_full()
      .h_12()
      .px_2()
      .gap(px(10.))
      .justify_start();
    trigger = match &info {
      Some(info) => trigger.children(identity("account-head", Some(info), "", compact, cx)),
      None => trigger
        .child(
          div()
            .flex()
            .flex_none()
            .size_8()
            .items_center()
            .justify_center()
            .child(Icon::new(IconName::CircleQuestionMark)),
        )
        .when(!compact, |t| {
          t.child(div().flex_1().truncate().text_sm().text_left().child("No account active"))
        }),
    };
    let trigger = trigger.when(!compact, |t| {
      t.child(
        Icon::new(IconName::ChevronsUpDown)
          .size_4()
          .flex_none()
          .text_color(muted),
      )
    });

    let this = cx.entity().downgrade();
    let close = this.clone();
    Popover::new("account-popover")
      .anchor(gpui_kit::Corner::BottomLeft)
      .open(self.open)
      .on_open_change(move |open, _, cx| {
        this
          .update(cx, |s, cx| {
            s.open = *open;
            cx.notify();
          })
          .ok();
      })
      .trigger_style(gpui_kit::StyleRefinement::default().w_full())
      .trigger(trigger)
      .content(move |_, _, cx| {
        let accounts = cx.accounts().clone();
        let mut list: Vec<_> = accounts.accounts.read(cx).clone().into_iter().collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        let close = close.clone();
        div()
          .flex()
          .flex_col()
          .w(px(200.))
          .when(list.is_empty(), |d| {
            d.items_center().gap_1().p_1().child(div().p_2().child("No Accounts")).child(
              Button::new("add-one")
                .outline()
                .small()
                .label("Add One")
                .icon(IconName::ExternalLink)
                .on_click({
                  let close = close.clone();
                  move |_, _, cx| {
                    close
                      .update(cx, |s, cx| {
                        s.open = false;
                        cx.notify();
                      })
                      .ok();
                    cx.navigate(Route::Settings(SettingsTab::Accounts));
                  }
                }),
            )
          })
          .children(list.into_iter().map(|(id, info)| {
            let row_id = SharedString::from(format!("account-{id}"));
            match info {
              Some(info) => {
                let close = close.clone();
                let select = id.clone();
                Button::new(ElementId::Name(row_id))
                  .ghost()
                  .w_full()
                  .h_12()
                  .p_2()
                  .gap(px(10.))
                  .justify_start()
                  .children(identity(
                    ElementId::Name(format!("head-{id}").into()),
                    Some(&info),
                    &id,
                    false,
                    cx,
                  ))
                  .on_click(move |_, _, cx| {
                    close
                      .update(cx, |s, cx| {
                        s.open = false;
                        cx.notify();
                      })
                      .ok();
                    let task = cx.accounts().set_active(select.clone());
                    cx.background_spawn(async move {
                      let _ = task.await.log_err();
                    })
                    .detach();
                  })
                  .into_any_element()
              }
              None => div()
                .id(ElementId::Name(row_id))
                .flex()
                .items_center()
                .gap(px(10.))
                .h_14()
                .p_2()
                .children(identity("failed", None, &id, false, cx))
                .test_support()
                .into_any_element(),
            }
          }))
      })
  }
}
