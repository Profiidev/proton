struct Uniforms {
  mvp: mat4x4<f32>,
  light_dir: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) normal: vec3<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) opaque: f32,
}

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) opaque: f32) -> VOut {
  var out: VOut;
  out.pos = u.mvp * vec4<f32>(pos, 1.0);
  out.normal = normal;
  out.uv = uv;
  out.opaque = opaque;
  return out;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
  let c = textureSample(tex, samp, in.uv);
  // ponytail: alpha test instead of sorted blending; skins are effectively 1-bit alpha
  if in.opaque < 0.5 && c.a < 0.5 {
    discard;
  }
  // skinview3d is almost flat (ambient 3/pi); a little directional light keeps depth readable
  let light = 0.85 + 0.2 * max(dot(normalize(in.normal), u.light_dir.xyz), 0.0);
  return vec4<f32>(c.rgb * light, 1.0);
}
