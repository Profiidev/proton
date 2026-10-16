//! shadcn `Tabs.List` + `Tabs.Trigger`. gpui-kit's segmented `TabBar` paints the active tab
//! `background` without a border and pads wider; this is the shadcn look:
//! list `bg-muted h-8 p-[3px] rounded-lg`, trigger `h-[calc(100%-1px)] px-1.5 rounded-md border
//! border-transparent text-sm font-medium text-muted-foreground hover:text-foreground`,
//! active `bg-input/30 border-input text-foreground`

use std::rc::Rc;

use gpui_kit::{
  App, Div, ElementId, FontWeight, InteractiveElement, ParentElement, SharedString, Stateful,
  StatefulInteractiveElement, Styled, Window,
  base::{ObservedElement, TestSupportExt},
  component::ActiveTheme,
  div,
  prelude::FluentBuilder,
  px,
};

/// Segmented tabs; every trigger's element id is its label
pub fn segmented_tabs<L: Into<SharedString>>(
  id: impl Into<ElementId>,
  labels: impl IntoIterator<Item = L>,
  selected: usize,
  on_click: impl Fn(&usize, &mut Window, &mut App) + 'static,
  cx: &App,
) -> ObservedElement<Stateful<Div>> {
  let theme = cx.theme();
  let (input, fg, muted) = (theme.input, theme.foreground, theme.muted_foreground);
  let on_click = Rc::new(on_click);
  div()
    .id(id)
    .test_support()
    .flex()
    .flex_none()
    .items_center()
    .h_8()
    .p(px(3.))
    .rounded(px(10.))
    .bg(theme.muted)
    .children(labels.into_iter().enumerate().map(|(ix, label)| {
      let label: SharedString = label.into();
      let active = ix == selected;
      let on_click = on_click.clone();
      div()
        .id(label.clone())
        .test_support()
        .flex()
        .items_center()
        .justify_center()
        .h(px(25.))
        .px(px(6.))
        .rounded(px(8.))
        .border_1()
        .border_color(if active {
          input
        } else {
          gpui_kit::transparent_black()
        })
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .cursor_pointer()
        .map(|el| {
          if active {
            el.bg(input.opacity(0.3)).text_color(fg)
          } else {
            el.text_color(muted).hover(move |s| s.text_color(fg))
          }
        })
        .on_click(move |_, window, cx| on_click(&ix, window, cx))
        .child(label)
    }))
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use gpui_kit::{self as gpui, AppContext, IntoElement, TestAppContext, test::TestWindowExt};

  use super::*;
  use crate::test_view;

  #[gpui::test]
  fn click_reports_the_index(cx: &mut TestAppContext) {
    let hit = Rc::new(Cell::new(usize::MAX));
    let (handle, _) = test_view::open(cx, {
      let hit = hit.clone();
      move |_, cx| {
        let hit = hit.clone();
        segmented_tabs("tabs", ["A", "B", "C"], 0, move |ix, _, _| hit.set(*ix), cx)
          .into_any_element()
      }
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.click("C", cx);
    })
    .unwrap();
    assert_eq!(hit.get(), 2);
  }
}
