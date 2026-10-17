//! Test seam for hardcoded API urls: tests point them at [`crate::test_http::FakeServer`].

use std::borrow::Cow;

/// Env var holding the fake origin, set by `FakeServer::start`
pub const FAKE_ORIGIN: &str = "PROTON_FAKE_ORIGIN";

/// `url` unchanged, or `https://host/path` as `{origin}/host/path` while a fake server runs
pub fn url(url: &str) -> Cow<'_, str> {
  #[cfg(any(test, feature = "test-support"))]
  if let Ok(origin) = std::env::var(FAKE_ORIGIN)
    && let Some(rest) = url.strip_prefix("https://")
  {
    return Cow::Owned(format!("{origin}/{rest}"));
  }
  Cow::Borrowed(url)
}
