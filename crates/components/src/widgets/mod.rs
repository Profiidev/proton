//! Shared, props-only widgets ported from `app/src/lib/components`

pub mod account;
pub mod content;
pub mod form;
pub mod format;
pub mod instance;
pub mod logs;
pub mod profile;
pub mod slider;
pub mod tabs;

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
  /// Mixing with black in oklab scales L, a and b alike, which scales linear sRGB by 0.6³
  pub fn log_background(background: Hsla) -> Hsla {
    let c = background.to_rgb();
    let mix = |v: f32| {
      let lin = if v <= 0.04045 {
        v / 12.92
      } else {
        ((v + 0.055) / 1.055).powf(2.4)
      } * 0.216;
      if lin <= 0.0031308 {
        lin * 12.92
      } else {
        1.055 * lin.powf(1. / 2.4) - 0.055
      }
    };
    gpui_kit::Rgba {
      r: mix(c.r),
      g: mix(c.g),
      b: mix(c.b),
      a: c.a,
    }
    .into()
  }
}

/// "Input tile": `border border-input bg-input/30 hover:bg-input/60 rounded-lg`
pub fn input_tile(cx: &App) -> Div {
  let input = cx.theme().input;
  div()
    .rounded(px(10.))
    .border_1()
    .border_color(input)
    .bg(input.opacity(0.3))
    .hover(move |s| s.bg(input.opacity(0.6)))
}

/// shadcn `Label`: `text-sm leading-none font-medium`
pub fn label(text: impl Into<SharedString>) -> Div {
  div()
    .text_sm()
    .line_height(px(14.))
    .font_weight(FontWeight::MEDIUM)
    .child(text.into())
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

#[cfg(test)]
mod tests {
  use gpui_kit::rgb;

  #[test]
  fn log_background_is_the_oklab_mix() {
    // browser: color-mix(in oklab, #09090b 60%, black) = #020202
    let c = super::colors::log_background(rgb(0x09090b).into()).to_rgb();
    for v in [c.r, c.g, c.b] {
      assert!((v * 255. - 2.).abs() < 0.6, "{v}");
    }
  }
}
