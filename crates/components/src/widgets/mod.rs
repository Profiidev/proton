//! Shared, props-only widgets ported from `app/src/lib/components`

pub mod account;
pub mod content;
pub mod form;
pub mod format;
pub mod instance;
pub mod logs;
pub mod profile;

use std::rc::Rc;

use gpui_kit::{
  App, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, Styled,
  Window, component::ActiveTheme, div, px,
};

/// Click callback passed down from pages
pub type Callback = Rc<dyn Fn(&mut Window, &mut App)>;

/// Value callback, like gpui-component's `on_change(&value, window, cx)`
pub type Handler<T> = Rc<dyn Fn(&T, &mut Window, &mut App)>;

/// Tailwind palette colors the web app used outside the theme tokens
pub mod colors {
  use gpui_kit::{Hsla, rgb};

  pub fn green_500() -> Hsla {
    rgb(0x22c55e).into()
  }
  pub fn yellow_500() -> Hsla {
    rgb(0xeab308).into()
  }
  pub fn amber_500() -> Hsla {
    rgb(0xf59e0b).into()
  }
  pub fn red_500() -> Hsla {
    rgb(0xef4444).into()
  }
  pub fn blue_500() -> Hsla {
    rgb(0x3b82f6).into()
  }
  /// `color-mix(in oklab, var(--background) 60%, black)`
  pub fn log_background(background: Hsla) -> Hsla {
    Hsla {
      l: background.l * 0.6,
      ..background
    }
  }
}

/// "Input tile": `border border-input bg-input/30 hover:bg-input/60 rounded-lg`
pub fn input_tile(cx: &App) -> Div {
  let input = cx.theme().input;
  div()
    .rounded_lg()
    .border_1()
    .border_color(input)
    .bg(input.opacity(0.3))
    .hover(move |s| s.bg(input.opacity(0.6)))
}

/// Neutral chip: `bg-secondary h-5 px-2 rounded-full text-xs font-medium`
pub fn chip(text: impl Into<SharedString>, cx: &App) -> Div {
  div()
    .flex()
    .flex_none()
    .items_center()
    .h_5()
    .px_2()
    .rounded_full()
    .bg(cx.theme().secondary)
    .text_xs()
    .font_weight(FontWeight::MEDIUM)
    .child(text.into())
}

/// Colored chip like `bg-green-500/15 text-green-500`, with a leading dot
pub fn dot_chip(text: impl Into<SharedString>, color: gpui_kit::Hsla) -> impl IntoElement {
  div()
    .flex()
    .flex_none()
    .items_center()
    .gap_1()
    .h_4()
    .px(px(6.))
    .rounded_full()
    .bg(color.opacity(0.15))
    .text_color(color)
    .text_xs()
    .font_weight(FontWeight::MEDIUM)
    .child(div().size(px(6.)).rounded_full().bg(color))
    .child(text.into())
}
