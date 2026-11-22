//! FerroChart inside an egui app: the chart shares
//! eframe's wgpu device, renders offscreen each time it changes, and an
//! egui-wgpu paint callback draws that texture into egui's render pass.
//! egui input over the chart is converted to `InputEvent`s. A side panel
//! drives the chart (live feed, indicators, drawing tools, scale and
//! crosshair modes) and shows its events.
//!
//! ```text
//! cargo run -p example-egui --release [-- --screenshot <file.png>]
//! ```
//!
//! With `--screenshot`, the app adds a trend line, waits for a few frames,
//! saves egui's own screenshot of the window (chart included, as composed
//! by the paint callback) and quits.
//!
//! The chart and the renderer are not `Send` (custom GPU series need not
//! be), so they stay in the app and render during `ui`. The paint callback
//! only carries the blit pipeline and the texture's bind group. Rendering
//! during `ui` submits before egui's own commands, so the texture is ready
//! when egui's pass samples it.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;
use eframe::egui_wgpu::{self, wgpu};
use ferrochart_core::chart::{ChartEvent, ClickTarget, SeriesData, SeriesOptions};
use ferrochart_core::data::{BarUpdate, IndicatorId, SeriesId};
use ferrochart_core::drawing::{Anchor, DrawingData};
use ferrochart_core::frame::{Invalidation, SeriesKind};
use ferrochart_core::geom::{Css, PointCss, SizeCss};
use ferrochart_core::interaction::{
    Button, CrosshairMode, CursorIcon, GesturePhase, InputEvent, Key, Modifiers, WheelDelta,
};
use ferrochart_core::plugin::Placement;
use ferrochart_core::price_scale::{PriceScaleMode, ScaleSide};
use ferrochart_core::render::Renderer;
use ferrochart_core::testing::random_walk;
use ferrochart_core::theme::Theme;
use ferrochart_core::{Chart, ChartOptions};
use ferrochart_indicators::{self as ind, Source};
use ferrochart_render_wgpu::{GpuContext, WgpuRenderer};

const START: i64 = 1_704_067_200_000; // 2024-01-01T00:00:00Z
const MINUTE: i64 = 60_000;
const TICK_MS: f64 = 50.0;
const TICKS_PER_BAR: u32 = 20; // a new (simulated) minute every second
const LOG_LINES: usize = 10;
/// Frames before the screenshot (the chart and egui settle).
const SCREENSHOT_AFTER_FRAMES: u32 = 30;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let screenshot = match (args.next().as_deref(), args.next()) {
        (None, _) => None,
        (Some("--screenshot"), Some(path)) => Some(PathBuf::from(path)),
        _ => {
            eprintln!("usage: example-egui [--screenshot <file.png>]");
            return ExitCode::FAILURE;
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FerroChart in egui")
            .with_inner_size([1280.0, 760.0]),
        // The blit pipeline is built for egui's pass without MSAA or depth.
        multisampling: 0,
        depth_buffer: 0,
        ..eframe::NativeOptions::default()
    };
    match eframe::run_native(
        "FerroChart in egui",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc, screenshot)?))),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Draws the chart's offscreen texture over the callback's viewport.
#[derive(Clone)]
struct ChartPaint {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
}

impl egui_wgpu::CallbackTrait for ChartPaint {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _resources: &egui_wgpu::CallbackResources,
    ) {
        // egui has set the viewport to the callback's rect and the scissor
        // to its clip rect; one triangle covers the viewport.
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

const BLIT_WGSL: &str = r"
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VOut;
    o.pos = vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    o.uv = uv;
    return o;
}

// The chart's pixels are sRGB-encoded (like the CPU backend's).
@fragment
fn fs_encoded(v: VOut) -> @location(0) vec4f {
    return textureSample(tex, samp, v.uv);
}

// For an sRGB target, which encodes again on write.
@fragment
fn fs_linear(v: VOut) -> @location(0) vec4f {
    let c = textureSample(tex, samp, v.uv);
    let lo = c.rgb / 12.92;
    let hi = pow((c.rgb + 0.055) / 1.055, vec3f(2.4));
    return vec4f(select(hi, lo, c.rgb <= vec3f(0.04045)), c.a);
}
";

struct Blit {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The bind group of the texture it was made for (the renderer replaces
    /// its texture when the size changes).
    bound: Option<(wgpu::Texture, wgpu::BindGroup)>,
}

impl Blit {
    fn new(device: &wgpu::Device, target: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("chart blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("chart blit"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("chart blit"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("chart blit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(if target.is_srgb() {
                    "fs_linear"
                } else {
                    "fs_encoded"
                }),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        // The texture has the viewport's pixel size, so nearest sampling
        // copies pixels 1:1.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("chart blit"),
            ..wgpu::SamplerDescriptor::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
            bound: None,
        }
    }

    /// The paint callback for the renderer's current texture.
    fn paint(&mut self, device: &wgpu::Device, renderer: &WgpuRenderer) -> Option<ChartPaint> {
        let texture = renderer.offscreen_texture()?;
        if self.bound.as_ref().is_none_or(|(t, _)| t != texture) {
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("chart blit"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(renderer.offscreen_view()?),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.bound = Some((texture.clone(), bind));
        }
        let (_, bind) = self.bound.as_ref()?;
        Some(ChartPaint {
            pipeline: self.pipeline.clone(),
            bind: bind.clone(),
        })
    }
}

/// A random-walk tick feed for the last bar.
struct Feed {
    bar: BarUpdate,
    ticks: u32,
    next_ms: f64,
    rng: u64,
}

impl Feed {
    /// Uniform in [-1, 1).
    fn noise(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1_u64 << 52) as f64 - 1.0
    }

    /// Applies the ticks due by `now_ms` (at most a second's worth).
    fn run(&mut self, chart: &mut Chart, series: SeriesId, now_ms: f64) -> Result<(), String> {
        if now_ms - self.next_ms > 1000.0 {
            self.next_ms = now_ms;
        }
        while self.next_ms <= now_ms {
            self.next_ms += TICK_MS;
            self.ticks += 1;
            if self.ticks == TICKS_PER_BAR {
                self.ticks = 0;
                let c = self.bar.close;
                self.bar = BarUpdate {
                    time: self.bar.time + MINUTE,
                    open: c,
                    high: c,
                    low: c,
                    close: c,
                    volume: Some(0.0),
                };
            }
            let close = (self.bar.close * (1.0 + 0.0008 * self.noise())).max(0.01);
            self.bar.close = close;
            self.bar.high = self.bar.high.max(close);
            self.bar.low = self.bar.low.min(close);
            self.bar.volume = Some(self.bar.volume.unwrap_or(0.0) + 1.0 + self.noise().abs() * 5.0);
            chart
                .update_last(series, self.bar)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Converts egui input over the chart's rect to chart input.
#[derive(Default)]
struct InputState {
    /// A button went down on the chart and is still held (moves outside
    /// the rect still go to the chart).
    captured: bool,
    hovered: bool,
    cursor: CursorIcon,
}

fn mods(m: egui::Modifiers) -> Modifiers {
    Modifiers {
        shift: m.shift,
        ctrl: m.ctrl,
        alt: m.alt,
        meta: m.mac_cmd,
    }
}

fn button(b: egui::PointerButton) -> Option<Button> {
    match b {
        egui::PointerButton::Primary => Some(Button::Left),
        egui::PointerButton::Middle => Some(Button::Middle),
        egui::PointerButton::Secondary => Some(Button::Right),
        _ => None,
    }
}

fn key(k: egui::Key) -> Option<Key> {
    Some(match k {
        egui::Key::ArrowLeft => Key::ArrowLeft,
        egui::Key::ArrowRight => Key::ArrowRight,
        egui::Key::ArrowUp => Key::ArrowUp,
        egui::Key::ArrowDown => Key::ArrowDown,
        egui::Key::Home => Key::Home,
        egui::Key::End => Key::End,
        egui::Key::Escape => Key::Escape,
        egui::Key::Delete | egui::Key::Backspace => Key::Delete,
        egui::Key::Enter => Key::Enter,
        // Undo/redo shortcuts (with Ctrl/⌘).
        egui::Key::Z => Key::Char('z'),
        egui::Key::Y => Key::Char('y'),
        _ => return None,
    })
}

fn cursor_icon(c: CursorIcon) -> egui::CursorIcon {
    match c {
        CursorIcon::Crosshair => egui::CursorIcon::Crosshair,
        CursorIcon::Grab => egui::CursorIcon::Grab,
        CursorIcon::Grabbing => egui::CursorIcon::Grabbing,
        CursorIcon::EwResize => egui::CursorIcon::ResizeHorizontal,
        CursorIcon::NsResize => egui::CursorIcon::ResizeVertical,
        CursorIcon::Pointer => egui::CursorIcon::PointingHand,
        CursorIcon::Move => egui::CursorIcon::Move,
        _ => egui::CursorIcon::Default,
    }
}

impl InputState {
    /// Passes this frame's egui events over `rect` to the chart.
    fn forward(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        response: &egui::Response,
        chart: &mut Chart,
    ) {
        let local = |p: egui::Pos2| PointCss {
            x: p.x - rect.min.x,
            y: p.y - rect.min.y,
        };
        let (events, time_ms, hover) =
            ui.input(|i| (i.events.clone(), i.time * 1000.0, i.pointer.hover_pos()));
        let mut send = |state: &mut Self, e: InputEvent| {
            state.cursor = chart.handle_input(e).cursor;
        };
        for e in events {
            match e {
                egui::Event::PointerMoved(p) => {
                    if rect.contains(p) || self.captured {
                        self.hovered = true;
                        let m = ui.input(|i| mods(i.modifiers));
                        send(
                            self,
                            InputEvent::PointerMove {
                                id: 0,
                                pos: local(p),
                                mods: m,
                                time_ms,
                            },
                        );
                    } else if self.hovered {
                        self.hovered = false;
                        send(self, InputEvent::PointerLeave);
                    }
                }
                egui::Event::PointerButton {
                    pos,
                    button: b,
                    pressed,
                    modifiers,
                } => {
                    let Some(b) = button(b) else { continue };
                    let m = mods(modifiers);
                    if pressed && rect.contains(pos) {
                        self.captured = true;
                        send(
                            self,
                            InputEvent::PointerDown {
                                id: 0,
                                pos: local(pos),
                                button: b,
                                mods: m,
                                time_ms,
                            },
                        );
                    } else if !pressed && self.captured {
                        self.captured = false;
                        send(
                            self,
                            InputEvent::PointerUp {
                                id: 0,
                                pos: local(pos),
                                button: b,
                                mods: m,
                                time_ms,
                            },
                        );
                    }
                }
                egui::Event::PointerGone if self.hovered && !self.captured => {
                    self.hovered = false;
                    send(self, InputEvent::PointerLeave);
                }
                egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } => {
                    let Some(p) = hover.filter(|p| rect.contains(*p)) else {
                        continue;
                    };
                    let delta = match unit {
                        // egui: positive moves the content right/down;
                        // the chart: positive scrolls right/down (DOM).
                        egui::MouseWheelUnit::Point => WheelDelta::Pixels {
                            x: Css(-delta.x),
                            y: Css(-delta.y),
                        },
                        egui::MouseWheelUnit::Line => WheelDelta::Lines {
                            x: -delta.x,
                            y: -delta.y,
                        },
                        egui::MouseWheelUnit::Page => WheelDelta::Lines {
                            x: -delta.x * 10.0,
                            y: -delta.y * 10.0,
                        },
                    };
                    send(
                        self,
                        InputEvent::Wheel {
                            pos: local(p),
                            delta,
                            mods: mods(modifiers),
                        },
                    );
                }
                // Touchpad pinch (egui gives the factor since the last event).
                egui::Event::Zoom(scale) => {
                    let Some(p) = hover.filter(|p| rect.contains(*p)) else {
                        continue;
                    };
                    send(
                        self,
                        InputEvent::Pinch {
                            phase: GesturePhase::Update,
                            center: local(p),
                            scale,
                        },
                    );
                }
                egui::Event::Key {
                    key: k,
                    pressed,
                    modifiers,
                    ..
                } if self.hovered => {
                    let Some(k) = key(k) else { continue };
                    send(
                        self,
                        InputEvent::Key {
                            key: k,
                            pressed,
                            mods: mods(modifiers),
                        },
                    );
                }
                _ => {}
            }
        }
        if response.double_clicked()
            && let Some(p) = hover
        {
            send(
                self,
                InputEvent::DoubleClick {
                    pos: local(p),
                    button: Button::Left,
                },
            );
        }
        if self.hovered || self.captured {
            ui.ctx().set_cursor_icon(cursor_icon(self.cursor));
        }
    }
}

struct App {
    device: wgpu::Device,
    renderer: WgpuRenderer,
    blit: Blit,
    chart: Chart,
    candles: SeriesId,
    feed: Feed,
    live: bool,
    ema: Option<IndicatorId>,
    boll: Option<IndicatorId>,
    rsi: Option<IndicatorId>,
    input: InputState,
    /// Size and pixel ratio of the last frame.
    size: (f32, f32, f32),
    log: VecDeque<String>,
    crosshair: String,
    render_ms: f64,
    error: Option<String>,
    /// `--screenshot`: the file and the frames left before taking it.
    screenshot: Option<(PathBuf, u32)>,
}

impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        screenshot: Option<PathBuf>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let rs = cc
            .wgpu_render_state
            .as_ref()
            .ok_or("eframe is not using wgpu")?;
        // Share eframe's device: the chart's texture can then be sampled
        // in egui's pass.
        let ctx = GpuContext {
            instance: rs.instance.clone(),
            adapter: rs.adapter.clone(),
            device: rs.device.clone(),
            queue: rs.queue.clone(),
        };
        let renderer = WgpuRenderer::offscreen(&ctx);
        let blit = Blit::new(&rs.device, rs.target_format);

        let data = random_walk(2024, 20_000, START, MINUTE)?;
        let (Some(&time), Some(&close)) = (data.time.last(), data.close.last()) else {
            return Err("no data".into());
        };
        // Low points 150 and 20 bars back, for the `--screenshot` trend line.
        let low_back = |bars: usize| {
            data.low
                .len()
                .checked_sub(bars + 1)
                .map(|i| (data.time[i], data.low[i]))
        };
        let line_anchors = low_back(150).zip(low_back(20));
        let mut chart = Chart::new(ChartOptions {
            theme: Theme::dark(),
            ..ChartOptions::default()
        });
        let pane = chart.main_pane();
        let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
        chart.add_series(pane, SeriesKind::Volume, SeriesOptions::volume_of(candles))?;
        chart.set_data(candles, SeriesData::Ohlc(data))?;
        let ema = chart.add_indicator(
            candles,
            Box::new(ind::ema(20, Source::Close)?),
            Placement::Overlay,
        )?;
        let rsi = chart.add_indicator(
            candles,
            Box::new(ind::rsi(14, Source::Close)?),
            Placement::NewPane,
        )?;
        chart.set_crosshair_mode(CrosshairMode::Magnet);
        if let (true, Some((a, b))) = (screenshot.is_some(), line_anchors) {
            let at = |(time, price)| Anchor { time, price };
            chart
                .drawings()
                .add(pane, &DrawingData::new("trend_line", vec![at(a), at(b)]))?;
        }
        cc.egui_ctx.set_theme(egui::Theme::Dark);

        Ok(Self {
            device: rs.device.clone(),
            renderer,
            blit,
            chart,
            candles,
            feed: Feed {
                bar: BarUpdate {
                    time: time + MINUTE,
                    open: close,
                    high: close,
                    low: close,
                    close,
                    volume: Some(0.0),
                },
                ticks: 0,
                next_ms: 0.0,
                rng: 0x9E37_79B9_7F4A_7C15,
            },
            live: true,
            ema: Some(ema),
            boll: None,
            rsi: Some(rsi),
            input: InputState::default(),
            size: (0.0, 0.0, 0.0),
            log: VecDeque::new(),
            crosshair: String::new(),
            render_ms: 0.0,
            error: None,
            screenshot: screenshot.map(|p| (p, SCREENSHOT_AFTER_FRAMES)),
        })
    }

    fn report<T, E: std::fmt::Display>(&mut self, r: Result<T, E>) -> Option<T> {
        r.map_err(|e| self.error = Some(e.to_string())).ok()
    }

    /// Adds or removes an indicator to match `on`.
    fn toggle(
        &mut self,
        slot: fn(&mut Self) -> &mut Option<IndicatorId>,
        on: bool,
        make: impl FnOnce(&mut Self) -> Result<IndicatorId, String>,
    ) {
        match (*slot(self), on) {
            (None, true) => {
                let r = make(self);
                *slot(self) = self.report(r);
            }
            (Some(id), false) => {
                let r = self.chart.remove_indicator(id);
                self.report(r);
                *slot(self) = None;
            }
            _ => {}
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("FerroChart");
        ui.label("Drag to pan, wheel or pinch to zoom, drag the axes to scale.");
        ui.separator();

        ui.checkbox(&mut self.live, "Live feed (20 ticks/s)");
        if ui.button("Fit content").clicked() {
            self.chart.fit_content();
        }

        ui.separator();
        ui.label("Indicators");
        let (mut ema, mut boll, mut rsi) =
            (self.ema.is_some(), self.boll.is_some(), self.rsi.is_some());
        ui.checkbox(&mut ema, "EMA 20");
        ui.checkbox(&mut boll, "Bollinger 20, 2");
        ui.checkbox(&mut rsi, "RSI 14 (own pane)");
        let src = self.candles;
        self.toggle(
            |s| &mut s.ema,
            ema,
            |s| {
                let i = ind::ema(20, Source::Close).map_err(|e| e.to_string())?;
                s.chart
                    .add_indicator(src, Box::new(i), Placement::Overlay)
                    .map_err(|e| e.to_string())
            },
        );
        self.toggle(
            |s| &mut s.boll,
            boll,
            |s| {
                let i = ind::boll(20, 2.0, Source::Close).map_err(|e| e.to_string())?;
                s.chart
                    .add_indicator(src, Box::new(i), Placement::Overlay)
                    .map_err(|e| e.to_string())
            },
        );
        self.toggle(
            |s| &mut s.rsi,
            rsi,
            |s| {
                let i = ind::rsi(14, Source::Close).map_err(|e| e.to_string())?;
                s.chart
                    .add_indicator(src, Box::new(i), Placement::NewPane)
                    .map_err(|e| e.to_string())
            },
        );

        ui.separator();
        ui.label("Crosshair");
        let mut mode = self.chart.crosshair_mode();
        ui.horizontal(|ui| {
            ui.radio_value(&mut mode, CrosshairMode::Normal, "Normal");
            ui.radio_value(&mut mode, CrosshairMode::Magnet, "Magnet");
            ui.radio_value(&mut mode, CrosshairMode::Hidden, "Hidden");
        });
        if mode != self.chart.crosshair_mode() {
            self.chart.set_crosshair_mode(mode);
        }

        ui.label("Price scale");
        let pane = self.chart.main_pane();
        if let Some(scale) = self.chart.price_scale(pane, ScaleSide::Right) {
            let mut m = scale.mode();
            ui.horizontal(|ui| {
                ui.radio_value(&mut m, PriceScaleMode::Normal, "Normal");
                ui.radio_value(&mut m, PriceScaleMode::Logarithmic, "Log");
                ui.radio_value(&mut m, PriceScaleMode::Percentage, "%");
            });
            if m != scale.mode() {
                let r = scale.set_mode(m);
                self.report(r);
            }
        }

        ui.separator();
        ui.label("Drawing tools");
        let active = self.chart.drawings().active_tool();
        ui.horizontal_wrapped(|ui| {
            for (kind, label) in [
                ("trend_line", "Trend line"),
                ("horizontal_line", "Horizontal"),
                ("rectangle", "Rectangle"),
                ("fib_retracement", "Fibonacci"),
                ("long_position", "Long position"),
            ] {
                if ui.selectable_label(active == Some(kind), label).clicked() {
                    let r = self.chart.drawings().start_tool(kind);
                    self.report(r);
                }
            }
        });
        ui.horizontal(|ui| {
            let d = self.chart.drawings();
            if ui
                .add_enabled(d.can_undo(), egui::Button::new("Undo"))
                .clicked()
            {
                d.undo();
            }
            let d = self.chart.drawings();
            if ui
                .add_enabled(d.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                d.redo();
            }
            if ui
                .add_enabled(active.is_some(), egui::Button::new("Cancel"))
                .clicked()
            {
                self.chart.drawings().cancel_tool();
            }
        });

        ui.separator();
        ui.monospace(&self.crosshair);
        ui.label(format!("chart frame: {:.2} ms CPU", self.render_ms));
        ui.separator();
        ui.label("Events");
        for line in &self.log {
            ui.monospace(line);
        }
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, e);
        }
    }

    fn collect_events(&mut self) {
        for e in self.chart.drain_events() {
            let line = match e {
                ChartEvent::CrosshairMove { point } => {
                    self.crosshair = match point.and_then(|p| p.bar) {
                        Some(b) => format!(
                            "O {:.2}  H {:.2}\nL {:.2}  C {:.2}",
                            b.open, b.high, b.low, b.close
                        ),
                        None => String::new(),
                    };
                    continue;
                }
                ChartEvent::Click {
                    logical_index,
                    price,
                    target,
                    ..
                } => {
                    let on = match target {
                        Some(ClickTarget::Drawing(d)) => format!(" on drawing {}", d.0),
                        Some(ClickTarget::Series(s)) => format!(" on series {}", s.0),
                        _ => String::new(),
                    };
                    format!("click bar {logical_index:.0} at {price:.2}{on}")
                }
                ChartEvent::DrawingCreated(d) => format!("drawing {} created", d.0),
                ChartEvent::DrawingModified(d) => format!("drawing {} modified", d.0),
                ChartEvent::DrawingRemoved(d) => format!("drawing {} removed", d.0),
                ChartEvent::DrawingSelected(Some(d)) => format!("drawing {} selected", d.0),
                ChartEvent::NeedMoreHistory { bars_before_start } => {
                    format!("need more history ({bars_before_start} bars left)")
                }
                ChartEvent::Warning(w) => format!("warning: {w:?}"),
                _ => continue,
            };
            if self.log.len() == LOG_LINES {
                self.log.pop_front();
            }
            self.log.push_back(line);
        }
    }

    fn chart_view(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let now_ms = ui.input(|i| i.time) * 1000.0;
        let ppp = ui.ctx().pixels_per_point();

        if self.live {
            let r = self.feed.run(&mut self.chart, self.candles, now_ms);
            self.report(r);
        }
        self.input.forward(ui, rect, &response, &mut self.chart);

        // Render when the chart changed (data, input, animations) or the
        // area did.
        let size = (rect.width(), rect.height(), ppp);
        if size != self.size {
            self.size = size;
            self.chart.resize(
                SizeCss {
                    w: size.0,
                    h: size.1,
                },
                ppp,
            );
        }
        if self.chart.needs_redraw() != Invalidation::None
            || self.renderer.offscreen_texture().is_none()
        {
            let t0 = std::time::Instant::now();
            self.chart.build_frame(now_ms, &self.renderer.measurer());
            let r = self
                .renderer
                .render(self.chart.frame(), &self.chart.data_view());
            self.report(r);
            self.render_ms = t0.elapsed().as_secs_f64() * 1000.0;
        }
        self.collect_events();

        if let Some(paint) = self.blit.paint(&self.device, &self.renderer) {
            ui.painter()
                .add(egui_wgpu::Callback::new_paint_callback(rect, paint));
        }

        // Wake up for the next tick or while the chart animates.
        if self.chart.needs_redraw() != Invalidation::None {
            ui.ctx().request_repaint();
        } else if self.live {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(
                    ((self.feed.next_ms - now_ms) / 1000.0).max(0.0),
                ));
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("controls")
            .default_size(260.0)
            .show(ui, |ui| self.side_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.chart_view(ui));
        self.take_screenshot(ui.ctx());
    }
}

impl App {
    /// `--screenshot`: counts frames down, then asks egui for a screenshot
    /// of the next frame, saves it and closes the window.
    fn take_screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, frames)) = &mut self.screenshot else {
            return;
        };
        if *frames > 0 {
            *frames -= 1;
            ctx.request_repaint();
            return;
        }
        let path = path.clone();
        self.screenshot = None;
        let done = ctx.clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
            egui::ScreenshotCallback::new(move |image| {
                let rgba: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(egui::Color32::to_array)
                    .collect();
                let [w, h] = image.size;
                match std::fs::write(&path, png(&rgba, w as u32, h as u32)) {
                    Ok(()) => println!("wrote {} ({w}×{h})", path.display()),
                    Err(e) => eprintln!("cannot write {}: {e}", path.display()),
                }
                done.send_viewport_cmd(egui::ViewportCommand::Close);
                done.request_repaint();
            }),
        ));
        ctx.request_repaint();
    }
}

/// An uncompressed PNG of RGBA8 pixels (stored deflate blocks), so the
/// example needs no image library.
fn png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0_u32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    0xEDB8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: [u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(&kind);
        out.extend_from_slice(body);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    }
    // Rows with a "no filter" byte in front.
    let row = width as usize * 4;
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for r in rgba.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(r);
    }
    // zlib stream of stored blocks (at most 65535 bytes each).
    let mut z = vec![0x78, 0x01];
    let blocks = raw.chunks(65_535).collect::<Vec<_>>();
    for (i, b) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        let len = b.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1_u32, 0_u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    chunk(&mut out, *b"IHDR", &header);
    chunk(&mut out, *b"IDAT", &z);
    chunk(&mut out, *b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;
    use ferrochart_core::testing::MonoMeasurer;

    const W: f32 = 800.0;
    const H: f32 = 450.0;

    fn chart() -> Chart {
        let mut chart = Chart::new(ChartOptions::default());
        chart.resize(SizeCss { w: W, h: H }, 1.0);
        let id = chart
            .add_series(
                chart.main_pane(),
                SeriesKind::Candlestick,
                SeriesOptions::default(),
            )
            .unwrap();
        chart
            .set_data(
                id,
                SeriesData::Ohlc(random_walk(1, 2000, START, MINUTE).unwrap()),
            )
            .unwrap();
        chart.build_frame(0.0, &MonoMeasurer);
        chart
    }

    /// One egui pass with `events`; the chart fills the screen except a
    /// 100 pt margin on the left (so its rect does not start at 0).
    fn pass(
        ctx: &egui::Context,
        input: &mut InputState,
        chart: &mut Chart,
        time: f64,
        events: Vec<egui::Event>,
    ) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(W + 100.0, H),
            )),
            time: Some(time),
            events,
            ..egui::RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            ui.horizontal(|ui| {
                ui.allocate_exact_size(egui::vec2(100.0, H), egui::Sense::hover());
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(W, H), egui::Sense::click_and_drag());
                input.forward(ui, rect, &response, chart);
            });
        });
        out.textures_delta.clear();
        chart.build_frame(time * 1000.0, &MonoMeasurer);
    }

    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn drag_pans_click_reports_and_wheel_zooms() {
        let ctx = egui::Context::default();
        let mut input = InputState::default();
        let mut chart = chart();
        let range = |c: &mut Chart| c.time_scale().visible_range();
        let before = range(&mut chart);

        // Drag 120 pt to the right: earlier bars come into view.
        let p = |x: f32| egui::pos2(100.0 + x, 200.0);
        pass(
            &ctx,
            &mut input,
            &mut chart,
            1.0,
            vec![egui::Event::PointerMoved(p(300.0)), button(p(300.0), true)],
        );
        pass(
            &ctx,
            &mut input,
            &mut chart,
            1.1,
            vec![egui::Event::PointerMoved(p(360.0))],
        );
        pass(
            &ctx,
            &mut input,
            &mut chart,
            1.2,
            vec![egui::Event::PointerMoved(p(420.0))],
        );
        pass(
            &ctx,
            &mut input,
            &mut chart,
            2.0,
            vec![button(p(420.0), false)],
        );
        let after = range(&mut chart);
        assert!(after.0 < before.0 - 5.0, "{before:?} -> {after:?}");
        assert_eq!(input.cursor, CursorIcon::Crosshair);
        chart.drain_events().for_each(drop);

        // A click at chart x = 300 (window x = 400) is reported at that x.
        pass(
            &ctx,
            &mut input,
            &mut chart,
            4.0,
            vec![button(p(300.0), true)],
        );
        pass(
            &ctx,
            &mut input,
            &mut chart,
            4.05,
            vec![button(p(300.0), false)],
        );
        let click = chart.drain_events().find_map(|e| match e {
            ChartEvent::Click { pos, .. } => Some(pos),
            _ => None,
        });
        assert_eq!(click, Some(PointCss { x: 300.0, y: 200.0 }));

        // Wheel up over the chart zooms in; over the left margin it does nothing.
        let wheel = |delta| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, delta),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: egui::Modifiers::NONE,
        };
        let span = |c: &mut Chart| {
            let (a, b) = range(c);
            b - a
        };
        let s0 = span(&mut chart);
        pass(
            &ctx,
            &mut input,
            &mut chart,
            6.0,
            vec![
                egui::Event::PointerMoved(egui::pos2(50.0, 200.0)),
                wheel(3.0),
            ],
        );
        assert_eq!(span(&mut chart), s0);
        assert!(!input.hovered);
        pass(
            &ctx,
            &mut input,
            &mut chart,
            6.1,
            vec![egui::Event::PointerMoved(p(400.0)), wheel(3.0)],
        );
        assert!(span(&mut chart) < s0, "{s0} -> {}", span(&mut chart));
    }
}
