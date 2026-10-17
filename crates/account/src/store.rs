use std::collections::HashMap;

use anyhow::{Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use proton_utils::{download::default_client, http, log::ErrorLogExt};

use super::{
  auth::{AuthInfo, refresh_mc_token},
  info::{ProfileInfo, get_profile_info},
};

const CAPE_CHANGE_URL: &str = "https://api.minecraftservices.com/minecraft/profile/capes/active";
pub const REMOVE_CAPE: &str = "REMOVE_CAPE";

pub struct AccountStore {
  accounts: HashMap<String, Option<AccountInfo>>,
  active: String,
  client: Client,
}

#[derive(Serialize, Deserialize, Clone)]
struct AccountInfo {
  auth: AuthInfo,
  profile: ProfileInfo,
}

pub struct LaunchInfo {
  pub id: String,
  pub name: String,
  pub access_token: String,
}

impl AccountStore {
  const ACCOUNT_KEY: &str = "accounts";
  const ACTIVE_ACCOUNT_KEY: &str = "active_account";

  pub fn new() -> Result<AccountStore> {
    let store = proton_store::store();
    let accounts: HashMap<String, Option<AccountInfo>> = store.get_or_default(Self::ACCOUNT_KEY)?;
    let active: String = store.get_or_default(Self::ACTIVE_ACCOUNT_KEY)?;

    Ok(AccountStore {
      accounts,
      active,
      client: default_client(),
    })
  }

  pub fn list_profiles(&self) -> HashMap<String, Option<ProfileInfo>> {
    self
      .accounts
      .iter()
      .map(|(id, info)| (id.clone(), info.as_ref().map(|info| info.profile.clone())))
      .collect()
  }

  async fn refresh_token(&mut self, id: &str) -> Result<()> {
    debug!("Refreshing mc token for {id}");
    if let Some(Some(account)) = self.accounts.get_mut(id) {
      if let Some(auth) = refresh_mc_token(&self.client, account.auth.clone()).await? {
        account.auth = auth;
      } else {
        self.accounts.insert(id.to_string(), None);
      };
    }

    Ok(())
  }

  async fn refresh_profile(&mut self, id: &str) -> Result<()> {
    debug!("Refreshing mc profile for {id}");
    if let Some(Some(account)) = self.accounts.get_mut(id) {
      let profile = get_profile_info(&self.client, &account.auth.mc_token).await?;
      account.profile = profile;
    }

    Ok(())
  }

  fn save(&self) -> Result<()> {
    let store = proton_store::store();
    store.set(Self::ACCOUNT_KEY, &self.accounts)?;
    store.set(Self::ACTIVE_ACCOUNT_KEY, &self.active)
  }

  /// Applies `change` and saves, memory is restored when the save fails so the UI never shows
  /// what was not written
  fn change(&mut self, change: impl FnOnce(&mut Self)) -> Result<()> {
    let (accounts, active) = (self.accounts.clone(), self.active.clone());
    change(self);
    let res = self.save();
    if res.is_err() {
      self.accounts = accounts;
      self.active = active;
    }
    res
  }

  pub async fn refresh_all(&mut self) -> Result<()> {
    let keys: Vec<String> = self.accounts.keys().cloned().collect();

    for id in &keys {
      //ignore result to prevent inconsistent saved data
      let _ = self.refresh_token(id).await.log_err();
      let _ = self.refresh_profile(id).await.log_err();
    }

    self.save()?;
    Ok(())
  }

  pub fn active(&self) -> &str {
    &self.active
  }

  pub fn active_id(&self) -> Option<String> {
    Some(self.active.clone()).filter(|a| !a.is_empty())
  }

  pub fn mc_token(&self, id: &str) -> Option<&String> {
    self
      .accounts
      .get(id)
      .and_then(|a| a.as_ref().map(|a| &a.auth.mc_token))
  }

  pub fn launch_info(&self, id: &str) -> Option<LaunchInfo> {
    self.accounts.get(id).and_then(|a| {
      a.as_ref().map(|a| LaunchInfo {
        id: a.profile.id.clone(),
        name: a.profile.name.clone(),
        access_token: a.auth.mc_token.clone(),
      })
    })
  }

  pub fn set_active(&mut self, id: String) -> Result<()> {
    debug!("Changing active account to {id}");
    self.change(|s| s.active = id)
  }

  pub fn remove_account(&mut self, id: &str) -> Result<()> {
    debug!("Removing account {id}");
    self.change(|s| {
      s.accounts.remove(id);
    })
  }

  /// Adds a freshly logged in account, it becomes active when none was
  pub fn add_account(&mut self, auth: AuthInfo, profile: ProfileInfo) -> Result<()> {
    debug!("Adding account {}", profile.id);
    self.change(|s| {
      if s.active.is_empty() {
        s.active = profile.id.clone();
      }
      s.accounts
        .insert(profile.id.clone(), Some(AccountInfo { auth, profile }));
    })
  }

  pub async fn refresh_auth(&mut self, id: &str) -> Result<()> {
    self.refresh_token(id).await?;
    self.save()
  }

  pub fn update_profile(&mut self, profile: ProfileInfo) -> Result<()> {
    if let Some(Some(account)) = self.accounts.get_mut(&profile.id) {
      account.profile = profile;
    }
    self.save()?;
    Ok(())
  }

  pub async fn select_cape_by_id(&mut self, account: &str, id: &str) -> Result<()> {
    debug!("Selecting cape {id} for account {account}");
    self.refresh_auth(account).await?;

    if let Some(Some(account)) = self.accounts.get_mut(account) {
      let req = if id == REMOVE_CAPE {
        self.client.delete(&*http::url(CAPE_CHANGE_URL))
      } else {
        self.client.put(&*http::url(CAPE_CHANGE_URL))
      };

      let res = req
        .bearer_auth(&account.auth.mc_token)
        .json(&CapeChangeReq {
          cape_id: id.to_string(),
        })
        .send()
        .await?
        .error_for_status()?;
      debug!("Got response with code: {}", res.status());

      let profile: ProfileInfo = res.json().await?;

      account.profile = profile;
      self.save()?;
      return Ok(());
    }

    bail!("Cape change error: unknown account {account}")
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapeChangeReq {
  cape_id: String,
}

#[cfg(test)]
mod tests {
  use proton_utils::{
    rt,
    test_http::{FakeServer, Response},
  };

  use super::*;
  use crate::{
    auth::tests::auth_info,
    info::tests::{profile, profile_json},
    mc_auth::tests::{hit, token_routes},
    tests::init_dirs,
  };

  const PROFILE: &str = "GET api.minecraftservices.com/minecraft/profile";
  const CAPE: &str = "api.minecraftservices.com/minecraft/profile/capes/active";

  fn with_id(id: &str, name: &str) -> ProfileInfo {
    ProfileInfo {
      id: id.into(),
      ..profile(name)
    }
  }

  #[test]
  fn empty_store_has_no_accounts() {
    init_dirs();
    let store = AccountStore::new().unwrap();
    assert!(store.list_profiles().is_empty());
    assert_eq!((store.active(), store.active_id()), ("", None));
    assert!(store.mc_token("x").is_none() && store.launch_info("x").is_none());
  }

  #[test]
  fn first_added_account_becomes_active_and_persists() {
    let data = init_dirs();
    let mut store = AccountStore::new().unwrap();
    store
      .add_account(auth_info([true; 4]), with_id("a", "A"))
      .unwrap();
    store
      .add_account(auth_info([true; 4]), with_id("b", "B"))
      .unwrap();
    assert_eq!(store.active_id().as_deref(), Some("a"));

    let launch = store.launch_info("b").unwrap();
    assert_eq!((launch.id.as_str(), launch.name.as_str()), ("b", "B"));
    assert_eq!(launch.access_token, "old-mc");

    // keys and format stay compatible with old store.json files
    let json: serde_json::Value =
      serde_json::from_slice(&std::fs::read(data.join("store.json")).unwrap()).unwrap();
    assert_eq!(json["active_account"], "a");
    assert_eq!(json["accounts"]["b"]["profile"]["name"], "B");
    assert_eq!(json["accounts"]["b"]["auth"]["mc_token"], "old-mc");

    let reloaded = AccountStore::new().unwrap();
    let mut ids: Vec<_> = reloaded.list_profiles().into_keys().collect();
    ids.sort();
    assert_eq!(ids, ["a", "b"]);
    assert_eq!(reloaded.active(), "a");
  }

  #[test]
  fn set_active_remove_and_update_profile() {
    init_dirs();
    let mut store = AccountStore::new().unwrap();
    store
      .add_account(auth_info([true; 4]), with_id("a", "A"))
      .unwrap();
    store.set_active("b".into()).unwrap();
    // any id is accepted, even one that is not logged in
    assert_eq!(store.active_id().as_deref(), Some("b"));
    store.set_active(String::new()).unwrap();
    assert_eq!(store.active_id(), None);

    store.update_profile(with_id("a", "Renamed")).unwrap();
    // unknown ids are ignored instead of adding a token-less account
    store.update_profile(with_id("ghost", "G")).unwrap();
    let list = store.list_profiles();
    assert_eq!(list.len(), 1);
    assert_eq!(list["a"].as_ref().unwrap().name, "Renamed");

    store.remove_account("a").unwrap();
    store.remove_account("a").unwrap();
    assert!(AccountStore::new().unwrap().list_profiles().is_empty());
  }

  #[test]
  fn failed_saves_leave_the_accounts_unchanged() {
    let data = init_dirs();
    let mut store = AccountStore::new().unwrap();
    store
      .add_account(auth_info([true; 4]), with_id("a", "A"))
      .unwrap();
    // the store's temp file can't be created anymore, every save fails
    std::fs::create_dir(data.join("store.json.tmp")).unwrap();

    assert!(store.set_active("b".into()).is_err());
    assert!(store.remove_account("a").is_err());
    assert!(
      store
        .add_account(auth_info([true; 4]), with_id("c", "C"))
        .is_err()
    );
    // memory matches the disk, the UI shows nothing that was not written
    assert_eq!(store.active(), "a");
    let mut ids: Vec<_> = store.list_profiles().into_keys().collect();
    ids.sort();
    assert_eq!(ids, ["a"]);
  }

  #[test]
  fn refresh_all_updates_profiles_and_expires_dead_logins() {
    init_dirs();
    let server = FakeServer::start();
    token_routes(&server);
    server.route(
      "POST",
      "/login.microsoftonline.com/consumers/oauth2/v2.0/token",
      400,
      "",
    );
    server.route(
      "GET",
      "/api.minecraftservices.com/minecraft/profile",
      200,
      profile_json("New"),
    );
    let mut store = AccountStore::new().unwrap();
    store
      .add_account(auth_info([true; 4]), with_id("ok", "Old"))
      .unwrap();
    store
      .add_account(auth_info([false; 4]), with_id("dead", "D"))
      .unwrap();

    rt::block_on(store.refresh_all()).unwrap();
    let list = AccountStore::new().unwrap().list_profiles();
    assert!(list["dead"].is_none(), "login expired");
    // the profile endpoint answers with its own id, the stored key stays
    assert_eq!(list["ok"].as_ref().unwrap().name, "New");
    assert!(store.mc_token("dead").is_none());
    assert_eq!(
      hit(&server.calls())
        .iter()
        .filter(|c| *c == PROFILE)
        .count(),
      1
    );
  }

  #[test]
  fn refresh_all_keeps_accounts_when_requests_fail() {
    init_dirs();
    let server = FakeServer::start();
    server.route(
      "GET",
      "/api.minecraftservices.com/minecraft/profile",
      503,
      "",
    );
    server.route("POST", "/user.auth.xboxlive.com/user/authenticate", 500, "");
    let mut store = AccountStore::new().unwrap();
    store
      .add_account(auth_info([true; 4]), with_id("a", "A"))
      .unwrap();
    store
      .add_account(auth_info([false, false, false, true]), with_id("b", "B"))
      .unwrap();

    rt::block_on(store.refresh_all()).unwrap();
    let list = store.list_profiles();
    assert_eq!(list["a"].as_ref().unwrap().name, "A");
    assert_eq!(list["b"].as_ref().unwrap().name, "B");
    assert_eq!(store.mc_token("b").unwrap(), "old-mc");
  }

  #[test]
  fn refresh_auth_of_unknown_or_expired_account_is_a_no_op() {
    init_dirs();
    let server = FakeServer::start();
    let mut store = AccountStore::new().unwrap();
    rt::block_on(store.refresh_auth("ghost")).unwrap();
    rt::block_on(store.refresh_auth("")).unwrap();
    assert!(server.calls().is_empty());
    assert!(store.list_profiles().is_empty());
  }

  #[test]
  fn cape_change_puts_or_deletes_with_the_token() {
    for (cape, method) in [("c1", "PUT"), (REMOVE_CAPE, "DELETE")] {
      init_dirs();
      let server = FakeServer::start();
      server.route_fn(method, &format!("/{CAPE}"), |_| {
        Response::new(
          200,
          profile_json("Caped").replace("\"id\":\"uuid\"", "\"id\":\"a\""),
        )
      });
      let mut store = AccountStore::new().unwrap();
      store
        .add_account(auth_info([true; 4]), with_id("a", "A"))
        .unwrap();

      rt::block_on(store.select_cape_by_id("a", cape)).unwrap();
      let calls = server.calls();
      assert_eq!(hit(&calls), [format!("{method} {CAPE}")]);
      assert_eq!(calls[0].header("authorization"), Some("Bearer old-mc"));
      assert_eq!(calls[0].body_str(), format!(r#"{{"capeId":"{cape}"}}"#));
      let saved = AccountStore::new().unwrap().list_profiles();
      assert_eq!(saved["a"].as_ref().unwrap().name, "Caped");
    }
  }

  #[test]
  fn cape_change_refreshes_an_expired_token_first() {
    init_dirs();
    let server = FakeServer::start();
    token_routes(&server);
    server.route("PUT", &format!("/{CAPE}"), 200, profile_json("A"));
    let mut store = AccountStore::new().unwrap();
    store
      .add_account(auth_info([false, true, true, true]), with_id("a", "A"))
      .unwrap();

    rt::block_on(store.select_cape_by_id("a", "c1")).unwrap();
    let calls = server.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].header("authorization"), Some("Bearer mc"));
  }

  #[test]
  fn cape_change_errors_keep_the_profile() {
    init_dirs();
    let server = FakeServer::start();
    let mut store = AccountStore::new().unwrap();
    store
      .add_account(auth_info([true; 4]), with_id("a", "A"))
      .unwrap();

    let err = rt::block_on(store.select_cape_by_id("ghost", "c1")).unwrap_err();
    assert_eq!(err.to_string(), "Cape change error: unknown account ghost");
    assert!(server.calls().is_empty());

    for (status, body) in [
      (400, profile_json("X")),
      (500, String::new()),
      (200, "{".into()),
    ] {
      server.route("PUT", &format!("/{CAPE}"), status, body);
      assert!(
        rt::block_on(store.select_cape_by_id("a", "c1")).is_err(),
        "{status}"
      );
    }
    assert_eq!(store.list_profiles()["a"].as_ref().unwrap().name, "A");
  }
}
