use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Url, UserAttentionType, WebviewWindowBuilder};
use thiserror::Error;
use tokio::time::sleep;

const CLIENT_ID: &str = "dd35660a-6381-41f8-bb34-2a36669581d0";
const REDIRECT_URI: &str = "https://proton.profidev.io/backend";
const SCOPE: &str = "XboxLive.signin offline_access";
const AUTH_WINDOW_LABEL: &str = "ms_auth";
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
enum AuthError {
  #[error("Other Error")]
  Other,
  #[error("Timeout")]
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
  let res: MCTokenRes = client
    .post(MC_TOKEN_URL)
    .json(&MCTokenReq {
      identity_token: format!("XBL3.0 x={};{}", user_hash, xbox_security_token),
    })
    .send()
    .await?
    .json()
    .await?;

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
  let res: XboxAuthRes = client
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
    .json()
    .await?;

  Ok(Token {
    token: res.token,
    expires: res.not_after,
  })
}

pub async fn get_xbox_token(client: &Client, ms_access_token: &str) -> Result<TokenUserHash> {
  let res: XboxAuthRes = client
    .post(XBOX_TOKEN_URL)
    .json(&XboxAuthReq {
      properties: XboxAuthProps::Normal(XboxAuthPropsNormal {
        auth_method: XBOX_AUTH_METHOD.into(),
        site_name: XBOX_SITE_NAME.into(),
        rps_ticket: format!("d={}", ms_access_token),
      }),
      relying_party: XBOX_RELYING_PARTY.into(),
      token_type: TOKEN_TYPE.into(),
    })
    .send()
    .await?
    .json()
    .await?;

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

  if res.status() != StatusCode::OK {
    return Ok(None);
  }

  let res: MSTokenRes = res.json().await?;

  Ok(Some(MsToken {
    access_token: res.access_token,
    access_token_expires: Utc::now() + Duration::seconds(res.expires_in as i64),
    refresh_token: res.refresh_token,
  }))
}

pub async fn get_ms_token(client: &Client, handle: &AppHandle) -> Result<MsToken> {
  let start = Utc::now();

  if let Some(window) = handle.get_webview_window(AUTH_WINDOW_LABEL) {
    window.close()?;
  }

  let window = WebviewWindowBuilder::new(
    handle,
    AUTH_WINDOW_LABEL,
    tauri::WebviewUrl::External(Url::parse_with_params(
      MS_AUTHORIZE_URL,
      vec![
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("redirect_uri", REDIRECT_URI),
        ("prompt", PROMPT),
      ],
    )?),
  )
  .zoom_hotkeys_enabled(false)
  .title("Sign into Proton")
  .always_on_top(true)
  .center()
  .build()?;

  window.request_user_attention(Some(UserAttentionType::Critical))?;

  while (Utc::now() - start) < Duration::minutes(10) {
    window.title().map_err(|_| AuthError::Other)?;

    if window.url()?.as_str().starts_with(REDIRECT_URI) {
      let url = window.url()?;

      let code = url.query_pairs().find(|(key, _)| key == "code");

      window.close()?;

      let code = code.ok_or(AuthError::Other)?.1.to_string();

      let res: MSTokenRes = client
        .post(MS_TOKEN_URL)
        .form(&vec![
          ("client_id", CLIENT_ID),
          ("code", &code),
          ("redirect_uri", REDIRECT_URI),
          ("grant_type", "authorization_code"),
        ])
        .send()
        .await?
        .json()
        .await?;

      return Ok(MsToken {
        access_token: res.access_token,
        access_token_expires: Utc::now() + Duration::seconds(res.expires_in as i64),
        refresh_token: res.refresh_token,
      });
    }

    sleep(std::time::Duration::from_millis(50)).await;
  }

  window.close()?;

  Err(AuthError::Timeout.into())
}
