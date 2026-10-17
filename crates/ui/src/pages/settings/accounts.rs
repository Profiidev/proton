//! `/settings/accounts`: list, browser login, select, remove

use gpui_kit::{
  AnyElement, Context, ElementId, FontWeight, IntoElement, ParentElement, Render, SharedString,
  Styled, Subscription, WeakEntity, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    notification::{Notification, NotificationType},
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_account::{AccountsExt, LoginState, auth::LoginStatus};
use proton_components::widgets::{account::AccountImage, form::open_destroy_dialog};

use super::{heading, offline_guard, page, ring_card, textures, toast};

/// Id of the "Waiting for Microsoft Login" toast
struct LoginToast;

pub struct AccountsPage {
  adding: bool,
  /// the login toast's Cancel was pressed, no result toast then
  cancelled: bool,
  _subs: [Subscription; 3],
}

pub fn step_message(step: &LoginStatus) -> &'static str {
  match step {
    LoginStatus::Ms => "Logging in to Xbox",
    LoginStatus::Xbox => "Logging in to Xbox Security",
    LoginStatus::XboxSecurity => "Logging in to Minecraft",
    LoginStatus::Mc => "Retrieving Minecraft Profile",
  }
}

impl AccountsPage {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let accounts = cx.accounts().clone();
    let subs = [
      cx.observe(&accounts.accounts, |_, _, cx| cx.notify()),
      cx.observe(&accounts.active, |_, _, cx| cx.notify()),
      cx.observe_in(&accounts.login, window, |this, login, window, cx| {
        if let (true, LoginState::Step(step)) = (this.adding, login.read(cx)) {
          let message = step_message(step);
          this.login_toast(message, window, cx);
        }
      }),
    ];
    Self {
      adding: false,
      cancelled: false,
      _subs: subs,
    }
  }

  fn login_toast(&self, message: &'static str, window: &mut Window, cx: &mut Context<Self>) {
    let page: WeakEntity<Self> = cx.entity().downgrade();
    let note = Notification::new()
      .id::<LoginToast>()
      .icon(Icon::new(IconName::LoaderCircle))
      .message(message)
      .autohide(false)
      .action(move |_, _, _| {
        let page = page.clone();
        Button::new("cancel-login")
          .cursor_pointer()
          .small()
          .outline()
          .label("Cancel")
          .on_click(move |_, _, cx| {
            page.update(cx, |p, _| p.cancelled = true).ok();
            cx.accounts().cancel_login();
          })
      });
    window.push_notification(note, cx);
  }

  fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.adding || offline_guard("add an Account", window, cx) {
      return;
    }
    self.adding = true;
    self.cancelled = false;
    self.login_toast("Waiting for Microsoft Login", window, cx);
    cx.notify();
    let task = cx.accounts().login();
    cx.spawn_in(window, async move |this, cx| {
      let res = task.await;
      this
        .update_in(cx, |this, window, cx| {
          window.remove_notification::<LoginToast>(cx);
          this.adding = false;
          cx.notify();
          if this.cancelled {
            return;
          }
          match res {
            Ok(()) => toast(
              window,
              cx,
              NotificationType::Success,
              "Successfully added Account",
            ),
            Err(e) => {
              tracing::error!("Login failed: {e:#}");
              toast(window, cx, NotificationType::Error, "Failed to add Account")
            }
          }
        })
        .ok();
    })
    .detach();
  }
}

fn select(id: String, window: &mut Window, cx: &mut gpui_kit::App) {
  let task = cx.accounts().set_active(id);
  window
    .spawn(cx, async move |cx| {
      if task.await.is_err() {
        cx.update(|window, cx| {
          toast(
            window,
            cx,
            NotificationType::Error,
            "Failed to switch Account",
          )
        })
        .ok();
      }
    })
    .detach();
}

fn remove(id: String, name: SharedString, window: &mut Window, cx: &mut gpui_kit::App) {
  open_destroy_dialog(
    window,
    cx,
    "Remove account",
    format!("{name} is signed out and removed from Proton."),
    "Remove",
    move |window, cx| {
      let task = cx.accounts().remove(id.clone());
      window
        .spawn(cx, async move |cx| {
          let (kind, message) = match task.await {
            Ok(()) => (NotificationType::Success, "Successfully removed Account"),
            Err(_) => (NotificationType::Error, "Failed to remove Account"),
          };
          cx.update(|window, cx| toast(window, cx, kind, message))
            .ok();
        })
        .detach();
    },
  );
}

impl Render for AccountsPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let accounts = cx.accounts();
    let active = accounts.active_id(cx).cloned();
    let mut list: Vec<_> = accounts.accounts.read(cx).clone().into_iter().collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    let theme = cx.theme();
    let muted = theme.muted_foreground;

    let header = div()
      .flex()
      .items_center()
      .gap_2()
      .child(heading(
        "Accounts",
        Some("The account marked Selected is used when you press Play.".into()),
        cx,
      ))
      .child(
        Button::new("add-account")
          .cursor_pointer()
          .primary()
          .label("Add account")
          .icon(IconName::Plus)
          .loading(self.adding)
          .disabled(self.adding)
          .when(self.adding, |b| b.cursor_default())
          .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
      );

    let mut children: Vec<AnyElement> = vec![header.into_any_element()];
    if list.is_empty() {
      children.push(
        div()
          .text_color(muted)
          .child("No accounts available")
          .into_any_element(),
      );
    }
    for (id, info) in list {
      let selected = active.as_deref() == Some(id.as_str());
      let image = match &info {
        Some(info) => AccountImage::new(
          ElementId::Name(format!("head-{id}").into()),
          textures::active_skin(info).map(|s| textures::head(s.url.clone())),
        )
        .into_any_element(),
        None => div()
          .flex()
          .flex_none()
          .size_8()
          .p_1()
          .child(Icon::new(IconName::CircleQuestionMark).size_6())
          .into_any_element(),
      };
      let name: SharedString = info
        .as_ref()
        .map_or("Login failed".into(), |i| i.name.clone().into());
      let shown_id = info.as_ref().map_or(id.clone(), |i| i.id.clone());
      let action = if selected {
        Some(
          div()
            .flex()
            .items_center()
            .h_5()
            .px_2()
            .rounded_full()
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .child("Selected")
            .into_any_element(),
        )
      } else {
        info.as_ref().map(|_| {
          let id = id.clone();
          Button::new(ElementId::Name(format!("select-{id}").into()))
            .cursor_pointer()
            .outline()
            .label("Select")
            .on_click(move |_, window, cx| select(id.clone(), window, cx))
            .into_any_element()
        })
      };
      let remove_id = id.clone();
      let remove_name = name.clone();
      children.push(
        ring_card(cx)
          .flex()
          .items_center()
          .gap_3()
          .px(px(14.))
          .py_3()
          .child(image)
          .child(
            div()
              .flex()
              .flex_col()
              .flex_1()
              .min_w_0()
              .child(
                div()
                  .truncate()
                  .line_height(px(24.))
                  .font_weight(FontWeight::MEDIUM)
                  .child(name),
              )
              .child(
                div()
                  .truncate()
                  .line_height(px(16.))
                  .font_family("monospace")
                  .text_xs()
                  .text_color(muted)
                  .child(shown_id),
              ),
          )
          .children(action)
          .child(
            Button::new(ElementId::Name(format!("remove-{id}").into()))
              .cursor_pointer()
              .ghost()
              .icon(Icon::new(IconName::Trash).text_color(theme.danger))
              .tooltip("Remove account")
              .on_click(move |_, window, cx| {
                remove(remove_id.clone(), remove_name.clone(), window, cx)
              }),
          )
          .into_any_element(),
      );
    }
    page("accounts-page", px(12.), children)
  }
}
