use std::fmt::Display;

use anyhow::Result;
use proton_utils::http;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;
use url::Url;

const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ProfileInfo {
  pub id: String,
  pub name: String,
  pub skins: Vec<Skin>,
  pub capes: Vec<Cape>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
  pub id: String,
  pub state: State,
  pub url: Url,
  pub texture_key: String,
  pub variant: SkinVariant,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum State {
  Active,
  Inactive,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SkinVariant {
  Classic,
  Slim,
}

impl Display for SkinVariant {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SkinVariant::Classic => write!(f, "classic"),
      SkinVariant::Slim => write!(f, "slim"),
    }
  }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Cape {
  pub id: String,
  pub state: State,
  pub url: Url,
  pub alias: String,
}

pub async fn get_profile_info(client: &Client, mc_token: &str) -> Result<ProfileInfo> {
  debug!("Retrieving player profile");
  let res = client
    .get(&*http::url(MC_PROFILE_URL))
    .bearer_auth(mc_token)
    .send()
    .await?
    .error_for_status()?;
  debug!("Got response with code: {}", res.status());

  Ok(res.json().await?)
}

#[cfg(test)]
pub(crate) mod tests {
  use proton_utils::{rt, test_http::FakeServer};

  use super::*;

  /// A profile with an active skin and an inactive cape, as the profile endpoints answer
  pub(crate) fn profile_json(name: &str) -> String {
    format!(
      r#"{{"id":"uuid","name":"{name}","skins":[{{"id":"s1","state":"ACTIVE","url":"http://textures.minecraft.net/texture/s1","textureKey":"k1","variant":"SLIM"}}],"capes":[{{"id":"c1","state":"INACTIVE","url":"http://textures.minecraft.net/texture/c1","alias":"Migrator"}}],"profileActions":{{}}}}"#
    )
  }

  pub(crate) fn profile(name: &str) -> ProfileInfo {
    serde_json::from_str(&profile_json(name)).unwrap()
  }

  #[test]
  fn get_profile_info_sends_the_token_and_parses() {
    let server = FakeServer::start();
    server.route(
      "GET",
      &FakeServer::path(MC_PROFILE_URL),
      200,
      profile_json("Stévé"),
    );
    let info = rt::block_on(get_profile_info(&Client::new(), "tok")).unwrap();
    assert_eq!(info.name, "Stévé");
    assert_eq!(info.skins[0].state, State::Active);
    assert_eq!(info.skins[0].variant, SkinVariant::Slim);
    assert_eq!(info.capes[0].alias, "Migrator");
    assert_eq!(
      server.calls()[0].header("authorization"),
      Some("Bearer tok")
    );
  }

  #[test]
  fn get_profile_info_errors() {
    let cases = [
      (401, profile_json("a")),
      // no minecraft profile on the account
      (404, r#"{"error":"NOT_FOUND"}"#.into()),
      (500, String::new()),
      (200, "{".into()),
      (200, r#"{"id":"uuid","name":"a"}"#.into()),
      (200, profile_json("a").replace("ACTIVE", "active")),
    ];
    let server = FakeServer::start();
    for (status, body) in cases {
      server.route(
        "GET",
        &FakeServer::path(MC_PROFILE_URL),
        status,
        body.clone(),
      );
      assert!(
        rt::block_on(get_profile_info(&Client::new(), "t")).is_err(),
        "{status} {body}"
      );
    }
  }

  #[test]
  fn skin_variant_display_and_serde() {
    for (variant, text, json) in [
      (SkinVariant::Classic, "classic", "\"CLASSIC\""),
      (SkinVariant::Slim, "slim", "\"SLIM\""),
    ] {
      assert_eq!(variant.to_string(), text);
      assert_eq!(serde_json::to_string(&variant).unwrap(), json);
    }
    let info = profile("a");
    let back: ProfileInfo = serde_json::from_value(serde_json::to_value(&info).unwrap()).unwrap();
    assert_eq!(back, info);
  }
}
