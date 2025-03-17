use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Url, UserAttentionType, WebviewWindowBuilder};
use thiserror::Error;
use tokio::time::sleep;

use crate::store::TauriAppStoreExt;

const CLIENT_ID: &str = "dd35660a-6381-41f8-bb34-2a36669581d0";
const REDIRECT_URI: &str = "https://proton.profidev.io/backend";
const SCOPE: &str = "XboxLive.signin offline_access";

const AUTH_KEY: &str = "mc_auth_info";
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

#[derive(Serialize, Deserialize, Default, Debug)]
struct AuthInfo {
  mc_token: String,
  mc_token_expire: DateTime<Utc>,
  user_hash: String,
  xbox_security_token: String,
  xbox_security_token_expire: DateTime<Utc>,
  xbox_token: String,
  xbox_token_expire: DateTime<Utc>,
  ms_access_token: String,
  ms_access_token_expire: DateTime<Utc>,
  ms_refresh_token: String,
}

#[derive(Error, Debug)]
enum AuthError {
  #[error("Other Error")]
  Other,
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

pub async fn get_minecraft_token(handle: AppHandle, client: &Client) -> Result<String> {
  let store = handle.app_store()?;
  let mut info: AuthInfo = store.get_or_default(AUTH_KEY)?;

  if Utc::now() < info.mc_token_expire {
    return Ok(info.mc_token);
  }

  check_xbox_security_token(client, &mut info, handle).await?;

  let mc_res: MCTokenRes = client
    .post(MC_TOKEN_URL)
    .json(&MCTokenReq {
      identity_token: format!("XBL3.0 x={};{}", &info.user_hash, &info.xbox_security_token),
    })
    .send()
    .await?
    .json()
    .await?;

  info.mc_token = mc_res.access_token;
  info.mc_token_expire = Utc::now() + Duration::seconds(mc_res.expires_in as i64);

  store.set(AUTH_KEY, &info)?;

  store.store.save()?;

  Ok(info.mc_token)
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

async fn check_xbox_security_token(
  client: &Client,
  info: &mut AuthInfo,
  handle: AppHandle,
) -> Result<()> {
  if Utc::now() < info.xbox_security_token_expire {
    return Ok(());
  }

  check_xbox_token(client, info, handle).await?;

  let res: XboxAuthRes = client
    .post(XBOX_SECURITY_TOKEN_URL)
    .json(&XboxAuthReq {
      properties: XboxAuthProps::Security(XboxAuthPropsSecurity {
        sandbox_id: SANDBOX_ID.into(),
        user_tokens: vec![info.xbox_token.clone()],
      }),
      relying_party: XBOX_SECURITY_RELYING_PARTY.into(),
      token_type: TOKEN_TYPE.into(),
    })
    .send()
    .await?
    .json()
    .await?;

  info.xbox_security_token = res.token;
  info.xbox_security_token_expire = res.not_after;

  Ok(())
}

async fn check_xbox_token(client: &Client, info: &mut AuthInfo, handle: AppHandle) -> Result<()> {
  if Utc::now() < info.xbox_token_expire {
    return Ok(());
  }

  check_ms_token(client, info, handle).await?;

  let res: XboxAuthRes = client
    .post(XBOX_TOKEN_URL)
    .json(&XboxAuthReq {
      properties: XboxAuthProps::Normal(XboxAuthPropsNormal {
        auth_method: XBOX_AUTH_METHOD.into(),
        site_name: XBOX_SITE_NAME.into(),
        rps_ticket: format!("d={}", info.ms_access_token),
      }),
      relying_party: XBOX_RELYING_PARTY.into(),
      token_type: TOKEN_TYPE.into(),
    })
    .send()
    .await?
    .json()
    .await?;

  info.xbox_token = res.token;
  info.xbox_token_expire = res.not_after;
  info.user_hash = res
    .display_claims
    .get("xui")
    .and_then(|list| list.first().and_then(|map| map.get("uhs")))
    .ok_or(AuthError::Other)?
    .clone();

  Ok(())
}

#[derive(Deserialize)]
struct MSTokenRes {
  expires_in: u32,
  access_token: String,
  refresh_token: String,
}

async fn check_ms_token(client: &Client, info: &mut AuthInfo, handle: AppHandle) -> Result<()> {
  if Utc::now() < info.ms_access_token_expire {
    return Ok(());
  }

  let res = client
    .post(MS_TOKEN_URL)
    .form(&vec![
      ("client_id", CLIENT_ID),
      ("scope", SCOPE),
      ("refresh_token", &info.ms_refresh_token),
      ("grant_type", "refresh_token"),
    ])
    .send()
    .await?;

  if res.status() != StatusCode::OK {
    ms_auth(client, info, handle).await?;
    return Ok(());
  }

  let res: MSTokenRes = res.json().await?;

  info.ms_access_token = res.access_token;
  info.ms_access_token_expire = Utc::now() + Duration::seconds(res.expires_in as i64);
  info.ms_refresh_token = res.refresh_token;

  Ok(())
}

async fn ms_auth(client: &Client, info: &mut AuthInfo, handle: AppHandle) -> Result<()> {
  let start = Utc::now();

  let window = WebviewWindowBuilder::new(
    &handle,
    "auth",
    tauri::WebviewUrl::External(Url::parse_with_params(
      MS_AUTHORIZE_URL,
      vec![
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("redirect_uri", REDIRECT_URI),
      ],
    )?),
  )
  .min_inner_size(420.0, 632.0)
  .inner_size(420.0, 632.0)
  .max_inner_size(420.0, 632.0)
  .zoom_hotkeys_enabled(false)
  .title("Sign into Proton")
  .always_on_top(true)
  .center()
  .build()?;

  window.request_user_attention(Some(UserAttentionType::Critical))?;

  while (Utc::now() - start) < Duration::minutes(10) {
    window.title().map_err(|_| AuthError::Other)?;

    if window
      .url()?
      .as_str()
      .starts_with("https://proton.profidev.io/backend")
    {
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

      info.ms_access_token = res.access_token;
      info.ms_access_token_expire = Utc::now() + Duration::seconds(res.expires_in as i64);
      info.ms_refresh_token = res.refresh_token;

      return Ok(());
    }

    sleep(std::time::Duration::from_millis(50)).await;
  }

  window.close()?;

  Ok(())
}
