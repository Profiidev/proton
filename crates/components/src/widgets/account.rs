//! Account head avatar

use gpui_kit::{
  App, ElementId, ImageSource, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
  StatefulInteractiveElement, Styled, StyledImage, TestSupportExt, Window,
  assets::IconName,
  component::{Icon, skeleton::Skeleton, tooltip::Tooltip},
  div, img, px,
};

/// 32px skin head (`account_get_skin(..).head`); skeleton while loading,
/// `CircleHelp` + tooltip "Image could not be loaded" without or on a broken image
#[derive(IntoElement)]
pub struct AccountImage {
  id: ElementId,
  head: Option<ImageSource>,
  loading: bool,
  size: Pixels,
}

impl AccountImage {
  pub fn new(id: impl Into<ElementId>, head: Option<ImageSource>) -> Self {
    Self {
      id: id.into(),
      head,
      loading: false,
      size: px(32.),
    }
  }

  /// The head is still being fetched
  pub fn loading(mut self, loading: bool) -> Self {
    self.loading = loading;
    self
  }

  pub fn size(mut self, size: Pixels) -> Self {
    self.size = size;
    self
  }
}

impl RenderOnce for AccountImage {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let size = self.size;
    let skeleton = move || Skeleton::new().size(size).into_any_element();
    let id = self.id.clone();
    let unknown = move || {
      div()
        .id(id.clone())
        .flex()
        .flex_none()
        .size(size)
        .items_center()
        .justify_center()
        .child(Icon::new(IconName::CircleQuestionMark))
        .tooltip(|window, cx| Tooltip::new("Image could not be loaded").build(window, cx))
        .test_support()
        .into_any_element()
    };
    if self.loading {
      return skeleton();
    }
    match self.head {
      // ponytail: no nearest-neighbour sampling in gpui, the account service
      // hands in a head already upscaled to the display size
      Some(src) => img(src)
        .id(self.id)
        .flex_none()
        .size(size)
        .rounded(px(4.))
        .with_loading(skeleton)
        .with_fallback(unknown)
        .into_any_element(),
      None => unknown(),
    }
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{self as gpui, AppContext, TestAppContext, test::TestWindowExt};

  use super::*;
  use crate::test_view;

  #[gpui::test]
  fn renders_head_skeleton_and_fallback(cx: &mut TestAppContext) {
    let (handle, _) = test_view::open(cx, |_, _| {
      div()
        .flex()
        .child(AccountImage::new("head", Some("icons/proton.svg".into())))
        .child(AccountImage::new("loading", None).loading(true))
        .child(AccountImage::new("unknown", None))
        .into_any_element()
    });
    cx.update_window(handle, |_, window, cx| {
      window.render_frame(cx);
      window.hover("unknown", cx);
    })
    .unwrap();
  }
}
