//! Pages, one folder per area. `register_all` adds them to the shell's page registry.

pub mod discover;

use gpui_kit::App;

/// Registers every page and hook, call before the main window opens
pub fn register_all(cx: &mut App) {
  discover::register(cx);
  discover::register_hooks(cx);
}
