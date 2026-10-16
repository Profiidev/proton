//! `cargo run -p proton_skin_render --example viewer [skin.png] [cape.png]`
//! Drag the big model to orbit. `SKIN_ANIM=walk|run|wave|none` picks the animation.

use std::sync::Arc;

use gpui_kit::*;
use proton_skin_render::{Animation, MiniSkinViewer, ModelType, SkinTextures, SkinViewer};

struct Demo {
  viewer: Entity<SkinViewer>,
  textures: Arc<SkinTextures>,
}

impl Render for Demo {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    let tile = |flipped| {
      div()
        .rounded_lg()
        .bg(rgb(0x27272a))
        .child(MiniSkinViewer::new(self.textures.clone()).flipped(flipped))
    };
    div()
      .size_full()
      .flex()
      .gap_4()
      .p_4()
      .bg(rgb(0x09090b))
      .child(
        div()
          .rounded_lg()
          .bg(rgb(0x27272a))
          .child(self.viewer.clone()),
      )
      .child(
        div()
          .flex()
          .flex_col()
          .gap_4()
          .child(tile(false))
          .child(tile(true)),
      )
  }
}

fn main() {
  let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
  let mut args = std::env::args().skip(1);
  let skin = std::fs::read(args.next().unwrap_or(format!("{fixtures}/skin.png"))).unwrap();
  let cape = std::fs::read(args.next().unwrap_or(format!("{fixtures}/cape.png"))).unwrap();
  let textures = Arc::new(SkinTextures::from_png(&skin, Some(&cape)).unwrap());
  let animation = match std::env::var("SKIN_ANIM").as_deref() {
    Ok("walk") => Animation::Walking,
    Ok("run") => Animation::Running,
    Ok("wave") => Animation::Waving,
    Ok("none") => Animation::None,
    _ => Animation::Idle,
  };

  gpui_kit::application().run(move |cx| {
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(380.0), px(320.0)),
        cx,
      ))),
      app_id: Some("proton-skin-viewer".into()),
      ..Default::default()
    };
    cx.open_window(options, |_, cx| {
      cx.new(|cx| {
        let viewer = cx.new(|cx| {
          let mut v = SkinViewer::new(cx);
          v.set_skin(Some(textures.clone()), cx);
          v.set_model(ModelType::Classic, cx);
          v.set_animation(animation, cx);
          v
        });
        Demo { viewer, textures }
      })
    })
    .unwrap();
    cx.activate(true);
  });
}
