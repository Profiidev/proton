//! Theme, fonts and assets ([`assets`]), shared props-only widgets ([`widgets`]) and
//! test helpers ([`render_count`], `test_view`). No service state.

pub mod assets;
pub mod widgets;

/// Render counters for perf tests. Views call [`render_count::hit`] at the top of `render`;
/// it compiles to nothing outside tests and the `test-support` feature.
pub mod render_count {
  #[cfg(any(test, feature = "test-support"))]
  thread_local! {
    static COUNTS: std::cell::RefCell<std::collections::HashMap<&'static str, usize>> =
      Default::default();
  }

  /// Counts one render of `T`
  #[inline]
  pub fn hit<T: ?Sized>() {
    add(std::any::type_name::<T>(), 1);
  }

  /// Adds `n` to the counter `key` (e.g. rows a virtual list rendered)
  #[inline]
  pub fn add(_key: &'static str, _n: usize) {
    #[cfg(any(test, feature = "test-support"))]
    COUNTS.with(|c| *c.borrow_mut().entry(_key).or_default() += _n);
  }

  /// Renders of `T` so far on this thread
  #[cfg(any(test, feature = "test-support"))]
  pub fn of<T: ?Sized>() -> usize {
    get(std::any::type_name::<T>())
  }

  #[cfg(any(test, feature = "test-support"))]
  pub fn get(key: &str) -> usize {
    COUNTS.with(|c| c.borrow().get(key).copied().unwrap_or(0))
  }
}

/// gpui test helpers: a themed window drawing a closure
#[cfg(any(test, feature = "test-support"))]
pub mod test_view {
  use gpui_kit::{
    AnyElement, AnyWindowHandle, App, AppContext, Context, Entity, IntoElement, Render,
    TestAppContext, Window, WindowOptions,
  };

  type Build = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

  /// Draws whatever `build` returns, every frame
  pub struct View(Build);

  impl Render for View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
      (self.0)(window, cx)
    }
  }

  /// A kit-initialized app with the Proton theme and a window drawing `build`
  pub fn open(
    cx: &mut TestAppContext,
    build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
  ) -> (AnyWindowHandle, Entity<View>) {
    cx.update(|cx| {
      gpui_kit::init(cx);
      crate::assets::load(cx).unwrap();
      // animations run on the wall clock: a click mid dialog-entrance misses its target
      cx.set_reduce_motion(true);
      crate::assets::open_window(WindowOptions::default(), cx, |_, cx| {
        cx.new(|_| View(Box::new(build)))
      })
      .unwrap()
    })
  }

  /// Completes a frame
  pub fn draw(handle: AnyWindowHandle, cx: &mut TestAppContext) {
    use gpui_kit::test::TestWindowExt;
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
      .unwrap();
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{self as gpui, IntoElement, ParentElement, Styled, TestAppContext, div, px};

  use super::*;

  struct A;
  struct B;

  #[test]
  fn render_counts_are_per_type_and_key() {
    assert_eq!(render_count::of::<A>(), 0);
    render_count::hit::<A>();
    render_count::hit::<A>();
    render_count::hit::<B>();
    assert_eq!(render_count::of::<A>(), 2);
    assert_eq!(render_count::of::<B>(), 1);

    render_count::add("rows", 5);
    render_count::add("rows", 0);
    assert_eq!(render_count::get("rows"), 5);
    assert_eq!(render_count::get("unknown"), 0);
  }

  #[test]
  fn render_counts_are_per_thread() {
    render_count::hit::<A>();
    let other = std::thread::spawn(render_count::of::<A>).join().unwrap();
    assert_eq!(other, 0);
    assert_eq!(render_count::of::<A>(), 1);
  }

  #[gpui::test]
  fn test_view_redraws_the_closure_every_frame(cx: &mut TestAppContext) {
    let (handle, _) = test_view::open(cx, |_, _| {
      render_count::hit::<A>();
      div().w(px(10.)).child("x").into_any_element()
    });
    let before = render_count::of::<A>();
    assert!(before >= 1);
    test_view::draw(handle, cx);
    test_view::draw(handle, cx);
    assert_eq!(render_count::of::<A>(), before + 2);
    // the theme is set up and motion is reduced for clicks
    assert!(cx.update(|cx| cx.reduce_motion()));
  }
}
