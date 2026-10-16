use anyhow::Result;
use chrono::{DateTime, Utc};
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::mc_auth::{
  get_minecraft_token, get_ms_token, get_xbox_security_token, get_xbox_token, refresh_ms_token,
};

const ACCOUNT_LOGIN_STATUS_EVENT: &str = "account-login-status";

#[derive(Serialize, Clone)]
enum LoginStatus {
  Ms,
  Xbox,
  XboxSecurity,
  Mc,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct AuthInfo {
  pub mc_token: String,
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

pub async fn ms_mc_login(client: &Client, handle: &AppHandle) -> Result<AuthInfo> {
  debug!("Trying to get ms token");
  let ms_token = get_ms_token(client, handle).await?;
  handle.emit(ACCOUNT_LOGIN_STATUS_EVENT, LoginStatus::Ms)?;

  debug!("Retrieving xbox token");
  let xbox_token = get_xbox_token(client, &ms_token.access_token).await?;
  handle.emit(ACCOUNT_LOGIN_STATUS_EVENT, LoginStatus::Xbox)?;

  debug!("Retrieving xbox security token");
  let xbox_security_token = get_xbox_security_token(client, &xbox_token.token).await?;
  handle.emit(ACCOUNT_LOGIN_STATUS_EVENT, LoginStatus::XboxSecurity)?;

  debug!("Retrieving mc token");
  let mc_token =
    get_minecraft_token(client, &xbox_token.user_hash, &xbox_security_token.token).await?;
  handle.emit(ACCOUNT_LOGIN_STATUS_EVENT, LoginStatus::Mc)?;

  debug!("Auth done");
  Ok(AuthInfo {
    mc_token: mc_token.token,
    mc_token_expire: mc_token.expires,
    user_hash: xbox_token.user_hash,
    xbox_security_token: xbox_security_token.token,
    xbox_security_token_expire: xbox_security_token.expires,
    xbox_token: xbox_token.token,
    xbox_token_expire: xbox_token.expires,
    ms_access_token: ms_token.access_token,
    ms_access_token_expire: ms_token.access_token_expires,
    ms_refresh_token: ms_token.refresh_token,
  })
}

pub async fn refresh_mc_token(client: &Client, mut info: AuthInfo) -> Result<Option<AuthInfo>> {
  debug!("Checking if mc token is valid");
  if Utc::now() < info.mc_token_expire {
    debug!("MC token valid. returning");
    return Ok(Some(info));
  }

  'mc_token: {
    debug!("Checking if xbox security token is valid");
    if Utc::now() < info.xbox_security_token_expire {
      debug!("Xbox security token valid. returning");
      break 'mc_token;
    }

    'xbox_security_token: {
      debug!("Checking if xbox token is valid");
      if Utc::now() < info.xbox_token_expire {
        debug!("Xbox token valid. returning");
        break 'xbox_security_token;
      }

      'xbox_token: {
        debug!("Checking if ms token is valid");
        if Utc::now() < info.ms_access_token_expire {
          debug!("MS token valid. returning");
          break 'xbox_token;
        }

        debug!("Trying to refresh ms token");
        let Some(res) = refresh_ms_token(client, &info.ms_refresh_token).await? else {
          debug!("Tokens expired new login needed");
          return Ok(None);
        };

        info.ms_access_token = res.access_token;
        info.ms_access_token_expire = res.access_token_expires;
        info.ms_refresh_token = res.refresh_token;
      };

      debug!("Refreshing xbox token");
      let res = get_xbox_token(client, &info.ms_access_token).await?;

      info.xbox_token = res.token;
      info.xbox_token_expire = res.expires;
      info.user_hash = res.user_hash;
    };

    debug!("Refreshing xbox security token");
    let res = get_xbox_security_token(client, &info.xbox_token).await?;
    info.xbox_security_token = res.token;
    info.xbox_security_token_expire = res.expires;
  };

  debug!("Refreshing mc token");
  let res = get_minecraft_token(client, &info.user_hash, &info.xbox_security_token).await?;
  info.mc_token = res.token;
  info.mc_token_expire = res.expires;

  debug!("Refresh complete");
  Ok(Some(info))
}
