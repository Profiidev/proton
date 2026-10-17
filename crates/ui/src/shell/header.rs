use gpui_kit::{
  App, AppContext, Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
  Pixels, Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowControlArea,
  assets::IconName,
  base::TestSupportExt,
  component::{ActiveTheme, Icon, InteractiveElementExt, StyledExt},
  div, img,
  prelude::FluentBuilder,
  px,
};

use crate::{
  router::{Crumb, Route, RouterExt, breadcrumbs},
  shell::{ShellData, ShellDataExt, ShellEvent, downloads::DownloadsPanel},
};

pub const HEADER_HEIGHT: Pixels = px(40.);

/// Window title bar: logo and breadcrumbs. The actions on the right are [`HeaderActions`], a
/// sibling, so download ticks never re-render the cached header.
pub struct Header {
  crumbs: Vec<Crumb>,
  /// left button is down on the drag region
  should_move: bool,
}

impl Header {
  pub fn new(cx: &mut Context<Self>) -> Self {
    let router = cx.router();
    let data = cx.shell_data();
    cx.observe(&router, Self::refresh).detach();
    cx.observe(&data, Self::refresh).detach();
    Self {
      crumbs: crumbs(cx),
      should_move: false,
    }
  }

  fn refresh<T>(&mut self, _: Entity<T>, cx: &mut Context<Self>) {
    let crumbs = crumbs(cx);
    if crumbs != self.crumbs {
      self.crumbs = crumbs;
      cx.notify();
    }
  }
}

fn crumbs(cx: &App) -> Vec<Crumb> {
  breadcrumbs(&cx.route().to_url(), &cx.shell_data().read(cx).names)
}

#[derive(PartialEq)]
struct ActionsState {
  offline: bool,
  hide_controls: bool,
}

impl ActionsState {
  fn new(data: &ShellData) -> Self {
    Self {
      offline: data.offline,
      hide_controls: data.hide_window_controls,
    }
  }
}

/// Right end of the title bar: offline badge, downloads, window controls
pub struct HeaderActions {
  state: ActionsState,
  downloads: Entity<DownloadsPanel>,
}

impl HeaderActions {
  pub fn new(cx: &mut Context<Self>) -> Self {
    let data = cx.shell_data();
    cx.observe(&data, |this, data, cx| {
      let state = ActionsState::new(data.read(cx));
      if state != this.state {
        this.state = state;
        cx.notify();
      }
    })
    .detach();
    Self {
      state: ActionsState::new(data.read(cx)),
      downloads: cx.new(DownloadsPanel::new),
    }
  }
}

fn control(
  id: &'static str,
  icon: IconName,
  icon_size: Pixels,
  cx: &App,
) -> gpui_kit::base::ObservedElement<gpui_kit::Stateful<gpui_kit::Div>> {
  let hover = if id == "close" {
    cx.theme().danger
  } else {
    cx.theme().accent.opacity(0.5)
  };
  div()
    .id(id)
    .test_support()
    .size_8()
    .flex()
    .items_center()
    .justify_center()
    .rounded_full()
    .cursor_pointer()
    .hover(move |s| s.bg(hover))
    .child(Icon::new(icon).size(icon_size))
}

impl Render for Header {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    proton_components::render_count::hit::<Self>();
    let theme = cx.theme();
    let last = self.crumbs.len().saturating_sub(1);
    let (muted, fg) = (theme.muted_foreground, theme.foreground);
    let mut crumbs = vec![];
    for (ix, crumb) in self.crumbs.iter().enumerate() {
      if ix > 0 {
        crumbs.push(
          Icon::new(IconName::ChevronRight)
            .size_3p5()
            .text_color(muted)
            .into_any_element(),
        );
      }
      let item = div()
        .id(SharedString::from(format!("crumb-{ix}")))
        .test_support()
        .whitespace_nowrap()
        .child(crumb.label.clone());
      crumbs.push(if ix == last {
        item.text_color(fg).into_any_element()
      } else {
        let href = crumb.href.clone();
        item
          .text_color(muted)
          .cursor_pointer()
          .hover(move |s| s.text_color(fg))
          .on_click(move |_, _, cx| {
            if let Some(route) = Route::from_url(&href) {
              cx.navigate(route);
            }
          })
          .into_any_element()
      });
    }
    div()
      .id("header")
      .flex()
      .flex_shrink_0()
      .items_center()
      .w_full()
      .h(HEADER_HEIGHT)
      .text_sm()
      .window_control_area(WindowControlArea::Drag)
      .on_mouse_down(
        MouseButton::Left,
        cx.listener(|this, _, _, _| this.should_move = true),
      )
      .on_mouse_up(
        MouseButton::Left,
        cx.listener(|this, _, _, _| this.should_move = false),
      )
      .on_mouse_down_out(cx.listener(|this, _, _, _| this.should_move = false))
      .on_mouse_move(cx.listener(|this, _, window, _| {
        if this.should_move {
          this.should_move = false;
          window.start_window_move();
        }
      }))
      .on_double_click(|_, window, _| window.zoom_window())
      .on_mouse_down(MouseButton::Right, |e, window, _| {
        window.show_window_menu(e.position)
      })
      .child(img("icons/proton.svg").size_7().mx_2().flex_shrink_0())
      .child(div().mr_4().font_medium().child("Proton"))
      .child(div().w_px().h_5().flex_shrink_0().bg(theme.border))
      .child(
        div()
          .mx_4()
          .min_w_0()
          .overflow_hidden()
          .flex()
          .items_center()
          // shadcn breadcrumb-list gap-1.5
          .gap_1p5()
          .children(crumbs),
      )
  }
}

impl Render for HeaderActions {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    proton_components::render_count::hit::<Self>();
    let theme = cx.theme();
    let data = cx.shell_data();
    div()
      .id("header-actions")
      .h(HEADER_HEIGHT)
      .flex_shrink_0()
      .flex()
      .items_center()
      .gap_1()
      .pr_1()
      .text_sm()
      .when(self.state.offline, |el| {
        el.child(
          div()
            .id("offline")
            .test_support()
            .mr_2()
            .h_5()
            .px_2()
            .flex()
            .items_center()
            .rounded_full()
            .cursor_pointer()
            .bg(theme.danger.opacity(0.2))
            .text_color(theme.danger)
            .text_xs()
            .font_medium()
            .child("Offline")
            .on_click(move |_, _, cx| data.update(cx, |_, cx| cx.emit(ShellEvent::Reconnect))),
        )
      })
      .child(self.downloads.clone())
      .when(!self.state.hide_controls, |el| {
        el.child(
          control("minimize", IconName::Minus, px(18.), cx)
            .on_click(|_, window, _| window.minimize_window()),
        )
        .child(
          control("maximize", IconName::Square, px(16.), cx)
            .on_click(|_, window, _| window.zoom_window()),
        )
        .child(control("close", IconName::X, px(18.), cx).on_click(|_, _, cx| cx.quit()))
      })
  }
}
