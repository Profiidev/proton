//! Player geometry, texture normalisation and poses, ported 1:1 from skinview3d
//! (`model.js`, `animation.js`) and skinview-utils (`process.js`).

use std::f32::consts::PI;
use std::hash::{Hash, Hasher};

use anyhow::{Result, bail};
use glam::{EulerRot, Mat4, Quat, Vec3};
use image::{RgbaImage, imageops};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ModelType {
  #[default]
  Classic,
  Slim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BackEquipment {
  None,
  #[default]
  Cape,
  Elytra,
}

/// Same set as the web viewer's animation select.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Animation {
  None,
  #[default]
  Idle,
  Walking,
  Running,
  Waving,
}

/// Decoded, normalised textures. Build once per skin, render many frames.
#[derive(Clone)]
pub struct SkinTextures {
  /// Square skin (64x64 or HD multiple); legacy 64x32 skins are converted.
  pub skin: RgbaImage,
  /// Cape on a 64x32 (or HD multiple) canvas.
  pub cape: Option<RgbaImage>,
  /// Hash of the source bytes, used as cache key.
  pub hash: u64,
}

impl SkinTextures {
  pub fn from_png(skin: &[u8], cape: Option<&[u8]>) -> Result<Self> {
    let mut hasher = std::hash::DefaultHasher::new();
    (skin, cape).hash(&mut hasher);
    let skin = normalize_skin(image::load_from_memory(skin)?.into_rgba8())?;
    let cape = match cape {
      Some(c) => Some(normalize_cape(image::load_from_memory(c)?.into_rgba8())?),
      None => None,
    };
    Ok(Self {
      skin,
      cape,
      hash: hasher.finish(),
    })
  }
}

fn has_transparency(img: &RgbaImage, w: u32, h: u32) -> bool {
  (0..h).any(|y| (0..w).any(|x| img.get_pixel(x, y).0[3] != 255))
}

fn clear(img: &mut RgbaImage, s: u32, x: u32, y: u32, w: u32, h: u32) {
  for py in y * s..(y + h) * s {
    for px in x * s..(x + w) * s {
      img.put_pixel(px, py, image::Rgba([0; 4]));
    }
  }
}

pub fn normalize_skin(src: RgbaImage) -> Result<RgbaImage> {
  let (w, h) = src.dimensions();
  let legacy = match () {
    _ if w == h && w >= 64 => false,
    _ if w == 2 * h && w >= 64 => true,
    _ => bail!("bad skin size: {w}x{h}"),
  };
  let s = w / 64;
  let mut img = RgbaImage::new(w, w);
  imageops::replace(&mut img, &src, 0, 0);

  if legacy {
    // mirror right arm/leg into the 1.8 left arm/leg slots
    let copies: [(u32, u32, u32, u32, u32, u32); 12] = [
      (4, 16, 4, 4, 20, 48),
      (8, 16, 4, 4, 24, 48),
      (0, 20, 4, 12, 24, 52),
      (4, 20, 4, 12, 20, 52),
      (8, 20, 4, 12, 16, 52),
      (12, 20, 4, 12, 28, 52),
      (44, 16, 4, 4, 36, 48),
      (48, 16, 4, 4, 40, 48),
      (40, 20, 4, 12, 40, 52),
      (44, 20, 4, 12, 36, 52),
      (48, 20, 4, 12, 32, 52),
      (52, 20, 4, 12, 44, 52),
    ];
    for (sx, sy, cw, ch, dx, dy) in copies {
      let part = imageops::crop_imm(&img, sx * s, sy * s, cw * s, ch * s).to_image();
      imageops::replace(
        &mut img,
        &imageops::flip_horizontal(&part),
        (dx * s) as i64,
        (dy * s) as i64,
      );
    }
  }

  // old skins often have an opaque background in the overlay area
  if !has_transparency(&img, w, if legacy { w / 2 } else { w }) {
    clear(&mut img, s, 32, 0, 32, 16);
    if !legacy {
      for (x, y, cw, ch) in [
        (0, 32, 16, 16),
        (16, 32, 24, 16),
        (40, 32, 24, 16),
        (0, 48, 16, 16),
        (48, 48, 16, 16),
      ] {
        clear(&mut img, s, x, y, cw, ch);
      }
    }
  }
  Ok(img)
}

pub fn normalize_cape(src: RgbaImage) -> Result<RgbaImage> {
  let (w, h) = src.dimensions();
  let s = if w == 2 * h {
    w as f32 / 64.0
  } else if w * 17 == h * 22 {
    w as f32 / 22.0
  } else if w * 11 == h * 23 {
    w as f32 / 46.0
  } else {
    bail!("bad cape size: {w}x{h}");
  };
  let mut img = RgbaImage::new((64.0 * s) as u32, (32.0 * s) as u32);
  imageops::replace(&mut img, &src, 0, 0);
  Ok(img)
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
  pub pos: [f32; 3],
  pub normal: [f32; 3],
  pub uv: [f32; 2],
  /// 1.0 = inner layer (alpha forced opaque), 0.0 = alpha tested
  pub opaque: f32,
}

/// Vertices for one draw: `skin` uses the skin texture, `cape` the cape texture.
#[derive(Default)]
pub struct Mesh {
  pub skin: Vec<Vertex>,
  pub cape: Vec<Vertex>,
}

/// three.js object transform: T * R(euler XYZ) * S
fn tr(pos: [f32; 3], rot: [f32; 3]) -> Mat4 {
  Mat4::from_rotation_translation(
    Quat::from_euler(EulerRot::XYZ, rot[0], rot[1], rot[2]),
    pos.into(),
  )
}

/// Box of `size` centred at the origin of `m`, UV-mapped like skinview3d `setUVs`.
#[allow(clippy::too_many_arguments)]
fn cuboid(
  out: &mut Vec<Vertex>,
  m: Mat4,
  size: [f32; 3],
  uv: [f32; 5],
  tex: [f32; 2],
  opaque: bool,
) {
  let [u, v, w, h, d] = uv;
  let [hx, hy, hz] = size.map(|s| s / 2.0);
  // texture rects (x1, y1, x2, y2) in texels
  let top = [u + d, v, u + w + d, v + d];
  let bottom = [u + w + d, v, u + w * 2.0 + d, v + d];
  let left = [u, v + d, u + d, v + d + h];
  let front = [u + d, v + d, u + w + d, v + d + h];
  let right = [u + w + d, v + d, u + w + d * 2.0, v + h + d];
  let back = [u + w + d * 2.0, v + d, u + w * 2.0 + d * 2.0, v + h + d];
  // corners TL, TR, BL, BR as seen from outside (three.js BoxGeometry order)
  type Face = ([f32; 3], [[f32; 3]; 4], [f32; 4], bool);
  let faces: [Face; 6] = [
    (
      [1., 0., 0.],
      [[hx, hy, hz], [hx, hy, -hz], [hx, -hy, hz], [hx, -hy, -hz]],
      right,
      false,
    ),
    (
      [-1., 0., 0.],
      [
        [-hx, hy, -hz],
        [-hx, hy, hz],
        [-hx, -hy, -hz],
        [-hx, -hy, hz],
      ],
      left,
      false,
    ),
    (
      [0., 1., 0.],
      [[-hx, hy, -hz], [hx, hy, -hz], [-hx, hy, hz], [hx, hy, hz]],
      top,
      false,
    ),
    (
      [0., -1., 0.],
      [
        [-hx, -hy, hz],
        [hx, -hy, hz],
        [-hx, -hy, -hz],
        [hx, -hy, -hz],
      ],
      bottom,
      true,
    ),
    (
      [0., 0., 1.],
      [[-hx, hy, hz], [hx, hy, hz], [-hx, -hy, hz], [hx, -hy, hz]],
      front,
      false,
    ),
    (
      [0., 0., -1.],
      [
        [hx, hy, -hz],
        [-hx, hy, -hz],
        [hx, -hy, -hz],
        [-hx, -hy, -hz],
      ],
      back,
      false,
    ),
  ];
  let nm = m.inverse().transpose();
  for (n, c, [x1, y1, x2, y2], flip_v) in faces {
    let (y1, y2) = if flip_v { (y2, y1) } else { (y1, y2) };
    let uvs = [[x1, y1], [x2, y1], [x1, y2], [x2, y2]];
    let normal = nm.transform_vector3(n.into()).normalize().to_array();
    for i in [0, 2, 1, 2, 3, 1] {
      out.push(Vertex {
        pos: m.transform_point3(c[i].into()).to_array(),
        normal,
        uv: [uvs[i][0] / tex[0], uvs[i][1] / tex[1]],
        opaque: opaque as u8 as f32,
      });
    }
  }
}

#[derive(Default, Clone, Copy)]
struct Pose {
  head: [f32; 3],
  left_arm: [f32; 3],
  right_arm: [f32; 3],
  left_leg: [f32; 3],
  right_leg: [f32; 3],
  cape_x: f32,
  player_pos: [f32; 3],
  player_rot: [f32; 3],
}

const CAPE_DEFAULT_ANGLE: f32 = 10.8 * PI / 180.0;

fn pose(anim: Animation, time: f32) -> Pose {
  let mut p = Pose {
    cape_x: CAPE_DEFAULT_ANGLE,
    ..Default::default()
  };
  match anim {
    Animation::None => {}
    Animation::Idle => {
      let t = time * 2.0;
      p.left_arm[2] = t.cos() * 0.03 + PI * 0.02;
      p.right_arm[2] = (t + PI).cos() * 0.03 - PI * 0.02;
      p.cape_x = t.sin() * 0.01 + PI * 0.06;
    }
    Animation::Walking => {
      let t = time * 8.0;
      p.left_leg[0] = t.sin() * 0.5;
      p.right_leg[0] = (t + PI).sin() * 0.5;
      p.left_arm = [(t + PI).sin() * 0.5, 0.0, t.cos() * 0.03 + PI * 0.02];
      p.right_arm = [t.sin() * 0.5, 0.0, (t + PI).cos() * 0.03 - PI * 0.02];
      p.head = [(t / 5.0).sin() * 0.1, (t / 4.0).sin() * 0.2, 0.0];
      p.cape_x = (t / 1.5).sin() * 0.06 + PI * 0.06;
    }
    Animation::Running => {
      let t = time * 15.0 + PI * 0.5;
      p.left_leg[0] = (t + PI).cos() * 1.3;
      p.right_leg[0] = t.cos() * 1.3;
      p.left_arm = [t.cos() * 1.5, 0.0, t.cos() * 0.1 + PI * 0.1];
      p.right_arm = [(t + PI).cos() * 1.5, 0.0, (t + PI).cos() * 0.1 - PI * 0.1];
      p.player_pos = [t.cos() * 0.15, (t * 2.0).cos(), 0.0];
      p.player_rot[2] = (t + PI).cos() * 0.01;
      p.cape_x = (t * 2.0).sin() * 0.1 + PI * 0.3;
    }
    Animation::Waving => {
      let t = time * PI;
      // skinview3d really sets 180 rad here
      p.left_arm = [180.0, 0.0, t.sin() * 0.5];
    }
  }
  p
}

pub fn build_mesh(
  model: ModelType,
  back: BackEquipment,
  anim: Animation,
  time: f32,
  has_cape: bool,
) -> Mesh {
  let p = pose(anim, time);
  let mut mesh = Mesh::default();
  let player = tr(p.player_pos, p.player_rot);
  let skin = player * tr([0.0, 8.0, 0.0], [0.0; 3]);
  let st = [64.0, 64.0];
  let out = &mut mesh.skin;
  let arm_w = if model == ModelType::Slim { 3.0 } else { 4.0 };
  let arm_off = if model == ModelType::Slim { 0.5 } else { 1.0 };

  // (group transform, mesh offset, inner size, inner uv, outer size, outer uv)
  type Part = (Mat4, [f32; 3], [f32; 3], [f32; 2], [f32; 3], [f32; 2]);
  let parts: [Part; 6] = [
    (
      tr([0.0; 3], p.head),
      [0.0, 4.0, 0.0],
      [8.0, 8.0, 8.0],
      [0.0, 0.0],
      [9.0, 9.0, 9.0],
      [32.0, 0.0],
    ),
    (
      tr([0.0, -6.0, 0.0], [0.0; 3]),
      [0.0; 3],
      [8.0, 12.0, 4.0],
      [16.0, 16.0],
      [8.5, 12.5, 4.5],
      [16.0, 32.0],
    ),
    (
      tr([-5.0, -2.0, 0.0], p.right_arm),
      [-arm_off, -4.0, 0.0],
      [arm_w, 12.0, 4.0],
      [40.0, 16.0],
      [arm_w + 0.5, 12.5, 4.5],
      [40.0, 32.0],
    ),
    (
      tr([5.0, -2.0, 0.0], p.left_arm),
      [arm_off, -4.0, 0.0],
      [arm_w, 12.0, 4.0],
      [32.0, 48.0],
      [arm_w + 0.5, 12.5, 4.5],
      [48.0, 48.0],
    ),
    (
      tr([-1.9, -12.0, -0.1], p.right_leg),
      [0.0, -6.0, 0.0],
      [4.0, 12.0, 4.0],
      [0.0, 16.0],
      [4.5, 12.5, 4.5],
      [0.0, 32.0],
    ),
    (
      tr([1.9, -12.0, -0.1], p.left_leg),
      [0.0, -6.0, 0.0],
      [4.0, 12.0, 4.0],
      [16.0, 48.0],
      [4.5, 12.5, 4.5],
      [0.0, 48.0],
    ),
  ];
  for (group, off, inner, iuv, outer, ouv) in parts {
    let m = skin * group * tr(off, [0.0; 3]);
    // UV box dims are the inner size (the overlay is inflated, not re-mapped)
    let [w, h, d] = inner;
    cuboid(out, m, inner, [iuv[0], iuv[1], w, h, d], st, true);
    cuboid(out, m, outer, [ouv[0], ouv[1], w, h, d], st, false);
  }

  if has_cape {
    let ct = [64.0, 32.0];
    match back {
      BackEquipment::None => {}
      BackEquipment::Cape => {
        let m = player * tr([0.0, 8.0, -2.0], [p.cape_x, PI, 0.0]) * tr([0.0, -8.0, 0.5], [0.0; 3]);
        cuboid(
          &mut mesh.cape,
          m,
          [10.0, 16.0, 1.0],
          [0.0, 0.0, 10.0, 16.0, 1.0],
          ct,
          false,
        );
      }
      BackEquipment::Elytra => {
        let base = player * tr([0.0, 8.0, -2.0], [0.0; 3]);
        let wing = 0.261_799_4;
        for side in [1.0f32, -1.0] {
          let m = base
            * tr([5.0 * side, 0.0, 0.0], [wing, 0.01 * side, wing * side])
            * tr([-5.0 * side, -10.0, -1.0], [0.0; 3])
            * Mat4::from_scale(Vec3::new(side, 1.0, 1.0));
          cuboid(
            &mut mesh.cape,
            m,
            [12.0, 22.0, 4.0],
            [22.0, 0.0, 10.0, 20.0, 2.0],
            ct,
            false,
          );
        }
      }
    }
  }
  mesh
}
