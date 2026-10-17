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
