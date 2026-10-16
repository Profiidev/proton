use std::sync::Arc;

use gpui_kit::{
  App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, px,
  size,
};
use proton_ui::shell::Shell;

fn main() {
  proton_utils::dir::migrate();
  proton_utils::log::init();

  let http = proton_reqwest::client().expect("Failed to create the HTTP client");
  gpui_kit::application()
    .with_assets(proton_components::assets::Assets)
    .with_http_client(Arc::new(http))
    .run(|cx| {
      gpui_kit::init(cx);
      proton_components::assets::load(cx).expect("Failed to load the theme");
      proton_config::init(cx);
      proton_ui::init(cx);
      open_main_window(cx);
    });
}

fn open_main_window(cx: &mut App) {
  let options = WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
      None,
      size(px(1300.), px(800.)),
      cx,
    ))),
    titlebar: Some(TitlebarOptions {
      title: Some("Proton".into()),
      appears_transparent: true,
      traffic_light_position: None,
    }),
    window_decorations: Some(WindowDecorations::Client),
    window_min_size: Some(size(px(600.), px(400.))),
    app_id: Some("io.profidev.proton".into()),
    ..Default::default()
  };
  gpui_kit::open_window(options, cx, |window, cx| {
    cx.new(|cx| Shell::new(window, cx))
  })
  .expect("Failed to open the window");
}
