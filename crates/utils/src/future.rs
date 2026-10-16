use std::time::Duration;

use anyhow::Result;
use tokio::{sync::watch, task::JoinSet, time::sleep};

use crate::rt::{JoinHandle, spawn};

pub const MAX_PARALLEL_DEFAULT: usize = 20;

pub struct FuturePool<O, F>
where
  F: Future<Output = O> + Send + 'static,
  O: Send + 'static,
{
  futures: Vec<F>,
}

impl<O, F> FuturePool<O, F>
where
  F: Future<Output = O> + Send + 'static,
  O: Send + 'static,
{
  pub fn new(futures: Vec<F>) -> Self {
    FuturePool { futures }
  }

  pub async fn run(self, max_parallel: Option<usize>) -> Vec<Result<O>> {
    self.run_cb(max_parallel, |_, _| {}).await
  }

  pub async fn run_cb<C: Fn(usize, usize)>(
    self,
    max_parallel: Option<usize>,
    cb: C,
  ) -> Vec<Result<O>> {
    let mut left = self.futures;
    let mut running = JoinSet::new();
    let total = left.len();

    for _ in 0..max_parallel.unwrap_or(MAX_PARALLEL_DEFAULT) {
      if let Some(future) = left.pop() {
        running.spawn(future);
      }
    }

    let mut results = Vec::new();
    let mut done = 0;
    while let Some(result) = running.join_next().await {
      results.push(result.map_err(|e| e.into()));
      if let Some(future) = left.pop() {
        running.spawn(future);
      }

      done += 1;
      cb(done, total);
    }

    results
  }
}

/// Calls `f` with the latest data at most once per `delay`, the last data is always delivered
pub struct UpdateLimiter<T: Clone + Send + Sync + 'static> {
  _task: JoinHandle<()>,
  sender: watch::Sender<Option<T>>,
}

impl<T: Clone + Send + Sync + 'static> UpdateLimiter<T> {
  pub fn new<F: Fn(T) + Send + 'static>(delay: Duration, f: F) -> Self {
    let (sender, mut receiver) = watch::channel(None);

    // changed() still reports unseen data after the sender is dropped, so the final update is sent
    let task = spawn(async move {
      while receiver.changed().await.is_ok() {
        let data = receiver.borrow_and_update().clone();
        if let Some(data) = data {
          f(data);
        }
        sleep(delay).await;
      }
    });

    Self {
      sender,
      _task: task,
    }
  }

  pub fn call(&self, data: T) {
    self.sender.send_replace(Some(data));
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{Arc, Mutex};

  use super::*;

  #[tokio::test]
  async fn limiter_delivers_the_last_update() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let limiter = UpdateLimiter::new(Duration::from_millis(20), move |n| {
      sink.lock().unwrap().push(n)
    });
    for n in 0..1000 {
      limiter.call(n);
    }
    drop(limiter);
    sleep(Duration::from_millis(100)).await;
    let seen = seen.lock().unwrap();
    assert!(seen.len() < 10);
    assert_eq!(seen.last(), Some(&999));
  }
}
