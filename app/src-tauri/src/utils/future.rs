use std::time::{Duration, Instant};

use anyhow::Result;
use tauri::async_runtime::{JoinHandle, spawn};
use tokio::{
  select,
  sync::mpsc::{self, Sender},
  task::JoinSet,
  time::sleep,
};

use crate::utils::log::ResultLogExt;

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

pub struct UpdateLimiter<T: Send + Sync + 'static> {
  _task: JoinHandle<()>,
  sender: Sender<T>,
}

impl<T: Send + Sync + 'static> UpdateLimiter<T> {
  pub fn new<F: Fn(T) + Send + 'static>(delay: Duration, f: F) -> Self {
    let (sender, mut receiver) = mpsc::channel(100);

    let task = spawn(async move {
      let mut data = None;
      let mut last_update = Instant::now();

      loop {
        let elapsed = last_update.elapsed();
        let remaining = if elapsed >= delay {
          // also try to trigger a update here
          // so when the updates over the channel are faster than the sleep timer resolution
          // a update gets triggered in any case
          if let Some(data_val) = data {
            f(data_val);
            data = None;
            last_update = Instant::now();
          }
          delay
        } else {
          delay - elapsed
        };

        select! {
          new_data = receiver.recv() => if new_data.is_some() {
            data = new_data;
          } else {
            // one last update before ending the update limiter
            if let Some(data) = data {
              f(data);
            }
            break;
          },
          _ = sleep(remaining) => {
            if let Some(data_val) = data {
              f(data_val);
              data = None;
              last_update = Instant::now();
            }
          },
        };
      }
    });

    Self {
      sender,
      _task: task,
    }
  }

  pub fn call(&self, data: T) -> Result<()> {
    Ok(self.sender.try_send(data).log()?)
  }
}
