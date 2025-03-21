use std::fmt::Display;

pub trait ResultLogExt {
  fn log(self) -> Self;
}

impl<T, E: Display> ResultLogExt for Result<T, E> {
  fn log(self) -> Self {
    if let Err(err) = &self {
      log::error!("Error: {}", err);
    }

    self
  }
}
