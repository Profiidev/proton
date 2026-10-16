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

  pub async fn refresh(&self, id: &str) -> Result<()> {
    let mut store = self.store.lock().await;
    let res = store.refresh(id).await.log_err();
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

  /// Refreshes the token of the active account (errors only logged, the old token may still
  /// work) and returns what the game needs. `None` without a logged in active account.
  pub async fn launch_info(&self) -> Option<LaunchInfo> {
    let mut store = self.store.lock().await;
    let active = store.active().to_string();
    let _ = store.refresh_auth(&active).await.log_err();
    self.publish(&store);
    store.launch_info(&active)
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
