//! Snapshot tests. `UPDATE_SNAPSHOTS=1 cargo test -p proton_skin_render` regenerates the
//! synthetic fixtures and the reference renders.

use std::{f32::consts::PI, path::PathBuf, time::Instant};

use image::{Rgba, RgbaImage};
use proton_skin_render::{
  Animation, BackEquipment, ModelType, RenderParams, Renderer, SkinTextures,
};

fn fixtures() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn update() -> bool {
  std::env::var_os("UPDATE_SNAPSHOTS").is_some()
}

fn fill(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, c: [u8; 4]) {
  for py in y..y + h {
    for px in x..x + w {
      img.put_pixel(px, py, Rgba(c));
    }
  }
}

fn shade(c: [u8; 3], f: f32) -> [u8; 4] {
  let [r, g, b] = c.map(|v| (v as f32 * f).min(255.0) as u8);
  [r, g, b, 255]
}

/// Box-unwrapped part: top/bottom/right/front/left/back each get a distinct shade.
fn part(img: &mut RgbaImage, u: u32, v: u32, w: u32, h: u32, d: u32, c: [u8; 3]) {
  fill(img, u + d, v, w, d, shade(c, 1.3));
  fill(img, u + w + d, v, w, d, shade(c, 0.5));
  fill(img, u, v + d, d, h, shade(c, 0.8));
  fill(img, u + d, v + d, w, h, shade(c, 1.0));
  fill(img, u + w + d, v + d, d, h, shade(c, 0.9));
  fill(img, u + w + 2 * d, v + d, w, h, shade(c, 0.65));
}

/// Synthetic (non-Mojang) skin: distinct colours per part and face, eyes, partial overlays.
fn synthetic_skin() -> RgbaImage {
  let mut img = RgbaImage::new(64, 64);
  let skin = [224, 172, 128];
  part(&mut img, 0, 0, 8, 8, 8, skin); // head
  fill(&mut img, 8, 0, 8, 8, [90, 50, 20, 255]); // hair top
  fill(&mut img, 9, 12, 2, 1, [255, 255, 255, 255]); // eyes
  fill(&mut img, 13, 12, 2, 1, [255, 255, 255, 255]);
  fill(&mut img, 10, 12, 1, 1, [40, 60, 200, 255]);
  fill(&mut img, 13, 12, 1, 1, [40, 60, 200, 255]);
  fill(&mut img, 11, 14, 2, 1, [150, 70, 60, 255]); // mouth
  part(&mut img, 16, 16, 8, 12, 4, [40, 160, 170]); // body
  part(&mut img, 40, 16, 4, 12, 4, [40, 160, 170]); // right arm
  fill(&mut img, 44, 28, 4, 4, shade(skin, 1.0)); // right hand
  part(&mut img, 32, 48, 4, 12, 4, [200, 120, 40]); // left arm (orange: tells arms apart)
  part(&mut img, 0, 16, 4, 12, 4, [50, 60, 150]); // right leg
  part(&mut img, 16, 48, 4, 12, 4, [120, 50, 150]); // left leg (purple)
  // overlays: hat band, jacket stripe, sleeve cuffs, knee pads
  fill(&mut img, 32, 14, 32, 2, [230, 30, 30, 255]);
  fill(&mut img, 20, 40, 8, 2, [250, 220, 40, 255]);
  fill(&mut img, 40, 44, 16, 2, [250, 220, 40, 255]);
  fill(&mut img, 48, 60, 16, 2, [250, 220, 40, 255]);
  fill(&mut img, 4, 42, 4, 2, [20, 20, 20, 255]);
  fill(&mut img, 4, 58, 4, 2, [20, 20, 20, 255]);
  img
}

fn synthetic_cape() -> RgbaImage {
  let mut img = RgbaImage::new(64, 32);
  fill(&mut img, 0, 0, 22, 17, [160, 20, 30, 255]);
  fill(&mut img, 4, 4, 4, 4, [250, 210, 40, 255]); // emblem on the outside face
  fill(&mut img, 22, 0, 24, 22, [90, 90, 110, 255]); // elytra
  fill(&mut img, 34, 2, 6, 6, [250, 210, 40, 255]);
  img
}

fn textures() -> SkinTextures {
  let dir = fixtures();
  if update() {
    std::fs::create_dir_all(&dir).unwrap();
    synthetic_skin().save(dir.join("skin.png")).unwrap();
    synthetic_cape().save(dir.join("cape.png")).unwrap();
  }
  let skin = std::fs::read(dir.join("skin.png")).unwrap();
  let cape = std::fs::read(dir.join("cape.png")).unwrap();
  SkinTextures::from_png(&skin, Some(&cape)).unwrap()
}

fn renderer() -> Option<Renderer> {
  match Renderer::new() {
    Ok(r) => Some(r),
    Err(e) => {
      println!("skipping: no wgpu adapter ({e:#})");
      None
    }
  }
}

/// Mean absolute channel difference must stay below `tol` (GPU rasterisation differs slightly).
fn assert_snapshot(name: &str, img: &RgbaImage) {
  let path = fixtures().join(format!("{name}.png"));
  if update() || !path.exists() {
    img.save(&path).unwrap();
    return;
  }
  let reference = image::open(&path).unwrap().into_rgba8();
  assert_eq!(
    reference.dimensions(),
    img.dimensions(),
    "{name}: size changed"
  );
  let diff: u64 = reference
    .as_raw()
    .iter()
    .zip(img.as_raw())
    .map(|(a, b)| a.abs_diff(*b) as u64)
    .sum();
  let mean = diff as f64 / img.as_raw().len() as f64;
  assert!(
    mean < 1.5,
    "{name}: mean diff {mean:.3} vs {}",
    path.display()
  );
}

#[test]
fn snapshots() {
  let Some(r) = renderer() else { return };
  let tex = textures();
  let cases = [
    ("front", RenderParams::default()),
    (
      "back_cape",
      RenderParams {
        yaw: PI,
        ..Default::default()
      },
    ),
    (
      "slim_walk",
      RenderParams {
        yaw: 0.6,
        pitch: 0.3,
        model: ModelType::Slim,
        animation: Animation::Walking,
        time: 0.2,
        ..Default::default()
      },
    ),
    (
      "elytra",
      RenderParams {
        yaw: 2.5,
        back: BackEquipment::Elytra,
        ..Default::default()
      },
    ),
  ];
  for (name, p) in cases {
    assert_snapshot(name, &r.render(&tex, &p).unwrap());
  }
}

#[test]
fn legacy_skin_is_converted() {
  let modern = synthetic_skin();
  let legacy = image::imageops::crop_imm(&modern, 0, 0, 64, 32).to_image();
  let mut png = Vec::new();
  legacy
    .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
    .unwrap();
  let tex = SkinTextures::from_png(&png, None).unwrap();
  assert_eq!(tex.skin.dimensions(), (64, 64));
  // left leg front (20,52) is the mirrored right leg front (4,20)
  assert_eq!(tex.skin.get_pixel(23, 52), modern.get_pixel(4, 20));
}

#[test]
fn perf() {
  let Some(r) = renderer() else { return };
  let tex = textures();
  let p = RenderParams {
    animation: Animation::Walking,
    ..Default::default()
  };
  r.render(&tex, &p).unwrap();
  let n = 30;
  let start = Instant::now();
  for i in 0..n {
    r.render(
      &tex,
      &RenderParams {
        time: i as f32 * 0.016,
        ..p
      },
    )
    .unwrap();
  }
  println!(
    "300x400 frame: {:.2} ms",
    start.elapsed().as_secs_f64() * 1000.0 / n as f64
  );
}
