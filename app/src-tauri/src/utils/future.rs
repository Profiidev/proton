use std::future::Future;

use anyhow::Result;
use tokio::task::JoinSet;

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
