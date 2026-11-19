//! Potok GPU (`wgpu`): wspólny shader dla podglądu w egui i eksportu offscreen.

use std::num::NonZeroU64;
use std::sync::Arc;

use eframe::egui_wgpu;
use eframe::wgpu;

use crate::document::{EditParams, PxRect, WHITE};
use crate::theme;

pub const SHADER_SRC: &str = include_str!("shader.wgsl");

/// Format tekstur źródłowych: wartości gamma bez konwersji (dokładny round-trip 8-bit przy eksporcie).
const SOURCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
const EXPORT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Bok kafelka przy renderowaniu eksportu (ogranicza rozmiar tekstury docelowej i bufora odczytu).
const EXPORT_TILE: u32 = 4096;

/// Konfiguracja `eframe`: prosimy o maksymalny rozmiar tekstury, jaki obsługuje karta
/// (domyślnie egui żąda 8192 px, a zdjęcia z aparatów potrafią być większe).
pub fn wgpu_configuration() -> egui_wgpu::WgpuConfiguration {
    wgpu_configuration_with(None)
}

/// Jak [`wgpu_configuration`], z wymuszonym zestawem backendów (przeglądarka: WebGPU / WebGL2).
pub fn wgpu_configuration_with(backends: Option<wgpu::Backends>) -> egui_wgpu::WgpuConfiguration {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    if let Some(b) = backends {
        setup.instance_descriptor.backends = b;
    }
    setup.device_descriptor = Arc::new(|adapter| {
        let base = if adapter.get_info().backend == wgpu::Backend::Gl {
            wgpu::Limits::downlevel_webgl2_defaults()
        } else {
            wgpu::Limits::default()
        };
        wgpu::DeviceDescriptor {
            label: Some("AsOne wgpu device"),
            required_limits: wgpu::Limits {
                max_texture_dimension_2d: adapter.limits().max_texture_dimension_2d.min(16384),
                ..base
            },
            ..Default::default()
        }
    });
    egui_wgpu::WgpuConfiguration {
        wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

/// Parametry shadera (układ zgodny ze `struct Params` w `shader.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub view: [f32; 4],
    pub sizes: [f32; 4],
    pub crop: [f32; 4],
    pub margin: [f32; 4],
    pub bg: [f32; 4],
    pub canvas: [f32; 4],
    pub flags: [f32; 4],
    pub extra: [f32; 4],
}

/// Geometria tego, co rysujemy: obraz wynikowy (kadr + marginesy) albo całe źródło (tryb kadrowania).
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub out_size: [f32; 2],
    pub crop_origin: [f32; 2],
    pub crop_size: [f32; 2],
    pub margin_lt: [f32; 2],
}

impl Layout {
    pub fn new(edits: &EditParams, src: [u32; 2], full_source: bool) -> Self {
        if full_source {
            let s = [src[0] as f32, src[1] as f32];
            return Self { out_size: s, crop_origin: [0.0; 2], crop_size: s, margin_lt: [0.0; 2] };
        }
        let c = edits.crop_px(src);
        let out = edits.output_size(src);
        let m = edits.margins;
        Self {
            out_size: [out[0] as f32, out[1] as f32],
            crop_origin: [c.x0 as f32, c.y0 as f32],
            crop_size: [c.width() as f32, c.height() as f32],
            margin_lt: [
                m[crate::document::LEFT] as f32,
                m[crate::document::TOP] as f32,
            ],
        }
    }

    /// Gdzie w układzie pikseli źródła leży lewy górny róg rysowanego obrazu.
    pub fn origin_in_src(&self) -> [f32; 2] {
        [
            self.crop_origin[0] - self.margin_lt[0],
            self.crop_origin[1] - self.margin_lt[1],
        ]
    }
}

pub enum RenderMode {
    Preview { checker_px: f32, target_srgb: bool, inspection: bool },
    Export,
}

/// Składa parametry shadera. `origin_px` – pozycja (0,0) obrazu w px bufora,
/// `zoom_px` – ile px bufora przypada na 1 px obrazu.
pub fn build_uniforms(
    edits: &EditParams,
    layout: &Layout,
    src: [u32; 2],
    mip_count: u32,
    origin_px: [f32; 2],
    zoom_px: f32,
    mode: RenderMode,
) -> Uniforms {
    let lod = if zoom_px >= 1.0 {
        0.0
    } else {
        (1.0 / zoom_px).log2().clamp(0.0, mip_count.saturating_sub(1) as f32)
    };
    let (preview, checker_px, srgb, bg, has_bg, inspection) = match mode {
        RenderMode::Preview { checker_px, target_srgb, inspection } => (
            true,
            checker_px,
            target_srgb,
            edits.bg_color.unwrap_or(WHITE),
            edits.bg_color.is_some(),
            inspection,
        ),
        // JPEG nie ma kanału alfa: bez wybranego tła spłaszczamy na biało.
        RenderMode::Export => (false, 8.0, false, edits.bg_color.unwrap_or(WHITE), true, false),
    };
    let nearest = preview && zoom_px >= 2.0;
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    Uniforms {
        view: [origin_px[0], origin_px[1], zoom_px, lod],
        sizes: [layout.out_size[0], layout.out_size[1], src[0] as f32, src[1] as f32],
        crop: [
            layout.crop_origin[0],
            layout.crop_origin[1],
            layout.crop_size[0],
            layout.crop_size[1],
        ],
        margin: [layout.margin_lt[0], layout.margin_lt[1], checker_px, flag(nearest)],
        bg,
        canvas: theme::p().canvas_rgba(),
        flags: [flag(has_bg), flag(edits.use_bg_removal), flag(preview), flag(srgb)],
        extra: [flag(inspection), 0.0, 0.0, 0.0],
    }
}

/// Wspólne zasoby GPU (urządzenie, pipeline'y, samplery). Bezpieczne do użycia z innych wątków.
pub struct GpuShared {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub preview_pipeline: wgpu::RenderPipeline,
    pub export_pipeline: wgpu::RenderPipeline,
    pub sampler_linear: wgpu::Sampler,
    pub target_is_srgb: bool,
    pub max_texture_dim: u32,
}

impl GpuShared {
    pub fn new(rs: &egui_wgpu::RenderState) -> Arc<Self> {
        Self::from_parts(rs.device.clone(), rs.queue.clone(), rs.target_format)
    }

    /// Bez egui (testy, przyszły tryb wsadowy na GPU). `target_format` – format podglądu.
    pub fn from_parts(device: wgpu::Device, queue: wgpu::Queue, target_format: wgpu::TextureFormat) -> Arc<Self> {

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("asone shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("asone bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(size_of::<Uniforms>() as u64),
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                sampler_entry(3),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("asone pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let make_pipeline = |label: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let preview_pipeline = make_pipeline("asone preview", target_format);
        let export_pipeline = make_pipeline("asone export", EXPORT_FORMAT);

        let sampler = |label, filter: wgpu::FilterMode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            })
        };
        let sampler_linear = sampler("asone linear", wgpu::FilterMode::Linear);

        let max_texture_dim = device.limits().max_texture_dimension_2d;
        Arc::new(Self {
            target_is_srgb: target_format.is_srgb(),
            device,
            queue,
            bind_group_layout,
            preview_pipeline,
            export_pipeline,
            sampler_linear,
            max_texture_dim,
        })
    }

    fn uniform_buffer(&self, label: &str) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind_group(
        &self,
        uniforms: &wgpu::Buffer,
        source_view: &wgpu::TextureView,
        mask_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("asone bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(source_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(mask_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler_linear) },
            ],
        })
    }
}

/// Jeden poziom mipmapy (RGBA8, ciasno upakowany).
pub struct MipLevel {
    pub size: [u32; 2],
    pub data: Vec<u8>,
}

pub fn mip_count(size: [u32; 2]) -> u32 {
    32 - size[0].max(size[1]).max(1).leading_zeros()
}

/// Zasoby GPU jednego dokumentu.
pub struct DocGpu {
    pub source_texture: wgpu::Texture,
    pub mask_texture: wgpu::Texture,
    pub source_view: wgpu::TextureView,
    pub mask_view: wgpu::TextureView,
    /// Parametry podglądu (osobny bufor na dokument, bo kilka zakładek może być widocznych naraz).
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub size: [u32; 2],
    pub mip_count: u32,
}

impl DocGpu {
    /// Tworzy teksturę źródłową (z gotowymi mipmapami) i maskę `R8Unorm`.
    pub fn new(shared: &GpuShared, levels: &[MipLevel], mask: &[u8]) -> Self {
        let size = levels[0].size;
        let extent = |s: [u32; 2]| wgpu::Extent3d { width: s[0], height: s[1], depth_or_array_layers: 1 };

        let source_texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("source_texture"),
            size: extent(size),
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SOURCE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (i, level) in levels.iter().enumerate() {
            shared.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &source_texture,
                    mip_level: i as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &level.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * level.size[0]),
                    rows_per_image: Some(level.size[1]),
                },
                extent(level.size),
            );
        }

        let mask_texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mask_texture"),
            size: extent(size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: MASK_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload_mask_region(
            &shared.queue,
            &mask_texture,
            mask,
            size,
            PxRect { x0: 0, y0: 0, x1: size[0], y1: size[1] },
        );

        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mask_view = mask_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let uniform_buffer = shared.uniform_buffer("preview uniforms");
        let bind_group = shared.bind_group(&uniform_buffer, &source_view, &mask_view);
        Self {
            source_texture,
            mask_texture,
            source_view,
            mask_view,
            uniform_buffer,
            bind_group,
            size,
            mip_count: levels.len() as u32,
        }
    }
}

/// Wysyła na GPU tylko zmieniony prostokąt maski (*dirty rect*).
pub fn upload_mask_region(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    mask: &[u8],
    size: [u32; 2],
    r: PxRect,
) {
    if r.is_empty() {
        return;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: r.x0, y: r.y0, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        mask,
        wgpu::TexelCopyBufferLayout {
            offset: r.y0 as u64 * size[0] as u64 + r.x0 as u64,
            bytes_per_row: Some(size[0]),
            rows_per_image: None,
        },
        wgpu::Extent3d { width: r.width(), height: r.height(), depth_or_array_layers: 1 },
    );
}

/// Rysowanie płótna wewnątrz egui (`egui_wgpu::Callback`).
pub struct CanvasCallback {
    pub shared: Arc<GpuShared>,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub uniforms: Uniforms,
}

impl egui_wgpu::CallbackTrait for CanvasCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        _resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.uniforms));
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _resources: &egui_wgpu::CallbackResources,
    ) {
        render_pass.set_pipeline(&self.shared.preview_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

/// Dane wejściowe renderowania offscreen jednego dokumentu.
pub struct OffscreenSource {
    pub source_view: wgpu::TextureView,
    pub mask_view: wgpu::TextureView,
    pub size: [u32; 2],
    pub mip_count: u32,
}

/// Renderuje obraz wynikowy (kadr + marginesy + maska + tło) w dokładnym rozmiarze
/// i zwraca czyste piksele RGB (bez żadnych metadanych). Duże obrazy są renderowane kafelkami.
pub fn render_offscreen(
    shared: &GpuShared,
    src: &OffscreenSource,
    edits: &EditParams,
) -> Result<([u32; 2], Vec<u8>), String> {
    render_offscreen_tiled(shared, src, edits, EXPORT_TILE.min(shared.max_texture_dim))
}

/// Jak [`render_offscreen`], z jawnym bokiem kafelka (testy składania kafelków).
pub fn render_offscreen_tiled(
    shared: &GpuShared,
    src: &OffscreenSource,
    edits: &EditParams,
    tile: u32,
) -> Result<([u32; 2], Vec<u8>), String> {
    let device = &shared.device;
    let queue = &shared.queue;
    let layout = Layout::new(edits, src.size, false);
    let [w, h] = edits.output_size(src.size);
    let tile = tile.clamp(1, shared.max_texture_dim);
    let (tex_w, tex_h) = (w.min(tile), h.min(tile));

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("export target"),
        size: wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: EXPORT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_row = |width: u32| (width * 4).div_ceil(align) * align;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("export readback"),
        size: padded_row(tex_w) as u64 * tex_h as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let uniform_buffer = shared.uniform_buffer("export uniforms");
    let bind_group = shared.bind_group(&uniform_buffer, &src.source_view, &src.mask_view);

    let mut rgb = vec![0u8; w as usize * h as usize * 3];
    for ty in (0..h).step_by(tile as usize) {
        for tx in (0..w).step_by(tile as usize) {
            let (cw, ch) = ((w - tx).min(tile), (h - ty).min(tile));
            let uniforms = build_uniforms(
                edits,
                &layout,
                src.size,
                src.mip_count,
                [-(tx as f32), -(ty as f32)],
                1.0,
                RenderMode::Export,
            );
            queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("export encoder"),
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("export pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&shared.export_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            let row = padded_row(cw);
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row),
                        rows_per_image: Some(ch),
                    },
                },
                wgpu::Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
            );
            queue.submit([encoder.finish()]);

            let slice = readback.slice(..row as u64 * ch as u64);
            let (done_tx, done_rx) = crossbeam_channel::bounded(1);
            slice.map_async(wgpu::MapMode::Read, move |r| {
                let _ = done_tx.send(r);
            });
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| format!("GPU: {e}"))?;
            done_rx
                .recv()
                .map_err(|_| "GPU: przerwano odczyt".to_string())?
                .map_err(|e| format!("GPU: odczyt bufora: {e}"))?;
            {
                let data = slice.get_mapped_range().map_err(|e| format!("GPU: mapowanie bufora: {e}"))?;
                for y in 0..ch as usize {
                    let src_row = &data[y * row as usize..][..cw as usize * 4];
                    let dst_start = ((ty as usize + y) * w as usize + tx as usize) * 3;
                    let dst_row = &mut rgb[dst_start..dst_start + cw as usize * 3];
                    for (d, s) in dst_row.as_chunks_mut::<3>().0.iter_mut().zip(src_row.as_chunks::<4>().0) {
                        d.copy_from_slice(&s[..3]);
                    }
                }
            }
            readback.unmap();
        }
    }
    Ok(([w, h], rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_wgsl_jest_poprawny_i_zgodny_z_uniformami() {
        let module = naga::front::wgsl::parse_str(SHADER_SRC).unwrap_or_else(|e| panic!("{}", e.emit_to_string(SHADER_SRC)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let params = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Params"))
            .map(|(_, t)| t.inner.clone())
            .expect("struct Params");
        match params {
            naga::TypeInner::Struct { span, .. } => assert_eq!(span as usize, size_of::<Uniforms>()),
            other => panic!("Params to nie struktura: {other:?}"),
        }
        for entry in ["vs_main", "fs_main"] {
            assert!(module.entry_points.iter().any(|e| e.name == entry), "brak {entry}");
        }
    }

    /// WebGL2 / OpenGL (przeglądarka, Crostini bez Vulkana): shader musi dać się przetłumaczyć
    /// na GLSL ES 3.0 – m.in. jedna tekstura nie może mieć dwóch samplerów.
    #[test]
    fn shader_tlumaczy_sie_na_glsl_es_dla_webgl2() {
        let module = naga::front::wgsl::parse_str(SHADER_SRC).unwrap();
        let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
            .validate(&module)
            .unwrap();
        for (stage, entry) in [(naga::ShaderStage::Vertex, "vs_main"), (naga::ShaderStage::Fragment, "fs_main")] {
            let options = naga::back::glsl::Options {
                version: naga::back::glsl::Version::Embedded { version: 300, is_webgl: true },
                ..Default::default()
            };
            let pipeline = naga::back::glsl::PipelineOptions {
                shader_stage: stage,
                entry_point: entry.into(),
                multiview: None,
            };
            let mut out = String::new();
            let mut writer = naga::back::glsl::Writer::new(
                &mut out,
                &module,
                &info,
                &options,
                &pipeline,
                naga::proc::BoundsCheckPolicies::default(),
            )
            .unwrap_or_else(|e| panic!("{entry}: {e:?}"));
            writer.write().unwrap_or_else(|e| panic!("{entry}: {e:?}"));
            assert!(out.contains("#version 300 es"));
        }
    }

    #[test]
    fn liczba_mipmap() {
        assert_eq!(mip_count([1, 1]), 1);
        assert_eq!(mip_count([2, 1]), 2);
        assert_eq!(mip_count([4000, 3000]), 12);
        assert_eq!(mip_count([4096, 10]), 13);
    }

    #[test]
    fn uniformy_wielkosc_i_lod() {
        // Układ WGSL: 8 × vec4<f32>.
        assert_eq!(size_of::<Uniforms>(), 8 * 16);
        let e = EditParams::default();
        let l = Layout::new(&e, [1000, 1000], false);
        let u = build_uniforms(&e, &l, [1000, 1000], 10, [0.0; 2], 0.25, RenderMode::Export);
        assert!((u.view[3] - 2.0).abs() < 1e-6);
        let u = build_uniforms(&e, &l, [1000, 1000], 10, [0.0; 2], 4.0, RenderMode::Export);
        assert_eq!(u.view[3], 0.0);
        // Eksport zawsze ma tło (JPEG bez alfy).
        assert_eq!(u.flags[0], 1.0);
    }
}
