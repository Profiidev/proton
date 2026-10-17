//! shadcn `Slider` (single, horizontal). gpui-kit's slider draws a 6px track in one colour and a
//! 16px thumb; this is the shadcn look: track `bg-muted h-1 rounded-full`, range `bg-primary`,
//! thumb `size-3 rounded-full border border-ring bg-white`, `active:ring-3 ring-ring/50`.
//! bits-ui "contain" positioning: the thumb centre runs from 6px to width-6px, the pointer maps
//! over the full width.
//!
//! ```ignore
//! let slider = cx.new(|cx| Slider::new(1024., 8192., 256., 2048., cx));
//! cx.subscribe(&slider, |_, _, e: &SliderEvent, _| match e { .. });
//! ```

use gpui_kit::{
  Bounds, Context, DispatchPhase, EventEmitter, FocusHandle, InteractiveElement, IntoElement,
  KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
  Render, Styled, Window, base::TestSupportExt, canvas, component::ActiveTheme, div,
  prelude::FluentBuilder, px, relative, white,
};

const THUMB: f32 = 12.;

pub enum SliderEvent {
  /// live value while dragging / per key press
  Change(f32),
  /// pointer released or key pressed: commit
  Release(f32),
}

/// Clamps to `min..=max` and rounds to the nearest `step` counted from `min` (bits-ui: an
/// off-grid `max` is not a step)
pub fn snap(value: f32, min: f32, max: f32, step: f32) -> f32 {
  let v = value.clamp(min, max);
  if step <= 0. {
    return v;
  }
  let last = ((max - min) / step).floor();
  min + ((v - min) / step).round().min(last) * step
}

pub struct Slider {
  value: f32,
  min: f32,
  max: f32,
  step: f32,
  disabled: bool,
  dragging: bool,
  /// focused by keyboard: shows the ring like `focus-visible`
  keyboard: bool,
  bounds: Bounds<Pixels>,
  focus: FocusHandle,
}

impl EventEmitter<SliderEvent> for Slider {}

impl Slider {
  pub fn new(min: f32, max: f32, step: f32, value: f32, cx: &mut Context<Self>) -> Self {
    Self {
      value: snap(value, min, max, step),
      min,
      max,
      step,
      disabled: false,
      dragging: false,
      keyboard: false,
      bounds: Bounds::default(),
      focus: cx.focus_handle(),
    }
  }

  pub fn value(&self) -> f32 {
    self.value
  }

  /// Sets the value without emitting events
  pub fn set_value(&mut self, value: f32, cx: &mut Context<Self>) {
    self.value = snap(value, self.min, self.max, self.step);
    cx.notify();
  }

  pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
    self.disabled = disabled;
    self.dragging &= !disabled;
    cx.notify();
  }

  fn change(&mut self, value: f32, cx: &mut Context<Self>) {
    let value = snap(value, self.min, self.max, self.step);
    if value != self.value {
      self.value = value;
      cx.emit(SliderEvent::Change(value));
      cx.notify();
    }
  }

  fn change_to_x(&mut self, x: Pixels, cx: &mut Context<Self>) {
    let width = self.bounds.size.width;
    let frac = if width > px(0.) {
      ((x - self.bounds.left()) / width).clamp(0., 1.)
    } else {
      0.
    };
    self.change(self.min + frac * (self.max - self.min), cx);
  }

  fn release(&mut self, cx: &mut Context<Self>) {
    self.dragging = false;
    cx.emit(SliderEvent::Release(self.value));
    cx.notify();
  }
}

impl Render for Slider {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let span = self.max - self.min;
    let frac = if span > 0. {
      (self.value - self.min) / span
    } else {
      0.
    };
    let ring = self.dragging || (self.keyboard && self.focus.is_focused(window));
    let entity = cx.entity();
    let dragging = self.dragging;
    let half = px(THUMB / 2.);
    // positions the thumb centre over the inset `6px..width-6px` box
    let inset = || div().absolute().top_0().bottom_0().left(half).right(half);

    div()
      .id("slider")
      .test_support()
      .track_focus(&self.focus)
      .relative()
      .w_full()
      .h_1()
      .when(self.disabled, |d| d.opacity(0.5))
      .child(
        div()
          .absolute()
          .size_full()
          .overflow_hidden()
          .rounded_full()
          .bg(theme.muted)
          .child(
            inset().child(
              div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(-half)
                .right(relative(1. - frac))
                .bg(theme.primary),
            ),
          ),
      )
      .child(
        inset()
          .when(ring, |d| {
            d.child(
              div()
                .absolute()
                .size(px(THUMB + 6.))
                .top(px(-7.))
                .left(relative(frac))
                .ml(px(-(THUMB + 6.) / 2.))
                .rounded_full()
                .bg(theme.ring.opacity(0.5)),
            )
          })
          .child(
            div()
              .absolute()
              .size(px(THUMB))
              .top(px(-4.))
              .left(relative(frac))
              .ml(-half)
              .rounded_full()
              .border_1()
              .border_color(theme.ring)
              .bg(white()),
          ),
      )
      .child(
        canvas(
          |_, _, _| {},
          move |bounds, _, window, cx| {
            entity.update(cx, |s, _| s.bounds = bounds);
            if !dragging {
              return;
            }
            // window-wide while dragging: the pointer may leave the slider
            let e = entity.clone();
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
              if phase == DispatchPhase::Bubble {
                e.update(cx, |s, cx| s.change_to_x(ev.position.x, cx));
              }
            });
            window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
              if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left {
                entity.update(cx, |s, cx| {
                  if s.dragging {
                    s.release(cx)
                  }
                });
              }
            });
          },
        )
        .absolute()
        .size_full(),
      )
      .when(!self.disabled, |d| {
        d.on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
          let value = match e.keystroke.key.as_str() {
            "left" | "down" => this.value - this.step,
            "right" | "up" => this.value + this.step,
            "home" => this.min,
            "end" => this.max,
            _ => return,
          };
          this.keyboard = true;
          this.change(value, cx);
          this.release(cx);
          cx.stop_propagation();
        }))
        // hit area: the 12px thumb height plus bits-ui's `after:-inset-2`
        .child(
          div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(-10.))
            .bottom(px(-10.))
            .cursor_pointer()
            .on_mouse_down(
              MouseButton::Left,
              cx.listener(|this, e: &MouseDownEvent, window, cx| {
                window.focus(&this.focus, cx);
                this.keyboard = false;
                this.dragging = true;
                this.change_to_x(e.position.x, cx);
                cx.notify();
                cx.stop_propagation();
              }),
            ),
        )
      })
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use gpui_kit::{self as gpui, AppContext, Entity, TestAppContext, point, test::TestWindowExt};

  use super::*;
  use crate::test_view;

  #[test]
  fn snap_clamps_and_steps() {
    assert_eq!(snap(500., 1024., 8000., 256.), 1024.);
    assert_eq!(snap(9000., 1024., 8192., 256.), 8192.);
    assert_eq!(snap(1500., 1024., 8000., 256.), 1536.);
    assert_eq!(snap(1100., 1024., 8000., 256.), 1024.);
    // like bits-ui, an off-grid max snaps to the last step
    assert_eq!(snap(8000., 1024., 8000., 256.), 7936.);
    assert_eq!(snap(3.3, 0., 10., 0.), 3.3);
  }

  type Log = Rc<RefCell<Vec<(bool, f32)>>>;

  fn harness(cx: &mut TestAppContext) -> (gpui::AnyWindowHandle, Entity<Slider>, Log) {
    let slider = cx.new(|cx| Slider::new(0., 100., 10., 0., cx));
    let log: Log = Rc::default();
    cx.update(|cx| {
      let log = log.clone();
      cx.subscribe(&slider, move |_, e: &SliderEvent, _| {
        log.borrow_mut().push(match e {
          SliderEvent::Change(v) => (false, *v),
          SliderEvent::Release(v) => (true, *v),
        })
      })
      .detach();
    });
    let (handle, _) = test_view::open(cx, {
      let slider = slider.clone();
      move |_, _| {
        div()
          .p(px(20.))
          .w(px(240.))
          .child(slider.clone())
          .into_any_element()
      }
    });
    test_view::draw(handle, cx);
    (handle, slider, log)
  }

  #[gpui::test]
  fn click_jumps_to_value(cx: &mut TestAppContext) {
    let (handle, slider, log) = harness(cx);
    cx.update_window(handle, |_, window, cx| {
      // 200px wide: 70% of the track
      window.click_at("slider", point(px(140.), px(2.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(slider.read_with(cx, |s, _| s.value()), 70.);
    assert_eq!(*log.borrow(), [(false, 70.), (true, 70.)]);
  }

  #[gpui::test]
  fn drag_follows_pointer_past_the_end(cx: &mut TestAppContext) {
    let (handle, slider, log) = harness(cx);
    cx.update_window(handle, |_, window, cx| {
      let b = window.find("slider").bounds();
      let y = b.center().y;
      window.drag(
        point(b.left() + px(20.), y),
        point(b.right() + px(50.), y),
        cx,
      );
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(slider.read_with(cx, |s, _| s.value()), 100.);
    let log = log.borrow();
    assert_eq!(log.first(), Some(&(false, 10.)));
    assert_eq!(log.last(), Some(&(true, 100.)));
    assert_eq!(log.iter().filter(|(r, _)| *r).count(), 1);
  }

  #[gpui::test]
  fn disabled_is_inert(cx: &mut TestAppContext) {
    let (handle, slider, log) = harness(cx);
    slider.update(cx, |s, cx| s.set_disabled(true, cx));
    test_view::draw(handle, cx);
    cx.update_window(handle, |_, window, cx| {
      window.click_at("slider", point(px(140.), px(2.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(slider.read_with(cx, |s, _| s.value()), 0.);
    assert!(log.borrow().is_empty());
  }
}
