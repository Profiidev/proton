//! Profile icons, quick play icons and mod counts shared by the home and profiles pages.
//! Reloaded whenever the profile lists change; notifies only when something differs.

use std::{collections::HashMap, sync::Arc};

use gpui_kit::{App, AppContext, Entity, Global, Image, ImageFormat, ImageSource, Task};
use proton_profiles::{
  ProfilesExt,
  config::{QuickPlayInfo, QuickPlayType},
};
use proton_utils::log::ErrorLogExt;

/// (profile id, quick play type, quick play id)
type QuickPlayKey = (String, u8, String);

fn qp_key(profile: &str, qp: &QuickPlayInfo) -> QuickPlayKey {
  let kind = match qp.r#type {
    QuickPlayType::Singleplayer => 0,
    QuickPlayType::Multiplayer => 1,
    QuickPlayType::Realms => 2,
  };
  (profile.to_string(), kind, qp.id.clone())
}

#[derive(Default)]
pub struct ProfileMeta {
  icons: HashMap<String, Arc<Image>>,
  quick_play_icons: HashMap<QuickPlayKey, Arc<Image>>,
  mods: HashMap<String, usize>,
  _reload: Option<Task<()>>,
}

impl ProfileMeta {
  /// Custom profile icon, None for the default
  pub fn icon(&self, profile: &str) -> Option<ImageSource> {
    self.icons.get(profile).cloned().map(ImageSource::from)
  }

  pub fn quick_play_icon(&self, profile: &str, qp: &QuickPlayInfo) -> Option<ImageSource> {
    self
      .quick_play_icons
      .get(&qp_key(profile, qp))
      .cloned()
      .map(ImageSource::from)
  }

  /// Mod jars in the profile, None until counted
  pub fn mods(&self, profile: &str) -> Option<usize> {
    self.mods.get(profile).copied()
  }
}

struct GlobalMeta(Entity<ProfileMeta>);

impl Global for GlobalMeta {}

/// The shared cache, created (and loaded) on first use. Needs `proton_profiles::init`.
pub fn profile_meta(cx: &mut App) -> Entity<ProfileMeta> {
  if let Some(meta) = cx.try_global::<GlobalMeta>() {
    return meta.0.clone();
  }
  let meta = cx.new(|_| ProfileMeta::default());
  cx.set_global(GlobalMeta(meta.clone()));
  let profiles = cx.profiles().clone();
  for list in [&profiles.favorites, &profiles.history] {
    let meta = meta.clone();
    cx.observe(list, move |_, cx| reload(&meta, cx)).detach();
  }
  {
    let meta = meta.clone();
    cx.observe(&profiles.profiles, move |_, cx| reload(&meta, cx))
      .detach();
  }
  reload(&meta, cx);
  meta
}

/// Detects png/jpeg (the only formats the icon picker accepts) so gpui can decode it
fn image(bytes: Vec<u8>) -> Arc<Image> {
  let format = if bytes.starts_with(&[0xff, 0xd8]) {
    ImageFormat::Jpeg
  } else {
    ImageFormat::Png
  };
  Arc::new(Image::from_bytes(format, bytes))
}

fn reload(meta: &Entity<ProfileMeta>, cx: &mut App) {
  let service = cx.profiles().clone();
  let ids: Vec<String> = service
    .profiles
    .read(cx)
    .iter()
    .map(|p| p.id.clone())
    .collect();
  let quick_play: Vec<(String, QuickPlayInfo)> = service
    .favorites
    .read(cx)
    .iter()
    .chain(service.history.read(cx).iter())
    .filter_map(|e| Some((e.profile.id.clone(), e.quick_play.clone()?)))
    .collect();
  let icons: Vec<_> = ids
    .iter()
    .map(|id| (id.clone(), service.icon(id.clone())))
    .collect();
  let qp_icons: Vec<_> = quick_play
    .into_iter()
    .map(|(id, qp)| (qp_key(&id, &qp), service.quick_play_icon(cx, id, qp)))
    .collect();
  let mods = service.mods_count();

  let entity = meta.clone();
  // replacing the task cancels a reload still in flight
  let task = cx.spawn(async move |cx| {
    let mut next_icons = HashMap::new();
    for (id, icon) in icons {
      if let Ok(Some(bytes)) = icon.await.log_err() {
        next_icons.insert(id, bytes);
      }
    }
    let mut next_qp = HashMap::new();
    for (key, icon) in qp_icons {
      if let Ok(Some(bytes)) = icon.await {
        next_qp.insert(key, bytes);
      }
    }
    let mods = mods.await.log_err().unwrap_or_default();
    entity.update(cx, |meta, cx| {
      let mut changed = false;
      if !same(&meta.icons, &next_icons) {
        meta.icons = next_icons.into_iter().map(|(k, v)| (k, image(v))).collect();
        changed = true;
      }
      if !same(&meta.quick_play_icons, &next_qp) {
        meta.quick_play_icons = next_qp.into_iter().map(|(k, v)| (k, image(v))).collect();
        changed = true;
      }
      if meta.mods != mods {
        meta.mods = mods;
        changed = true;
      }
      if changed {
        cx.notify();
      }
    });
  });
  meta.update(cx, |meta, _| meta._reload = Some(task));
}

fn same<K: Eq + std::hash::Hash>(old: &HashMap<K, Arc<Image>>, new: &HashMap<K, Vec<u8>>) -> bool {
  old.len() == new.len()
    && new
      .iter()
      .all(|(k, v)| old.get(k).is_some_and(|i| i.bytes == *v))
}
