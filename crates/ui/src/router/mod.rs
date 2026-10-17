//! The [`Route`] enum (old SvelteKit URLs in and out), back/forward history and breadcrumbs.

mod breadcrumbs;
mod route;
#[cfg(test)]
pub(crate) use route::tests::all_routes;

pub use breadcrumbs::{Crumb, Names, breadcrumbs};
pub use route::*;

use gpui_kit::{App, AppContext, Context, Entity, EventEmitter, Global, Subscription};

/// ponytail: history is capped, nobody walks back 100 pages
const HISTORY: usize = 100;

/// The current route plus back/forward history. Lives as a global entity, see [`RouterExt`].
pub struct Router {
  current: Route,
  back: Vec<Route>,
  forward: Vec<Route>,
}

/// Emitted with the new route after every change
#[derive(Debug, Clone)]
pub struct RouteChanged(pub Route);

impl EventEmitter<RouteChanged> for Router {}

impl Router {
  pub fn new(current: Route) -> Self {
    Self {
      current,
      back: vec![],
      forward: vec![],
    }
  }

  pub fn current(&self) -> &Route {
    &self.current
  }

  pub fn can_back(&self) -> bool {
    !self.back.is_empty()
  }

  pub fn can_forward(&self) -> bool {
    !self.forward.is_empty()
  }

  /// Goes to `route`, pushing the current one onto the back stack
  pub fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
    if route == self.current {
      return;
    }
    let previous = std::mem::replace(&mut self.current, route);
    self.back.push(previous);
    if self.back.len() > HISTORY {
      self.back.remove(0);
    }
    self.forward.clear();
    self.changed(cx);
  }

  /// Swaps the current route without touching history (redirects, restore)
  pub fn replace(&mut self, route: Route, cx: &mut Context<Self>) {
    if route != self.current {
      self.current = route;
      self.changed(cx);
    }
  }

  pub fn back(&mut self, cx: &mut Context<Self>) {
    if let Some(route) = self.back.pop() {
      let previous = std::mem::replace(&mut self.current, route);
      self.forward.push(previous);
      self.changed(cx);
    }
  }

  pub fn forward(&mut self, cx: &mut Context<Self>) {
    if let Some(route) = self.forward.pop() {
      let previous = std::mem::replace(&mut self.current, route);
      self.back.push(previous);
      self.changed(cx);
    }
  }

  fn changed(&mut self, cx: &mut Context<Self>) {
    cx.emit(RouteChanged(self.current.clone()));
    cx.notify();
  }

  /// Calls `f` after every route change, e.g. to persist it in the settings
  pub fn on_change(cx: &mut App, f: impl Fn(&Route, &mut App) + 'static) -> Subscription {
    let router = cx.router();
    cx.subscribe(&router, move |_, RouteChanged(route), cx| f(route, cx))
  }
}

struct GlobalRouter(Entity<Router>);

impl Global for GlobalRouter {}

/// Sets up the global router at `initial`
pub fn init(cx: &mut App, initial: Route) {
  let router = cx.new(|_| Router::new(initial));
  cx.set_global(GlobalRouter(router));
}

pub trait RouterExt {
  fn router(&self) -> Entity<Router>;
  fn route(&self) -> Route;
  fn navigate(&mut self, route: Route);
}

impl RouterExt for App {
  fn router(&self) -> Entity<Router> {
    self.global::<GlobalRouter>().0.clone()
  }

  fn route(&self) -> Route {
    self.router().read(self).current.clone()
  }

  fn navigate(&mut self, route: Route) {
    self.router().update(self, |r, cx| r.navigate(route, cx));
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use gpui_kit::{self as gpui, TestAppContext};

  use super::*;

  #[gpui::test]
  fn back_and_forward(cx: &mut TestAppContext) {
    cx.update(|cx| init(cx, Route::Home));
    let seen = Rc::new(RefCell::new(vec![]));
    let log = seen.clone();
    let _sub = cx.update(|cx| Router::on_change(cx, move |r, _| log.borrow_mut().push(r.clone())));
    let router = cx.update(|cx| cx.router());
    let current = |cx: &mut TestAppContext| cx.update(|cx| cx.route());

    cx.update(|cx| cx.navigate(Route::Profiles));
    cx.update(|cx| cx.navigate(Route::Profiles)); // no-op
    cx.update(|cx| cx.navigate(Route::Instances));
    router.update(cx, |r, cx| r.back(cx));
    assert_eq!(current(cx), Route::Profiles);
    router.update(cx, |r, cx| r.back(cx));
    assert_eq!(current(cx), Route::Home);
    router.update(cx, |r, cx| r.back(cx)); // nothing left
    assert!(!router.read_with(cx, |r, _| r.can_back()));
    router.update(cx, |r, cx| r.forward(cx));
    assert_eq!(current(cx), Route::Profiles);
    // a new navigation drops the forward stack
    cx.update(|cx| cx.navigate(Route::Settings(SettingsTab::Java)));
    assert!(!router.read_with(cx, |r, _| r.can_forward()));
    router.update(cx, |r, cx| r.replace(Route::Home, cx));
    router.update(cx, |r, cx| r.back(cx));
    assert_eq!(current(cx), Route::Profiles);

    cx.run_until_parked();
    assert_eq!(
      *seen.borrow(),
      [
        Route::Profiles,
        Route::Instances,
        Route::Profiles,
        Route::Home,
        Route::Profiles,
        Route::Settings(SettingsTab::Java),
        Route::Home,
        Route::Profiles,
      ]
    );
  }
}
