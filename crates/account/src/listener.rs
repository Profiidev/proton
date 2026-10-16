use gpui_kit::App;
use proton_utils::entity::WriteChangedExt;

use crate::{Accounts, handle::AccountMsg};

/// Moves what the tokio side reports into the entities, on the gpui thread
pub(crate) fn listen(cx: &mut App, rx: flume::Receiver<AccountMsg>, accounts: Accounts) {
  cx.spawn(async move |cx| {
    while let Ok(msg) = rx.recv_async().await {
      match msg {
        AccountMsg::Accounts(v) => accounts.accounts.write_changed(cx, v),
        AccountMsg::Active(v) => accounts.active.write_changed(cx, v),
        AccountMsg::Skins(v) => accounts.skins.write_changed(cx, v),
        AccountMsg::Login(v) => accounts.login.write_changed(cx, v),
      }
    }
  })
  .detach();
}
