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
  if let Some(window) = cx.windows().into_iter().next() {
    let _ = window.update(cx, |_, window, cx| window.push_notification(note, cx));
  }
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
