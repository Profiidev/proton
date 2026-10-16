//! Pages registered with the shell

pub mod content;

pub fn register_all(cx: &mut gpui_kit::App) {
  content::register(cx);
}
