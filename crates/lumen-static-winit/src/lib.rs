//! A window for a `lumen-static` application — `docs/plan-desktop.md`
//! T14.0.
//!
//! Until M14 this code lived in `examples/counter-window`, ~480 lines of
//! it, tangled with an agent socket and a profiler. A second application
//! copied it or reimplemented it, and two of the comments it carries
//! record bugs that took a debugging session each.
//!
//! # Three levels, because applications differ
//!
//! - [`run`] — a window in one call. Everything below the seam is the
//!   framework's; the application supplies a tree and a background.
//! - [`Surface`] and [`translate`] — the two halves, for an application
//!   that owns its own event loop because it has something else in it
//!   (the tally has an agent socket and an accessibility adapter).
//! - Neither: `lumen-static-shell` still has no window dependency, and
//!   `Host` still runs headless. That is what keeps `cargo test`
//!   free of a windowing system.
//!
//! The claim `lumen-static-shell` stopped on for three milestones was
//! that a front end adds exactly two things: window events into
//! `Host::post`, and `Canvas::pixels` blitted to the surface. This crate
//! is that claim, factored — [`translate`] is the first half and
//! [`Surface::present`] is the second.

use std::rc::Rc;
use std::time::Instant;

use lumen_static::event::App;
use lumen_static::Color;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

pub mod surface;
pub mod translate;

pub use surface::{canvas, sync_size, Frame, Surface};
pub use translate::{Cursor, Translated};

/// How a window is set up. Everything has a default except the tree.
pub struct Config {
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
    /// Cleared behind every damaged region before it is redrawn.
    pub page: Color,
}

impl Default for Config {
    fn default() -> Self {
        Config { title: "lumen-static", width: 640, height: 480, page: Color(255, 255, 255, 255) }
    }
}

/// Open a window on `app` and run until it closes.
///
/// The turnkey path. An application that needs more — an agent
/// endpoint, an accessibility adapter, a per-frame hook — builds its own
/// `ApplicationHandler` out of [`Surface`] and [`translate`], which is
/// what `examples/counter-window` does.
pub fn run<A: App + 'static>(app: A, config: Config) {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut win = Win::new(app, config);
    event_loop.run_app(&mut win).expect("run");
}

/// The turnkey handler behind [`run`].
struct Win<A: App> {
    host: Host<A>,
    config: Config,
    /// The *same* face the canvas rasterises with. Layout must measure
    /// with the metrics paint will draw with, or the two disagree — the
    /// first window measured with the block stand-in and painted with
    /// fontdue, so every box was too narrow and "TALLY" wrapped to
    /// "TAL / LY". The headless path had it right; the window did not,
    /// and only a screenshot of the live window showed it.
    metrics: Option<FontMetrics>,
    font: Option<FontStack>,
    /// `None` until `resumed`; winit owns window creation.
    window: Option<Rc<Window>>,
    surface: Option<Surface>,
    canvas: lumen_static_shell::software::Canvas,
    cursor: Cursor,
}

impl<A: App> Win<A> {
    fn new(app: A, config: Config) -> Self {
        let host = Host::new(app, config.width, config.height);
        let font = FontStack::system();
        if font.is_none() {
            // A machine with no font we recognise still gets a window;
            // it just gets the block stand-in in it, which is honest
            // about what happened.
            eprintln!("no system font found — falling back to block glyphs");
        }
        let metrics = font.clone().map(FontMetrics::new);
        let canvas = surface::canvas(config.width, config.height, font.as_ref());
        Win {
            host,
            config,
            metrics,
            font,
            window: None,
            surface: None,
            canvas,
            cursor: Cursor::default(),
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(surface)) = (self.window.clone(), self.surface.as_mut()) else {
            return;
        };
        let size = window.inner_size();
        surface::sync_size(
            &mut self.canvas,
            &mut self.host,
            self.font.as_ref(),
            size.width,
            size.height,
        );

        let start = Instant::now();
        match self.metrics.as_mut() {
            Some(m) => self.host.pump_with(m),
            None => self.host.pump(),
        };
        surface.present(&mut self.host, &mut self.canvas, self.config.page, start);
    }
}

impl<A: App> ApplicationHandler for Win<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title(self.config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width as f64,
                self.config.height as f64,
            ));
        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));

        // Before the first frame: layout is logical, the surface is
        // physical, and this is the only thing that relates them (M10b).
        let scale = window.scale_factor() as f32;
        self.host.set_scale(scale);
        if let Some(m) = self.metrics.as_mut() {
            m.set_scale(scale);
        }

        self.surface = Some(Surface::new(window.clone()));
        // Ask for the first frame explicitly: nothing else will.
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // A window can be dragged between monitors.
                self.host.set_scale(scale_factor as f32);
                if let Some(m) = self.metrics.as_mut() {
                    m.set_scale(scale_factor as f32);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::Resized(_) | WindowEvent::RedrawRequested => self.redraw(),

            ref event => {
                let host = &self.host;
                let t = translate::window_event(event, &mut self.cursor, |x, y| {
                    host.to_logical(x, y)
                });
                let any = t.events[0].is_some();
                for ev in t.iter() {
                    self.host.post(ev);
                }
                if any {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
        }
    }
}
