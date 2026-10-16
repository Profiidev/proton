use gpui_kit::{
  Anchor, App, ClickEvent, Context, FontWeight, ImageSource, InteractiveElement, IntoElement,
  ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
  assets::IconName,
  component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
    notification::Notification,
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::widgets::{
  colors, dot_chip,
  form::open_destroy_dialog,
  format::{format_datetime, running_label},
  profile::ProfileIcon,
};
use proton_config::OfflineExt;
use proton_content::Content;
use proton_profiles::{ProfilesExt, config::Profile};

use super::{MenuAction, find_profile, load_icon, run_menu_action};
use crate::{
  actions::{launch, toast, toast_error},
  router::{Route, RouterExt},
};

/// Icon, name, play/stop, favorite and the profile menu above the profile tabs
pub struct ProfileHeader {
  id: String,
  profile: Option<Profile>,
  icon: Option<ImageSource>,
  running: usize,
  pack_update: Option<String>,
  _subscriptions: Vec<Subscription>,
}

impl ProfileHeader {
  pub fn new(id: String, _: &mut Window, cx: &mut Context<Self>) -> Self {
    let profiles = cx.profiles().clone();
    let mut subscriptions = vec![
      cx.observe(&profiles.profiles, |this, _, cx| this.sync_profile(cx)),
      cx.observe(&profiles.instances, |this, _, cx| this.sync_running(cx)),
    ];
    if let Some(content) = cx.try_global::<Content>().cloned() {
      subscriptions.push(cx.observe(&content.pack_updates, |this, _, cx| {
        this.sync_pack_update(cx)
      }));
    }
    let mut this = Self {
      id,
      profile: None,
      icon: None,
      running: 0,
      pack_update: None,
      _subscriptions: subscriptions,
    };
    this.sync_profile(cx);
    this.sync_running(cx);
    this.sync_pack_update(cx);
    this
  }

  fn sync_profile(&mut self, cx: &mut Context<Self>) {
    self.profile = find_profile(&self.id, cx);
    if self.profile.is_none() {
      // deleted (or a stale saved route): back to the list like the old layout
      if matches!(cx.route(), Route::Profile { id, .. } if id == self.id) {
        let router = cx.router();
        cx.defer(move |cx| router.update(cx, |r, cx| r.replace(Route::Profiles, cx)));
      }
      return;
    }
    // ponytail: re-reads the icon on every profiles change, one small file
    let load = load_icon(self.id.clone(), cx);
    cx.spawn(async move |this, cx| {
      let icon = load.await;
      this.update(cx, |this, cx| {
        this.icon = icon;
        cx.notify();
      })
    })
    .detach();
    cx.notify();
  }

  fn sync_running(&mut self, cx: &mut Context<Self>) {
    let running = cx
      .profiles()
      .instances
      .read(cx)
      .iter()
      .filter(|i| i.profile_id == self.id)
      .count();
    if running != self.running {
      self.running = running;
      cx.notify();
    }
  }

  fn sync_pack_update(&mut self, cx: &mut Context<Self>) {
    let next = cx
      .try_global::<Content>()
      .and_then(|c| c.pack_updates.read(cx).0.get(&self.id))
      .map(|u| u.latest.version_number.clone());
    if next != self.pack_update {
      self.pack_update = next;
      cx.notify();
    }
  }
}

fn stop(id: String, name: String, running: usize, window: &mut Window, cx: &mut App) {
  let (title, description, button) = if running > 1 {
    (
      "Stop All Instances",
      format!("Are you sure you want to stop all {running} instances of profile \"{name}\"?"),
      "Stop all",
    )
  } else {
    (
      "Stop Instance",
      format!("Are you sure you want to stop the instance of profile \"{name}\"?"),
      "Stop",
    )
  };
  open_destroy_dialog(window, cx, title, description, button, move |_, cx| {
    let task = cx.profiles().stop_all(Some(id.clone()));
    cx.spawn(async move |cx| {
      if let Err(e) = task.await {
        cx.update(|cx| toast_error("Failed to stop instances", &e, cx));
      }
    })
    .detach();
  });
}

pub(super) fn delete(id: String, name: String, window: &mut Window, cx: &mut App) {
  let description = format!(
    "Are you sure you want to delete the profile \"{name}\"? This action cannot be undone."
  );
  open_destroy_dialog(
    window,
    cx,
    "Delete Profile",
    description,
    "Delete",
    move |_, cx| {
      let task = cx.profiles().remove(id.clone());
      cx.spawn(async move |cx| {
        if let Err(e) = task.await {
          cx.update(|cx| toast_error("Failed to delete profile", &e, cx));
        }
      })
      .detach();
    },
  );
}

fn repair(id: String, name: String, cx: &mut App) {
  if cx.offline().is_offline(cx) {
    toast(
      Notification::warning(
        "You are currently offline, please reconnect to the internet to repair a profile",
      ),
      cx,
    );
    return;
  }
  let task = cx.profiles().repair(cx, id);
  cx.spawn(async move |cx| {
    let result = task.await;
    cx.update(|cx| match result {
      Ok(()) => toast(
        Notification::success(format!("Repair of profile {name} complete")),
        cx,
      ),
      Err(e) => toast_error(&format!("Failed to repair profile {name}"), &e, cx),
    });
  })
  .detach();
}

fn open_dir(id: String, cx: &mut App) {
  let task = cx.profiles().open_path(id);
  cx.spawn(async move |cx| {
    if let Err(e) = task.await {
      cx.update(|cx| toast_error("Failed to open the profile directory", &e, cx));
    }
  })
  .detach();
}

pub(super) fn set_favorite(id: String, favorite: bool, cx: &mut App) {
  let task = cx.profiles().set_favorite(id, None, favorite);
  cx.spawn(async move |cx| {
    if let Err(e) = task.await {
      cx.update(|cx| toast_error("Failed to update favorites", &e, cx));
    }
  })
  .detach();
}

impl Render for ProfileHeader {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let Some(profile) = &self.profile else {
      return div().into_any_element();
    };
    let theme = cx.theme();
    let muted_fg = theme.muted_foreground;
    let (id, name) = (profile.id.clone(), profile.name.clone());
    let running = self.running;
    let created = format_datetime(&profile.created_at.with_timezone(&chrono::Local));
    let played = profile.last_played.map_or("Never".into(), |t| {
      format_datetime(&t.with_timezone(&chrono::Local))
    });

    let info = div()
      .flex()
      .flex_col()
      .gap_1()
      .flex_1()
      .min_w(px(260.))
      .child(
        div()
          .flex()
          .flex_wrap()
          .items_center()
          .gap_2()
          .child(
            div()
              .text_2xl()
              .line_height(px(32.))
              .font_weight(FontWeight::SEMIBOLD)
              .child(name.clone()),
          )
          .when(running > 0, |el| {
            el.child(dot_chip(running_label(running), colors::green_500()))
          })
          .when_some(self.pack_update.clone(), |el, v| {
            let id = id.clone();
            el.child(
              div()
                .id("pack-update")
                .cursor_pointer()
                .h_5()
                .px_2()
                .rounded_full()
                .flex()
                .items_center()
                .gap_1()
                .bg(theme.primary.opacity(0.15))
                .text_color(theme.primary)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .child(Icon::new(IconName::CircleArrowUp).size_3())
                .child(format!("Pack update {v}"))
                .on_click(move |_, w, cx| {
                  run_menu_action(MenuAction::ChangeVersion, id.clone(), w, cx)
                }),
            )
          }),
      )
      .child(
        div()
          .text_sm()
          .line_height(px(20.))
          .text_color(muted_fg)
          .child(format!("{:?} {}", profile.loader, profile.version)),
      )
      .child(
        div()
          .text_xs()
          .line_height(px(16.))
          .text_color(muted_fg)
          .child(format!("Created {created} · Last played {played}")),
      );

    let play =
      |id: String| move |_: &ClickEvent, _: &mut Window, cx: &mut App| launch(id.clone(), None, cx);
    let favorite = profile.favorite;
    let modpack = profile.modpack.is_some();

    let actions = div()
      .flex()
      .items_center()
      .gap_2()
      .map(|el| {
        if running > 0 {
          let (id, name) = (id.clone(), name.clone());
          el.child(
            Button::new("launch-another")
              .outline()
              .icon(IconName::Plus)
              .label("Launch another")
              .on_click(play(id.clone())),
          )
          .child(
            Button::new("stop")
              .danger()
              .icon(IconName::Square)
              .label(if running > 1 { "Stop all" } else { "Stop" })
              .on_click(move |_, window, cx| stop(id.clone(), name.clone(), running, window, cx)),
          )
        } else {
          el.child(
            Button::new("play")
              .primary()
              .icon(IconName::Play)
              .label("Play")
              .on_click(play(id.clone())),
          )
        }
      })
      .child({
        let id = id.clone();
        Button::new("favorite")
          .outline()
          .accessibility_label("Toggle favorite")
          .map(|b| {
            if favorite {
              b.icon(Icon::new(IconName::StarFill).text_color(colors::yellow_500()))
            } else {
              b.icon(Icon::new(IconName::Star).text_color(muted_fg))
            }
          })
          .on_click(move |_, _, cx| set_favorite(id.clone(), !favorite, cx))
      })
      .child(
        Button::new("menu")
          .outline()
          .icon(IconName::Ellipsis)
          .accessibility_label("Profile menu")
          .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, cx| {
            let danger = cx.theme().danger;
            let item = |label: &'static str, icon: IconName| {
              PopupMenuItem::new(SharedString::from(label)).icon(Icon::new(icon))
            };
            let (a, b, c, d, e, f) = (
              id.clone(),
              id.clone(),
              id.clone(),
              id.clone(),
              id.clone(),
              id.clone(),
            );
            let (n1, n2) = (name.clone(), name.clone());
            menu
              .min_w(px(210.))
              .item(
                item("Open directory", IconName::FolderOpen)
                  .on_click(move |_, _, cx| open_dir(a.clone(), cx)),
              )
              .item(
                item("Repair profile", IconName::Wrench)
                  .on_click(move |_, _, cx| repair(b.clone(), n1.clone(), cx)),
              )
              .when(modpack, |menu| {
                menu.item(item("Change pack version", IconName::RefreshCw).on_click(
                  move |_, w, cx| run_menu_action(MenuAction::ChangeVersion, c.clone(), w, cx),
                ))
              })
              .item(
                item("Duplicate profile", IconName::Copy).on_click(move |_, w, cx| {
                  run_menu_action(MenuAction::Duplicate, d.clone(), w, cx)
                }),
              )
              .item(
                item("Export as .mrpack", IconName::Download)
                  .on_click(move |_, w, cx| run_menu_action(MenuAction::Export, e.clone(), w, cx)),
              )
              .item(
                PopupMenuItem::element(move |_, _| {
                  div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(danger)
                    .child(Icon::new(IconName::Trash).small())
                    .child("Delete profile")
                })
                .on_click(move |_, w, cx| delete(f.clone(), n2.clone(), w, cx)),
              )
          }),
      );

    div()
      .flex()
      .flex_shrink_0()
      .flex_wrap()
      .items_center()
      .gap_4()
      .pt_1()
      .pb_4()
      .border_b_1()
      .border_color(theme.border)
      .child(
        div()
          .flex_none()
          .size_20()
          .rounded(px(10.))
          .overflow_hidden()
          .bg(theme.muted)
          .child(
            ProfileIcon::new(self.icon.clone())
              .size(px(80.))
              .fallback_size(px(40.)),
          ),
      )
      .child(info)
      .child(actions)
      .into_any_element()
  }
}
