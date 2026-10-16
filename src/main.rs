use std::sync::Arc;

use gpui_kit::{
  App, AppContext, Bounds, Context, IntoElement, ParentElement, Render, Styled, TitlebarOptions,
  Window, WindowBounds, WindowDecorations, WindowOptions,
  component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
  },
  div, px, size,
};
use proton_components::card::{Card, card_content, card_description, card_header, card_title};

fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
    )
    .init();

  let http = proton_reqwest::client().expect("Failed to create the HTTP client");
  gpui_kit::application()
    .with_assets(proton_components::assets::Assets)
    .with_http_client(Arc::new(http))
    .run(|cx| {
      gpui_kit::init(cx);
      proton_components::assets::load(cx).expect("Failed to load the theme");
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
    app_id: Some("io.profidev.proton".into()),
    ..Default::default()
  };
  gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Demo)).expect("Failed to open the window");
}

/// ponytail: placeholder until proton_ui's shell lands
struct Demo;

impl Render for Demo {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .size_full()
      .flex()
      .items_center()
      .justify_center()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(
        Card::new()
          .w_96()
          .child(
            card_header()
              .child(card_title("Proton"))
              .child(card_description("gpui rewrite, theme preview", cx)),
          )
          .child(card_content().child("shadcn Zinc Blue Dark with Geist."))
          .footer(Button::new("ok").primary().label("Continue")),
      )
  }
}
