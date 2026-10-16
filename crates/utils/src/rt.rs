//! The global tokio runtime. gpui's executor is not tokio, so reqwest, tokio::fs/net/process and
//! friends must run here. gpui code awaits the returned `JoinHandle`s.

use std::sync::LazyLock;

use tokio::runtime::Runtime;
pub use tokio::task::JoinHandle;

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .thread_name("proton-tokio")
    .build()
    .expect("Failed to start the tokio runtime")
});

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  RUNTIME.spawn(future)
}

pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
  F: FnOnce() -> R + Send + 'static,
  R: Send + 'static,
{
  RUNTIME.spawn_blocking(f)
}

/// Only for startup and tests, panics when called from inside the runtime
pub fn block_on<F: Future>(future: F) -> F::Output {
  RUNTIME.block_on(future)
}

#[cfg(test)]
mod tests {
  use gpui_kit::{self as gpui, AppContext, TestAppContext};

  struct Probe(Option<u32>);

  /// The pattern from PATTERNS.md: tokio work awaited from a view task
  #[gpui::test]
  async fn tokio_result_reaches_a_view(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let view = cx.new(|_| Probe(None));
    let task = view.update(cx, |_, cx| {
      cx.spawn(async move |this, cx| {
        let value = super::spawn(async { 42 }).await?;
        this.update(cx, |probe, cx| {
          probe.0 = Some(value);
          cx.notify();
        })
      })
    });
    task.await.unwrap();
    assert_eq!(view.read_with(cx, |probe, _| probe.0), Some(42));
  }

  #[test]
  fn spawn_runs_on_the_runtime() {
    assert_eq!(super::block_on(super::spawn(async { 1 + 1 })).unwrap(), 2);
    assert_eq!(super::block_on(super::spawn_blocking(|| 3)).unwrap(), 3);
  }
}
