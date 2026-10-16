use std::{
  collections::HashMap,
  time::{Duration, Instant},
};

use gpui_kit::{
  Animation, AnimationExt, App, Context, Entity, FontWeight, Global, ImageSource,
  InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement,
  Styled, Subscription, Window,
  assets::IconName,
  base::TestSupportExt,
  component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
    tab::{Tab, TabBar},
  },
  div,
  prelude::FluentBuilder,
  px,
};
use proton_components::{
  animation::ease_out_cubic,
  widgets::{colors, form::open_destroy_dialog, format::format_datetime, profile::QuickPlayIcon},
};
use proton_profiles::{
  Profiles, ProfilesExt,
  config::{QuickPlayInfo, QuickPlayType},
  server::ServerStatus,
};

use super::png;
use crate::{
  actions::{launch, toast_error},
  router::{ProfileTab, QuickPlayTab, Route, RouterExt},
};

/// Like the old `quick_play_server_status` cache
const STATUS_TTL: Duration = Duration::from_secs(60);
const FADE: Duration = Duration::from_millis(200);

enum Status {
  Pending,
  Done(Instant, Option<ServerStatus>),
}

/// Server status per address, shared by all quick play pages
#[derive(Default)]
struct StatusCache(HashMap<String, Status>);

impl Global for StatusCache {}

/// "Singleplayer" | "Multiplayer" | "Realms" tabs over the profile's quick play entries
pub struct QuickPlayPage {
  id: String,
  tab: QuickPlayTab,
  entries: Entity<Vec<QuickPlayInfo>>,
  /// keyed by [`key`], None while loading or without an icon
  icons: HashMap<String, Option<ImageSource>>,
  _subscription: Subscription,
}

fn key(item: &QuickPlayInfo) -> String {
  format!("{:?}:{}", item.r#type, item.id)
}

impl QuickPlayPage {
  pub fn new(id: String, tab: QuickPlayTab, _: &mut Window, cx: &mut Context<Self>) -> Self {
    let entries = cx.profiles().clone().quick_play(&id, cx);
    let subscription = cx.observe(&entries, |this, _, cx| {
      this.load(cx);
      cx.notify();
    });
    let mut this = Self {
      id,
      tab,
      entries,
      icons: HashMap::new(),
      _subscription: subscription,
    };
    this.load(cx);
    this
  }

  /// Fetches missing icons and stale server statuses of the visible tab
  fn load(&mut self, cx: &mut Context<Self>) {
    let ty = tab_type(self.tab);
    let items: Vec<_> = self
      .entries
      .read(cx)
      .iter()
      .filter(|q| q.r#type == ty)
      .cloned()
      .collect();
    for item in &items {
      let key = key(item);
      if self.icons.contains_key(&key) {
        continue;
      }
      self.icons.insert(key.clone(), None);
      let task = cx
        .profiles()
        .quick_play_icon(cx, self.id.clone(), item.clone());
      cx.spawn(async move |this, cx| {
        let icon = task.await.ok().flatten().map(png);
        this.update(cx, |this, cx| {
          this.icons.insert(key, icon);
          cx.notify();
        })
      })
      .detach();
    }
    if ty == QuickPlayType::Multiplayer {
      for item in items {
        fetch_status(item.id, cx);
      }
    }
  }

  pub(super) fn list(&self, cx: &App) -> Vec<QuickPlayInfo> {
    let ty = tab_type(self.tab);
    let mut list: Vec<_> = self
      .entries
      .read(cx)
      .iter()
      .filter(|q| q.r#type == ty)
      .cloned()
      .collect();
    list.sort_by_key(|q| std::cmp::Reverse(q.last_played_time));
    list
  }
}

/// Starts a lookup unless a fresh or pending one exists; notifies the page when done
fn fetch_status(address: String, cx: &mut Context<QuickPlayPage>) {
  let cache = cx.default_global::<StatusCache>();
  match cache.0.get(&address) {
    Some(Status::Pending) => return,
    Some(Status::Done(at, _)) if at.elapsed() < STATUS_TTL => return,
    _ => {}
  }
  cache.0.insert(address.clone(), Status::Pending);
  cx.spawn(async move |this, cx| {
    let status = Profiles::server_status(address.clone()).await;
    cx.update(|cx| {
      cx.default_global::<StatusCache>()
        .0
        .insert(address, Status::Done(Instant::now(), status));
    });
    this.update(cx, |_, cx| cx.notify())
  })
  .detach();
}

fn tab_type(tab: QuickPlayTab) -> QuickPlayType {
  match tab {
    QuickPlayTab::Singleplayer => QuickPlayType::Singleplayer,
    QuickPlayTab::Multiplayer => QuickPlayType::Multiplayer,
    QuickPlayTab::Realms => QuickPlayType::Realms,
  }
}

/// Segmented tabs that navigate on click
pub(super) fn tabs<T: Copy + PartialEq + 'static>(
  id: &'static str,
  items: &'static [(&'static str, T)],
  selected: T,
  go: impl Fn(T, &mut App) + 'static,
) -> impl IntoElement {
  let ix = items.iter().position(|(_, t)| *t == selected).unwrap_or(0);
  div().flex().child(
    TabBar::new(id)
      .segmented()
      .selected_index(ix)
      .children(items.iter().map(|(label, _)| Tab::new().label(*label)))
      .on_click(move |ix, _, cx| go(items[*ix].1, cx)),
  )
}

/// Dashed "nothing here" box
pub(super) fn empty(title: &'static str, description: &'static str, cx: &App) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .items_center()
    .gap_2()
    .p_6()
    .rounded_xl()
    .border_1()
    .border_dashed()
    .border_color(cx.theme().border)
    .text_sm()
    .child(div().font_weight(FontWeight::MEDIUM).child(title))
    .child(
      div()
        .text_color(cx.theme().muted_foreground)
        .child(description),
    )
}

fn status_chip(status: &ServerStatus, cx: &App) -> impl IntoElement {
  let (label, bg, fg) = if status.online {
    let green = colors::green_500();
    (
      format!("{} / {} online", status.players_online, status.players_max),
      green.opacity(0.15),
      green,
    )
  } else {
    (
      "Offline".into(),
      cx.theme().secondary,
      cx.theme().muted_foreground,
    )
  };
  div()
    .flex()
    .flex_none()
    .items_center()
    .h_5()
    .px_2()
    .rounded_full()
    .bg(bg)
    .text_color(fg)
    .text_xs()
    .font_weight(FontWeight::MEDIUM)
    .child(label)
}

impl QuickPlayPage {
  fn row(&self, ix: usize, last: bool, item: QuickPlayInfo, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let singleplayer = item.r#type == QuickPlayType::Singleplayer;
    let subtitle: SharedString = match item.r#type {
      QuickPlayType::Multiplayer => item.id.clone().into(),
      _ => format!(
        "Last played {}",
        format_datetime(&item.last_played_time.with_timezone(&chrono::Local))
      )
      .into(),
    };
    let icon = self.icons.get(&key(&item)).cloned().flatten();
    let status = match cx
      .try_global::<StatusCache>()
      .and_then(|c| c.0.get(&item.id))
    {
      Some(Status::Done(_, Some(s))) if item.r#type == QuickPlayType::Multiplayer => {
        Some(status_chip(s, cx))
      }
      _ => None,
    };
    let id = self.id.clone();
    let accent = theme.accent.opacity(0.4);
    let play = {
      let (id, item) = (id.clone(), item.clone());
      move |_: &_, _: &mut Window, cx: &mut App| launch(id.clone(), Some(item.clone()), cx)
    };

    div()
      .id(SharedString::from(format!("entry-{ix}")))
      .flex()
      .items_center()
      .gap_3()
      .px_3()
      .py(px(10.))
      .when(!last, |el| el.border_b_1().border_color(theme.border))
      .hover(move |s| s.bg(accent))
      .child(QuickPlayIcon::new(icon, singleplayer).size(px(40.)))
      .child(
        div()
          .flex()
          .flex_col()
          .flex_1()
          .min_w_0()
          .child(
            div()
              .truncate()
              .text_sm()
              .font_weight(FontWeight::MEDIUM)
              .child(item.name.clone()),
          )
          .child(
            div()
              .truncate()
              .text_xs()
              .text_color(theme.muted_foreground)
              .child(subtitle),
          ),
      )
      .map(|el| {
        if singleplayer {
          let favorite = item.favorite;
          let (fav_id, fav_item) = (id.clone(), item.clone());
          let (rm_id, rm_item) = (id.clone(), item.clone());
          el.child(
            Button::new("play")
              .primary()
              .icon(IconName::Play)
              .label("Play")
              .on_click(play),
          )
          .child(
            Button::new("favorite")
              .outline()
              .accessibility_label(if favorite {
                "Remove from favorites"
              } else {
                "Add to favorites"
              })
              .map(|b| {
                if favorite {
                  b.icon(Icon::new(IconName::StarFill).text_color(colors::yellow_500()))
                } else {
                  b.icon(IconName::Star)
                }
              })
              .on_click(move |_, _, cx| {
                set_favorite(fav_id.clone(), fav_item.clone(), !favorite, cx)
              }),
          )
          .child(
            Button::new("remove")
              .outline()
              .accessibility_label("Delete world")
              .icon(Icon::new(IconName::Trash).text_color(theme.danger))
              .on_click(move |_, window, cx| remove(rm_id.clone(), rm_item.clone(), window, cx)),
          )
        } else {
          el.children(status)
            .child(Button::new("play").primary().label("Join").on_click(play))
        }
      })
      .test_support()
  }
}

fn set_favorite(id: String, item: QuickPlayInfo, favorite: bool, cx: &mut App) {
  let task = cx.profiles().set_favorite(id, Some(item), favorite);
  cx.spawn(async move |cx| {
    if let Err(e) = task.await {
      cx.update(|cx| toast_error("Failed to update favorites", &e, cx));
    }
  })
  .detach();
}

fn remove(id: String, item: QuickPlayInfo, window: &mut Window, cx: &mut App) {
  let description = format!(
    "Are you sure you want to remove the quick play entry {}?",
    item.name
  );
  open_destroy_dialog(
    window,
    cx,
    "Remove Entry",
    description,
    "Remove",
    move |_, cx| {
      let task = cx.profiles().remove_quick_play(id.clone(), item.clone());
      cx.spawn(async move |cx| {
        if let Err(e) = task.await {
          cx.update(|cx| toast_error("Failed to remove the quick play entry", &e, cx));
        }
      })
      .detach();
    },
  );
}

const TABS: &[(&str, QuickPlayTab)] = &[
  ("Singleplayer", QuickPlayTab::Singleplayer),
  ("Multiplayer", QuickPlayTab::Multiplayer),
  ("Realms", QuickPlayTab::Realms),
];

impl Render for QuickPlayPage {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let list = self.list(cx);
    let entries = self.entries.read(cx);
    let note = !entries.iter().any(|q| q.r#type != QuickPlayType::Realms);
    let id = self.id.clone();

    let body = if list.is_empty() {
      let (title, description) = match self.tab {
        QuickPlayTab::Singleplayer => (
          "No worlds found",
          "Join a singleplayer world to add it here.",
        ),
        QuickPlayTab::Multiplayer => ("No servers found", "Join a server to add it here."),
        QuickPlayTab::Realms => (
          "No realms found",
          "This account does not own or belong to a Realm.",
        ),
      };
      empty(title, description, cx).into_any_element()
    } else {
      let count = list.len();
      let rows = list
        .into_iter()
        .enumerate()
        .map(|(ix, item)| self.row(ix, ix + 1 == count, item, cx));
      div()
        .id("quick-play-list")
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded_xl()
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover.opacity(0.5))
        .children(rows)
        .map(|el| {
          if cx.reduce_motion() {
            el.into_any_element()
          } else {
            // the element id stays, so this plays once when the list first shows up
            el.with_animation(
              "quick-play-fade",
              Animation::new(FADE).with_easing(ease_out_cubic),
              |el, t| el.opacity(t),
            )
            .into_any_element()
          }
        })
    };

    div()
      .id("quick-play")
      .size_full()
      .overflow_y_scroll()
      .flex()
      .flex_col()
      .gap_3()
      .child(tabs("quick-play-tabs", TABS, self.tab, move |tab, cx| {
        cx.navigate(Route::Profile {
          id: id.clone(),
          tab: ProfileTab::QuickPlay(tab),
        })
      }))
      .child(body)
      .when(note, |el| {
        el.child(div().text_xs().text_color(theme.muted_foreground).child(
          "Quick play needs Minecraft 1.20 (23w14a) or newer. Join a world once to add it here.",
        ))
      })
  }
}
