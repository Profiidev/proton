//! shadcn/pleiades Card: `rounded-xl ring-1 bg-card text-sm`, 16px spacing

use gpui_kit::{
  AnyElement, App, Div, IntoElement, ParentElement, Refineable, RenderOnce, SharedString,
  StyleRefinement, Styled, Window, component::ActiveTheme, div, prelude::FluentBuilder, px,
};

#[derive(IntoElement, Default)]
pub struct Card {
  style: StyleRefinement,
  children: Vec<AnyElement>,
  footer: Option<AnyElement>,
}

impl Card {
  pub fn new() -> Self {
    Self::default()
  }

  /// Pinned to the bottom edge, like `<Card.Footer>`
  pub fn footer(mut self, footer: impl IntoElement) -> Self {
    self.footer = Some(footer.into_any_element());
    self
  }
}

impl ParentElement for Card {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for Card {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Card {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let has_footer = self.footer.is_some();
    let mut card = div()
      .flex()
      .flex_col()
      .gap_4()
      .pt_4()
      .overflow_hidden()
      .rounded(px(14.))
      .border_1()
      .border_color(theme.border)
      .bg(theme.popover)
      .text_color(theme.popover_foreground)
      .text_sm()
      .children(self.children)
      .when_some(self.footer, |card, footer| {
        card.child(
          div()
            .flex()
            .items_center()
            .p_4()
            .rounded_b(px(14.))
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.muted.opacity(0.5))
            .child(footer),
        )
      })
      .when(!has_footer, |card| card.pb_4());
    card.style().refine(&self.style);
    card
  }
}

/// Title + description stack, `<Card.Header>`
pub fn card_header() -> Div {
  div().flex().flex_col().gap_1().px_4()
}

/// `<Card.Title>`
pub fn card_title(text: impl Into<SharedString>) -> Div {
  div()
    .text_base()
    .font_weight(gpui_kit::FontWeight::MEDIUM)
    .line_height(gpui_kit::relative(1.375))
    .child(text.into())
}

/// `<Card.Description>`
pub fn card_description(text: impl Into<SharedString>, cx: &App) -> Div {
  div()
    .text_sm()
    .text_color(cx.theme().muted_foreground)
    .child(text.into())
}

/// `<Card.Content>`
pub fn card_content() -> Div {
  div().px_4()
}

#[cfg(test)]
mod tests {
  use gpui_kit::{self as gpui, TestAppContext, component::button::Button};

  use super::*;
  use crate::test_view;

  #[gpui::test]
  fn renders_with_and_without_footer(cx: &mut TestAppContext) {
    let (handle, _) = test_view::open(cx, |_, cx| {
      div()
        .child(
          Card::new()
            .w_80()
            .child(
              card_header()
                .child(card_title("Title"))
                .child(card_description("Description", cx)),
            )
            .child(card_content().child("Content"))
            .footer(Button::new("ok").label("Ok")),
        )
        .child(Card::new().child(card_content().child("No footer")))
        .into_any_element()
    });
    test_view::draw(handle, cx);
    cx.update(|cx| {
      assert!(cx.theme().is_dark());
      assert_eq!(cx.theme().theme_name(), crate::assets::THEME);
      assert_eq!(cx.theme().font_family.as_ref(), "Geist");
    });
  }
}
