//! Pages, one module per area. Each registers itself with the shell's page factory.

use gpui_kit::App;

pub mod content;
pub mod home;
pub mod instances;
pub mod profile;
pub mod profiles;
pub mod settings;

/// Registers every page. Call after [`crate::init`] and the service inits, before the window opens.
pub fn register_all(cx: &mut App) {
  content::register(cx);
  home::register(cx);
  instances::register(cx);
  profile::register(cx);
  profiles::register(cx);
  settings::register(cx);
}
