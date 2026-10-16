//! The profile Discover installs into (`content-target.svelte.ts`).
// integrator: unify with 2d's TargetPicker state
// ponytail: not persisted across restarts (the old app used localStorage), save in settings if missed

use gpui_kit::{App, Global};

#[derive(Default)]
pub struct ContentTarget(pub Option<String>);

impl Global for ContentTarget {}

pub fn set(cx: &mut App, profile: String) {
  cx.set_global(ContentTarget(Some(profile)));
}

pub fn get(cx: &App) -> Option<String> {
  cx.try_global::<ContentTarget>().and_then(|t| t.0.clone())
}
