use std::{
  collections::HashMap,
  sync::Mutex,
  time::{Duration, Instant},
};

use reqwest::{
  Client, RequestBuilder, StatusCode, Url,
  header::{HeaderMap, RETRY_AFTER},
  redirect::Policy,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{ContentError, Result, USER_AGENT};

const TTL: Duration = Duration::from_secs(5 * 60);
const RETRIES: u32 = 3;

/// HTTP client for one provider. Default headers (e.g. the CurseForge key) only go to this provider.
pub struct Http {
  pub client: Client,
  /// Same headers without the total timeout, for file downloads
  pub download: Client,
  cache: Mutex<HashMap<String, (Instant, Value)>>,
}

impl Http {
  /// `redirect` applies to file downloads
  pub fn new(headers: HeaderMap, redirect: Policy) -> Self {
    Self {
      client: Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .default_headers(headers.clone())
        .build()
        .unwrap(),
      download: Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .default_headers(headers)
        .redirect(redirect)
        .build()
        .unwrap(),
      cache: Mutex::default(),
    }
  }

  pub async fn get<T: DeserializeOwned>(&self, url: Url) -> Result<T> {
    parse(send(self.client.get(url)).await?)
  }

  /// GET through the 5 minute cache
  pub async fn get_cached<T: DeserializeOwned>(&self, url: Url) -> Result<T> {
    let key = url.to_string();
    let hit = self
      .cache
      .lock()
      .unwrap()
      .get(&key)
      .filter(|(at, _)| at.elapsed() < TTL)
      .map(|(_, v)| v.clone());
    if let Some(value) = hit {
      return parse(value);
    }

    let value = send(self.client.get(url)).await?;
    let mut cache = self.cache.lock().unwrap();
    cache.retain(|_, (at, _)| at.elapsed() < TTL);
    cache.insert(key, (Instant::now(), value.clone()));
    drop(cache);
    parse(value)
  }

  pub async fn post<T: DeserializeOwned>(&self, url: Url, body: &impl Serialize) -> Result<T> {
    parse(send(self.client.post(url).json(body)).await?)
  }
}

fn parse<T: DeserializeOwned>(value: Value) -> Result<T> {
  serde_json::from_value(value).map_err(|e| {
    tracing::warn!("Failed to parse content response: {e}");
    ContentError::Network
  })
}

/// Sends the request, retrying 429 and 5xx with `Retry-After` or 1s, 2s, 4s backoff
async fn send(req: RequestBuilder) -> Result<Value> {
  let mut delay = 1;
  for attempt in 0..=RETRIES {
    let res = req
      .try_clone()
      .expect("request body is not a stream")
      .send()
      .await
      .map_err(|e| {
        tracing::warn!("Content request failed: {e}");
        ContentError::Network
      })?;
    let status = res.status();

    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
      if attempt == RETRIES {
        tracing::warn!(
          "Content request returned {status} after retries: {}",
          res.url()
        );
        return Err(if status == StatusCode::TOO_MANY_REQUESTS {
          ContentError::RateLimited
        } else {
          ContentError::Network
        });
      }
      let wait = res
        .headers()
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
        .unwrap_or(delay)
        .min(10);
      tokio::time::sleep(Duration::from_secs(wait)).await;
      delay *= 2;
      continue;
    }

    return match status {
      StatusCode::NOT_FOUND => Err(ContentError::NotFound),
      s if s.is_success() => res.json().await.map_err(|e| {
        tracing::warn!("Failed to read content response: {e}");
        ContentError::Network
      }),
      s => {
        tracing::warn!("Content request returned {s}: {}", res.url());
        Err(ContentError::Network)
      }
    };
  }
  unreachable!()
}

/// Builds `base/segments...?query`, encoding each segment
pub fn api(base: &str, segments: &[&str], query: &[(&str, &str)]) -> Url {
  let mut url = Url::parse(base).expect("valid api base");
  url.path_segments_mut().expect("api base").extend(segments);
  if !query.is_empty() {
    url.query_pairs_mut().extend_pairs(query);
  }
  url
}
