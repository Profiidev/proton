//! Content dialogs and widgets shared by Discover, the project page, the profiles list and the
//! profile content tabs.
//!
//! Public API (other pages code against it):
//! - [`InstallDialog::open`]`(project, target, window, cx)`: pick a profile + version, plan,
//!   install as a downloads-panel job, toast.
//! - [`ModpackInstallDialog::open`]`(project, window, cx)`, [`ModpackInstallDialog::open_plan`]
//!   (imported file), [`ModpackInstallDialog::open_change`] (switch a modpack profile's version).
//! - [`ManualDownloads::open`]`(files, on_done, window, cx)`: standalone dialog that waits for
//!   restricted CurseForge files in the download folders; [`ManualDownloads`] is also a view
//!   embedded in the install dialogs.
//! - [`TargetPicker`], [`set_target`] / [`target`]: the remembered profile Discover installs into.
//! - [`FromModpackTab`], [`ImportFileTab`]: create-dialog tab bodies.
//! - [`open_duplicate_dialog`], [`export_mrpack`]: profile menu actions.

mod create_tabs;
mod install;
pub mod logic;
mod manual;
mod modpack;
mod profile_actions;
pub mod target_picker;

use std::{collections::HashMap, sync::Arc};

use gpui_kit::{App, AppContext, Entity, Global, Image, ImageSource};
use proton_content::model::{ContentKind, ProjectSummary, Source};
use proton_profiles::ProfilesExt;
use proton_utils::log::ErrorLogExt;

pub use create_tabs::{FromModpackTab, ImportFileTab};
pub use install::InstallDialog;
pub use manual::ManualDownloads;
pub use modpack::ModpackInstallDialog;
pub use profile_actions::{export_mrpack, open_duplicate_dialog};
pub use target_picker::TargetPicker;

/// The project a dialog installs
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRef {
  pub source: Source,
  pub id: String,
  pub kind: ContentKind,
  pub title: String,
  pub icon_url: Option<String>,
  /// Game versions and loaders of the project, for the compatibility of the profile tiles.
  /// Empty means "unknown, allow all".
  pub game_versions: Vec<String>,
  pub loaders: Vec<String>,
}

impl From<&ProjectSummary> for ProjectRef {
  fn from(p: &ProjectSummary) -> Self {
    Self {
      source: p.source,
      id: p.id.clone(),
      kind: p.kind,
      title: p.name.clone(),
      icon_url: p.icon_url.clone(),
      game_versions: p.game_versions.clone(),
      loaders: p.loaders.clone(),
    }
  }
}

/// `localStorage['content-target']` of the web app
const TARGET_KEY: &str = "content-target";

struct GlobalTarget(Entity<Option<String>>);

impl Global for GlobalTarget {}

/// The remembered target profile id, observe it to follow picker changes
pub fn target(cx: &mut App) -> Entity<Option<String>> {
  if let Some(g) = cx.try_global::<GlobalTarget>() {
    return g.0.clone();
  }
  let stored = proton_store::store()
    .get::<String>(TARGET_KEY)
    .log_err()
    .ok()
    .flatten();
  let entity = cx.new(|_| stored);
  cx.set_global(GlobalTarget(entity.clone()));
  entity
}

/// Remembers `profile` as the target, e.g. before navigating to Discover from a profile
pub fn set_target(cx: &mut App, profile: Option<String>) {
  let entity = target(cx);
  let _ = match &profile {
    Some(id) => proton_store::store().set(TARGET_KEY, id),
    None => proton_store::store().delete(TARGET_KEY),
  }
  .log_err();
  entity.update(cx, |t, cx| {
    if *t != profile {
      *t = profile;
      cx.notify();
    }
  });
}

/// Profile id → decoded icon (None = default icon), loaded once per profile
#[derive(Default)]
pub struct ProfileIcons(HashMap<String, Option<Arc<Image>>>);

struct GlobalIcons(Entity<ProfileIcons>);

impl Global for GlobalIcons {}

/// The profile's icon; loads it in the background on first use and notifies the returned
/// entity, so observe it (or re-render through it) to pick the image up.
pub fn profile_icon(id: &str, cx: &mut App) -> Option<ImageSource> {
  let icons = profile_icons(cx);
  if let Some(icon) = icons.read(cx).0.get(id) {
    return icon.clone().map(ImageSource::Image);
  }
  icons.update(cx, |i, _| i.0.insert(id.to_string(), None));
  let load = cx.profiles().icon(id.to_string());
  let id = id.to_string();
  cx.spawn(async move |cx| {
    if let Ok(Some(bytes)) = load.await.log_err() {
      let image = crate::pages::profiles::icon_image(bytes);
      icons.update(cx, |i, cx| {
        i.0.insert(id, Some(image));
        cx.notify();
      });
    }
  })
  .detach();
  None
}

/// Observable icon cache, views showing profile icons observe it once
pub fn profile_icons(cx: &mut App) -> Entity<ProfileIcons> {
  if let Some(g) = cx.try_global::<GlobalIcons>() {
    return g.0.clone();
  }
  let e = cx.new(|_| ProfileIcons::default());
  cx.set_global(GlobalIcons(e.clone()));
  e
}

/// The `ContentError` behind a service error
pub fn content_error(err: &anyhow::Error) -> Option<&proton_content::ContentError> {
  err.downcast_ref()
}
