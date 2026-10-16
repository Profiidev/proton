//! Pages, one module per area. Each registers itself with the shell's page factory.

use gpui_kit::App;

pub mod home;
pub mod profiles;

/// Registers every page; call once after `proton_ui::init`, before the window opens
pub fn register_all(cx: &mut App) {
  home::register(cx);
  profiles::register(cx);
}
