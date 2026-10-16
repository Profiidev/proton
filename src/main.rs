use std::sync::Arc;

use proton_ui::app::single_instance;

fn main() {
  proton_utils::dir::migrate();
  proton_utils::log::init();

  let listener = match single_instance::acquire(&single_instance::default_path()) {
    Ok(Some(listener)) => Some(listener),
    Ok(None) => {
      tracing::info!("Proton is already running, focused its window");
      return;
    }
    Err(e) => {
      tracing::error!("Single instance check failed, starting anyway: {e}");
      None
    }
  };

  let http = proton_reqwest::client().expect("Failed to create the HTTP client");
  gpui_kit::application()
    .with_assets(proton_components::assets::Assets)
    .with_http_client(Arc::new(http))
    .run(|cx| {
      gpui_kit::init(cx);
      proton_components::assets::load(cx).expect("Failed to load the theme");
      proton_config::init(cx);
      proton_versions::init(cx);
      proton_account::init(cx).expect("Failed to load the accounts");
      proton_profiles::init(cx);
      proton_content::init(cx);
      proton_ui::init(cx);
      let window = proton_ui::app::start(cx).expect("Failed to open the window");
      if let Some(listener) = listener {
        single_instance::focus_on_request(listener, window, cx);
      }
    });
}
