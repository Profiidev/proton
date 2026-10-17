use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::mc_auth::{
  get_minecraft_token, get_ms_token, get_xbox_security_token, get_xbox_token, refresh_ms_token,
};

/// Token step that just finished during a login
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoginStatus {
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

pub async fn ms_mc_login(client: &Client, on_step: impl Fn(LoginStatus)) -> Result<AuthInfo> {
  debug!("Trying to get ms token");
  let ms_token = get_ms_token(client).await?;
  on_step(LoginStatus::Ms);

  debug!("Retrieving xbox token");
  let xbox_token = get_xbox_token(client, &ms_token.access_token).await?;
  on_step(LoginStatus::Xbox);

  debug!("Retrieving xbox security token");
  let xbox_security_token = get_xbox_security_token(client, &xbox_token.token).await?;
  on_step(LoginStatus::XboxSecurity);

  debug!("Retrieving mc token");
  let mc_token =
    get_minecraft_token(client, &xbox_token.user_hash, &xbox_security_token.token).await?;
  on_step(LoginStatus::Mc);

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

#[cfg(test)]
pub(crate) mod tests {
  use chrono::Duration;
  use proton_utils::{rt, test_http::FakeServer};

  use super::*;
  use crate::mc_auth::tests::{hit, token_routes};

  const MS: &str = "POST login.microsoftonline.com/consumers/oauth2/v2.0/token";
  const XBOX: &str = "POST user.auth.xboxlive.com/user/authenticate";
  const XSTS: &str = "POST xsts.auth.xboxlive.com/xsts/authorize";
  const MC: &str = "POST api.minecraftservices.com/authentication/login_with_xbox";

  /// Auth with the tokens up to `valid` (mc, xsts, xbox, ms) still valid, the rest expired
  pub(crate) fn auth_info(valid: [bool; 4]) -> AuthInfo {
    let at = |ok: bool| Utc::now() + Duration::hours(if ok { 1 } else { -1 });
    AuthInfo {
      mc_token: "old-mc".into(),
      mc_token_expire: at(valid[0]),
      user_hash: "old-hash".into(),
      xbox_security_token: "old-xsts".into(),
      xbox_security_token_expire: at(valid[1]),
      xbox_token: "old-xbl".into(),
      xbox_token_expire: at(valid[2]),
      ms_access_token: "old-ms".into(),
      ms_access_token_expire: at(valid[3]),
      ms_refresh_token: "old-refresh".into(),
    }
  }

  #[test]
  fn refresh_only_renews_expired_tokens() {
    // the first still valid token in mc -> xsts -> xbox -> ms order decides where to restart
    let cases: [([bool; 4], &[&str]); 6] = [
      ([true, true, true, true], &[]),
      ([true, false, false, false], &[]),
      ([false, true, false, false], &[MC]),
      ([false, false, true, false], &[XSTS, MC]),
      ([false, false, false, true], &[XBOX, XSTS, MC]),
      ([false, false, false, false], &[MS, XBOX, XSTS, MC]),
    ];
    for (valid, want) in cases {
      let server = FakeServer::start();
      token_routes(&server);
      let info = rt::block_on(refresh_mc_token(&Client::new(), auth_info(valid)))
        .unwrap()
        .unwrap();
      assert_eq!(hit(&server.calls()), want, "{valid:?}");
      let fresh = if want.is_empty() { "old-mc" } else { "mc" };
      assert_eq!(info.mc_token, fresh, "{valid:?}");
      assert!(info.mc_token_expire > Utc::now());
    }
  }

  #[test]
  fn full_refresh_chains_the_new_tokens() {
    let server = FakeServer::start();
    token_routes(&server);
    let info = rt::block_on(refresh_mc_token(&Client::new(), auth_info([false; 4])))
      .unwrap()
      .unwrap();
    let calls = server.calls();
    assert!(calls[0].body_str().contains("refresh_token=old-refresh"));
    // each step gets the token the previous one just returned
    assert!(calls[1].body_str().contains(r#""RpsTicket":"d=ms2""#));
    assert!(calls[2].body_str().contains(r#""UserTokens":["xbl"]"#));
    assert!(calls[3].body_str().contains("XBL3.0 x=hash;xsts"));
    assert_eq!(
      (
        info.ms_access_token.as_str(),
        info.ms_refresh_token.as_str(),
        info.xbox_token.as_str(),
        info.user_hash.as_str(),
        info.xbox_security_token.as_str(),
      ),
      ("ms2", "r2", "xbl", "hash", "xsts")
    );
  }

  #[test]
  fn partial_refresh_reuses_the_stored_tokens() {
    let server = FakeServer::start();
    token_routes(&server);
    rt::block_on(refresh_mc_token(
      &Client::new(),
      auth_info([false, true, false, false]),
    ))
    .unwrap();
    assert!(
      server.calls()[0]
        .body_str()
        .contains("XBL3.0 x=old-hash;old-xsts")
    );
  }

  #[test]
  fn dead_refresh_token_needs_a_new_login() {
    let server = FakeServer::start();
    token_routes(&server);
    server.route(
      "POST",
      "/login.microsoftonline.com/consumers/oauth2/v2.0/token",
      400,
      r#"{"error":"invalid_grant"}"#,
    );
    let res = rt::block_on(refresh_mc_token(&Client::new(), auth_info([false; 4]))).unwrap();
    assert!(res.is_none());
    assert_eq!(hit(&server.calls()), [MS]);
  }

  #[test]
  fn a_failing_step_stops_the_chain() {
    let steps = [
      ("/user.auth.xboxlive.com/user/authenticate", &[MS, XBOX][..]),
      (
        "/xsts.auth.xboxlive.com/xsts/authorize",
        &[MS, XBOX, XSTS][..],
      ),
      (
        "/api.minecraftservices.com/authentication/login_with_xbox",
        &[MS, XBOX, XSTS, MC][..],
      ),
    ];
    for (path, want) in steps {
      for status in [401, 500] {
        let server = FakeServer::start();
        token_routes(&server);
        server.route("POST", path, status, "");
        let res = rt::block_on(refresh_mc_token(&Client::new(), auth_info([false; 4])));
        assert!(res.is_err(), "{path} {status}");
        assert_eq!(hit(&server.calls()), want, "{path} {status}");
      }
    }
  }

  #[test]
  fn auth_info_round_trips_through_json() {
    let info = auth_info([true, false, true, false]);
    let back: AuthInfo = serde_json::from_value(serde_json::to_value(&info).unwrap()).unwrap();
    assert_eq!(back.mc_token_expire, info.mc_token_expire);
    assert_eq!(back.ms_refresh_token, "old-refresh");
  }
}
