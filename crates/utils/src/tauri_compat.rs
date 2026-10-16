//! TEMPORARY shim so versions/account/profiles/content keep compiling while they still use
//! tauri. Delete this module once no crate imports it.
//!
//! Migrate off: drop the `update_data(..)` call and refresh the crate's own gpui `Entity` after
//! the mutation instead (`write_changed`, or send an event through the crate's handle, see
//! `crates/utils/PATTERNS.md`).

#[derive(Clone, Copy, Debug)]
pub enum UpdateType {
  Accounts,
  AccountActive,
  AccountSkins,
  Versions,
  Profiles,
  ProfileLogs,
  ProfileQuickPlay,
  ProfileContent,
  Instances,
  InstanceLogs,
  Settings,
  Offline,
  ContentUpdates,
}

/// No-op, the `data-update` event had only the webview as listener
pub fn update_data<H>(_handle: &H, r#type: UpdateType) {
  tracing::trace!("Dropped update event for type {type:?}");
}
