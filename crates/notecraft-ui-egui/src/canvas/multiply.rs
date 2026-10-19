//! True multiply blending for the highlighter, as an `egui_wgpu` paint callback.
//!
//! egui's painter has no multiply blend, and a translucent mesh darkens wherever its own
//! triangles overlap. OneNote's highlighter multiplies over the page: rule lines and text show
//! through the color, and two strokes that cross darken each other, but one stroke never
//! darkens itself. The callback gets both with offscreen textures the size of the window:
//!
//! 1. The accumulation texture `A` is cleared to white.
//! 2. For each stroke, the scratch texture `S` is cleared to white and the stroke's mesh is
//!    drawn into it with a MIN blend. Min is idempotent, so triangles of one stroke that
//!    overlap change nothing, and the anti-aliased fringe still fades.
//! 3. `S` is multiplied into `A` (inside the stroke's pixel box).
//! 4. The callback's `paint` multiplies `A` into the frame inside egui's own render pass,
//!    which draws after the page text and images, so they show through the color.
//!
//! The app installs the pipelines once with [`install`] when it creates the window. Without
//! that (tests, a build with another backend), [`ready`] is false and the canvas keeps the
//! opaque pre-blend in `ink.rs`. Nothing stored changes: the stroke keeps its color and tool.
//!
//! Assumptions about egui's pass, which eframe's defaults satisfy: one sample per pixel and no
//! depth buffer. Colors multiply as stored in the frame (gamma space for the usual Unorm
//! surface), the same space the page is drawn in.

use egui_wgpu::wgpu;
use egui_wgpu::{CallbackResources, CallbackTrait, RenderState, ScreenDescriptor};
use wgpu::util::DeviceExt;

/// Most highlighter strokes the callback draws in a frame; a page with more uses the opaque
/// pre-blend, so a frame never does an unbounded amount of GPU work.
pub(super) const MAX_STROKES: usize = 128;

/// The format of the scratch and accumulation textures.
const LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Bytes of one vertex: position in clip space, color, and strength.
const VERTEX_BYTES: u64 = 6 * 4;

const SHADER: &str = "
struct In {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) strength: f32,
};
struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) strength: f32,
};
@vertex
fn vs_stroke(v: In) -> Out {
    var o: Out;
    o.pos = vec4<f32>(v.pos, 0.0, 1.0);
    o.color = v.color;
    o.strength = v.strength;
    return o;
}
@fragment
fn fs_stroke(i: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(mix(vec3<f32>(1.0), i.color, clamp(i.strength, 0.0, 1.0)), 1.0);
}

@group(0) @binding(0) var layer: texture_2d<f32>;
@vertex
fn vs_full(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle that covers the whole viewport.
    let x = f32(i32(index & 1u) * 4 - 1);
    let y = f32(i32(index >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}
@fragment
fn fs_layer(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(layer, vec2<i32>(frag.xy), 0);
}
";

/// One highlighter stroke, ready for the GPU: a mesh in screen points.
#[derive(Clone, Debug)]
pub(super) struct GpuStroke {
    /// Screen position in points, coverage 0 to 1 (the anti-aliased edge).
    pub vertices: Vec<([f32; 2], f32)>,
    pub indices: Vec<u32>,
    /// The stroke's color.
    pub rgb: [u8; 3],
    /// How far the color goes from white (1) toward the color (0) at full coverage.
    pub strength: f32,
}

/// The paint callback for one frame's highlighter strokes.
pub(super) struct HighlightCallback {
    pub strokes: Vec<GpuStroke>,
}

/// Whether [`install`] ran for this egui context.
pub(super) fn ready(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(ready_id()))
        .unwrap_or(false)
}

fn ready_id() -> egui::Id {
    egui::Id::new("notecraft.multiply.ready")
}

/// Build the pipelines for the window's surface and tell the canvas it can use them. Call it
/// once, when the app creates its window.
pub fn install(ctx: &egui::Context, state: &RenderState) {
    // The layers hold colors as stored in the frame; an sRGB surface would encode them a
    // second time, so it keeps the opaque pre-blend.
    if state.target_format.is_srgb() {
        return;
    }
    let gpu = Gpu::new(&state.device, state.target_format);
    state.renderer.write().callback_resources.insert(gpu);
    ctx.data_mut(|data| data.insert_temp(ready_id(), true));
}

/// Pipelines and per-size textures; stored in egui_wgpu's callback resources.
struct Gpu {
    stroke_pipeline: wgpu::RenderPipeline,
    fold_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
    layer_layout: wgpu::BindGroupLayout,
    targets: Option<Targets>,
    /// Whether this frame's prepare filled the accumulation texture.
    drawn: bool,
}

struct Targets {
    size: [u32; 2],
    scratch: wgpu::TextureView,
    accum: wgpu::TextureView,
    scratch_bind: wgpu::BindGroup,
    accum_bind: wgpu::BindGroup,
}

impl Gpu {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("notecraft_highlighter"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("notecraft_highlighter_layer"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        });
        let stroke_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("notecraft_highlighter_stroke_layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let layer_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("notecraft_highlighter_layer_layout"),
                bind_group_layouts: &[Some(&layer_layout)],
                immediate_size: 0,
            });
        let min = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Min,
        };
        // Result = source x destination; the destination's alpha stays.
        let multiply = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        };
        let stroke_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("notecraft_highlighter_stroke"),
            layout: Some(&stroke_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_stroke"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2, 1 => Float32x3, 2 => Float32
                    ],
                })],
            },
            primitive,
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_stroke"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: LAYER_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: min,
                        alpha: min,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let layer_pipeline = |label: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layer_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_full"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive,
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_layer"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(multiply),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            stroke_pipeline,
            fold_pipeline: layer_pipeline("notecraft_highlighter_fold", LAYER_FORMAT),
            present_pipeline: layer_pipeline("notecraft_highlighter_present", target_format),
            layer_layout,
            targets: None,
            drawn: false,
        }
    }

    /// Make the textures for a window of `size` pixels, again when the size changes.
    fn ensure_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        let stale = self.targets.as_ref().is_none_or(|t| t.size != size);
        if stale {
            let make = |label: &str| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0].max(1),
                        height: size[1].max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: LAYER_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            };
            let bind = |view: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("notecraft_highlighter_layer"),
                    layout: &self.layer_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    }],
                })
            };
            let scratch = make("notecraft_highlighter_scratch");
            let accum = make("notecraft_highlighter_accum");
            let scratch_bind = bind(&scratch);
            let accum_bind = bind(&accum);
            self.targets = Some(Targets {
                size,
                scratch,
                accum,
                scratch_bind,
                accum_bind,
            });
        }
    }
}

fn white() -> wgpu::Color {
    wgpu::Color::WHITE
}

/// A render pass over `view`, cleared to white or loaded.
fn begin<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    clear: bool,
) -> wgpu::RenderPass<'a> {
    let load = if clear {
        wgpu::LoadOp::Clear(white())
    } else {
        wgpu::LoadOp::Load
    };
    begin_load(encoder, view, load)
}

fn begin_load<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("notecraft_highlighter"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// A stroke as vertex bytes in clip space for a window of `size` pixels, and its pixel box
/// `[x0, y0, x1, y1]`.
fn stroke_bytes(stroke: &GpuStroke, scale: f32, size: [u32; 2]) -> (Vec<u8>, [u32; 4]) {
    let width = size[0].max(1) as f32;
    let height = size[1].max(1) as f32;
    let mut bytes = Vec::with_capacity(stroke.vertices.len() * VERTEX_BYTES as usize);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let [r, g, b] = stroke.rgb.map(|c| f32::from(c) / 255.0);
    for ([x, y], coverage) in &stroke.vertices {
        let (px, py) = (x * scale, y * scale);
        x0 = x0.min(px);
        y0 = y0.min(py);
        x1 = x1.max(px);
        y1 = y1.max(py);
        let values = [
            px / width * 2.0 - 1.0,
            1.0 - py / height * 2.0,
            r,
            g,
            b,
            coverage.clamp(0.0, 1.0) * stroke.strength,
        ];
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    let clamp = |v: f32, max: u32| (v.max(0.0) as u32).min(max);
    let boxed = [
        clamp((x0 - 2.0).floor(), size[0]),
        clamp((y0 - 2.0).floor(), size[1]),
        clamp((x1 + 2.0).ceil(), size[0]),
        clamp((y1 + 2.0).ceil(), size[1]),
    ];
    (bytes, boxed)
}

impl CallbackTrait for HighlightCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = resources.get_mut::<Gpu>() else {
            return Vec::new();
        };
        gpu.drawn = false;
        let size = screen.size_in_pixels;
        if size[0] == 0 || size[1] == 0 || self.strokes.is_empty() {
            return Vec::new();
        }
        gpu.ensure_targets(device, size);
        let Some(targets) = gpu.targets.as_ref() else {
            return Vec::new();
        };
        // The accumulation texture starts white.
        drop(begin(encoder, &targets.accum, true));
        for stroke in self.strokes.iter().take(MAX_STROKES) {
            if stroke.indices.len() < 3 || stroke.vertices.is_empty() {
                continue;
            }
            let (bytes, [x0, y0, x1, y1]) = stroke_bytes(stroke, screen.pixels_per_point, size);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("notecraft_highlighter_vertices"),
                contents: &bytes,
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_bytes: Vec<u8> = stroke
                .indices
                .iter()
                .flat_map(|index| index.to_le_bytes())
                .collect();
            let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("notecraft_highlighter_indices"),
                contents: &index_bytes,
                usage: wgpu::BufferUsages::INDEX,
            });
            {
                let mut pass = begin(encoder, &targets.scratch, true);
                pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
                pass.set_pipeline(&gpu.stroke_pipeline);
                pass.set_vertex_buffer(0, vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..stroke.indices.len() as u32, 0, 0..1);
            }
            {
                let mut pass = begin(encoder, &targets.accum, false);
                pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
                pass.set_pipeline(&gpu.fold_pipeline);
                pass.set_bind_group(0, &targets.scratch_bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        gpu.drawn = true;
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(gpu) = resources.get::<Gpu>() else {
            return;
        };
        let Some(targets) = gpu.targets.as_ref().filter(|_| gpu.drawn) else {
            return;
        };
        pass.set_pipeline(&gpu.present_pipeline);
        pass.set_bind_group(0, &targets.accum_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::*;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        let mut future = pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
                return value;
            }
            std::thread::yield_now();
        }
    }

    /// A square as a stroke: two triangles twice over, like the overlapping triangles of a
    /// real ribbon.
    fn square(min: f32, max: f32, rgb: [u8; 3]) -> GpuStroke {
        let corner = |x: f32, y: f32| ([x, y], 1.0);
        GpuStroke {
            vertices: vec![
                corner(min, min),
                corner(max, min),
                corner(max, max),
                corner(min, max),
            ],
            indices: vec![0, 1, 2, 0, 2, 3, 0, 1, 2, 0, 2, 3],
            rgb,
            strength: 1.0,
        }
    }

    #[test]
    fn without_install_the_callback_is_not_ready() {
        let ctx = egui::Context::default();
        assert!(!ready(&ctx));
    }

    #[test]
    fn vertices_become_clip_space_and_a_pixel_box() {
        let stroke = square(10.0, 20.0, [255, 0, 0]);
        let (bytes, boxed) = stroke_bytes(&stroke, 2.0, [100, 50]);
        assert_eq!(bytes.len(), 4 * VERTEX_BYTES as usize);
        let value = |i: usize| {
            let at = i * 4;
            f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        // The first corner is (10, 10) points = (20, 20) pixels of a 100 x 50 window.
        assert!((value(0) - (20.0 / 100.0 * 2.0 - 1.0)).abs() < 1e-6);
        assert!((value(1) - (1.0 - 20.0 / 50.0 * 2.0)).abs() < 1e-6);
        assert_eq!(value(2), 1.0);
        assert_eq!(boxed, [18, 18, 42, 42]);
        // A stroke off the window is clamped to its edge.
        let (_, boxed) = stroke_bytes(&square(-500.0, -400.0, [0, 0, 0]), 1.0, [100, 50]);
        assert_eq!(boxed, [0, 0, 0, 0]);
    }

    /// Run the callback on a real device, when there is one, and read back the frame.
    fn render(strokes: Vec<GpuStroke>) -> Option<Vec<[u8; 4]>> {
        const SIZE: u32 = 64;
        let instance = wgpu::Instance::default();
        let adapter =
            block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut resources = CallbackResources::new();
        resources.insert(Gpu::new(&device, format));
        let extent = wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        };
        let frame = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = frame.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        // The page: light gray.
        drop(begin_load(
            &mut encoder,
            &view,
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.8,
                g: 0.8,
                b: 0.8,
                a: 1.0,
            }),
        ));
        let callback = HighlightCallback { strokes };
        let screen = ScreenDescriptor {
            size_in_pixels: [SIZE, SIZE],
            pixels_per_point: 1.0,
        };
        callback.prepare(&device, &queue, &screen, &mut encoder, &mut resources);
        {
            let mut pass = begin_load(&mut encoder, &view, wgpu::LoadOp::Load).forget_lifetime();
            let whole = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 64.0));
            let info = egui::PaintCallbackInfo {
                viewport: whole,
                clip_rect: whole,
                pixels_per_point: 1.0,
                screen_size_px: [SIZE, SIZE],
            };
            callback.paint(info, &mut pass, &resources);
        }
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(SIZE * SIZE * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            frame.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIZE * 4),
                    rows_per_image: Some(SIZE),
                },
            },
            extent,
        );
        queue.submit([encoder.finish()]);
        readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let data = readback.slice(..).get_mapped_range().ok()?;
        Some(
            data.as_chunks::<4>()
                .0
                .iter()
                .map(|p| [p[0], p[1], p[2], p[3]])
                .collect(),
        )
    }

    #[test]
    fn strokes_multiply_over_the_page_and_each_other_but_not_themselves() {
        let Some(pixels) = render(vec![
            square(8.0, 40.0, [128, 255, 255]),
            square(24.0, 56.0, [128, 255, 255]),
        ]) else {
            // No graphics adapter here (a headless runner): the callback cannot run.
            return;
        };
        let at = |x: usize, y: usize| pixels[y * 64 + x];
        // Bare page: light gray (0.8 of 255 = 204).
        assert_eq!(at(2, 2), [204, 204, 204, 255]);
        // One stroke: red x 0.5 even though its triangles overlap.
        let one = at(12, 12);
        assert!((i32::from(one[0]) - 102).abs() <= 2, "{one:?}");
        assert_eq!(&one[1..], &[204, 204, 255]);
        // Where two strokes cross, they darken each other.
        let both = at(32, 32);
        assert!((i32::from(both[0]) - 51).abs() <= 2, "{both:?}");
    }
}
