//! A fake HTTP/1.1 server for tests. Hardcoded API urls reach it through [`crate::http::url`],
//! urls inside canned responses can point at [`FakeServer::origin`] directly.

use std::{
  net::SocketAddr,
  sync::{Arc, Mutex},
};

use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
  net::{TcpListener, TcpStream},
};

use crate::{http::FAKE_ORIGIN, rt};

/// One request the server received
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
  pub method: String,
  /// Path with query, e.g. `/api.modrinth.com/v2/project?x=1`
  pub path: String,
  pub headers: Vec<(String, String)>,
  pub body: Vec<u8>,
}

impl Call {
  pub fn header(&self, name: &str) -> Option<&str> {
    let (_, value) = self
      .headers
      .iter()
      .find(|(n, _)| n.eq_ignore_ascii_case(name))?;
    Some(value)
  }

  pub fn body_str(&self) -> &str {
    std::str::from_utf8(&self.body).unwrap_or_default()
  }
}

pub struct Response {
  pub status: u16,
  pub headers: Vec<(String, String)>,
  pub body: Vec<u8>,
}

impl Response {
  pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
    Self {
      status,
      headers: Vec::new(),
      body: body.into(),
    }
  }

  pub fn header(mut self, name: &str, value: &str) -> Self {
    self.headers.push((name.into(), value.into()));
    self
  }
}

type Handler = Box<dyn Fn(&Call) -> Response + Send + Sync>;

#[derive(Default)]
struct State {
  routes: Vec<(String, String, Handler)>,
  calls: Vec<Call>,
}

/// Routes match on method and path without the query, the latest route wins. Unknown routes 404.
#[derive(Clone)]
pub struct FakeServer {
  addr: SocketAddr,
  state: Arc<Mutex<State>>,
}

impl FakeServer {
  /// Binds a free port and points [`crate::http::url`] at it for this process
  pub fn start() -> Self {
    let std = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std.set_nonblocking(true).unwrap();
    let addr = std.local_addr().unwrap();
    let state = Arc::new(Mutex::new(State::default()));

    let accept_state = state.clone();
    rt::spawn(async move {
      let listener = TcpListener::from_std(std).unwrap();
      while let Ok((stream, _)) = listener.accept().await {
        rt::spawn(serve(stream, accept_state.clone()));
      }
    });

    let server = Self { addr, state };
    // nextest runs one process per test, so the env var is not shared
    unsafe { std::env::set_var(FAKE_ORIGIN, server.origin()) };
    server
  }

  /// `http://127.0.0.1:port`
  pub fn origin(&self) -> String {
    format!("http://{}", self.addr)
  }

  /// `https://host/path` as the server sees it, for routes: `/host/path`
  pub fn path(url: &str) -> String {
    format!("/{}", url.strip_prefix("https://").unwrap_or(url))
  }

  pub fn route(&self, method: &str, path: &str, status: u16, body: impl Into<Vec<u8>>) -> &Self {
    let body = body.into();
    self.route_fn(method, path, move |_| Response::new(status, body.clone()))
  }

  pub fn route_fn(
    &self,
    method: &str,
    path: &str,
    handler: impl Fn(&Call) -> Response + Send + Sync + 'static,
  ) -> &Self {
    let route = (method.into(), path.into(), Box::new(handler) as Handler);
    self.state.lock().unwrap().routes.push(route);
    self
  }

  pub fn calls(&self) -> Vec<Call> {
    self.state.lock().unwrap().calls.clone()
  }
}

async fn serve(stream: TcpStream, state: Arc<Mutex<State>>) -> std::io::Result<()> {
  let mut reader = BufReader::new(stream);
  let mut line = String::new();
  reader.read_line(&mut line).await?;
  let mut parts = line.split_whitespace();
  let method = parts.next().unwrap_or_default().to_string();
  let path = parts.next().unwrap_or_default().to_string();

  let mut headers = Vec::new();
  loop {
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 || line.trim().is_empty() {
      break;
    }
    if let Some((name, value)) = line.split_once(':') {
      headers.push((name.trim().to_string(), value.trim().to_string()));
    }
  }
  let call = Call {
    method,
    path,
    headers,
    body: Vec::new(),
  };
  let len = call
    .header("content-length")
    .and_then(|l| l.parse().ok())
    .unwrap_or(0);
  let mut body = vec![0; len];
  reader.read_exact(&mut body).await?;
  let call = Call { body, ..call };

  let res = {
    let mut state = state.lock().unwrap();
    state.calls.push(call.clone());
    let bare = call.path.split('?').next().unwrap_or_default();
    match state
      .routes
      .iter()
      .rev()
      .find(|(m, p, _)| *m == call.method && p == bare)
    {
      Some((_, _, handler)) => handler(&call),
      None => Response::new(404, "no route"),
    }
  };

  let mut head = format!(
    "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
    res.status,
    res.body.len()
  );
  for (name, value) in &res.headers {
    head.push_str(&format!("{name}: {value}\r\n"));
  }
  head.push_str("\r\n");
  let mut stream = reader.into_inner();
  stream.write_all(head.as_bytes()).await?;
  stream.write_all(&res.body).await?;
  stream.shutdown().await
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{download::default_client, http::url};

  async fn get(path: &str) -> (u16, String) {
    let res = default_client().get(&*url(path)).send().await.unwrap();
    (res.status().as_u16(), res.text().await.unwrap())
  }

  #[test]
  fn routes_and_records_calls() {
    let server = FakeServer::start();
    server.route("GET", "/api.example.org/v1", 200, "first");
    // the latest route wins, so tests can change an answer mid-way
    server.route("GET", "/api.example.org/v1", 201, "second");

    rt::block_on(async {
      assert_eq!(
        get("https://api.example.org/v1?q=ü").await,
        (201, "second".into())
      );
      assert_eq!(
        get("https://api.example.org/missing").await,
        (404, "no route".into())
      );
      let res = default_client()
        .post(&*url("https://api.example.org/v1"))
        .body("payload")
        .send()
        .await
        .unwrap();
      // a POST route was never added
      assert_eq!(res.status(), 404);
    });

    let calls = server.calls();
    let paths: Vec<_> = calls
      .iter()
      .map(|c| (c.method.as_str(), c.path.as_str()))
      .collect();
    assert_eq!(
      paths,
      [
        ("GET", "/api.example.org/v1?q=%C3%BC"),
        ("GET", "/api.example.org/missing"),
        ("POST", "/api.example.org/v1"),
      ]
    );
    assert_eq!(calls[2].body_str(), "payload");
    assert_eq!(calls[2].header("CONTENT-LENGTH"), Some("7"));
  }

  #[test]
  fn handlers_see_the_request_and_set_headers() {
    let server = FakeServer::start();
    server.route_fn("POST", "/echo", |call| {
      Response::new(200, call.body.clone()).header("Retry-After", "3")
    });
    let res = rt::block_on(async {
      let res = default_client()
        .post(format!("{}/echo", server.origin()))
        .body(vec![0u8, 255, 10])
        .send()
        .await
        .unwrap();
      let retry = res.headers()["retry-after"].to_str().unwrap().to_string();
      (retry, res.bytes().await.unwrap().to_vec())
    });
    assert_eq!(res, ("3".into(), vec![0, 255, 10]));
  }

  #[test]
  fn url_only_rewrites_https_while_a_server_runs() {
    unsafe { std::env::remove_var(FAKE_ORIGIN) };
    assert_eq!(url("https://a.b/c"), "https://a.b/c");
    let server = FakeServer::start();
    assert_eq!(
      url("https://a.b/c?d=e"),
      format!("{}/a.b/c?d=e", server.origin())
    );
    // already local or plain http stays as is
    assert_eq!(url("http://a.b/c"), "http://a.b/c");
    assert_eq!(FakeServer::path("https://a.b/c"), "/a.b/c");
  }
}
