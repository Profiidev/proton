//! App shell (header, sidebar, router, downloads panel) and the page registry.
//!
//! Start-up: `proton_ui::init(cx)`, register pages with [`shell::PageFactory`],
//! then open a window whose root is [`shell::Shell`].

pub mod actions;
pub mod app;
pub mod content;
pub mod pages;
pub mod router;
pub mod shell;
pub mod updater;

use gpui_kit::App;

/// Sets up the router (at Home) and the shell globals
pub fn init(cx: &mut App) {
  router::init(cx, router::Route::Home);
  shell::init(cx);
  updater::init(cx);
}
