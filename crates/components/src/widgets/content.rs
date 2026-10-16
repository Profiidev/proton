//! Discover result card and project icon

use std::rc::Rc;

use chrono::{DateTime, Utc};
use gpui_kit::{
  App, ElementId, FontWeight, ImageSource, InteractiveElement, IntoElement, ObjectFit,
  ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled, StyledImage,
  TestSupportExt, Window,
  assets::IconName,
  component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
  },
  div, img,
  prelude::FluentBuilder,
  px,
};

use super::{
  Callback, chip,
  format::{format_count, relative_time},
};

/// Project avatar on `bg-muted` (`rounded-md`, default 40px), `Box` fallback at half size
#[derive(IntoElement)]
pub struct ProjectIcon {
  icon: Option<ImageSource>,
  size: Pixels,
  radius: Pixels,
}

impl ProjectIcon {
  pub fn new(icon: Option<ImageSource>) -> Self {
    Self {
      icon,
      size: px(40.),
      radius: px(8.),
    }
  }

  pub fn size(mut self, size: Pixels) -> Self {
    self.size = size;
    self
  }

  /// `rounded-xl` on the project page header
  pub fn radius(mut self, radius: Pixels) -> Self {
    self.radius = radius;
    self
  }
}

impl RenderOnce for ProjectIcon {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let (muted_fg, half) = (theme.muted_foreground, self.size / 2.);
    let fallback = move || {
      Icon::new(IconName::Box)
        .size(half)
        .text_color(muted_fg)
        .into_any_element()
    };
    div()
      .flex()
      .flex_none()
      .items_center()
      .justify_center()
      .size(self.size)
      .rounded(self.radius)
      .overflow_hidden()
      .bg(theme.muted)
      .child(match self.icon {
        Some(src) => img(src)
          .size_full()
          .object_fit(ObjectFit::Cover)
          .with_fallback(fallback)
          .into_any_element(),
        None => fallback(),
      })
  }
}

/// Install button state of a [`ProjectCard`]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardAction {
  Install,
  Update,
  Installed,
}

/// Discover result: icon, name, `{source} · {author}`, 2-line description,
/// tag chips, downloads, relative update time and the install button
#[derive(IntoElement)]
pub struct ProjectCard {
  id: ElementId,
  name: SharedString,
  icon: Option<ImageSource>,
  /// `Modrinth · modmuss50`
  byline: SharedString,
  description: SharedString,
  tags: Vec<SharedString>,
  downloads: u64,
  updated: Option<DateTime<Utc>>,
  action: CardAction,
  on_open: Option<Callback>,
  on_install: Option<Callback>,
}

impl ProjectCard {
  pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      icon: None,
      byline: SharedString::default(),
      description: SharedString::default(),
      tags: Vec::new(),
      downloads: 0,
      updated: None,
      action: CardAction::Install,
      on_open: None,
      on_install: None,
    }
  }

  pub fn icon(mut self, icon: Option<ImageSource>) -> Self {
    self.icon = icon;
    self
  }

  pub fn byline(mut self, source: &str, author: &str) -> Self {
    self.byline = format!("{source} · {author}").into();
    self
  }

  pub fn description(mut self, description: impl Into<SharedString>) -> Self {
    self.description = description.into();
    self
  }

  /// Already deduplicated labels (categories, loaders, runs on)
  pub fn tags(mut self, tags: impl IntoIterator<Item = SharedString>) -> Self {
    self.tags = tags.into_iter().collect();
    self
  }

  pub fn downloads(mut self, downloads: u64) -> Self {
    self.downloads = downloads;
    self
  }

  pub fn updated(mut self, updated: DateTime<Utc>) -> Self {
    self.updated = Some(updated);
    self
  }

  pub fn action(mut self, action: CardAction) -> Self {
    self.action = action;
    self
  }

  /// Icon / name click (project page)
  pub fn on_open(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_open = Some(Rc::new(f));
    self
  }

  pub fn on_install(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_install = Some(Rc::new(f));
    self
  }
}

impl RenderOnce for ProjectCard {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let (muted_fg, fg) = (theme.muted_foreground, theme.foreground);
    let open = self.on_open;
    let clickable = |id: &'static str| {
      let open = open.clone();
      div()
        .id(id)
        .cursor_pointer()
        .when_some(open, |this, f| this.on_click(move |_, w, cx| f(w, cx)))
    };

    let info = div()
      .flex()
      .flex_col()
      .flex_1()
      .min_w_0()
      .gap_1()
      .child(
        div()
          .flex()
          .min_w_0()
          .items_baseline()
          .gap_2()
          .child(
            clickable("name")
              .truncate()
              .text_base()
              .font_weight(FontWeight::MEDIUM)
              .hover(|s| s.underline())
              .child(self.name)
              .test_support(),
          )
          .child(
            div()
              .truncate()
              .text_xs()
              .text_color(muted_fg)
              .child(self.byline),
          ),
      )
      .child(
        div()
          .text_sm()
          .text_color(muted_fg)
          .line_clamp(2)
          .child(self.description),
      )
      .child(
        div()
          .mt(px(2.))
          .flex()
          .flex_wrap()
          .gap(px(6.))
          .children(self.tags.into_iter().map(|t| chip(t, cx))),
      );

    let button = match self.action {
      CardAction::Installed => Button::new("install")
        .secondary()
        .small()
        .w_full()
        .disabled(true)
        .icon(IconName::Check)
        .label("Installed"),
      action => {
        let f = self.on_install;
        Button::new("install")
          .small()
          .w_full()
          .map(|b| {
            if action == CardAction::Update {
              b.outline().label("Update")
            } else {
              b.primary().label("Install")
            }
          })
          .when_some(f, |b, f| b.on_click(move |_, w, cx| f(w, cx)))
      }
    };

    let side = div()
      .flex()
      .flex_col()
      .flex_none()
      .w_32()
      .items_end()
      .gap_2()
      .text_xs()
      .text_color(muted_fg)
      .child(
        div()
          .flex()
          .items_center()
          .gap_1()
          .child(
            Icon::new(IconName::Download)
              .size(px(14.))
              .text_color(muted_fg),
          )
          .child(format_count(self.downloads)),
      )
      .when_some(self.updated, |this, t| {
        this.child(
          div()
            .text_right()
            .child(format!("Updated {}", relative_time(t, Utc::now()))),
        )
      })
      .child(button);

    div()
      .id(self.id)
      .flex()
      .items_center()
      .gap_4()
      .px_4()
      .py(px(14.))
      .rounded(px(14.))
      .border_1()
      .border_color(fg.opacity(0.1))
      .bg(theme.popover)
      .child(
        clickable("icon")
          .flex_none()
          .child(ProjectIcon::new(self.icon).size(px(72.))),
      )
      .child(info)
      .child(side)
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use gpui_kit::{self as gpui, AppContext, TestAppContext, test::TestWindowExt};

  use super::*;
  use crate::test_view;

  #[gpui::test]
  fn install_and_open(cx: &mut TestAppContext) {
    let hits = Rc::new(Cell::new((0, 0)));
    let (handle, _) = test_view::open(cx, {
      let hits = hits.clone();
      move |_, _| {
        let (a, b) = (hits.clone(), hits.clone());
        div()
          .w(px(800.))
          .child(
            ProjectCard::new("sodium", "Sodium")
              .byline("Modrinth", "jellysquid3")
              .description("A high-performance rendering engine")
              .tags(["Optimization".into(), "Fabric".into()])
              .downloads(246_000_000)
              .updated(Utc::now())
              .on_open(move |_, _| a.set((a.get().0 + 1, a.get().1)))
              .on_install(move |_, _| b.set((b.get().0, b.get().1 + 1))),
          )
          .child(ProjectCard::new("iris", "Iris").action(CardAction::Installed))
          .into_any_element()
      }
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.within("sodium").click("name", cx);
      window.within("sodium").click("install", cx);
      // disabled
      window.within("iris").click("install", cx);
    })
    .unwrap();
    assert_eq!(hits.get(), (1, 1));
  }
}
