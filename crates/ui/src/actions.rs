//! App-wide actions shared by several pages (play buttons, toasts).

use gpui_kit::{
  App,
  component::{WindowExt, notification::Notification},
};
use proton_account::AccountsExt;
use proton_profiles::{ProfilesExt, config::QuickPlayInfo, store::LaunchInfo};

/// Shows a toast in the main window
pub fn toast(note: impl Into<Notification>, cx: &mut App) {
  let note = note.into();
  // deferred: from a click or event handler the window is mid-update and a nested update fails
  cx.defer(move |cx| {
    if let Some(window) = cx.windows().into_iter().next() {
      let _ = window.update(cx, |_, window, cx| window.push_notification(note, cx));
    }
  });
}

/// Shows an error toast with the message and the error chain
pub fn toast_error(message: &str, err: &anyhow::Error, cx: &mut App) {
  tracing::error!("{message}: {err:#}");
  toast(Notification::error(format!("{message}: {err}")), cx);
}

/// Launches a profile (optionally straight into a world/server) with the active account
pub fn launch(profile: String, quick_play: Option<QuickPlayInfo>, cx: &mut App) {
  let info = cx.accounts().launch_info();
  cx.spawn(async move |cx| {
    let auth = match info.await {
      Ok(Some(a)) => a,
      Ok(None) => {
        cx.update(|cx| toast(Notification::error("No account selected"), cx));
        return;
      }
      Err(e) => {
        cx.update(|cx| toast_error("Failed to launch", &e, cx));
        return;
      }
    };
    let auth = LaunchInfo {
      id: auth.id,
      name: auth.name,
      access_token: auth.access_token,
    };
    let task = cx.update(|cx| cx.profiles().launch(cx, profile, quick_play, auth));
    if let Err(e) = task.await {
      cx.update(|cx| toast_error("Failed to launch", &e, cx));
    }
  })
  .detach();
}

#[cfg(test)]
mod tests {
  use gpui_kit::{self as gpui, AnyWindowHandle, AppContext, IntoElement, TestAppContext, div};
  use proton_components::test_view;

  use super::*;
  use crate::pages::profiles::tests::{init_fixture, poll};

  const VANILLA: &str = "11111111-1111-4111-8111-111111111111";

  fn toasts(cx: &mut TestAppContext, handle: AnyWindowHandle) -> usize {
    cx.update_window(handle, |_, window, cx| window.notifications(cx).len())
      .unwrap()
  }

  fn open(cx: &mut TestAppContext) -> AnyWindowHandle {
    init_fixture(cx);
    test_view::open(cx, |_, _| div().into_any_element()).0
  }

  #[gpui::test]
  fn toast_without_a_window_is_dropped(cx: &mut TestAppContext) {
    // before the main window opens (startup errors), nothing to show it in
    cx.update(|cx| toast("early", cx));
    let handle = test_view::open(cx, |_, _| div().into_any_element()).0;
    cx.update(|cx| toast_error("Failed", &anyhow::anyhow!("boom"), cx));
    assert_eq!(toasts(cx, handle), 1);
  }

  #[gpui::test]
  fn toast_from_inside_a_window_callback_is_shown(cx: &mut TestAppContext) {
    let handle = open(cx);
    // like a click handler: the window is already being updated
    cx.update_window(handle, |_, _, cx| toast("inside", cx))
      .unwrap();
    assert_eq!(toasts(cx, handle), 1);
  }

  #[gpui::test]
  async fn launch_without_an_active_account_says_so(cx: &mut TestAppContext) {
    let handle = open(cx);
    let accounts = cx.update(|cx| cx.accounts().clone());
    accounts.set_active("nobody".into()).await.unwrap();
    cx.update(|cx| launch(VANILLA.into(), None, cx));
    poll(cx, |cx| {
      cx.windows()[0]
        .update(cx, |_, w, cx| w.notifications(cx).len())
        .unwrap()
        == 1
    });
    // nothing was started
    assert!(cx.update(|cx| cx.profiles().instances.read(cx).is_empty()));
    assert_eq!(toasts(cx, handle), 1);
  }

  #[gpui::test]
  fn launch_of_a_missing_profile_toasts_the_error(cx: &mut TestAppContext) {
    let handle = open(cx);
    cx.update(|cx| launch("missing".into(), None, cx));
    poll(cx, |cx| {
      cx.windows()[0]
        .update(cx, |_, w, cx| w.notifications(cx).len())
        .unwrap()
        == 1
    });
    assert!(cx.update(|cx| cx.profiles().instances.read(cx).is_empty()));
    assert_eq!(toasts(cx, handle), 1);
  }
}
