//! gpui-free half of the account service: a cheap handle for tokio code. Every mutation sends the
//! new state through the channel, `listener::listen` moves it into the entities.

use std::{
  collections::HashMap,
  sync::{Arc, Mutex as SyncMutex},
};

use anyhow::{Result, anyhow, bail};
use proton_config::offline::{OfflineHandle, OfflineResultExt};
use proton_utils::{download::default_client, log::ErrorLogExt, rt};
use reqwest::Client;
use tokio::{sync::Mutex, task::AbortHandle};
use tracing::warn;
use url::Url;

use crate::{
  auth::{LoginStatus, ms_mc_login},
  info::{ProfileInfo, get_profile_info},
  skin_store::{Cape, Skin, SkinStore},
  store::{AccountStore, LaunchInfo},
};

pub type AccountId = String;
pub type AccountList = HashMap<AccountId, Option<ProfileInfo>>;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum LoginState {
  #[default]
  Idle,
  /// Browser is open, waiting for the redirect
  Waiting,
  /// This token step finished
  Step(LoginStatus),
  Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccountMsg {
  Accounts(AccountList),
  Active(Option<AccountId>),
  Skins(Vec<Skin>),
  Login(LoginState),
}

#[derive(Clone)]
pub struct AccountsHandle {
  store: Arc<Mutex<AccountStore>>,
  skins: Arc<Mutex<SkinStore>>,
  login_task: Arc<SyncMutex<Option<AbortHandle>>>,
  offline: OfflineHandle,
  client: Client,
  tx: flume::Sender<AccountMsg>,
}

/// The handle, the listener's receiver and the initial accounts, active account and skins
pub(crate) type HandleInit = (
  AccountsHandle,
  flume::Receiver<AccountMsg>,
  AccountList,
  Option<AccountId>,
  Vec<Skin>,
);

impl AccountsHandle {
  pub(crate) fn new(offline: OfflineHandle) -> Result<HandleInit> {
    let store = AccountStore::new()?;
    let skins = SkinStore::new()?;
    let (list, active, skin_list) = (store.list_profiles(), store.active_id(), skins.list_skins());
    let (tx, rx) = flume::unbounded();
    let handle = Self {
      store: Arc::new(Mutex::new(store)),
      skins: Arc::new(Mutex::new(skins)),
      login_task: Default::default(),
      offline,
      client: default_client(),
      tx,
    };
    Ok((handle, rx, list, active, skin_list))
  }

  fn send(&self, msg: AccountMsg) {
    // the listener is gone only when the app quits
    let _ = self.tx.send(msg);
  }

  fn publish(&self, store: &AccountStore) {
    self.send(AccountMsg::Accounts(store.list_profiles()));
    self.send(AccountMsg::Active(store.active_id()));
  }

  pub async fn refresh_all(&self) -> Result<()> {
    let mut store = self.store.lock().await;
    let res = store
      .refresh_all()
      .await
      .check_online_state(&self.offline)
      .await;
    self.publish(&store);
    res
  }

  pub async fn remove(&self, id: &str) -> Result<()> {
    let mut store = self.store.lock().await;
    let res = store.remove_account(id).log_err();
    self.publish(&store);
    res
  }

  pub async fn set_active(&self, id: &str) -> Result<()> {
    let mut store = self.store.lock().await;
    let res = store.set_active(id.to_string()).log_err();
    self.publish(&store);
    res
  }

  /// Refreshes the token of `account` (the active one when `None`; errors only logged, the old
  /// token may still work) and returns what the game needs. `None` when it is not logged in.
  pub async fn launch_info(&self, account: Option<&str>) -> Option<LaunchInfo> {
    let mut store = self.store.lock().await;
    let id = account.unwrap_or(store.active()).to_string();
    let _ = store.refresh_auth(&id).await.log_err();
    self.publish(&store);
    store.launch_info(&id)
  }

  /// Starts the browser login on the tokio runtime. Abort the returned task or call
  /// [`Self::cancel_login`] to stop it, which frees the redirect port.
  pub fn start_login(&self) -> Result<rt::JoinHandle<Result<()>>> {
    let mut slot = self
      .login_task
      .lock()
      .map_err(|_| anyhow!("login lock poisoned"))?;
    if slot.as_ref().is_some_and(|t| !t.is_finished()) {
      bail!("A login is already in progress");
    }
    let handle = self.clone();
    let task = rt::spawn(async move { handle.login().await });
    *slot = Some(task.abort_handle());
    Ok(task)
  }

  pub fn cancel_login(&self) {
    if let Some(task) = self.login_task.lock().ok().and_then(|mut t| t.take()) {
      task.abort();
    }
    self.send(AccountMsg::Login(LoginState::Idle));
  }

  async fn login(&self) -> Result<()> {
    self.send(AccountMsg::Login(LoginState::Waiting));
    let res: Result<()> = async {
      let auth = ms_mc_login(&self.client, |step| {
        self.send(AccountMsg::Login(LoginState::Step(step)))
      })
      .await?;
      let profile = get_profile_info(&self.client, &auth.mc_token).await?;
      let mut store = self.store.lock().await;
      store.add_account(auth, profile)?;
      self.publish(&store);
      Ok(())
    }
    .await
    .check_online_state(&self.offline)
    .await;
    self.send(AccountMsg::Login(match &res {
      Ok(()) => LoginState::Idle,
      Err(e) => LoginState::Error(e.to_string()),
    }));
    res
  }

  pub async fn get_skin(&self, url: Url) -> Result<Skin> {
    self.skins.lock().await.get_skin_by_url(url).await.log_err()
  }

  pub async fn get_cape(&self, url: Url) -> Result<Cape> {
    self.skins.lock().await.get_cape_by_url(url).await.log_err()
  }

  pub async fn add_skin(&self, png: Vec<u8>) -> Result<Skin> {
    let mut skins = self.skins.lock().await;
    let res = skins.add_skin(None, &png).await.log_err();
    self.send(AccountMsg::Skins(skins.list_skins()));
    res
  }

  pub async fn remove_skin(&self, id: &str) -> Result<()> {
    let mut skins = self.skins.lock().await;
    let res = skins.remove_skin(id).log_err();
    self.send(AccountMsg::Skins(skins.list_skins()));
    res
  }

  /// Uploads/selects the skin for the active account
  pub async fn change_skin(&self, id: &str) -> Result<()> {
    let mut skins = self.skins.lock().await;
    let mut store = self.store.lock().await;
    let account = store.active().to_string();
    store
      .refresh_auth(&account)
      .await
      .check_online_state(&self.offline)
      .await?;
    let Some(token) = store.mc_token(&account).cloned() else {
      warn!("Account {account} not found");
      bail!("Active account not found");
    };
    let profile = skins
      .select_skin(id, &token)
      .await
      .check_online_state(&self.offline)
      .await?;
    store.update_profile(profile)?;
    self.publish(&store);
    Ok(())
  }

  /// Selects a cape for the active account, `REMOVE_CAPE` removes it
  pub async fn change_cape(&self, id: &str) -> Result<()> {
    let mut store = self.store.lock().await;
    let account = store.active().to_string();
    let res = store
      .select_cape_by_id(&account, id)
      .await
      .check_online_state(&self.offline)
      .await;
    self.publish(&store);
    res
  }
}

#[cfg(test)]
mod tests {
  use std::io::Cursor;

  use image::{ImageFormat, RgbaImage};
  use proton_utils::test_http::FakeServer;

  use super::*;
  use crate::{
    auth::tests::auth_info,
    info::tests::{profile, profile_json},
    mc_auth::tests::token_routes,
    store::REMOVE_CAPE,
    tests::init_dirs,
  };

  const PROFILE_PATH: &str = "/api.minecraftservices.com/minecraft/profile";

  fn png() -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    RgbaImage::new(64, 64)
      .write_to(&mut out, ImageFormat::Png)
      .unwrap();
    out.into_inner()
  }

  /// A saved, logged in account `uuid` named `A`, `valid` like [`auth_info`]
  fn seed_account(valid: [bool; 4]) {
    let mut store = AccountStore::new().unwrap();
    store.add_account(auth_info(valid), profile("A")).unwrap();
  }

  fn handle() -> (AccountsHandle, flume::Receiver<AccountMsg>) {
    let (handle, rx, ..) = AccountsHandle::new(OfflineHandle::new().0).unwrap();
    (handle, rx)
  }

  fn name_of(msgs: &[AccountMsg]) -> Option<String> {
    msgs.iter().find_map(|m| match m {
      AccountMsg::Accounts(list) => list.get("uuid")?.as_ref().map(|p| p.name.clone()),
      _ => None,
    })
  }

  #[test]
  fn new_returns_the_saved_state() {
    init_dirs();
    let (_, _, list, active, skins) = AccountsHandle::new(OfflineHandle::new().0).unwrap();
    assert!(list.is_empty() && active.is_none() && skins.is_empty());

    seed_account([true; 4]);
    let skin = rt::block_on(SkinStore::new().unwrap().add_skin(None, &png())).unwrap();
    let (_, _, list, active, skins) = AccountsHandle::new(OfflineHandle::new().0).unwrap();
    assert_eq!(list["uuid"], Some(profile("A")));
    assert_eq!(active.as_deref(), Some("uuid"));
    assert_eq!(skins, [skin]);
  }

  #[test]
  fn mutations_publish_the_accounts_and_active_id() {
    init_dirs();
    seed_account([true; 4]);
    let (handle, rx) = handle();

    rt::block_on(handle.set_active("b")).unwrap();
    let msgs: Vec<_> = rx.drain().collect();
    assert_eq!(msgs[1..], [AccountMsg::Active(Some("b".into()))]);
    assert_eq!(name_of(&msgs).as_deref(), Some("A"));

    rt::block_on(handle.remove("uuid")).unwrap();
    // removing again still publishes, the listener drops unchanged values
    rt::block_on(handle.remove("uuid")).unwrap();
    let empty = AccountMsg::Accounts(HashMap::new());
    let active = AccountMsg::Active(Some("b".into()));
    assert_eq!(
      rx.drain().collect::<Vec<_>>(),
      [empty.clone(), active.clone(), empty, active]
    );
  }

  #[test]
  fn launch_info_needs_a_logged_in_active_account() {
    init_dirs();
    let server = FakeServer::start();
    let (handle, rx) = handle();
    assert!(rt::block_on(handle.launch_info(None)).is_none());
    assert_eq!(rx.drain().count(), 2);

    seed_account([true; 4]);
    let (handle, _) = self::handle();
    let info = rt::block_on(handle.launch_info(None)).unwrap();
    assert_eq!((info.id.as_str(), info.name.as_str()), ("uuid", "A"));
    assert_eq!(info.access_token, "old-mc");
    assert!(server.calls().is_empty(), "a valid token is not refreshed");
    // an explicit account instead of the active one
    let named = rt::block_on(handle.launch_info(Some("uuid"))).unwrap();
    assert_eq!(named.name, "A");
    assert!(rt::block_on(handle.launch_info(Some("ghost"))).is_none());
  }

  #[test]
  fn launch_info_falls_back_to_the_old_token_when_refresh_fails() {
    init_dirs();
    let server = FakeServer::start();
    server.route(
      "POST",
      "/api.minecraftservices.com/authentication/login_with_xbox",
      503,
      "",
    );
    seed_account([false, true, true, true]);
    let (handle, _) = handle();
    assert_eq!(
      rt::block_on(handle.launch_info(None)).unwrap().access_token,
      "old-mc"
    );
    assert_eq!(server.calls().len(), 1);
  }

  #[test]
  fn launch_info_refreshes_an_expired_token() {
    init_dirs();
    let server = FakeServer::start();
    token_routes(&server);
    seed_account([false, true, true, true]);
    let (handle, _) = handle();
    assert_eq!(
      rt::block_on(handle.launch_info(None)).unwrap().access_token,
      "mc"
    );
    // the new token is saved
    assert_eq!(AccountStore::new().unwrap().mc_token("uuid").unwrap(), "mc");
  }

  #[test]
  fn refresh_all_publishes_the_new_profiles() {
    init_dirs();
    let server = FakeServer::start();
    server.route("GET", PROFILE_PATH, 200, profile_json("Fresh"));
    seed_account([true; 4]);
    let (handle, rx) = handle();
    rt::block_on(handle.refresh_all()).unwrap();
    assert_eq!(
      name_of(&rx.drain().collect::<Vec<_>>()).as_deref(),
      Some("Fresh")
    );
  }

  #[test]
  fn cancel_without_a_login_resets_the_state() {
    init_dirs();
    let (handle, rx) = handle();
    handle.cancel_login();
    handle.cancel_login();
    let idle = AccountMsg::Login(LoginState::Idle);
    assert_eq!(rx.drain().collect::<Vec<_>>(), [idle.clone(), idle]);
  }

  #[test]
  fn skin_changes_publish_the_skin_list() {
    init_dirs();
    let (handle, rx) = handle();
    let skin = rt::block_on(handle.add_skin(png())).unwrap();
    assert_eq!(
      rx.drain().collect::<Vec<_>>(),
      [AccountMsg::Skins(vec![skin.clone()])]
    );

    // a failed add still publishes the unchanged list
    assert!(rt::block_on(handle.add_skin(b"nope".to_vec())).is_err());
    rt::block_on(handle.remove_skin(&skin.id)).unwrap();
    assert_eq!(
      rx.drain().collect::<Vec<_>>(),
      [AccountMsg::Skins(vec![skin]), AccountMsg::Skins(vec![])]
    );
  }

  #[test]
  fn get_skin_and_cape_cache_the_textures() {
    init_dirs();
    let server = FakeServer::start();
    server.route("GET", "/s.png", 200, png());
    server.route("GET", "/c.png", 200, b"cape".to_vec());
    let (handle, _) = handle();
    let url = |p: &str| Url::parse(&format!("{}/{p}", server.origin())).unwrap();
    let skin = rt::block_on(handle.get_skin(url("s.png"))).unwrap();
    let cape = rt::block_on(handle.get_cape(url("c.png"))).unwrap();
    assert!(skin.data.exists() && cape.data.exists());
    assert!(rt::block_on(handle.get_skin(url("404.png"))).is_err());
  }

  #[test]
  fn change_skin_without_an_active_account_fails() {
    init_dirs();
    let server = FakeServer::start();
    let (handle, _) = handle();
    let err = rt::block_on(handle.change_skin("x")).unwrap_err();
    assert_eq!(err.to_string(), "Active account not found");
    assert!(server.calls().is_empty());
  }

  #[test]
  fn change_skin_uploads_and_publishes_the_profile() {
    init_dirs();
    let server = FakeServer::start();
    let changed = profile_json("Skinned");
    server.route("POST", &format!("{PROFILE_PATH}/skins"), 200, changed);
    seed_account([true; 4]);
    let (handle, rx) = handle();
    let skin = rt::block_on(handle.add_skin(png())).unwrap();
    rx.drain().for_each(drop);

    rt::block_on(handle.change_skin(&skin.id)).unwrap();
    assert_eq!(
      server.calls()[0].header("authorization"),
      Some("Bearer old-mc")
    );
    assert_eq!(
      name_of(&rx.drain().collect::<Vec<_>>()).as_deref(),
      Some("Skinned")
    );
  }

  #[test]
  fn change_cape_publishes_the_profile() {
    init_dirs();
    let server = FakeServer::start();
    server.route(
      "DELETE",
      &format!("{PROFILE_PATH}/capes/active"),
      200,
      profile_json("Bare"),
    );
    seed_account([true; 4]);
    let (handle, rx) = handle();
    rt::block_on(handle.change_cape(REMOVE_CAPE)).unwrap();
    assert_eq!(
      name_of(&rx.drain().collect::<Vec<_>>()).as_deref(),
      Some("Bare")
    );
  }
}
