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
  let mut url = Url::parse(&proton_utils::http::url(base)).expect("valid api base");
  url.path_segments_mut().expect("api base").extend(segments);
  if !query.is_empty() {
    url.query_pairs_mut().extend_pairs(query);
  }
  url
}

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  };

  use proton_utils::{
    rt,
    test_http::{FakeServer, Response},
  };
  use serde_json::json;

  use super::*;

  const BASE: &str = "https://api.test/v1";

  fn http() -> Http {
    Http::new(HeaderMap::new(), Policy::default())
  }

  fn get(url: Url) -> Result<Value> {
    rt::block_on(http().get(url))
  }

  /// Answers `fail` with `status` (Retry-After 0, so no real backoff), then 200
  fn flaky(server: &FakeServer, status: u16, fail: usize) -> Arc<AtomicUsize> {
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = attempts.clone();
    server.route_fn("GET", "/api.test/v1/x", move |_| {
      if seen.fetch_add(1, Ordering::SeqCst) < fail {
        Response::new(status, "busy").header("Retry-After", "0")
      } else {
        Response::new(200, r#"{"ok":true}"#)
      }
    });
    attempts
  }

  #[test]
  fn api_encodes_segments_and_query() {
    let url = api(BASE, &["project", "a b/c"], &[("q", "x&y"), ("e", "ü")]);
    assert_eq!(
      url.as_str(),
      "https://api.test/v1/project/a%20b%2Fc?q=x%26y&e=%C3%BC"
    );
    // no query keeps the url free of a trailing `?`
    assert_eq!(api(BASE, &[], &[]).as_str(), "https://api.test/v1");
  }

  #[test]
  fn api_reaches_the_fake_server() {
    let server = FakeServer::start();
    let url = api(BASE, &["x"], &[]);
    assert_eq!(url.as_str(), format!("{}/api.test/v1/x", server.origin()));
  }

  #[test]
  fn retries_429_and_5xx_until_success() {
    for status in [429, 500, 502, 503] {
      let server = FakeServer::start();
      let attempts = flaky(&server, status, 2);
      assert_eq!(get(api(BASE, &["x"], &[])).unwrap(), json!({"ok": true}));
      assert_eq!(attempts.load(Ordering::SeqCst), 3, "{status}");
    }
  }

  #[test]
  fn gives_up_after_three_retries() {
    for (status, err) in [(429, "RateLimited"), (503, "Network")] {
      let server = FakeServer::start();
      let attempts = flaky(&server, status, usize::MAX);
      let res = get(api(BASE, &["x"], &[]));
      assert_eq!(format!("{:?}", res.unwrap_err()), err);
      assert_eq!(attempts.load(Ordering::SeqCst), 4);
    }
  }

  #[test]
  fn client_errors_are_not_retried() {
    for (status, err) in [(404, "NotFound"), (400, "Network"), (401, "Network")] {
      let server = FakeServer::start();
      server.route("GET", "/api.test/v1/x", status, "{}");
      let res = get(api(BASE, &["x"], &[]));
      assert_eq!(format!("{:?}", res.unwrap_err()), err, "{status}");
      assert_eq!(server.calls().len(), 1);
    }
  }

  #[test]
  fn unparsable_retry_after_falls_back_to_backoff() {
    let server = FakeServer::start();
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = attempts.clone();
    server.route_fn("GET", "/api.test/v1/x", move |_| {
      if seen.fetch_add(1, Ordering::SeqCst) == 0 {
        // http dates are not parsed, the 1s backoff applies
        Response::new(503, "").header("Retry-After", "Wed, 21 Oct 2015 07:28:00 GMT")
      } else {
        Response::new(200, "1")
      }
    });
    let start = Instant::now();
    assert_eq!(get(api(BASE, &["x"], &[])).unwrap(), json!(1));
    assert!(start.elapsed() >= Duration::from_secs(1));
  }

  #[test]
  fn malformed_or_mismatched_json_is_a_network_error() {
    let server = FakeServer::start();
    server.route("GET", "/api.test/v1/bad", 200, "{not json");
    server.route("GET", "/api.test/v1/shape", 200, r#"{"a":1}"#);
    server.route("GET", "/api.test/v1/empty", 200, "");
    for path in ["bad", "empty"] {
      let res = get(api(BASE, &[path], &[]));
      assert!(matches!(res, Err(ContentError::Network)), "{path}");
    }
    let res: Result<Vec<String>> = rt::block_on(http().get(api(BASE, &["shape"], &[])));
    assert!(matches!(res, Err(ContentError::Network)));
  }

  #[test]
  fn unreachable_host_is_a_network_error() {
    // nothing listens on port 1
    let res = get(Url::parse("http://127.0.0.1:1/x").unwrap());
    assert!(matches!(res, Err(ContentError::Network)));
  }

  #[test]
  fn cache_hits_skip_the_request_per_url() {
    let server = FakeServer::start();
    server.route("GET", "/api.test/v1/x", 200, "[1]");
    let http = http();
    rt::block_on(async {
      for _ in 0..3 {
        let v: Vec<u8> = http.get_cached(api(BASE, &["x"], &[])).await.unwrap();
        assert_eq!(v, [1]);
      }
      // another query is another cache key
      let _: Vec<u8> = http
        .get_cached(api(BASE, &["x"], &[("a", "1")]))
        .await
        .unwrap();
      // plain get never reads the cache
      let _: Vec<u8> = http.get(api(BASE, &["x"], &[])).await.unwrap();
    });
    assert_eq!(server.calls().len(), 3);
  }

  #[test]
  fn cached_value_is_reparsed_per_type() {
    let server = FakeServer::start();
    server.route("GET", "/api.test/v1/x", 200, "[1]");
    let http = http();
    rt::block_on(async {
      let _: Vec<u8> = http.get_cached(api(BASE, &["x"], &[])).await.unwrap();
      // a hit with the wrong type fails like a fresh response would
      let res: Result<String> = http.get_cached(api(BASE, &["x"], &[])).await;
      assert!(matches!(res, Err(ContentError::Network)));
    });
    assert_eq!(server.calls().len(), 1);
  }

  #[test]
  fn errors_are_not_cached() {
    let server = FakeServer::start();
    server.route("GET", "/api.test/v1/x", 404, "");
    let http = http();
    rt::block_on(async {
      for _ in 0..2 {
        let res: Result<Value> = http.get_cached(api(BASE, &["x"], &[])).await;
        assert!(matches!(res, Err(ContentError::NotFound)));
      }
    });
    assert_eq!(server.calls().len(), 2);
  }

  #[test]
  fn post_sends_json_and_default_headers() {
    let server = FakeServer::start();
    server.route_fn("POST", "/api.test/v1/x", |call| {
      Response::new(200, call.body.clone())
    });
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", "secret".parse().unwrap());
    let http = Http::new(headers, Policy::default());
    let echoed: Value =
      rt::block_on(http.post(api(BASE, &["x"], &[]), &json!({"a": [1, 2]}))).unwrap();
    assert_eq!(echoed, json!({"a": [1, 2]}));

    let call = &server.calls()[0];
    assert_eq!(call.header("content-type"), Some("application/json"));
    assert_eq!(call.header("x-api-key"), Some("secret"));
    assert_eq!(call.header("user-agent"), Some(USER_AGENT));
  }

  #[test]
  fn retried_posts_resend_the_body() {
    let server = FakeServer::start();
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = attempts.clone();
    server.route_fn("POST", "/api.test/v1/x", move |call| {
      if seen.fetch_add(1, Ordering::SeqCst) == 0 {
        Response::new(500, "").header("Retry-After", "0")
      } else {
        Response::new(200, call.body.clone())
      }
    });
    let v: Value = rt::block_on(http().post(api(BASE, &["x"], &[]), &json!([3]))).unwrap();
    assert_eq!(v, json!([3]));
    let bodies: Vec<_> = server
      .calls()
      .iter()
      .map(|c| c.body_str().to_string())
      .collect();
    assert_eq!(bodies, ["[3]", "[3]"]);
  }
}
