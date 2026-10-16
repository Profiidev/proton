mod downloads;
mod header;
mod pages;
mod sidebar;

use std::{collections::HashMap, time::Duration};

use gpui_kit::{
  Animation, AnimationExt, AnyView, App, AppContext, Context, ElementId, Entity, EventEmitter,
  Global, InteractiveElement, IntoElement, MouseButton, NavigationDirection, ParentElement, Render,
  StyleRefinement, Styled, Window, component::ActiveTheme, div, prelude::FluentBuilder, px,
};
use proton_components::animation::ease_out_cubic;

pub use downloads::{DownloadRow, DownloadState, DownloadsExt, DownloadsPanel};
pub use header::{HEADER_HEIGHT, Header};
pub use pages::{PageFactory, Placeholder};
pub use sidebar::{COMPACT_BELOW, NavItem, Sidebar, SimpleSidebar, sidebar_width};

use crate::router::{Names, Route, RouterExt, SettingsTab};

/// ponytail: a handful of cached pages; bump if switching back feels slow
const CACHED_PAGES: usize = 8;
const PAGE_FADE: Duration = Duration::from_millis(150);

/// What the shell shows about the app's data. The services push into it
/// (`write_changed`), the shell never asks them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShellData {
  /// Breadcrumb labels
  pub names: Names,
  /// Running instances, green pill on "Instances"
  pub running: usize,
  /// profile id → number of content updates, amber chip on the profile's "Content" tab
  pub content_updates: HashMap<String, usize>,
  /// Shows the "Offline" badge
  pub offline: bool,
  pub hide_window_controls: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEvent {
  /// The "Offline" badge was clicked
  Reconnect,
}

impl EventEmitter<ShellEvent> for ShellData {}

struct GlobalShellData(Entity<ShellData>);

impl Global for GlobalShellData {}

pub trait ShellDataExt {
  fn shell_data(&self) -> Entity<ShellData>;
}

impl ShellDataExt for App {
  fn shell_data(&self) -> Entity<ShellData> {
    self.global::<GlobalShellData>().0.clone()
  }
}

pub(crate) fn init(cx: &mut App) {
  let data = cx.new(|_| ShellData::default());
  cx.set_global(GlobalShellData(data));
  downloads::init(cx);
}

/// Root view of the main window: header, sidebar and the current page
pub struct Shell {
  header: Entity<Header>,
  sidebar: Entity<Sidebar>,
  settings_nav: Entity<SimpleSidebar>,
  profile_nav: Entity<SimpleSidebar>,
  /// most recently used last
  pages: Vec<(Route, AnyView)>,
  profile_header: Option<(String, AnyView)>,
  /// bumps on every navigation, keys the page fade
  nav_count: usize,
}

impl Shell {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let router = cx.router();
    let data = cx.shell_data();
    cx.observe(&router, |this, _, cx| {
      this.nav_count += 1;
      this.sync_navs(cx);
      cx.notify();
    })
    .detach();
    // only the navs show shell data, the shell itself stays put
    cx.observe(&data, |this, _, cx| this.sync_navs(cx)).detach();

    let this = Self {
      header: cx.new(Header::new),
      sidebar: cx.new(|cx| Sidebar::new(window, cx)),
      settings_nav: cx.new(|_| SimpleSidebar::default()),
      profile_nav: cx.new(|_| SimpleSidebar::default()),
      pages: vec![],
      profile_header: None,
      nav_count: 0,
    };
    this.sync_navs(cx);
    this
  }

  fn sync_navs(&self, cx: &mut App) {
    let route = cx.route();
    let data = cx.shell_data().read(cx).clone();
    sidebar::sync_navs(&self.settings_nav, &self.profile_nav, &route, &data, cx);
  }

  fn page(&mut self, route: &Route, window: &mut Window, cx: &mut App) -> AnyView {
    if let Some(ix) = self.pages.iter().position(|(r, _)| r == route) {
      let entry = self.pages.remove(ix);
      self.pages.push(entry);
    } else {
      let view = PageFactory::build(route, window, cx);
      self.pages.push((route.clone(), view));
      if self.pages.len() > CACHED_PAGES {
        self.pages.remove(0);
      }
    }
    self.pages.last().map(|(_, v)| v.clone()).unwrap()
  }

  fn profile_header(&mut self, id: &str, window: &mut Window, cx: &mut App) -> Option<AnyView> {
    if self.profile_header.as_ref().is_none_or(|(i, _)| i != id) {
      let build = cx
        .try_global::<PageFactory>()
        .and_then(|f| f.profile_header.clone());
      self.profile_header = build.map(|b| (id.to_string(), b(id, window, cx)));
    }
    self.profile_header.as_ref().map(|(_, v)| v.clone())
  }
}

fn full() -> StyleRefinement {
  StyleRefinement::default().size_full()
}

impl Render for Shell {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let route = cx.route();
    // not `.cached()`: a re-rendered cached view re-renders its whole subtree, so a live child
    // (log lines, progress) would redraw the entire page. Uncached, a page notify re-renders
    // only the page root and dirty views; the page's own cached children are reused.
    let page = self.page(&route, window, cx);
    let page = div().relative().size_full().child(page).map(|el| {
      if cx.reduce_motion() {
        el.into_any_element()
      } else {
        el.with_animation(
          ElementId::NamedInteger("page".into(), self.nav_count as u64),
          Animation::new(PAGE_FADE).with_easing(ease_out_cubic),
          |el, t| el.opacity(t).top(px(6. * (1. - t))),
        )
        .into_any_element()
      }
    });

    let content = match &route {
      Route::Settings(tab) => div()
        .size_full()
        .flex()
        .gap_5()
        .px_5()
        .pt_4()
        .pb_6()
        .child(
          div()
            .w_40()
            .h_full()
            .flex_shrink_0()
            .child(self.settings_nav.clone().cached(full())),
        )
        .child(
          div()
            .flex_1()
            .min_w_0()
            .h_full()
            .when(*tab != SettingsTab::Skins, |el| el.max_w(px(720.)))
            .child(page),
        )
        .into_any_element(),
      Route::Profile { id, .. } => {
        let header = self.profile_header(id, window, cx);
        div()
          .size_full()
          .flex()
          .flex_col()
          .gap_4()
          .px_5()
          .pt_4()
          .pb_6()
          .children(header)
          .child(
            div()
              .flex()
              .flex_1()
              .min_h_0()
              .gap_4()
              .child(
                div()
                  .w_40()
                  .h_full()
                  .flex_shrink_0()
                  .child(self.profile_nav.clone().cached(full())),
              )
              .child(div().flex_1().min_w_0().h_full().child(page)),
          )
          .into_any_element()
      }
      _ => page.into_any_element(),
    };

    let theme = cx.theme();
    let header_style = StyleRefinement::default()
      .w_full()
      .h(HEADER_HEIGHT)
      .flex_shrink_0();
    let sidebar_style = StyleRefinement::default()
      .h_full()
      .w(sidebar_width(window))
      .flex_shrink_0();
    let router = cx.router();
    let forward = router.clone();

    div()
      .id("shell")
      .size_full()
      .flex()
      .flex_col()
      // the old app.html paints the body black, not `background`
      .bg(gpui_kit::black())
      .text_color(theme.foreground)
      .font_family(theme.font_family.clone())
      .on_mouse_down(
        MouseButton::Navigate(NavigationDirection::Back),
        move |_, _, cx| router.update(cx, |r, cx| r.back(cx)),
      )
      .on_mouse_down(
        MouseButton::Navigate(NavigationDirection::Forward),
        move |_, _, cx| forward.update(cx, |r, cx| r.forward(cx)),
      )
      .child(self.header.clone().cached(header_style))
      .child(div().h_px().w_full().flex_shrink_0().bg(theme.border))
      .child(
        div()
          .flex()
          .flex_1()
          .min_h_0()
          .w_full()
          .child(self.sidebar.clone().cached(sidebar_style))
          .child(div().w_px().h_full().flex_shrink_0().bg(theme.border))
          .child(div().id("main").flex_1().min_w_0().h_full().child(content)),
      )
  }
}

#[cfg(test)]
mod tests;
