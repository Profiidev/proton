pub mod glide;
pub mod smooth_retarget;

use std::time::Duration;

use gpui_kit::App;
pub use gpui_kit::base::animation::{ease_in_cubic, ease_in_out_cubic, ease_out_cubic};

/// `base`, or zero when the system asks for reduced motion
pub fn animation_duration(base: Duration, cx: &App) -> Duration {
  if cx.reduce_motion() {
    Duration::ZERO
  } else {
    base
  }
}

#[cfg(test)]
mod tests {
  use gpui_kit::{self as gpui, TestAppContext};

  use super::*;

  const BASE: Duration = Duration::from_millis(300);

  #[gpui::test]
  fn follows_reduce_motion(cx: &mut TestAppContext) {
    cx.update(|cx| {
      assert_eq!(animation_duration(BASE, cx), BASE);
      cx.set_reduce_motion(true);
      assert_eq!(animation_duration(BASE, cx), Duration::ZERO);
    });
  }
}
