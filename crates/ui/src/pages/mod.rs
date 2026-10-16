//! Pages, registered with [`crate::shell::PageFactory`] by [`register_all`]

pub mod profile;

use gpui_kit::App;

/// Registers every page. Call after [`crate::init`] and the service inits, before the window opens.
pub fn register_all(cx: &mut App) {
  profile::register(cx);
}
