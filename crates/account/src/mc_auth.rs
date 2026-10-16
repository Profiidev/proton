use std::{
  collections::HashMap,
  net::{Ipv4Addr, Ipv6Addr},
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
  io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::{TcpListener, TcpStream},
  time::timeout,
};
use tracing::{debug, warn};
use url::Url;

use proton_utils::CLIENT_ID;

const REDIRECT_URI: &str = "http://localhost:51701";
const REDIRECT_PORT: u16 = 51701;
const LOGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10 * 60);
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

const SCOPE: &str = "XboxLive.signin offline_access";
const PROMPT: &str = "select_account";

const SANDBOX_ID: &str = "RETAIL";
const TOKEN_TYPE: &str = "JWT";
const XBOX_SECURITY_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";
const XBOX_RELYING_PARTY: &str = "http://auth.xboxlive.com";
const XBOX_AUTH_METHOD: &str = "RPS";
const XBOX_SITE_NAME: &str = "user.auth.xboxlive.com";

const MC_TOKEN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const XBOX_SECURITY_TOKEN_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const XBOX_TOKEN_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const MS_AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";

pub struct Token {
  pub token: String,
  pub expires: DateTime<Utc>,
}

pub struct TokenUserHash {
  pub token: String,
  pub expires: DateTime<Utc>,
  pub user_hash: String,
}

pub struct MsToken {
  pub access_token: String,
  pub access_token_expires: DateTime<Utc>,
  pub refresh_token: String,
}

#[derive(Error, Debug)]
pub enum AuthError {
  #[error("Port {0} is already in use, close the program using it and try again")]
  PortInUse(u16),
  #[error("Login state mismatch, possible CSRF attempt")]
  StateMismatch,
  #[error("Microsoft login failed: {0}")]
  Denied(String),
  #[error("Login timed out")]
  Timeout,
  #[error("Invalid Response")]
  InvalidRes,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MCTokenReq {
  identity_token: String,
}

#[derive(Deserialize)]
struct MCTokenRes {
  access_token: String,
  expires_in: u32,
}

pub async fn get_minecraft_token(
  client: &Client,
  user_hash: &str,
  xbox_security_token: &str,
) -> Result<Token> {
  debug!("Sending mc token request");
  let res = client
    .post(MC_TOKEN_URL)
    .json(&MCTokenReq {
      identity_token: format!("XBL3.0 x={user_hash};{xbox_security_token}"),
    })
    .send()
    .await?
    .error_for_status()?;
  debug!("Got response with code: {}", res.status());

  let res: MCTokenRes = res.json().await?;

  Ok(Token {
    token: res.access_token,
    expires: Utc::now() + Duration::seconds(res.expires_in as i64),
  })
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XboxAuthReq {
  properties: XboxAuthProps,
  relying_party: String,
  token_type: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum XboxAuthProps {
  Security(XboxAuthPropsSecurity),
  Normal(XboxAuthPropsNormal),
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XboxAuthPropsSecurity {
  sandbox_id: String,
  user_tokens: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct XboxAuthPropsNormal {
  auth_method: String,
  site_name: String,
  rps_ticket: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxAuthRes {
  not_after: DateTime<Utc>,
  token: String,
  display_claims: HashMap<String, Vec<HashMap<String, String>>>,
}

pub async fn get_xbox_security_token(client: &Client, xbox_token: &str) -> Result<Token> {
  debug!("Sending xbox security token request");
  let res = client
    .post(XBOX_SECURITY_TOKEN_URL)
    .json(&XboxAuthReq {
      properties: XboxAuthProps::Security(XboxAuthPropsSecurity {
        sandbox_id: SANDBOX_ID.into(),
        user_tokens: vec![xbox_token.to_string()],
      }),
      relying_party: XBOX_SECURITY_RELYING_PARTY.into(),
      token_type: TOKEN_TYPE.into(),
    })
    .send()
    .await?
    .error_for_status()?;
  debug!("Got response with code: {}", res.status());

  let res: XboxAuthRes = res.json().await?;

  Ok(Token {
    token: res.token,
    expires: res.not_after,
  })
}

pub async fn get_xbox_token(client: &Client, ms_access_token: &str) -> Result<TokenUserHash> {
  debug!("Sending xbox token request");
  let res = client
    .post(XBOX_TOKEN_URL)
    .json(&XboxAuthReq {
      properties: XboxAuthProps::Normal(XboxAuthPropsNormal {
        auth_method: XBOX_AUTH_METHOD.into(),
        site_name: XBOX_SITE_NAME.into(),
        rps_ticket: format!("d={ms_access_token}"),
      }),
      relying_party: XBOX_RELYING_PARTY.into(),
      token_type: TOKEN_TYPE.into(),
    })
    .send()
    .await?
    .error_for_status()?;
  debug!("Got response with code: {}", res.status());

  let res: XboxAuthRes = res.json().await?;

  let user_hash = res
    .display_claims
    .get("xui")
    .and_then(|list| list.first().and_then(|map| map.get("uhs")))
    .ok_or(AuthError::InvalidRes)?
    .clone();

  Ok(TokenUserHash {
    token: res.token,
    expires: res.not_after,
    user_hash,
  })
}

#[derive(Deserialize)]
struct MSTokenRes {
  expires_in: u32,
  access_token: String,
  refresh_token: String,
}

pub async fn refresh_ms_token(client: &Client, ms_refresh_token: &str) -> Result<Option<MsToken>> {
  debug!("Sending ms token refresh request");
  let res = client
    .post(MS_TOKEN_URL)
    .form(&vec![
      ("client_id", CLIENT_ID),
      ("scope", SCOPE),
      ("refresh_token", ms_refresh_token),
      ("grant_type", "refresh_token"),
    ])
    .send()
    .await?;

  debug!("Got response with code: {}", res.status());
  // expired or revoked refresh token (invalid_grant), the user has to log in again
  if res.status() == StatusCode::BAD_REQUEST || res.status() == StatusCode::UNAUTHORIZED {
    return Ok(None);
  }

  let res: MSTokenRes = res.error_for_status()?.json().await?;

  Ok(Some(MsToken {
    access_token: res.access_token,
    access_token_expires: Utc::now() + Duration::seconds(res.expires_in as i64),
    refresh_token: res.refresh_token,
  }))
}

/// Browser login: binds the redirect port, opens the authorize url and waits for the redirect.
/// Dropping (aborting) the future closes the listener and frees the port.
pub async fn get_ms_token(client: &Client) -> Result<MsToken> {
  let state = uuid::Uuid::new_v4().to_string();
  // bind before opening the browser, so the redirect cannot arrive before we listen
  let listener = RedirectListener::bind(REDIRECT_PORT).await?;

  let url = authorize_url(&state)?;
  debug!("Opening login in the browser: {url}");
  open::that_detached(url.as_str())?;

  let code = timeout(LOGIN_TIMEOUT, wait_for_code(&listener, &state))
    .await
    .map_err(|_| AuthError::Timeout)??;
  drop(listener);

  debug!("Retrieving ms token with code");
  let res = client
    .post(MS_TOKEN_URL)
    .form(&vec![
      ("client_id", CLIENT_ID),
      ("code", &code),
      ("redirect_uri", REDIRECT_URI),
      ("grant_type", "authorization_code"),
    ])
    .send()
    .await?
    .error_for_status()?;
  debug!("Got response with code: {}", res.status());

  let res: MSTokenRes = res.json().await?;

  Ok(MsToken {
    access_token: res.access_token,
    access_token_expires: Utc::now() + Duration::seconds(res.expires_in as i64),
    refresh_token: res.refresh_token,
  })
}

pub fn authorize_url(state: &str) -> Result<Url> {
  Ok(Url::parse_with_params(
    MS_AUTHORIZE_URL,
    [
      ("client_id", CLIENT_ID),
      ("response_type", "code"),
      ("scope", SCOPE),
      ("redirect_uri", REDIRECT_URI),
      ("prompt", PROMPT),
      ("state", state),
    ],
  )?)
}

/// Listens on 127.0.0.1 and, when available, [::1], since browsers may resolve `localhost` to
/// either
struct RedirectListener {
  v4: TcpListener,
  v6: Option<TcpListener>,
}

impl RedirectListener {
  /// Port 0 picks a free port for v4 and reuses it for v6
  async fn bind(port: u16) -> Result<Self> {
    let v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
      .await
      .map_err(|e| -> anyhow::Error {
        if e.kind() == std::io::ErrorKind::AddrInUse {
          AuthError::PortInUse(port).into()
        } else {
          e.into()
        }
      })?;
    let port = v4.local_addr()?.port();
    let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port))
      .await
      .inspect_err(|e| debug!("No IPv6 login listener: {e}"))
      .ok();
    Ok(Self { v4, v6 })
  }

  async fn accept(&self) -> std::io::Result<TcpStream> {
    let (stream, _) = match &self.v6 {
      Some(v6) => tokio::select! {
        r = self.v4.accept() => r,
        r = v6.accept() => r,
      },
      None => self.v4.accept().await,
    }?;
    Ok(stream)
  }
}

const PAGE_OK: &str = "<!doctype html><html><body style=\"font-family:sans-serif\"><h2>Login successful</h2><p>You can close this tab and return to Proton.</p></body></html>";
const PAGE_ERR: &str = "<!doctype html><html><body style=\"font-family:sans-serif\"><h2>Login failed</h2><p>You can close this tab and return to Proton.</p></body></html>";

/// Accepts redirect connections until one carries a code (or an error) for our state
async fn wait_for_code(listener: &RedirectListener, state: &str) -> Result<String> {
  loop {
    let mut stream = listener.accept().await?;
    let mut line = String::new();
    let read = timeout(
      READ_TIMEOUT,
      BufReader::new(&mut stream).read_line(&mut line),
    )
    .await;
    if !matches!(read, Ok(Ok(_))) {
      continue;
    }

    let result = parse_redirect(&line, state);
    let (status, body) = match &result {
      Ok(None) => ("404 Not Found", ""),
      Ok(Some(_)) => ("200 OK", PAGE_OK),
      Err(_) => ("400 Bad Request", PAGE_ERR),
    };
    let res = format!(
      "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
      body.len()
    );
    if let Err(e) = stream.write_all(res.as_bytes()).await {
      warn!("Failed to answer the login redirect: {e}");
    }
    let _ = stream.shutdown().await;

    if let Some(code) = result? {
      return Ok(code);
    }
  }
}

/// Parses the request line of a redirect. `Ok(None)` for unrelated requests (favicon, ...).
fn parse_redirect(request_line: &str, state: &str) -> Result<Option<String>> {
  let Some(target) = request_line
    .strip_prefix("GET ")
    .and_then(|r| r.split_whitespace().next())
  else {
    return Ok(None);
  };
  let Ok(url) = Url::parse(&format!("http://localhost{target}")) else {
    return Ok(None);
  };
  if url.path() != "/" {
    return Ok(None);
  }
  let query: HashMap<_, _> = url.query_pairs().collect();
  let (code, error) = (query.get("code"), query.get("error"));
  if code.is_none() && error.is_none() {
    return Ok(None);
  }
  if query.get("state").map(|s| s.as_ref()) != Some(state) {
    return Err(AuthError::StateMismatch.into());
  }
  if let Some(error) = error {
    let desc = query.get("error_description").unwrap_or(error);
    return Err(AuthError::Denied(desc.to_string()).into());
  }
  Ok(code.map(|c| c.to_string()))
}

#[cfg(test)]
mod tests {
  use tokio::{io::AsyncReadExt, net::TcpStream};

  use super::*;

  async fn get(addr: std::net::SocketAddr, target: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
      .write_all(format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
      .await
      .unwrap();
    let mut res = String::new();
    stream.read_to_string(&mut res).await.unwrap();
    res
  }

  async fn server(
    state: &str,
  ) -> (
    std::net::SocketAddr,
    Option<std::net::SocketAddr>,
    tokio::task::JoinHandle<Result<String>>,
  ) {
    let listener = RedirectListener::bind(0).await.unwrap();
    let addr = listener.v4.local_addr().unwrap();
    let v6 = listener.v6.as_ref().map(|l| l.local_addr().unwrap());
    let state = state.to_string();
    let task = tokio::spawn(async move { wait_for_code(&listener, &state).await });
    (addr, v6, task)
  }

  #[tokio::test]
  async fn accepts_on_ipv6() {
    let (addr, v6, task) = server("s1").await;
    let Some(v6) = v6 else {
      eprintln!("IPv6 loopback unavailable, skipping");
      return;
    };
    assert_eq!(v6.port(), addr.port());
    assert!(
      get(v6, "/?code=six&state=s1")
        .await
        .starts_with("HTTP/1.1 200")
    );
    assert_eq!(task.await.unwrap().unwrap(), "six");
  }

  #[tokio::test]
  async fn extracts_code_and_ignores_unrelated() {
    let (addr, _, task) = server("s1").await;
    assert!(get(addr, "/favicon.ico").await.starts_with("HTTP/1.1 404"));
    assert!(get(addr, "/").await.starts_with("HTTP/1.1 404"));
    let res = get(addr, "/?code=abc%2Fd&state=s1").await;
    assert!(res.starts_with("HTTP/1.1 200") && res.contains("Login successful"));
    assert_eq!(task.await.unwrap().unwrap(), "abc/d");
  }

  #[tokio::test]
  async fn rejects_state_mismatch() {
    let (addr, _, task) = server("s1").await;
    assert!(
      get(addr, "/?code=abc&state=evil")
        .await
        .contains("Login failed")
    );
    let err = task.await.unwrap().unwrap_err();
    assert!(matches!(err.downcast_ref(), Some(AuthError::StateMismatch)));
  }

  #[tokio::test]
  async fn reports_error_param() {
    let (addr, _, task) = server("s1").await;
    get(
      addr,
      "/?error=access_denied&error_description=User+cancelled&state=s1",
    )
    .await;
    let err = task.await.unwrap().unwrap_err();
    assert_eq!(err.to_string(), "Microsoft login failed: User cancelled");
  }

  #[tokio::test]
  async fn cancel_frees_the_port_and_busy_port_is_clear() {
    let (addr, _, task) = server("s1").await;
    let busy = RedirectListener::bind(addr.port()).await.err().unwrap();
    assert!(matches!(busy.downcast_ref(), Some(AuthError::PortInUse(p)) if *p == addr.port()));

    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    RedirectListener::bind(addr.port()).await.unwrap();
  }

  #[test]
  fn authorize_url_has_all_params() {
    let url = authorize_url("xyz").unwrap();
    let q: HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(q["state"], "xyz");
    assert_eq!(q["redirect_uri"], REDIRECT_URI);
    assert_eq!(q["scope"], SCOPE);
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["prompt"], "select_account");
    assert_eq!(q["client_id"], CLIENT_ID);
  }
}
