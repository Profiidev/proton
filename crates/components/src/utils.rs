use gpui_kit::{AppContext, Entity};

/// Updates an entity and notifies observers only when the value really changed
pub trait WriteChangedExt<T> {
  fn write_changed<C: AppContext>(&self, cx: &mut C, next: T);
}

impl<T: PartialEq + 'static> WriteChangedExt<T> for Entity<T> {
  fn write_changed<C: AppContext>(&self, cx: &mut C, next: T) {
    self.update(cx, |value, cx| {
      if *value != next {
        *value = next;
        cx.notify();
      }
    });
  }
}

/// Logs the error of a result and passes it on
pub trait ErrorLogExt {
  fn log_err(self) -> Self;
}

impl<T, E: std::fmt::Debug> ErrorLogExt for Result<T, E> {
  fn log_err(self) -> Self {
    if let Err(e) = &self {
      tracing::warn!("{:?}", e);
    }
    self
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use gpui_kit::{self as gpui, AppContext, TestAppContext};

  use super::*;

  fn counted<T: 'static>(cx: &mut TestAppContext, value: T) -> (Entity<T>, Rc<Cell<usize>>) {
    let entity = cx.new(|_| value);
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    cx.update(|cx| {
      cx.observe(&entity, move |_, _| seen.set(seen.get() + 1))
        .detach()
    });
    (entity, count)
  }

  #[gpui::test]
  fn notifies_only_on_change(cx: &mut TestAppContext) {
    let (entity, count) = counted(cx, 1);
    entity.write_changed(cx, 1);
    cx.run_until_parked();
    assert_eq!(count.get(), 0);
    entity.write_changed(cx, 2);
    cx.run_until_parked();
    assert_eq!((count.get(), entity.read_with(cx, |v, _| *v)), (1, 2));
    entity.write_changed(cx, 2);
    cx.run_until_parked();
    assert_eq!(count.get(), 1);
  }

  #[test]
  fn log_err_returns_its_input() {
    assert_eq!(Ok::<_, &str>(1).log_err(), Ok(1));
    assert_eq!(Err::<i32, _>("boom").log_err(), Err("boom"));
  }
}
