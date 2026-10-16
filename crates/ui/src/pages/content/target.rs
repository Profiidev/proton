//! The profile Discover installs into (`content-target.svelte.ts`).

use gpui_kit::App;

pub fn set(cx: &mut App, profile: String) {
  crate::content::set_target(cx, Some(profile));
}

pub fn get(cx: &mut App) -> Option<String> {
  crate::content::target(cx).read(cx).clone()
}
