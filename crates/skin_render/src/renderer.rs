use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow};
use glam::{Mat4, Vec3};
use image::RgbaImage;
use wgpu::util::DeviceExt;

use crate::model::{Animation, BackEquipment, ModelType, SkinTextures, Vertex, build_mesh};

const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderParams {
  /// Output size in physical pixels.
  pub width: u32,
  pub height: u32,
  /// Camera orbit around the player in radians; 0/0 = front view, PI = back view.
  pub yaw: f32,
  pub pitch: f32,
  /// skinview3d zoom (the web viewers use 0.8).
  pub zoom: f32,
  pub model: ModelType,
  pub back: BackEquipment,
  pub animation: Animation,
  /// Animation progress in seconds.
  pub time: f32,
}

impl Default for RenderParams {
  fn default() -> Self {
    Self {
      width: 300,
      height: 400,
      yaw: 0.0,
      pitch: 0.0,
      zoom: 0.8,
      model: ModelType::Classic,
      back: BackEquipment::Cape,
      animation: Animation::None,
      time: 0.0,
    }
  }
}

/// Offscreen wgpu renderer. Creating one is slow, so share it (see [`Renderer::global`]).
pub struct Renderer {
  device: wgpu::Device,
  queue: wgpu::Queue,
  pipeline: wgpu::RenderPipeline,
  uniform_bgl: wgpu::BindGroupLayout,
  tex_bgl: wgpu::BindGroupLayout,
  sampler: wgpu::Sampler,
}

impl Renderer {
  /// Process-wide renderer, created on first use.
  pub fn global() -> Result<&'static Renderer> {
    static GLOBAL: OnceLock<Result<Renderer, String>> = OnceLock::new();
    GLOBAL
      .get_or_init(|| Renderer::new().map_err(|e| format!("{e:#}")))
      .as_ref()
      .map_err(|e| anyhow!("{e}"))
  }

  pub fn new() -> Result<Self> {
    let instance =
      wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
      pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .context("no wgpu adapter")?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
      label: Some("skin_render"),
      required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
      ..Default::default()
    }))?;

    let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
    let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: None,
      entries: &[wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
          ty: wgpu::BufferBindingType::Uniform,
          has_dynamic_offset: false,
          min_binding_size: None,
        },
        count: None,
      }],
    });
    let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      label: None,
      entries: &[
        wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
          },
          count: None,
        },
        wgpu::BindGroupLayoutEntry {
          binding: 1,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
          count: None,
        },
      ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: None,
      bind_group_layouts: &[Some(&uniform_bgl), Some(&tex_bgl)],
      immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("skin"),
      layout: Some(&layout),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: Some("vs"),
        compilation_options: Default::default(),
        buffers: &[wgpu::VertexBufferLayout {
          array_stride: size_of::<Vertex>() as u64,
          step_mode: wgpu::VertexStepMode::Vertex,
          attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32],
        }],
      },
      fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fs"),
        compilation_options: Default::default(),
        targets: &[Some(COLOR.into())],
      }),
      // overlay and cape are double sided; inner boxes are closed so culling buys nothing
      primitive: wgpu::PrimitiveState::default(),
      depth_stencil: Some(wgpu::DepthStencilState {
        format: DEPTH,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: Default::default(),
        bias: Default::default(),
      }),
      multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
      multiview_mask: None,
      cache: None,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default()); // nearest

    Ok(Self {
      device,
      queue,
      pipeline,
      uniform_bgl,
      tex_bgl,
      sampler,
    })
  }

  fn texture_bind_group(&self, img: &RgbaImage) -> wgpu::BindGroup {
    let tex = self.device.create_texture_with_data(
      &self.queue,
      &wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
          width: img.width(),
          height: img.height(),
          depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
      },
      wgpu::util::TextureDataOrder::LayerMajor,
      img.as_raw(),
    );
    let view = tex.create_view(&Default::default());
    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: None,
      layout: &self.tex_bgl,
      entries: &[
        wgpu::BindGroupEntry {
          binding: 0,
          resource: wgpu::BindingResource::TextureView(&view),
        },
        wgpu::BindGroupEntry {
          binding: 1,
          resource: wgpu::BindingResource::Sampler(&self.sampler),
        },
      ],
    })
  }

  /// Renders one frame. Output is straight (non-premultiplied) RGBA with a transparent background.
  pub fn render(&self, tex: &SkinTextures, p: &RenderParams) -> Result<RgbaImage> {
    let (w, h) = (p.width.max(1), p.height.max(1));
    let d = &self.device;

    // camera like skinview3d: fov 50, distance from zoom, orbiting the origin
    let fov = 50f32.to_radians();
    let dist = (4.5 + 16.5 / (fov / 2.0).tan() / p.zoom).clamp(10.0, 256.0);
    let pitch = p.pitch.clamp(-1.55, 1.55);
    let eye = dist
      * Vec3::new(
        p.yaw.sin() * pitch.cos(),
        pitch.sin(),
        p.yaw.cos() * pitch.cos(),
      );
    let mvp = Mat4::perspective_rh(fov, w as f32 / h as f32, 0.1, 2000.0)
      * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
    let mut uniforms = [0f32; 20];
    uniforms[..16].copy_from_slice(&mvp.to_cols_array());
    uniforms[16..19].copy_from_slice(&eye.normalize().to_array());
    let ubuf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: None,
      contents: bytemuck::cast_slice(&uniforms),
      usage: wgpu::BufferUsages::UNIFORM,
    });
    let ubg = d.create_bind_group(&wgpu::BindGroupDescriptor {
      label: None,
      layout: &self.uniform_bgl,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: ubuf.as_entire_binding(),
      }],
    });

    let mesh = build_mesh(p.model, p.back, p.animation, p.time, tex.cape.is_some());
    let mut draws = vec![(self.texture_bind_group(&tex.skin), mesh.skin)];
    if let Some(cape) = tex.cape.as_ref().filter(|_| !mesh.cape.is_empty()) {
      draws.push((self.texture_bind_group(cape), mesh.cape));
    }
    let draws: Vec<_> = draws
      .into_iter()
      .map(|(bg, verts)| {
        let buf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
          label: None,
          contents: bytemuck::cast_slice(&verts),
          usage: wgpu::BufferUsages::VERTEX,
        });
        (bg, buf, verts.len() as u32)
      })
      .collect();

    let target = |format, samples, usage| {
      d.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
          width: w,
          height: h,
          depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
      })
    };
    let msaa = target(COLOR, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let depth = target(DEPTH, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let resolve = target(
      COLOR,
      1,
      wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let (msaa_v, depth_v, resolve_v) = (
      msaa.create_view(&Default::default()),
      depth.create_view(&Default::default()),
      resolve.create_view(&Default::default()),
    );

    let padded =
      (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = d.create_buffer(&wgpu::BufferDescriptor {
      label: None,
      size: (padded * h) as u64,
      usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
      mapped_at_creation: false,
    });

    let mut enc = d.create_command_encoder(&Default::default());
    {
      let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &msaa_v,
          depth_slice: None,
          resolve_target: Some(&resolve_v),
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            store: wgpu::StoreOp::Discard,
          },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
          view: &depth_v,
          depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: wgpu::StoreOp::Discard,
          }),
          stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
      });
      pass.set_pipeline(&self.pipeline);
      pass.set_bind_group(0, &ubg, &[]);
      for (bg, buf, n) in &draws {
        pass.set_bind_group(1, bg, &[]);
        pass.set_vertex_buffer(0, buf.slice(..));
        pass.draw(0..*n, 0..1);
      }
    }
    enc.copy_texture_to_buffer(
      resolve.as_image_copy(),
      wgpu::TexelCopyBufferInfo {
        buffer: &readback,
        layout: wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(padded),
          rows_per_image: None,
        },
      },
      wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
      },
    );
    self.queue.submit([enc.finish()]);

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| _ = tx.send(r));
    d.poll(wgpu::PollType::wait_indefinitely())?;
    rx.recv()??;

    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in data.chunks(padded as usize) {
      out.extend_from_slice(&row[..(w * 4) as usize]);
    }
    drop(data);
    readback.unmap();
    // the MSAA resolve against a transparent clear yields premultiplied edges
    for px in out.as_chunks_mut::<4>().0 {
      if px[3] != 0 && px[3] != 255 {
        let a = px[3] as u32;
        for c in &mut px[..3] {
          *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
        }
      }
    }
    RgbaImage::from_raw(w, h, out).context("readback size")
  }
}
