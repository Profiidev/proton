use anyhow::Result;
use gpui_kit::{App, AppContext, Entity, Global};
use proton_config::OfflineExt;
use proton_utils::rt;
use url::Url;

pub use crate::handle::{AccountId, AccountList, AccountsHandle, LoginState};
use crate::{
  skin_store::{Cape, Skin},
  store::LaunchInfo,
};

pub mod auth;
mod handle;
pub mod info;
mod listener;
pub mod mc_auth;
pub mod skin_store;
pub mod store;

#[derive(Clone)]
pub struct Accounts {
  /// all accounts, `None` when the login expired
  pub accounts: Entity<AccountList>,
  pub active: Entity<Option<AccountId>>,
  /// locally stored skins
  pub skins: Entity<Vec<Skin>>,
  pub login: Entity<LoginState>,
  /// Clone this into tokio code
  pub handle: AccountsHandle,
}

impl Global for Accounts {}

pub trait AccountsExt {
  fn accounts(&self) -> &Accounts;
}

impl AccountsExt for App {
  fn accounts(&self) -> &Accounts {
    self.global::<Accounts>()
  }
}

/// Loads the account and skin stores. Needs `proton_config::init` first (offline handle).
pub fn init(cx: &mut App) -> Result<()> {
  let (handle, rx, list, active, skins) = AccountsHandle::new(cx.offline().handle.clone())?;
  let accounts = Accounts {
    accounts: cx.new(|_| list),
    active: cx.new(|_| active),
    skins: cx.new(|_| skins),
    login: cx.new(|_| LoginState::Idle),
    handle,
  };
  listener::listen(cx, rx, accounts.clone());
  cx.set_global(accounts);
  Ok(())
}

/// Runs the future on the tokio runtime
fn on_rt<T: Send + 'static>(
  f: impl Future<Output = Result<T>> + Send + 'static,
) -> impl Future<Output = Result<T>> {
  let task = rt::spawn(f);
  async move { task.await? }
}

macro_rules! service_fn {
  ($(#[$m:meta])* $name:ident($($arg:ident: $ty:ty),*) -> $ret:ty, |$h:ident| $body:expr) => {
    $(#[$m])*
    pub fn $name(&self, $($arg: $ty),*) -> impl Future<Output = Result<$ret>> + use<> {
      let $h = self.handle.clone();
      on_rt(async move { $body.await })
    }
  };
}

impl Accounts {
  pub fn active_id<'c>(&self, cx: &'c App) -> Option<&'c AccountId> {
    self.active.read(cx).as_ref()
  }

  service_fn!(refresh_all() -> (), |h| h.refresh_all());
  service_fn!(refresh(id: AccountId) -> (), |h| h.refresh(&id));
  service_fn!(remove(id: AccountId) -> (), |h| h.remove(&id));
  service_fn!(set_active(id: AccountId) -> (), |h| h.set_active(&id));
  service_fn!(
    /// Downloads (or loads the cached) skin png
    get_skin(url: Url) -> Skin, |h| h.get_skin(url));
  service_fn!(get_cape(url: Url) -> Cape, |h| h.get_cape(url));
  service_fn!(add_skin(png: Vec<u8>) -> Skin, |h| h.add_skin(png));
  service_fn!(remove_skin(id: String) -> (), |h| h.remove_skin(&id));
  service_fn!(change_skin(id: String) -> (), |h| h.change_skin(&id));
  service_fn!(
    /// `store::REMOVE_CAPE` removes the cape
    change_cape(id: String) -> (), |h| h.change_cape(&id));
  service_fn!(
    /// Refreshed launch info of the active account, `None` without one
    launch_info() -> Option<LaunchInfo>, |h| async move { Ok(h.launch_info().await) });

  /// Opens the browser login; progress shows in `login`. Resolves when the account was added,
  /// `Ok(())` too when it was cancelled.
  pub fn login(&self) -> impl Future<Output = Result<()>> + use<> {
    let task = self.handle.start_login();
    async move {
      match task?.await {
        Ok(res) => res,
        Err(e) if e.is_cancelled() => Ok(()),
        Err(e) => Err(e.into()),
      }
    }
  }

  pub fn cancel_login(&self) {
    self.handle.cancel_login();
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use gpui_kit::{self as gpui, TestAppContext};

  use super::*;

  fn init_test(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap().keep();
    proton_store::init_at(dir.join("store.json"));
    cx.update(|cx| {
      proton_config::init(cx);
      init(cx).unwrap();
    });
  }

  #[gpui::test]
  async fn active_change_notifies_once(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    init_test(cx);
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    let accounts = cx.update(|cx| {
      let accounts = cx.accounts().clone();
      cx.observe(&accounts.active, move |_, _| seen.set(seen.get() + 1))
        .detach();
      accounts
    });
    assert_eq!(cx.update(|cx| accounts.active_id(cx).cloned()), None);

    accounts.set_active("a".into()).await.unwrap();
    cx.run_until_parked();
    accounts.set_active("a".into()).await.unwrap();
    cx.run_until_parked();

    assert_eq!(count.get(), 1);
    assert_eq!(
      cx.update(|cx| accounts.active_id(cx).cloned()),
      Some("a".into())
    );
  }
}
