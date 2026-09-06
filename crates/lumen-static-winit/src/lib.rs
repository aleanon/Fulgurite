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

pub mod clipboard;
pub mod dialog;
pub mod cursor;
pub mod surface;
pub mod windows;
pub mod translate;

pub use clipboard::Clipboard;
pub use dialog::Dialogs;
pub use windows::{Pane, Panes};
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
    /// Single/double/triple click counting (T14.3): it needs a clock,
    /// which the core does not have.
    clicks: translate::ClickCounter,
    /// Modifiers as of the last `ModifiersChanged`. Kept here as well as
    /// in the host because translation needs it *before* the event is
    /// queued: it decides whether a keystroke types (T14.2).
    mods: lumen_static::Mods,
    clipboard: Clipboard,
    /// The cursor the window is currently showing, so an unchanged
    /// frame does not ask the platform to set it again.
    cursor_shown: lumen_static::style::Cursor,
    /// When the last frame was drawn, for the animation tick (T15.4).
    last_frame: Option<Instant>,
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
            clicks: Default::default(),
            mods: lumen_static::Mods::NONE,
            clipboard: Clipboard::new(),
            cursor_shown: lumen_static::style::Cursor::Default,
            last_frame: None,
        }
    }

    /// Ctrl+C, Ctrl+X and Ctrl+V (or Cmd on macOS) — T14.5.
    ///
    /// Handled here rather than in a widget because the *clipboard* is
    /// the platform's; what the widget owns is the selection and the
    /// typing, and both of those are `Host` calls. Returns whether the
    /// keystroke was a clipboard shortcut, so the caller knows not to
    /// route it on.
    fn clipboard_shortcut(&mut self, event: &WindowEvent) -> bool {
        let WindowEvent::KeyboardInput { event, .. } = event else { return false };
        if event.state != winit::event::ElementState::Pressed {
            return false;
        }
        if !(self.mods.has(lumen_static::Mods::CTRL) || self.mods.has(lumen_static::Mods::META)) {
            return false;
        }
        let Some(lumen_static::Key::Char(c)) = translate::key_of(&event.logical_key) else {
            return false;
        };
        match c {
            'c' | 'x' => {
                let Some(text) = self.host.copy().map(str::to_owned) else { return true };
                self.clipboard.set(&text);
                if c == 'x' {
                    // Cut is copy plus the delete the field already
                    // does: one edit path, so the caret and the shaped
                    // cache cannot disagree with a bespoke one.
                    self.host.post(lumen_static::Event::Key {
                        key: lumen_static::Key::Backspace,
                        down: true,
                    });
                }
                true
            }
            'v' => {
                if let Some(text) = self.clipboard.get() {
                    self.host.paste(&text);
                }
                true
            }
            _ => false,
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
        // Animation, before the pump: a tick changes state, and the
        // bindings the pump runs are what put that state on screen.
        let dt = self.last_frame.replace(start).map_or(0.0, |t| {
            start.duration_since(t).as_secs_f32() * 1000.0
        });
        let animating = self.host.tick(dt);

        match self.metrics.as_mut() {
            Some(m) => self.host.pump_with(m),
            None => self.host.pump(),
        };
        surface.present(&mut self.host, &mut self.canvas, self.config.page, start);
        // After the frame: the hover bit is set by dispatch and the
        // shape is read from the resolved style, both of which the pump
        // above has just brought up to date (T14.6).
        cursor::apply(&window, self.host.cursor(), &mut self.cursor_shown);

        // Another frame only while something is moving. A still UI
        // idles, which is what keeps the idle frame numbers in
        // `docs/results-frame-cost.md` true.
        if animating {
            window.request_redraw();
        }
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
        // Tell the platform this window accepts composed input, or an
        // IME never starts and CJK entry is impossible rather than
        // merely awkward (T14.8).
        window.set_ime_allowed(true);
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

            // Input-method composition (T14.8). Not routed through
            // `translate`, because a composition step carries a string
            // and `Event` is `Copy` and 16 bytes — it reaches the
            // focused node through `Host::compose` instead.
            WindowEvent::Ime(ref ime) => {
                use lumen_static::event::Composition;
                use winit::event::Ime;
                let changed = match ime {
                    Ime::Preedit(text, _) => self.host.compose(Composition::Preedit(text)),
                    Ime::Commit(text) => self.host.compose(Composition::Commit(text)),
                    Ime::Disabled => self.host.compose(Composition::Cancel),
                    Ime::Enabled => false,
                };
                if changed {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }

            WindowEvent::Resized(_) | WindowEvent::RedrawRequested => self.redraw(),

            ref event if self.clipboard_shortcut(event) => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            ref event => {
                let host = &self.host;
                let t = translate::window_event(event, &mut self.cursor, &mut self.clicks, self.mods, |x, y| {
                    host.to_logical(x, y)
                });
                if let winit::event::WindowEvent::ModifiersChanged(m) = event {
                    self.mods = translate::mods(m.state());
                }
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
