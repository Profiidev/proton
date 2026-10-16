//! Phase 2 pages, each area registers its routes with [`crate::shell::PageFactory`]

pub mod settings;

use gpui_kit::App;

/// Registers every page, call before the main window opens
pub fn register_all(cx: &mut App) {
  settings::register(cx);
}
