//! Phase 2 pages, registered with [`crate::shell::PageFactory`] before the window opens

pub mod instances;

use gpui_kit::App;

/// Registers every page. Needs the service globals (`proton_profiles::init`, ...).
pub fn register_all(cx: &mut App) {
  instances::register(cx);
}
