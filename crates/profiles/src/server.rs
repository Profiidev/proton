use std::time::Duration;

use url::Url;

use proton_utils::download::default_client;

const STATUS_BASE_URL: &str = "https://api.mcstatus.io/v2/status/java";

#[derive(Debug, Clone, PartialEq)]
pub struct ServerStatus {
  pub online: bool,
  pub players_online: u32,
  pub players_max: u32,
}

/// Status of a java server, None when the lookup failed. Needs the tokio runtime.
pub async fn server_status(address: &str) -> Option<ServerStatus> {
  let mut url = Url::parse(&proton_utils::http::url(STATUS_BASE_URL)).ok()?;
  // push encodes the address, so `/`, `?` or `#` can not change the request target
  url.path_segments_mut().ok()?.push(address);
  let resp = default_client()
    .get(url)
    .timeout(Duration::from_secs(5))
    .send()
    .await
    .ok()?;
  parse_server_status(&resp.text().await.ok()?)
}

fn parse_server_status(json: &str) -> Option<ServerStatus> {
  let value: serde_json::Value = serde_json::from_str(json).ok()?;
  let count = |key: &str| value["players"][key].as_u64().unwrap_or(0) as u32;
  Some(ServerStatus {
    online: value["online"].as_bool()?,
    players_online: count("online"),
    players_max: count("max"),
  })
}

#[cfg(test)]
mod tests {
  use proton_utils::{http::FAKE_ORIGIN, rt, test_http::FakeServer};

  use super::*;

  const PATH: &str = "/api.mcstatus.io/v2/status/java";

  fn status(online: bool, players_online: u32, players_max: u32) -> Option<ServerStatus> {
    Some(ServerStatus {
      online,
      players_online,
      players_max,
    })
  }

  #[test]
  fn server_status_maps_each_response() {
    let cases: [(u16, &str, Option<ServerStatus>); 5] = [
      (
        200,
        r#"{"online":true,"players":{"online":3,"max":10}}"#,
        status(true, 3, 10),
      ),
      (200, r#"{"online":false}"#, status(false, 0, 0)),
      (200, "<html>", None),
      (200, "", None),
      // the status code is not checked, an error body has no `online`
      (500, r#"{"error":"boom"}"#, None),
    ];
    for (code, body, expected) in cases {
      let server = FakeServer::start();
      server.route("GET", &format!("{PATH}/mc.example.org"), code, body);
      assert_eq!(
        rt::block_on(server_status("mc.example.org")),
        expected,
        "{code} {body}"
      );
    }
  }

  #[test]
  fn server_status_encodes_the_address_into_one_segment() {
    let server = FakeServer::start();
    // no route, so 404 "no route" is not json
    for address in ["a.b:25565", "x/../y?q=1#f", "ü ber"] {
      assert_eq!(rt::block_on(server_status(address)), None);
    }
    let paths: Vec<_> = server.calls().into_iter().map(|c| c.path).collect();
    assert_eq!(
      paths,
      [
        format!("{PATH}/a.b:25565"),
        format!("{PATH}/x%2F..%2Fy%3Fq=1%23f"),
        format!("{PATH}/%C3%BC%20ber"),
      ]
    );
  }

  #[test]
  fn server_status_is_none_when_nothing_listens() {
    // a port that was just freed, connection refused instead of a hang
    let port = std::net::TcpListener::bind("127.0.0.1:0")
      .unwrap()
      .local_addr()
      .unwrap()
      .port();
    // SAFETY: nextest runs every test in its own process
    unsafe { std::env::set_var(FAKE_ORIGIN, format!("http://127.0.0.1:{port}")) };
    assert_eq!(rt::block_on(server_status("a.b")), None);
  }

  #[test]
  fn parse_server_status_handles_online_offline_and_garbage() {
    assert_eq!(
      parse_server_status(r#"{"online":true,"players":{"online":5,"max":20,"list":[]}}"#),
      Some(ServerStatus {
        online: true,
        players_online: 5,
        players_max: 20
      })
    );
    assert_eq!(
      parse_server_status(r#"{"online":false,"host":"x"}"#),
      Some(ServerStatus {
        online: false,
        players_online: 0,
        players_max: 0
      })
    );
    assert_eq!(parse_server_status("not json"), None);
    assert_eq!(parse_server_status(r#"{"error":"bad"}"#), None);
  }
}
