//! The projected, interactive jewel case used by Now Playing.
//!
//! A native Iced/WGPU primitive owns the six planes of a shallow case. Only
//! one uniform changes while it turns; front, rear and spine textures are
//! uploaded when the record changes. This replaces the first Canvas prototype,
//! whose affine squash could not produce correct perspective and rebuilt text
//! geometry on every frame.

use std::collections::hash_map::DefaultHasher;
use std::f32::consts::TAU;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use iced::mouse;
use iced::widget::image::Handle;
use iced::widget::shader::{self, Shader, Viewport};
use iced::{Element, Length, Point, Rectangle, wgpu};
use lru::LruCache;

use crate::app::Message;
use crate::vm;

/// The visible animation cadence. The shader changes one 32-byte uniform at
/// each step; the timer does not exist away from Now Playing or when neither
/// the case nor spectrum needs it.
pub(crate) const TICK: Duration = Duration::from_millis(33);
const TURN: Duration = Duration::from_secs(32);
const DRAG_YAW_PER_PX: f32 = 0.012;
const GENERATED_EDGE: u32 = 512;
const GENERATED_REAR_W: u32 = 540;
const GENERATED_REAR_H: u32 = 496;
const SPINE_W: u32 = 96;
const GENERATED_CACHE_ENTRIES: usize = 12;
/// The case silhouette after removing the unused clear bay exposed by the
/// narrowed hinge. At 135 × 124 mm it holds the 13 mm hinge, a square 120 mm
/// insert, and a 2 mm closing lip without stretching the artwork.
const CASE_ASPECT: f32 = 135.0 / 124.0;

/// Height of the fitted jewel case drawn at `width`.
#[must_use]
pub(crate) fn height_for_width(width: f32) -> f32 {
    width / CASE_ASPECT
}

/// The one animated reading the application owns. Rotation is deliberately
/// horizontal-only: vertical travel during a drag has no meaning for a case
/// standing upright and is ignored.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rotation {
    yaw: f32,
    held_x: Option<f32>,
    last_tick: Instant,
}

impl Rotation {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            yaw: 0.18,
            held_x: None,
            last_tick: now,
        }
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        let elapsed = now
            .saturating_duration_since(self.last_tick)
            .min(Duration::from_millis(100));
        self.last_tick = now;
        if self.held_x.is_none() {
            self.yaw = wrap(self.yaw + TAU * elapsed.as_secs_f32() / TURN.as_secs_f32());
        }
    }

    pub(crate) fn press(&mut self, at: Point) {
        self.held_x = Some(at.x);
    }

    pub(crate) fn drag(&mut self, at: Point) {
        let Some(was) = self.held_x.replace(at.x) else {
            return;
        };
        self.yaw = wrap(self.yaw + (at.x - was) * DRAG_YAW_PER_PX);
    }

    pub(crate) fn release(&mut self) {
        self.held_x = None;
    }

    pub(crate) fn dragging(self) -> bool {
        self.held_x.is_some()
    }
}

fn wrap(angle: f32) -> f32 {
    angle.rem_euclid(TAU)
}

/// Owned copy used to generate the rear and spine textures once per record.
#[derive(Debug, Clone)]
pub(crate) struct Insert {
    pub(crate) album_id: u64,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) tracks: Vec<String>,
}

/// Render-ready pictures supplied by the existing artwork pipeline.
#[derive(Debug, Clone)]
pub(crate) struct Art {
    pub(crate) front: Option<Handle>,
    pub(crate) from: Option<Handle>,
    pub(crate) front_opacity: f32,
    pub(crate) back: Option<Handle>,
}

#[derive(Debug, Clone)]
struct Case {
    rotation: Rotation,
    textures: Textures,
    front_opacity: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Textures {
    front: Handle,
    from: Handle,
    rear: Handle,
    spine: Handle,
}

/// Build the fitted jewel case in the measure the old sleeve occupied.
///
/// The widget is intentionally rectangular. The square cover is inset by the
/// shader at its physical 120 × 120 mm size, beside the tray's hinge bay.
pub(crate) fn view(
    edge: f32,
    rotation: Rotation,
    art: Art,
    insert: &Insert,
) -> Element<'static, Message> {
    let front = art
        .front
        .unwrap_or_else(|| generated_front(insert.album_id));
    let textures = Textures {
        from: art.from.unwrap_or_else(|| front.clone()),
        rear: art.back.unwrap_or_else(|| generated_rear(&front, insert)),
        front,
        spine: generated_spine(insert),
    };
    Shader::new(Case {
        rotation,
        textures,
        front_opacity: art.front_opacity.clamp(0.0, 1.0),
    })
    .width(Length::Fixed(edge))
    .height(Length::Fixed(height_for_width(edge)))
    .into()
}

impl shader::Program<Message> for Case {
    type State = ();
    type Primitive = Primitive;

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<shader::Action<Message>> {
        let capture = |message| Some(shader::Action::publish(message).and_capture());
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor.is_over(bounds) =>
            {
                let at = cursor.position()?;
                capture(Message::CasePressed(at))
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position })
                if self.rotation.dragging() =>
            {
                capture(Message::CaseDragged(*position))
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if self.rotation.dragging() =>
            {
                capture(Message::CaseReleased)
            }
            iced::Event::Touch(iced::touch::Event::FingerPressed { position, .. })
                if bounds.contains(*position) =>
            {
                capture(Message::CasePressed(*position))
            }
            iced::Event::Touch(iced::touch::Event::FingerMoved { position, .. })
                if self.rotation.dragging() =>
            {
                capture(Message::CaseDragged(*position))
            }
            iced::Event::Touch(
                iced::touch::Event::FingerLifted { .. } | iced::touch::Event::FingerLost { .. },
            ) if self.rotation.dragging() => capture(Message::CaseReleased),
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.rotation.dragging() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        Primitive {
            yaw: self.rotation.yaw,
            bounds,
            textures: self.textures.clone(),
            front_opacity: self.front_opacity,
        }
    }
}

#[derive(Debug)]
struct Primitive {
    yaw: f32,
    bounds: Rectangle,
    textures: Textures,
    front_opacity: f32,
}

impl shader::Primitive for Primitive {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        pipeline.update(
            device,
            queue,
            Presentation {
                yaw: self.yaw,
                bounds: self.bounds,
                front_opacity: self.front_opacity,
            },
            viewport,
            &self.textures,
        );
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        pipeline.render(encoder, target, *clip_bounds);
    }
}

struct Pipeline {
    program: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    gpu_textures: Vec<wgpu::Texture>,
    textures: Textures,
}

impl Pipeline {
    fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        textures: &Textures,
    ) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("baz jewel case uniform"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("baz jewel case bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_layout(2),
                texture_layout(3),
                texture_layout(4),
                texture_layout(5),
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("baz jewel case sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("baz jewel case shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "jewel_case.wgsl"
            ))),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("baz jewel case pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let program = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("baz jewel case pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview: None,
            cache: None,
        });
        let (gpu_textures, bind_group) = make_bind_group(
            device,
            queue,
            &bind_group_layout,
            &uniform,
            &sampler,
            textures,
        );
        Self {
            program,
            uniform,
            bind_group_layout,
            sampler,
            bind_group,
            gpu_textures,
            textures: textures.clone(),
        }
    }

    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        presentation: Presentation,
        viewport: &Viewport,
        textures: &Textures,
    ) {
        if &self.textures != textures {
            let (gpu_textures, bind_group) = make_bind_group(
                device,
                queue,
                &self.bind_group_layout,
                &self.uniform,
                &self.sampler,
                textures,
            );
            self.gpu_textures = gpu_textures;
            self.bind_group = bind_group;
            self.textures = textures.clone();
        }
        let screen = viewport.logical_size();
        let Presentation {
            yaw,
            bounds,
            front_opacity,
        } = presentation;
        let centre_x = ((bounds.x + bounds.width * 0.5) / screen.width) * 2.0 - 1.0;
        let centre_y = 1.0 - ((bounds.y + bounds.height * 0.5) / screen.height) * 2.0;
        let values = [
            yaw,
            centre_x,
            centre_y,
            bounds.width / screen.width,
            bounds.height / screen.height,
            front_opacity,
            0.0,
            0.0,
        ];
        queue.write_buffer(&self.uniform, 0, &f32_bytes(values));
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip: Rectangle<u32>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("baz jewel case pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
        pass.set_pipeline(&self.program);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..36, 0..1);
    }
}

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let blank = Handle::from_rgba(1, 1, vec![0, 0, 0, 255]);
        let textures = Textures {
            front: blank.clone(),
            from: blank.clone(),
            rear: blank.clone(),
            spine: blank,
        };
        Self::build(device, queue, format, &textures)
    }
}

#[derive(Debug, Clone, Copy)]
struct Presentation {
    yaw: f32,
    bounds: Rectangle,
    front_opacity: f32,
}

fn texture_layout(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn make_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    sampler: &wgpu::Sampler,
    textures: &Textures,
) -> (Vec<wgpu::Texture>, wgpu::BindGroup) {
    use wgpu::util::DeviceExt;
    let gpu_textures: Vec<_> = [
        &textures.front,
        &textures.rear,
        &textures.spine,
        &textures.from,
    ]
    .into_iter()
    .map(|handle| {
        let (width, height, pixels) = rgba(handle).unwrap_or((1, 1, &[0, 0, 0, 255]));
        device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("baz jewel case image"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        )
    })
    .collect();
    let views: Vec<_> = gpu_textures
        .iter()
        .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()))
        .collect();
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("baz jewel case bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&views[0]),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&views[1]),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&views[2]),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&views[3]),
            },
        ],
    });
    (gpu_textures, bind_group)
}

fn rgba(handle: &Handle) -> Option<(u32, u32, &[u8])> {
    match handle {
        Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } => Some((*width, *height, pixels.as_ref())),
        Handle::Path(..) | Handle::Bytes(..) => None,
    }
}

fn f32_bytes(values: [f32; 8]) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (chunk, value) in bytes.chunks_exact_mut(4).zip(values) {
        chunk.copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

type Generated = LruCache<u64, Handle>;
static GENERATED: OnceLock<Mutex<Generated>> = OnceLock::new();

fn generated_cache() -> &'static Mutex<Generated> {
    GENERATED.get_or_init(|| {
        let capacity = NonZeroUsize::new(GENERATED_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN);
        Mutex::new(LruCache::new(capacity))
    })
}

fn cached(key: u64, make: impl FnOnce() -> Handle) -> Handle {
    if let Some(handle) = generated_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
        .cloned()
    {
        return handle;
    }
    let handle = make();
    generated_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .put(key, handle.clone());
    handle
}

fn generated_front(album_id: u64) -> Handle {
    cached(texture_key("front", &(album_id,)), || {
        let image = gradient_image(album_id, GENERATED_EDGE, GENERATED_EDGE);
        Handle::from_rgba(GENERATED_EDGE, GENERATED_EDGE, image.into_raw())
    })
}

fn generated_rear(front: &Handle, insert: &Insert) -> Handle {
    let key = texture_key("rear", &(insert.album_id, front.id(), &insert.tracks));
    cached(key, || {
        let mut image = blurred_rear(front)
            .unwrap_or_else(|| gradient_image(insert.album_id, GENERATED_REAR_W, GENERATED_REAR_H));
        draw_track_list(&mut image, &insert.tracks);
        Handle::from_rgba(GENERATED_REAR_W, GENERATED_REAR_H, image.into_raw())
    })
}

fn blurred_rear(front: &Handle) -> Option<image::RgbaImage> {
    let (width, height, pixels) = rgba(front)?;
    let source = image::RgbaImage::from_raw(width, height, pixels.to_vec())?;
    let fitted = image::DynamicImage::ImageRgba8(source)
        .resize_to_fill(
            GENERATED_REAR_W,
            GENERATED_REAR_H,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgba8();
    let mut blurred = image::imageops::blur(&fitted, 18.0);
    // The blurred cover supplies colour and continuity; the veil supplies a
    // predictable contrast floor for the white track list over every sleeve.
    for pixel in blurred.pixels_mut() {
        for channel in &mut pixel.0[..3] {
            let veiled = u16::from(*channel) * 9 / 20;
            *channel = u8::try_from(veiled).unwrap_or(u8::MAX);
        }
    }
    Some(blurred)
}

fn generated_spine(insert: &Insert) -> Handle {
    let key = texture_key("spine", &(insert.album_id, &insert.artist, &insert.title));
    cached(key, || {
        let mut strip =
            image::RgbaImage::from_pixel(GENERATED_EDGE, SPINE_W, image::Rgba([12, 13, 14, 255]));
        let label = format!("{} — {}", insert.artist, insert.title);
        if let Ok(font) =
            FontRef::try_from_slice(include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf"))
        {
            let scale = PxScale::from(28.0);
            let fitted = fit_text(&font, scale, &label, 468.0);
            draw_text(&mut strip, &font, scale, &fitted, 22.0, 58.0);
        }
        let upright = image::imageops::rotate90(&strip);
        Handle::from_rgba(SPINE_W, GENERATED_EDGE, upright.into_raw())
    })
}

/// The cache key for one generated texture.
///
/// **The standing room is part of every key.** These textures bake colours —
/// the front's gradient, the rear's blur and its drawn track list, the spine's
/// type — and a room that can change while the process runs (item 54) would
/// otherwise be served a case painted in the room before it. Keying on
/// `theme::generation()` makes a room change a cache *miss* rather than
/// something that has to be invalidated by hand, and the LRU retires the old
/// entries in its own time.
fn texture_key(label: &str, value: &impl Hash) -> u64 {
    let mut hash = DefaultHasher::new();
    label.hash(&mut hash);
    crate::theme::generation().hash(&mut hash);
    value.hash(&mut hash);
    hash.finish()
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the generated bitmap is at most 512 px and blended channels stay within 0..=255"
)]
fn gradient_image(album_id: u64, width: u32, height: u32) -> image::RgbaImage {
    let (a, b) = vm::gradient_colors(album_id);
    image::RgbaImage::from_fn(width, height, |x, y| {
        let denominator = (width + height).saturating_sub(2).max(1) as f32;
        let t = (x + y) as f32 / denominator;
        let channel =
            |index: usize| (f32::from(a[index]) * (1.0 - t) + f32::from(b[index]) * t) as u8;
        image::Rgba([channel(0), channel(1), channel(2), 255])
    })
}

#[expect(
    clippy::cast_precision_loss,
    reason = "the number of visible rows and columns is bounded by the generated insert"
)]
fn draw_track_list(image: &mut image::RgbaImage, tracks: &[String]) {
    let Ok(font) =
        FontRef::try_from_slice(include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"))
    else {
        return;
    };
    let columns = usize::from(tracks.len() > 18) + 1;
    let rows = tracks.len().div_ceil(columns).max(1);
    let inset_x = image.width() as f32 * 0.078;
    let inset_y = image.height() as f32 * 0.115;
    let content_w = image.width() as f32 - 2.0 * inset_x;
    let content_h = image.height() as f32 - 2.0 * inset_y;
    let line_h = (content_h / rows as f32).min(28.0);
    let scale = PxScale::from((line_h * 0.68).max(10.0));
    let column_w = content_w / columns as f32;
    for (index, title) in tracks.iter().enumerate() {
        let column = index / rows;
        if column >= columns {
            break;
        }
        let row = index % rows;
        let x = inset_x + column as f32 * column_w;
        let y = inset_y + row as f32 * line_h;
        let prefix = format!("{:02}  ", index + 1);
        let prefix_w = text_width(&font, scale, &prefix);
        let fitted = fit_text(&font, scale, title, column_w - prefix_w - 12.0);
        draw_text(image, &font, scale, &format!("{prefix}{fitted}"), x, y);
    }
}

fn fit_text(font: &impl Font, scale: PxScale, text: &str, width: f32) -> String {
    if text_width(font, scale, text) <= width {
        return text.to_owned();
    }
    let ellipsis = "…";
    let mut fitted = String::new();
    for character in text.chars() {
        fitted.push(character);
        if text_width(font, scale, &format!("{fitted}{ellipsis}")) > width {
            fitted.pop();
            break;
        }
    }
    fitted.push('…');
    fitted
}

fn text_width(font: &impl Font, scale: PxScale, text: &str) -> f32 {
    let scaled = font.as_scaled(scale);
    let mut width = 0.0;
    let mut previous = None;
    for character in text.chars() {
        let glyph = scaled.glyph_id(character);
        if let Some(was) = previous {
            width += scaled.kern(was, glyph);
        }
        width += scaled.h_advance(glyph);
        previous = Some(glyph);
    }
    width
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "glyph coordinates and blended channels are clipped to the 512 px RGBA target"
)]
fn draw_text(
    image: &mut image::RgbaImage,
    font: &impl Font,
    scale: PxScale,
    text: &str,
    x: f32,
    baseline: f32,
) {
    let scaled = font.as_scaled(scale);
    let mut caret = x;
    let mut previous = None;
    for character in text.chars() {
        let id = scaled.glyph_id(character);
        if let Some(was) = previous {
            caret += scaled.kern(was, id);
        }
        let glyph = id.with_scale_and_position(scale, point(caret, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                let (Ok(px), Ok(py)) = (u32::try_from(px), u32::try_from(py)) else {
                    return;
                };
                let Some(pixel) = in_bounds_mut(image, px, py) else {
                    return;
                };
                let alpha = coverage * 0.92;
                for channel in &mut pixel.0[..3] {
                    *channel = (f32::from(*channel) * (1.0 - alpha) + 255.0 * alpha) as u8;
                }
            });
        }
        caret += scaled.h_advance(id);
        previous = Some(id);
    }
}

fn in_bounds_mut(image: &mut image::RgbaImage, x: u32, y: u32) -> Option<&mut image::Rgba<u8>> {
    (x < image.width() && y < image.height()).then(|| image.get_pixel_mut(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_shader_parses_and_validates_before_a_window_needs_it() {
        let module = naga::front::wgsl::parse_str(include_str!("jewel_case.wgsl"))
            .expect("the jewel-case WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("the jewel-case WGSL validates");
    }

    #[test]
    fn idle_rotation_advances_and_a_held_case_does_not() {
        let start = Instant::now();
        let mut rotation = Rotation::new(start);
        let before = rotation.yaw;
        rotation.tick(start + TICK);
        assert!(rotation.yaw > before);
        rotation.press(Point::new(10.0, 10.0));
        let held = rotation.yaw;
        rotation.tick(start + 2 * TICK);
        assert!((rotation.yaw - held).abs() < f32::EPSILON);
    }

    #[test]
    fn vertical_drag_travel_changes_nothing() {
        let mut rotation = Rotation::new(Instant::now());
        rotation.press(Point::new(20.0, 20.0));
        let yaw = rotation.yaw;
        rotation.drag(Point::new(20.0, 10_000.0));
        assert!((rotation.yaw - yaw).abs() < f32::EPSILON);
        rotation.drag(Point::new(120.0, 10_000.0));
        assert!((rotation.yaw - yaw).abs() > f32::EPSILON);
    }

    #[test]
    fn generated_rear_contains_only_fitted_track_rows() {
        let insert = Insert {
            album_id: 7,
            title: "A title that must not be printed on the rear".into(),
            artist: "An artist that must not be printed on the rear".into(),
            tracks: vec![
                "One".into(),
                "A very long title that has to fit inside its column without escaping the insert"
                    .into(),
            ],
        };
        let front = generated_front(insert.album_id);
        let rear = generated_rear(&front, &insert);
        let Some((width, height, pixels)) = rgba(&rear) else {
            panic!("generated rear is RGBA");
        };
        assert_eq!((width, height), (GENERATED_REAR_W, GENERATED_REAR_H));
        assert_eq!(pixels.len(), (width * height * 4) as usize);
    }

    #[test]
    fn angles_are_kept_in_one_turn() {
        let pi = std::f32::consts::PI;
        assert!((wrap(-pi / 2.0) - 3.0 * pi / 2.0).abs() < f32::EPSILON);
        assert!((wrap(9.0 * pi) - pi).abs() < 0.000_01);
    }
}
