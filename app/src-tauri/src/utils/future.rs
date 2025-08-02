use std::{future::Future, pin::Pin};

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

  pub async fn run<C: Fn(usize, usize)>(
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

pub enum DataOrFuture<T> {
  Data(T),
  Future(Pin<Box<dyn Future<Output = Result<T>> + Send>>),
}

impl<T> DataOrFuture<T> {
  pub fn data(data: T) -> Self {
    DataOrFuture::Data(data)
  }

  pub fn future<F>(future: F) -> Self
  where
    F: Future<Output = Result<T>> + Send + 'static,
  {
    DataOrFuture::Future(Box::pin(future))
  }

  pub fn is_data(&self) -> bool {
    matches!(self, DataOrFuture::Data(_))
  }

  pub async fn resolve(self) -> Result<T> {
    match self {
      DataOrFuture::Data(data) => Ok(data),
      DataOrFuture::Future(future) => future.await,
    }
  }
}
