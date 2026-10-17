use std::time::Duration;

use anyhow::Result;
use tokio::{sync::watch, task::JoinSet, time::sleep};

use crate::rt::{JoinHandle, spawn};

const MAX_PARALLEL: usize = 20;

/// Runs `futures` on tokio, at most [`MAX_PARALLEL`] at once; `cb(done, total)` after each
pub async fn run_pool<O, F>(mut futures: Vec<F>, cb: impl Fn(usize, usize)) -> Vec<Result<O>>
where
  F: Future<Output = O> + Send + 'static,
  O: Send + 'static,
{
  let total = futures.len();
  let mut running = JoinSet::new();
  for future in futures.drain(futures.len().saturating_sub(MAX_PARALLEL)..) {
    running.spawn(future);
  }

  let mut results = Vec::with_capacity(total);
  while let Some(result) = running.join_next().await {
    results.push(result.map_err(Into::into));
    if let Some(future) = futures.pop() {
      running.spawn(future);
    }
    cb(results.len(), total);
  }
  results
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
