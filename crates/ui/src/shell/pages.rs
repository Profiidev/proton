use std::rc::Rc;

use gpui_kit::{
  AnyView, App, AppContext, Context, Global, IntoElement, ParentElement, Render, SharedString,
  Styled, Window, component::ActiveTheme, div,
};

use crate::router::Route;

type Matches = fn(&Route) -> bool;
type BuildPage = Rc<dyn Fn(&Route, &mut Window, &mut App) -> AnyView>;
type BuildProfileHeader = Rc<dyn Fn(&str, &mut Window, &mut App) -> AnyView>;
type BuildView = Rc<dyn Fn(&mut Window, &mut App) -> AnyView>;

/// Page registry: Phase 2 crates register their views here, the shell never
/// names them. Register before the main window opens.
///
/// Pages are laid out at the full size of the content area and are cached per
/// route, so a page root is `size_full()` and scrolls on its own.
#[derive(Default)]
pub struct PageFactory {
  pages: Vec<(Matches, BuildPage)>,
  pub(crate) profile_header: Option<BuildProfileHeader>,
  pub(crate) sidebar_footer: Option<BuildView>,
}

impl Global for PageFactory {}

impl PageFactory {
  /// Builds every route `matches` accepts; the first matching registration wins
  pub fn register(
    cx: &mut App,
    matches: Matches,
    build: impl Fn(&Route, &mut Window, &mut App) -> AnyView + 'static,
  ) {
    cx.default_global::<Self>()
      .pages
      .push((matches, Rc::new(build)));
  }

  /// The header above the profile tabs (icon, name, play button), per profile id
  pub fn set_profile_header(
    cx: &mut App,
    build: impl Fn(&str, &mut Window, &mut App) -> AnyView + 'static,
  ) {
    cx.default_global::<Self>().profile_header = Some(Rc::new(build));
  }

  /// Below the sidebar separator (the account selector)
  pub fn set_sidebar_footer(
    cx: &mut App,
    build: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
  ) {
    cx.default_global::<Self>().sidebar_footer = Some(Rc::new(build));
  }

  pub(crate) fn build(route: &Route, window: &mut Window, cx: &mut App) -> AnyView {
    let build = cx
      .try_global::<Self>()
      .and_then(|f| f.pages.iter().find(|(m, _)| m(route)))
      .map(|(_, b)| b.clone());
    match build {
      Some(build) => build(route, window, cx),
      None => {
        let label = route.to_url().into();
        cx.new(|_| Placeholder(label)).into()
      }
    }
  }
}

/// Stands in for pages nobody registered yet
pub struct Placeholder(pub SharedString);

impl Render for Placeholder {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .size_full()
      .p_5()
      .text_sm()
      .text_color(cx.theme().muted_foreground)
      .child(self.0.clone())
  }
}
