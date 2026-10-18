//! Account service: Microsoft login, accounts, skins and capes. `cx.accounts()`; the tokio half
//! is [`AccountsHandle`], see `crates/utils/PATTERNS.md`.

use anyhow::Result;
use gpui_kit::{App, AppContext, Entity, Global};
use proton_config::OfflineExt;
use proton_utils::rt;

pub use crate::handle::{AccountId, AccountList, AccountsHandle, LoginState};
use crate::{skin_store::Skin, store::LaunchInfo};

pub mod auth;
mod handle;
pub mod info;
mod listener;
mod mc_auth;
mod skin_store;
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
  service_fn!(remove(id: AccountId) -> (), |h| h.remove(&id));
  service_fn!(set_active(id: AccountId) -> (), |h| h.set_active(&id));
  service_fn!(remove_skin(id: String) -> (), |h| h.remove_skin(&id));
  service_fn!(change_skin(id: String) -> (), |h| h.change_skin(&id));
  service_fn!(
    /// `store::REMOVE_CAPE` removes the cape
    change_cape(id: String) -> (), |h| h.change_cape(&id));
  service_fn!(
    /// Refreshed launch info of `account` (the active one when `None`), `None` without one
    launch_info(account: Option<AccountId>) -> Option<LaunchInfo>,
    |h| async move { Ok(h.launch_info(account.as_deref()).await) });

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
pub(crate) mod tests {
  use std::{cell::Cell, path::PathBuf, rc::Rc};

  use gpui_kit::{self as gpui, TestAppContext};

  use super::*;

  /// Temp data dir for the store and the skin files, returns the app data dir
  pub(crate) fn init_dirs() -> PathBuf {
    let dir = tempfile::tempdir().unwrap().keep();
    // nextest runs each test in its own process; skins live under the data dir
    unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
    let data = dir.join("io.profidev.proton");
    proton_store::init_at(data.join("store.json"));
    data
  }

  fn init_test(cx: &mut TestAppContext) {
    init_dirs();
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

  #[gpui::test]
  async fn listener_writes_entities_only_on_change(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    init_test(cx);
    let accounts = cx.update(|cx| cx.accounts().clone());
    let counts = Rc::new(Cell::new((0, 0, 0)));
    cx.update(|cx| {
      let seen = counts.clone();
      cx.observe(&accounts.skins, move |_, _| {
        let (s, a, l) = seen.get();
        seen.set((s + 1, a, l));
      })
      .detach();
      let seen = counts.clone();
      cx.observe(&accounts.accounts, move |_, _| {
        let (s, a, l) = seen.get();
        seen.set((s, a + 1, l));
      })
      .detach();
      let seen = counts.clone();
      cx.observe(&accounts.login, move |_, _| {
        let (s, a, l) = seen.get();
        seen.set((s, a, l + 1));
      })
      .detach();
    });

    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::new(64, 64)
      .write_to(&mut png, image::ImageFormat::Png)
      .unwrap();
    let handle = accounts.handle.clone();
    let skin = rt::spawn(async move { handle.add_skin(png.into_inner()).await })
      .await
      .unwrap()
      .unwrap();
    cx.run_until_parked();
    // unknown ids leave the lists as they were
    accounts.remove_skin("ghost".into()).await.unwrap();
    accounts.remove("ghost".into()).await.unwrap();
    // already idle
    accounts.cancel_login();
    cx.run_until_parked();

    assert_eq!(counts.get(), (1, 0, 0));
    assert_eq!(cx.update(|cx| accounts.skins.read(cx).clone()), [skin]);
    assert_eq!(
      accounts.launch_info(None).await.unwrap().map(|i| i.id),
      None
    );
    assert!(accounts.change_skin("ghost".into()).await.is_err());
  }
}
