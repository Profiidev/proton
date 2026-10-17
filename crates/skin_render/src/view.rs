use std::{
  f32::consts::PI,
  hash::{Hash, Hasher},
  sync::Arc,
  time::Instant,
};

use gpui_kit::*;
use image::{Frame, RgbaImage};

use crate::{Animation, BackEquipment, ModelType, RenderParams, Renderer, SkinTextures};

/// gpui wants BGRA.
pub fn to_render_image(mut img: RgbaImage) -> RenderImage {
  for px in img.pixels_mut() {
    px.0.swap(0, 2);
  }
  RenderImage::new(vec![Frame::new(img)])
}

/// Interactive 3D viewer (the web `MainSkinViewer` canvas): drag to orbit, plays an animation.
/// Frames are rendered on the background executor, one in flight at a time, always with the
/// latest parameters; nothing is rendered while the view is idle.
pub struct SkinViewer {
  textures: Option<Arc<SkinTextures>>,
  model: ModelType,
  back: BackEquipment,
  animation: Animation,
  size: Size<Pixels>,
  yaw: f32,
  pitch: f32,
  drag: Option<Point<Pixels>>,
  start: Instant,
  frame: Option<Arc<RenderImage>>,
  rendered: Option<RenderParams>,
  in_flight: bool,
  failed: bool,
}

impl SkinViewer {
  /// Defaults match the web viewer: 212x280, idle animation, cape.
  pub fn new(cx: &mut Context<Self>) -> Self {
    cx.on_release(|this, cx| {
      if let Some(frame) = this.frame.take() {
        cx.drop_image(frame, None);
      }
    })
    .detach();
    Self {
      textures: None,
      model: ModelType::Classic,
      back: BackEquipment::Cape,
      animation: Animation::Idle,
      size: size(px(212.0), px(280.0)),
      yaw: 0.0,
      pitch: 0.0,
      drag: None,
      start: Instant::now(),
      frame: None,
      rendered: None,
      in_flight: false,
      failed: false,
    }
  }

  pub fn set_skin(&mut self, textures: Option<Arc<SkinTextures>>, cx: &mut Context<Self>) {
    self.textures = textures;
    self.rendered = None;
    self.failed = false;
    cx.notify();
  }

  pub fn set_model(&mut self, model: ModelType, cx: &mut Context<Self>) {
    self.model = model;
    cx.notify();
  }

  pub fn set_back(&mut self, back: BackEquipment, cx: &mut Context<Self>) {
    self.back = back;
    cx.notify();
  }

  pub fn set_animation(&mut self, animation: Animation, cx: &mut Context<Self>) {
    self.animation = animation;
    self.start = Instant::now();
    cx.notify();
  }

  pub fn set_size(&mut self, size: Size<Pixels>, cx: &mut Context<Self>) {
    self.size = size;
    cx.notify();
  }

  fn animating(&self) -> bool {
    self.animation != Animation::None
  }

  fn params(&self, scale: f32) -> RenderParams {
    // a constant clock when idle keeps params equal, so no frames are rendered
    let t = if self.animating() {
      self.start.elapsed().as_secs_f32()
    } else {
      0.0
    };
    RenderParams {
      width: (f32::from(self.size.width) * scale) as u32,
      height: (f32::from(self.size.height) * scale) as u32,
      yaw: self.yaw,
      pitch: self.pitch,
      model: self.model,
      back: self.back,
      animation: self.animation,
      time: t,
    }
  }

  fn render_frame(&mut self, params: RenderParams, window: &mut Window, cx: &mut Context<Self>) {
    let Some(textures) = self.textures.clone() else {
      return;
    };
    self.in_flight = true;
    self.rendered = Some(params);
    cx.spawn_in(window, async move |this, cx| {
      let result = cx
        .background_spawn(async move {
          Renderer::global()?
            .render(&textures, &params)
            .map(to_render_image)
        })
        .await;
      this
        .update_in(cx, |this, window, cx| {
          this.in_flight = false;
          match result {
            Ok(img) => {
              if let Some(old) = this.frame.replace(Arc::new(img)) {
                cx.drop_image(old, Some(window));
              }
            }
            Err(e) => {
              tracing::error!("skin render failed: {e:#}");
              this.failed = true;
            }
          }
          cx.notify();
        })
        .ok();
    })
    .detach();
  }
}

impl Render for SkinViewer {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let params = self.params(window.scale_factor());
    let stale = self.rendered.is_none_or(|r| r != params);
    if !self.in_flight && !self.failed && stale {
      self.render_frame(params, window, cx);
    }
    // keep the animation clock ticking even when a frame finishes between vsyncs
    if self.animating() && !self.failed {
      window.request_animation_frame();
    }

    div()
      .id("skin-viewer")
      .w(self.size.width)
      .h(self.size.height)
      .cursor_grab()
      .on_mouse_down(
        MouseButton::Left,
        cx.listener(|this, e: &MouseDownEvent, _, _| this.drag = Some(e.position)),
      )
      .on_mouse_up(
        MouseButton::Left,
        cx.listener(|this, _: &MouseUpEvent, _, _| this.drag = None),
      )
      .on_mouse_up_out(
        MouseButton::Left,
        cx.listener(|this, _: &MouseUpEvent, _, _| this.drag = None),
      )
      .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
        let Some(last) = this.drag else { return };
        if e.pressed_button != Some(MouseButton::Left) {
          this.drag = None;
          return;
        }
        // OrbitControls: a drag of the full element height is one full turn
        let h = f32::from(this.size.height).max(1.0);
        let d = e.position - last;
        this.yaw -= 2.0 * PI * f32::from(d.x) / h;
        this.pitch = (this.pitch + 2.0 * PI * f32::from(d.y) / h).clamp(-1.55, 1.55);
        this.drag = Some(e.position);
        cx.notify();
      }))
      .children(self.frame.clone().map(|f| img(f).size_full()))
  }
}

#[derive(Clone)]
struct MiniSource(Arc<SkinTextures>, RenderParams);

impl Hash for MiniSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    let p = &self.1;
    (
      self.0.hash,
      p.width,
      p.height,
      p.yaw.to_bits(),
      p.model,
      p.back,
    )
      .hash(state);
  }
}

enum MiniSkin {}

impl Asset for MiniSkin {
  type Source = MiniSource;
  type Output = Result<Arc<RenderImage>, ImageCacheError>;

  // async fn would capture the &mut App lifetime
  #[allow(clippy::manual_async_fn)]
  fn load(
    MiniSource(textures, params): MiniSource,
    _: &mut App,
  ) -> impl Future<Output = Self::Output> + Send + 'static {
    async move {
      Renderer::global()
        .and_then(|r| r.render(&textures, &params))
        .map(|img| Arc::new(to_render_image(img)))
        .map_err(|e| ImageCacheError::Other(Arc::new(e)))
    }
  }
}

/// Static front (or back, `flipped`) render, like the web `MiniSkinViewer` tiles.
/// Cached in gpui's asset cache per skin hash, model and size.
// ponytail: cache never evicts; call App::remove_asset if many skins churn
#[derive(IntoElement)]
pub struct MiniSkinViewer {
  textures: Arc<SkinTextures>,
  model: ModelType,
  flipped: bool,
  size: Size<Pixels>,
}

impl MiniSkinViewer {
  pub fn new(textures: Arc<SkinTextures>) -> Self {
    Self {
      textures,
      model: ModelType::Classic,
      flipped: false,
      size: size(px(100.0), px(110.0)),
    }
  }

  pub fn model(mut self, model: ModelType) -> Self {
    self.model = model;
    self
  }

  pub fn flipped(mut self, flipped: bool) -> Self {
    self.flipped = flipped;
    self
  }

  pub fn size(mut self, size: Size<Pixels>) -> Self {
    self.size = size;
    self
  }
}

impl RenderOnce for MiniSkinViewer {
  fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
    let scale = window.scale_factor();
    let source = MiniSource(
      self.textures,
      RenderParams {
        width: (f32::from(self.size.width) * scale) as u32,
        height: (f32::from(self.size.height) * scale) as u32,
        yaw: if self.flipped { PI } else { 0.0 },
        model: self.model,
        ..Default::default()
      },
    );
    img(ImageSource::Custom(Arc::new(move |window, cx| {
      window.use_asset::<MiniSkin>(&source, cx)
    })))
    .w(self.size.width)
    .h(self.size.height)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    collections::hash_map::DefaultHasher,
    f32::consts::PI,
    hash::{Hash, Hasher},
    io::Cursor,
    sync::Arc,
  };

  // explicit imports: the `gpui_kit::*` glob would shadow `#[test]`
  use gpui_kit::{
    self as gpui, AppContext, Entity, InputEvent, MouseMoveEvent, TestAppContext, WindowHandle,
    point, px, size, test::TestWindowExt,
  };
  use image::{Rgba, RgbaImage};

  use super::{MiniSkinViewer, MiniSource, SkinViewer, to_render_image};
  use crate::{Animation, BackEquipment, ModelType, RenderParams, SkinTextures};

  fn textures() -> Arc<SkinTextures> {
    let mut out = Cursor::new(Vec::new());
    RgbaImage::from_pixel(64, 64, Rgba([200, 100, 50, 255]))
      .write_to(&mut out, image::ImageFormat::Png)
      .unwrap();
    Arc::new(SkinTextures::from_png(&out.into_inner(), None).unwrap())
  }

  fn open(cx: &mut TestAppContext) -> (WindowHandle<SkinViewer>, Entity<SkinViewer>) {
    let handle = cx.add_window(|_, cx| SkinViewer::new(cx));
    let view = handle.entity(cx).unwrap();
    (handle, view)
  }

  fn frame(handle: WindowHandle<SkinViewer>, cx: &mut TestAppContext) {
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
      .unwrap();
  }

  #[test]
  fn render_image_is_bgra() {
    let img = RgbaImage::from_pixel(2, 1, Rgba([1, 2, 3, 4]));
    let out = to_render_image(img);
    assert_eq!(out.as_bytes(0).unwrap(), [3, 2, 1, 4, 3, 2, 1, 4]);
  }

  #[test]
  fn mini_cache_key_ignores_time_and_pose() {
    let hash = |source: &MiniSource| {
      let mut h = DefaultHasher::new();
      source.hash(&mut h);
      h.finish()
    };
    let tex = textures();
    let base = RenderParams::default();
    let key = hash(&MiniSource(tex.clone(), base));
    // tiles are static: time, pitch and animation never change the image
    let same = RenderParams {
      time: 9.0,
      pitch: 0.4,
      animation: Animation::Running,
      ..base
    };
    assert_eq!(hash(&MiniSource(tex.clone(), same)), key);
    for other in [
      RenderParams { yaw: PI, ..base },
      RenderParams {
        model: ModelType::Slim,
        ..base
      },
      RenderParams {
        width: base.width + 1,
        ..base
      },
      RenderParams {
        back: BackEquipment::Elytra,
        ..base
      },
    ] {
      assert_ne!(hash(&MiniSource(tex.clone(), other)), key);
    }
  }

  #[gpui::test]
  fn without_a_skin_nothing_renders(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    frame(handle, cx);
    cx.run_until_parked();
    view.read_with(cx, |v, _| {
      assert!(v.rendered.is_none() && v.frame.is_none() && !v.in_flight);
    });
  }

  #[gpui::test]
  fn renders_once_while_idle_and_again_on_change(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let (handle, view) = open(cx);
    view.update(cx, |v, cx| {
      v.set_animation(Animation::None, cx);
      v.set_skin(Some(textures()), cx);
    });
    frame(handle, cx);
    assert!(view.read_with(cx, |v, _| v.in_flight));
    cx.run_until_parked();
    let first = view.read_with(cx, |v, _| {
      assert!(!v.in_flight && !v.failed);
      v.rendered.unwrap()
    });
    assert!(view.read_with(cx, |v, _| v.frame.is_some()));
    assert_eq!(first.time, 0.0);
    // the test window has a scale factor of 2
    assert_eq!((first.width, first.height), (424, 560));

    // equal params: no second render
    frame(handle, cx);
    assert!(!view.read_with(cx, |v, _| v.in_flight));

    view.update(cx, |v, cx| v.set_model(ModelType::Slim, cx));
    frame(handle, cx);
    assert!(view.read_with(cx, |v, _| v.in_flight));
    cx.run_until_parked();
    assert_eq!(
      view.read_with(cx, |v, _| v.rendered.unwrap().model),
      ModelType::Slim
    );
  }

  #[gpui::test]
  fn a_failed_render_stops_until_a_new_skin(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    view.update(cx, |v, cx| {
      // idle would keep requesting frames, the test never parks
      v.animation = Animation::None;
      v.textures = Some(textures());
      v.failed = true;
      cx.notify();
    });
    frame(handle, cx);
    assert!(view.read_with(cx, |v, _| v.rendered.is_none()));

    view.update(cx, |v, cx| v.set_skin(Some(textures()), cx));
    assert!(!view.read_with(cx, |v, _| v.failed));
    cx.executor().allow_parking();
  }

  #[gpui::test]
  fn drag_orbits_and_clamps_the_pitch(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    view.update(cx, |v, cx| v.set_animation(Animation::None, cx));
    cx.update_window(handle.into(), |_, window, cx| {
      // half the 280px height is half a turn
      window.drag(point(px(150.), px(100.)), point(px(10.), px(100.)), cx);
    })
    .unwrap();
    view.read_with(cx, |v, _| {
      assert!((v.yaw - PI).abs() < 1e-4, "{}", v.yaw);
      assert_eq!(v.pitch, 0.0);
      assert!(v.drag.is_none(), "mouse up ends the drag");
    });

    cx.update_window(handle.into(), |_, window, cx| {
      window.drag(point(px(100.), px(10.)), point(px(100.), px(270.)), cx);
    })
    .unwrap();
    assert_eq!(view.read_with(cx, |v, _| v.pitch), 1.55);
    cx.update_window(handle.into(), |_, window, cx| {
      window.drag(point(px(100.), px(270.)), point(px(100.), px(0.)), cx);
      window.drag(point(px(100.), px(270.)), point(px(100.), px(0.)), cx);
    })
    .unwrap();
    assert_eq!(view.read_with(cx, |v, _| v.pitch), -1.55);
  }

  #[gpui::test]
  fn moving_without_the_button_drops_the_drag(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    view.update(cx, |v, _| v.drag = Some(point(px(10.), px(10.))));
    frame(handle, cx);
    cx.update_window(handle.into(), |_, window, cx| {
      // the button was released outside the window
      window.dispatch_event(
        MouseMoveEvent {
          position: point(px(100.), px(10.)),
          pressed_button: None,
          modifiers: Default::default(),
        }
        .to_platform_input(),
        cx,
      );
    })
    .unwrap();
    view.read_with(cx, |v, _| {
      assert!(v.drag.is_none());
      assert_eq!(v.yaw, 0.0);
    });
  }

  #[gpui::test]
  fn size_and_scale_set_the_render_size(cx: &mut TestAppContext) {
    let (_, view) = open(cx);
    view.update(cx, |v, cx| v.set_size(size(px(100.), px(50.)), cx));
    let p = view.read_with(cx, |v, _| v.params(2.0));
    assert_eq!((p.width, p.height), (200, 100));
  }

  #[test]
  fn mini_viewer_builder() {
    let tile = MiniSkinViewer::new(textures())
      .model(ModelType::Slim)
      .flipped(true)
      .size(size(px(10.), px(20.)));
    assert_eq!(tile.model, ModelType::Slim);
    assert!(tile.flipped);
    assert_eq!(tile.size, size(px(10.), px(20.)));
  }
}
