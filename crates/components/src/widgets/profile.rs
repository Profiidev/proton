//! Profile icon, quick play icon, the profile tile of the home/profiles grids
//! and the "Profiles" link button of empty states

use std::rc::Rc;

use gpui_kit::{
  App, ElementId, FontWeight, ImageSource, InteractiveElement, IntoElement, ObjectFit,
  ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled, StyledImage,
  TestSupportExt, Window,
  assets::IconName,
  component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
  },
  div, img,
  prelude::FluentBuilder,
  px, svg,
};

use super::{Callback, colors, dot_chip, form::filled_icon, format::running_label, input_tile};

/// lucide `star` with a fill, for the favorite marker (`fill-yellow-500`)
const STAR_FILLED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black" stroke="black" stroke-width="2" stroke-linejoin="round"><path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/></svg>"#;

/// Profile avatar (`rounded-md`, default 48px) with the lucide `Box` fallback
#[derive(IntoElement)]
pub struct ProfileIcon {
  icon: Option<ImageSource>,
  size: Pixels,
  fallback_size: Pixels,
}

impl ProfileIcon {
  pub fn new(icon: Option<ImageSource>) -> Self {
    Self {
      icon,
      size: px(48.),
      fallback_size: px(40.),
    }
  }

  pub fn size(mut self, size: Pixels) -> Self {
    self.size = size;
    self
  }

  pub fn fallback_size(mut self, size: Pixels) -> Self {
    self.fallback_size = size;
    self
  }
}

/// shadcn `Avatar` draws a 1px `border-border` overlay (blend lighten) over its content
fn avatar_ring(cx: &App) -> impl IntoElement {
  div()
    .absolute()
    .inset_0()
    .rounded(px(8.))
    .border_1()
    .border_color(cx.theme().border)
}

impl RenderOnce for ProfileIcon {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let fallback = self.fallback_size;
    let box_icon = move || Icon::new(IconName::Box).size(fallback).into_any_element();
    div()
      .flex()
      .flex_none()
      .items_center()
      .justify_center()
      .relative()
      .size(self.size)
      .rounded(px(8.))
      .overflow_hidden()
      .child(match self.icon {
        Some(src) => img(src)
          .size_full()
          .rounded(px(8.))
          .object_fit(ObjectFit::Cover)
          .with_fallback(box_icon)
          .into_any_element(),
        None => box_icon(),
      })
      .child(avatar_ring(cx))
  }
}

/// Quick play entry icon; fallback is `Globe` (singleplayer) or `Server` on `bg-muted`
#[derive(IntoElement)]
pub struct QuickPlayIcon {
  icon: Option<ImageSource>,
  singleplayer: bool,
  size: Pixels,
}

impl QuickPlayIcon {
  pub fn new(icon: Option<ImageSource>, singleplayer: bool) -> Self {
    Self {
      icon,
      singleplayer,
      size: px(48.),
    }
  }

  pub fn size(mut self, size: Pixels) -> Self {
    self.size = size;
    self
  }
}

impl RenderOnce for QuickPlayIcon {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let muted = cx.theme().muted;
    let name = if self.singleplayer {
      IconName::Globe
    } else {
      IconName::Server
    };
    let fallback = move || {
      div()
        .flex()
        .size_full()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .bg(muted)
        .child(Icon::new(name).size(px(20.)))
        .into_any_element()
    };
    div()
      .flex_none()
      .relative()
      .size(self.size)
      .rounded(px(8.))
      .overflow_hidden()
      .child(match self.icon {
        Some(src) => img(src)
          .size_full()
          .rounded(px(8.))
          .object_fit(ObjectFit::Cover)
          .with_fallback(fallback)
          .into_any_element(),
        None => fallback(),
      })
      .child(avatar_ring(cx))
  }
}

/// Grid tile of a profile or quick play entry (home favorites/history, profiles list)
#[derive(IntoElement)]
pub struct ProfileCard {
  id: ElementId,
  name: SharedString,
  /// `{loader} {version}`
  version: SharedString,
  /// [`super::format`]-built meta line, e.g. `4 mods · played 1 day ago`
  meta: SharedString,
  /// Quick play icon if it exists, else the profile icon
  icon: Option<ImageSource>,
  favorite: bool,
  running: usize,
  on_open: Option<Callback>,
  on_play: Option<Callback>,
  on_stop: Option<Callback>,
}

impl ProfileCard {
  pub fn new(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    version: impl Into<SharedString>,
  ) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      version: version.into(),
      meta: SharedString::default(),
      icon: None,
      favorite: false,
      running: 0,
      on_open: None,
      on_play: None,
      on_stop: None,
    }
  }

  pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
    self.meta = meta.into();
    self
  }

  pub fn icon(mut self, icon: Option<ImageSource>) -> Self {
    self.icon = icon;
    self
  }

  pub fn favorite(mut self, favorite: bool) -> Self {
    self.favorite = favorite;
    self
  }

  /// Running instances; > 0 shows the green chip and the Stop button
  pub fn running(mut self, running: usize) -> Self {
    self.running = running;
    self
  }

  pub fn on_open(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_open = Some(Rc::new(f));
    self
  }

  pub fn on_play(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_play = Some(Rc::new(f));
    self
  }

  pub fn on_stop(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_stop = Some(Rc::new(f));
    self
  }
}

impl RenderOnce for ProfileCard {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let muted_fg = theme.muted_foreground;
    let running = self.running > 0;

    let icon = div()
      .flex()
      .flex_none()
      .size_12()
      .rounded(px(8.))
      .overflow_hidden()
      .bg(theme.muted)
      .child(
        ProfileIcon::new(self.icon)
          .fallback_size(px(28.))
          .into_any_element(),
      );

    let title = div()
      .flex()
      .min_w_0()
      .items_center()
      .gap(px(6.))
      .child(
        div()
          .truncate()
          .text_sm()
          .line_height(px(18.))
          .font_weight(FontWeight::MEDIUM)
          .child(self.name),
      )
      .when(self.favorite, |this| {
        this.child(
          svg()
            .data(STAR_FILLED)
            .flex_none()
            .size_3()
            .text_color(colors::yellow_500()),
        )
      });

    let text = div()
      .flex()
      .flex_col()
      .flex_1()
      .min_w_0()
      .child(title)
      .child(
        div()
          .text_size(px(13.))
          .line_height(px(16.))
          .whitespace_nowrap()
          .text_color(muted_fg)
          .child(self.version),
      )
      .child(
        div()
          .flex()
          .h_4()
          .min_w_0()
          .items_center()
          .gap(px(6.))
          .when(running, |this| {
            this.child(dot_chip(running_label(self.running), colors::green_500()))
          })
          .child(
            div()
              .truncate()
              .text_xs()
              .text_color(muted_fg)
              .child(self.meta),
          ),
      );

    let on_open = self.on_open;
    let link = div()
      .id("open")
      .flex()
      .flex_1()
      .min_w_0()
      .items_center()
      .gap_3()
      .cursor_pointer()
      .child(icon)
      .child(text)
      .when_some(on_open, |this, f| this.on_click(move |_, w, cx| f(w, cx)))
      .test_support();

    let button = if running {
      let f = self.on_stop;
      filled_icon(Button::new("stop").cursor_pointer().danger())
        .icon(IconName::Square)
        .tooltip("Stop instances")
        .when_some(f, |b, f| b.on_click(move |_, w, cx| f(w, cx)))
    } else {
      let f = self.on_play;
      filled_icon(Button::new("play").cursor_pointer().primary())
        .icon(IconName::Play)
        .tooltip("Play profile")
        .when_some(f, |b, f| b.on_click(move |_, w, cx| f(w, cx)))
    };

    input_tile(cx)
      .id(self.id)
      .flex()
      .h(px(72.))
      .min_w_0()
      .items_center()
      .gap_3()
      .p(px(10.))
      .child(link)
      .child(button)
  }
}

/// Centered outline button "Profiles" + `ExternalLink` (empty states)
#[derive(IntoElement)]
pub struct ProfileExternalLink {
  on_click: Callback,
}

impl ProfileExternalLink {
  pub fn new(on_click: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    Self {
      on_click: Rc::new(on_click),
    }
  }
}

impl RenderOnce for ProfileExternalLink {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let f = self.on_click;
    div().mt_2().flex().justify_center().child(
      Button::new("profiles-link")
        .cursor_pointer()
        .outline()
        .gap_1p5()
        .child("Profiles")
        .child(Icon::new(IconName::ExternalLink).size_4())
        .on_click(move |_, w, cx| f(w, cx)),
    )
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use gpui_kit::{self as gpui, AppContext, TestAppContext, test::TestWindowExt};

  use super::*;
  use crate::test_view;

  #[gpui::test]
  fn card_play_stop_and_open(cx: &mut TestAppContext) {
    let hits = Rc::new(Cell::new((0, 0, 0)));
    let (handle, _) = test_view::open(cx, {
      let hits = hits.clone();
      move |_, _| {
        let (a, b, c) = (hits.clone(), hits.clone(), hits.clone());
        div()
          .w(px(400.))
          .child(
            ProfileCard::new("idle", "Vanilla Survival", "Vanilla 1.21.8")
              .meta("played 15 hours ago")
              .favorite(true)
              .on_open(move |_, _| a.set((a.get().0 + 1, a.get().1, a.get().2)))
              .on_play(move |_, _| b.set((b.get().0, b.get().1 + 1, b.get().2))),
          )
          .child(
            ProfileCard::new("busy", "Forge Classic", "Forge 1.20.1")
              .running(2)
              .on_stop(move |_, _| c.set((c.get().0, c.get().1, c.get().2 + 1))),
          )
          .child(QuickPlayIcon::new(None, true))
          .child(ProfileExternalLink::new(|_, _| {}))
          .into_any_element()
      }
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.within("idle").click("open", cx);
      window.within("idle").click("play", cx);
      window.within("busy").click("stop", cx);
      assert!(window.within("busy").try_find("play").is_none());
    })
    .unwrap();
    assert_eq!(hits.get(), (1, 1, 1));
  }
}
