use gpui_kit::{App, AppContext, Entity, EventEmitter, Global};
use proton_utils::{entity::WriteChangedExt, log::ErrorLogExt, rt};

use crate::{offline::OfflineHandle, settings::Settings};

mod listener;
pub mod offline;
pub mod settings;

#[derive(Clone)]
pub struct SettingsState {
  pub settings: Entity<Settings>,
}

impl Global for SettingsState {}

impl SettingsState {
  pub fn get<'c>(&self, cx: &'c App) -> &'c Settings {
    self.settings.read(cx)
  }
}

pub trait SettingsExt {
  fn settings(&self) -> &SettingsState;
}

impl SettingsExt for App {
  fn settings(&self) -> &SettingsState {
    self.global::<SettingsState>()
  }
}

/// Edits a copy of the settings, saves and notifies only when something changed
pub fn update(cx: &mut App, f: impl FnOnce(&mut Settings)) {
  let entity = cx.settings().settings.clone();
  let mut next = entity.read(cx).clone();
  f(&mut next);
  if next != *entity.read(cx) {
    let _ = next.save().log_err();
    entity.write_changed(cx, next);
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OfflineEvent {
  /// First reconnect before the startup manifest refresh succeeded, refresh them now
  Reconnected,
  ManifestRefreshError,
}

/// Only carries the [`OfflineEvent`]s, so toast subscribers are not woken by state changes
pub struct OfflineEvents;

impl EventEmitter<OfflineEvent> for OfflineEvents {}

#[derive(Clone)]
pub struct Offline {
  /// true while offline
  pub offline: Entity<bool>,
  pub events: Entity<OfflineEvents>,
  /// Clone this into tokio code
  pub handle: OfflineHandle,
}

impl Global for Offline {}

impl Offline {
  pub fn is_offline(&self, cx: &App) -> bool {
    *self.offline.read(cx)
  }

  /// Checks the connection on the tokio runtime, the entity follows through the listener
  pub fn try_reconnect(&self) -> impl Future<Output = bool> + use<> {
    let handle = self.handle.clone();
    async move {
      rt::spawn(async move { handle.check().await })
        .await
        .unwrap_or(false)
    }
  }
}

pub trait OfflineExt {
  fn offline(&self) -> &Offline;
}

impl OfflineExt for App {
  fn offline(&self) -> &Offline {
    self.global::<Offline>()
  }
}

/// Loads the settings from the store and starts the offline listener. Does not check the
/// connection, the startup sequence calls `try_reconnect`.
pub fn init(cx: &mut App) {
  let settings = Settings::load().log_err().unwrap_or_default();
  let settings = cx.new(|_| settings);
  cx.set_global(SettingsState { settings });

  let (handle, rx) = OfflineHandle::new();
  let offline = Offline {
    offline: cx.new(|_| false),
    events: cx.new(|_| OfflineEvents),
    handle,
  };
  listener::listen(cx, rx, offline.offline.clone(), offline.events.clone());
  cx.set_global(offline);
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use gpui_kit::{self as gpui, TestAppContext};

  use super::*;

  /// Saves go to a temp store instead of the real data dir
  fn init_test(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap().keep();
    proton_store::init_at(dir.join("store.json"));
    cx.update(init);
  }

  #[gpui::test]
  fn settings_update_notifies_only_on_change(cx: &mut TestAppContext) {
    init_test(cx);
    let count = Rc::new(RefCell::new(0));
    let seen = count.clone();
    cx.update(|cx| {
      let entity = cx.settings().settings.clone();
      cx.observe(&entity, move |_, _| *seen.borrow_mut() += 1)
        .detach();
      update(cx, |s| s.auto_update = false);
    });
    cx.run_until_parked();
    assert_eq!(*count.borrow(), 0);

    cx.update(|cx| update(cx, |s| s.auto_update = true));
    cx.run_until_parked();
    cx.update(|cx| update(cx, |s| s.auto_update = true));
    cx.run_until_parked();
    assert_eq!(*count.borrow(), 1);
    assert!(cx.update(|cx| cx.settings().get(cx).auto_update));
    assert!(Settings::load().unwrap().auto_update);
  }

  #[gpui::test]
  fn offline_handle_reaches_the_entity(cx: &mut TestAppContext) {
    init_test(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    let handle = cx.update(|cx| {
      let offline = cx.offline().clone();
      cx.subscribe(&offline.events, move |_, e: &OfflineEvent, _| {
        seen.borrow_mut().push(*e)
      })
      .detach();
      offline.handle
    });

    // from another thread, like a tokio task would
    let tokio_side = handle.clone();
    std::thread::spawn(move || tokio_side.report(false))
      .join()
      .unwrap();
    cx.run_until_parked();
    assert!(cx.update(|cx| cx.offline().is_offline(cx)));

    handle.report(true);
    handle.manifest_refresh_failed();
    cx.run_until_parked();
    assert!(!cx.update(|cx| cx.offline().is_offline(cx)));
    assert_eq!(
      *events.borrow(),
      [
        OfflineEvent::Reconnected,
        OfflineEvent::ManifestRefreshError
      ]
    );
  }
}
