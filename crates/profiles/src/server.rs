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
  let mut url = Url::parse(STATUS_BASE_URL).ok()?;
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
  use super::*;

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
