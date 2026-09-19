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
//! - [`Backend`] and [`translate`] — the two halves, for an application
//!   that owns its own event loop because it has something else in it
//!   (the tally has an agent socket and an accessibility adapter).
//! - Neither: `lumen-static-shell` still has no window dependency, and
//!   `Host` still runs headless. That is what keeps `cargo test`
//!   free of a windowing system.
//!
//! The claim `lumen-static-shell` stopped on for three milestones was
//! that a front end adds exactly two things: window events into
//! `Host::post`, and pixels put on the screen. This crate is that claim,
//! factored — [`translate`] is the first half and [`Backend`] is the
//! second.
//!
//! # Two backends, chosen at run time
//!
//! Until T15.12 the second half was written directly against softbuffer,
//! and "pixels put on the screen" meant `Canvas::pixels` blitted into a
//! surface buffer. It is now [`Backend`]: [`Software`] is that same
//! path, and `gpu::GpuBackend` (feature `gpu`) is `lumen-static-wgpu`.
//!
//! **The choice is at run time and it falls back.** A machine with no
//! driver, a container with no `/dev/dri`, a remote session, or a
//! `LUMEN_BACKEND=software` in the environment all still get a window;
//! the one difference is which line [`Backend::name`] prints. See
//! [`Prefer`] and `docs/results-gpu.md` §6.

use std::sync::Arc;
use std::time::Instant;

use lumen_static::event::App;
use lumen_static::style::Rule;
use lumen_static::Color;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[cfg(feature = "agent")]
pub mod agent;
pub mod backend;
pub mod clipboard;
pub mod dialog;
pub mod cursor;
#[cfg(feature = "gpu")]
pub mod gpu;

pub mod surface;
pub mod windows;
pub mod translate;

pub use backend::{Backend, Prefer, Software};
pub use clipboard::Clipboard;
pub use dialog::Dialogs;
#[cfg(feature = "gpu")]
pub use gpu::GpuBackend;
pub use windows::{Pane, Panes};
pub use surface::{canvas, sync_size, Frame, Surface};
pub use translate::{Cursor, Translated};

/// Open a window on the best backend available, falling back to the
/// software canvas.
///
/// **The fallback is the design, not a safety net.** The GPU renderer
/// is a separate crate so that nothing pays for it unasked, and it is
/// chosen at *runtime* so that nothing breaks when it is not there. A
/// machine with no driver, a container with no `/dev/dri`, a remote
/// session, an `LUMEN_BACKEND=software` in the environment: all of them
/// get a window, and the one difference is which line
/// [`Backend::name`] prints.
///
/// Without the `gpu` feature this is always the software canvas, and
/// `prefer` only decides whether to say so.
fn pick_backend<A: App>(
    window: Arc<Window>,
    font: Option<FontStack>,
    prefer: Prefer,
) -> Box<dyn Backend<A>> {
    #[cfg(feature = "gpu")]
    if prefer != Prefer::Software {
        match gpu::GpuBackend::new(window.clone(), font.clone()) {
            Some(g) => return Box::new(g),
            None => eprintln!("no GPU adapter — falling back to the software canvas"),
        }
    }
    #[cfg(not(feature = "gpu"))]
    if prefer == Prefer::Gpu {
        eprintln!("gpu requested, but this binary was built without the `gpu` feature");
    }
    Box::new(Software::new(window, font))
}

/// How a window is set up. Everything has a default except the tree.
pub struct Config {
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
    /// Cleared behind every damaged region before it is redrawn.
    pub page: Color,
    /// Which renderer to try for (T15.12). `Auto` is the GPU when this
    /// crate was built with the `gpu` feature and an adapter exists,
    /// and the software canvas otherwise.
    pub backend: Prefer,
    /// The stylesheet, applied once before the first frame.
    ///
    /// Added 2026-09-11 for `examples/glass`, which needs one and had
    /// only two ways to get it: hand-roll a window on [`Surface`] the
    /// way `examples/counter-window` does — 480 lines M14 existed to
    /// delete — or ship unstyled. A turnkey window that cannot be
    /// styled is a turnkey window for one application.
    ///
    /// Empty by default, which is what every node's `Node::defaults`
    /// already gives.
    pub stylesheet: Vec<Rule>,
    /// The palette the stylesheet's roles resolve against (M25/T25.11),
    /// set before the sheet is applied so the first frame is already in
    /// it. `Palette::LIGHT` by default; a running window swaps it with
    /// `app.setPalette` when the agent is compiled in.
    pub palette: lumen_static::style::Palette,
    /// Where `Cmd::Image` gets its pixels.
    ///
    /// Added 2026-09-13 for `examples/glasswall`, and for the same
    /// reason `stylesheet` was: without it a turnkey window cannot draw
    /// an image *at all*. Both backends can — `Canvas::images` and
    /// `Gpu::set_images` have existed since M14 and T15.12 — and there
    /// was simply no way to reach either from `run`, so every windowed
    /// application was one with no photographs in it.
    ///
    /// Taken once, when the backend is created. A `None` here is not an
    /// error: `ImageSource` already says a missing asset draws nothing.
    pub images: Option<Box<dyn lumen_static::ImageSource>>,
    /// Where to serve the agent endpoint, with `--features agent`.
    ///
    /// `Some("127.0.0.1:7654")` opens a JSON-RPC socket that drives this
    /// window: `input.click`, `input.type`, `ui.getTree`,
    /// `ui.screenshot` and the rest of the protocol
    /// `lumen-static-agent` has carried since M15. Until M23 there was
    /// no way to reach any of it from `run`, so steering a live window
    /// meant copying a socket into your own binary — see the
    /// `agent` module, which the feature compiles in.
    ///
    /// `None`, and without the feature the field is inert, which is what
    /// keeps a release build free of `observe`.
    pub agent: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            title: "lumen-static",
            width: 640,
            height: 480,
            page: Color(255, 255, 255, 255),
            backend: Prefer::Auto,
            stylesheet: Vec::new(),
            palette: lumen_static::style::Palette::LIGHT,
            images: None,
            agent: None,
        }
    }
}

/// Open a window on `app` and run until it closes.
///
/// The turnkey path. An application that needs more — an agent
/// endpoint, an accessibility adapter, a per-frame hook — builds its own
/// `ApplicationHandler` out of [`Surface`] and [`translate`], which is
/// what `examples/counter-window` does.
/// Which invalidation strategy a turnkey window runs.
///
/// **A build-time switch, and it had to be one.** `Host`'s invalidation
/// is a type parameter and `Win` holds a `Host` by value, so the choice
/// is fixed at construction — the same reason the comment in [`run`]
/// gives for building the user-event loop unconditionally. A `Config`
/// field would need two of everything and a branch.
///
/// **Why it exists at all (M27/T27.3).** Nothing outside `lumen-static`'s
/// own tests had ever instantiated [`lumen_static::Boundaries`]: not an
/// example, not a fixture, not this crate. So the strategy M26 spent
/// five defects on — *four of them invisible under `Everything`*, because
/// a clean subtree is skipped before a wrong answer can be observed —
/// was unreachable from every turnkey application. That is the fifth
/// time a capability has landed in the shell with no way to reach it
/// from the front door; `Config::stylesheet`, `Config::images`,
/// `Config::agent` and the overlay sink were the first four.
///
/// 256 marks because `Boundaries<N>` stores ranges, not nodes, and M26/
/// T26.5 fixed it wrapping its length at exactly that count.
#[cfg(not(feature = "boundaries"))]
pub type Inval = lumen_static::Everything;
#[cfg(feature = "boundaries")]
pub type Inval = lumen_static::Boundaries<256>;

/// The retention this crate's windows use. Named so `Host<A, Ret, Inval>`
/// reads the same at every site.
pub type Ret = lumen_static::retain::Incremental;

pub fn run<A: App + 'static>(app: A, config: Config) {
    #[cfg(feature = "agent")]
    {
        // **A user-event loop, so the socket thread has something to
        // send to.** Built unconditionally under the feature rather than
        // only when `config.agent` is set: the loop's type is fixed at
        // construction, and branching on a runtime value would need two
        // of them.
        let event_loop = EventLoop::<agent::UserEvent>::with_user_event()
            .build()
            .expect("event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        if let Some(addr) = config.agent.clone() {
            let proxy = event_loop.create_proxy();
            std::thread::spawn(move || agent::serve(addr, proxy));
        }
        let mut win = Win::new(app, config);
        event_loop.run_app(&mut win).expect("run");
        return;
    }
    #[cfg(not(feature = "agent"))]
    {
        let event_loop = EventLoop::new().expect("event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut win = Win::new(app, config);
        event_loop.run_app(&mut win).expect("run");
    }
}

/// The turnkey handler behind [`run`].
struct Win<A: App> {
    host: Host<A, Ret, Inval>,
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
    window: Option<Arc<Window>>,
    /// The renderer, chosen once the window exists — a GPU surface
    /// cannot be made without one, and neither can a softbuffer one.
    backend: Option<Box<dyn Backend<A>>>,
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
    /// The agent, when the endpoint is compiled in.
    #[cfg(feature = "agent")]
    agent: lumen_static_agent::Agent,
}

impl<A: App> Win<A> {
    fn new(app: A, config: Config) -> Self {
        let mut host = Host::new(app, config.width, config.height);
        host.set_palette(config.palette);
        if !config.stylesheet.is_empty() {
            host.set_stylesheet(&config.stylesheet);
        }
        let font = FontStack::system();
        if font.is_none() {
            // A machine with no font we recognise still gets a window;
            // it just gets the block stand-in in it, which is honest
            // about what happened.
            eprintln!("no system font found — falling back to block glyphs");
        }
        let metrics = font.clone().map(FontMetrics::new);
        Win {
            host,
            config,
            metrics,
            font,
            window: None,
            backend: None,
            cursor: Cursor::default(),
            clicks: Default::default(),
            mods: lumen_static::Mods::NONE,
            clipboard: Clipboard::new(),
            cursor_shown: lumen_static::style::Cursor::Default,
            last_frame: None,
            #[cfg(feature = "agent")]
            agent: lumen_static_agent::Agent::new(),
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
        let (Some(window), Some(backend)) = (self.window.clone(), self.backend.as_mut()) else {
            return;
        };
        let size = window.inner_size();
        backend.resize(&mut self.host, size.width.max(1), size.height.max(1));

        let start = Instant::now();
        // Animation, before the pump: a tick changes state, and the
        // bindings the pump runs are what put that state on screen.
        let dt = self.last_frame.replace(start).map_or(0.0, |t| {
            start.duration_since(t).as_secs_f32() * 1000.0
        });
        self.host.tick(dt);

        match self.metrics.as_mut() {
            Some(m) => self.host.pump_with(m),
            None => self.host.pump(),
        };
        // **Ask again, after the pump — M22/T22.4.**
        //
        // The tick above is where animations *advance*, and it has to
        // stay before the pump because the bindings the pump runs are
        // what put a ticked value on screen. But its answer is stale by
        // the time the pump returns: a press dispatched *during* the
        // pump starts an animation the tick could not have seen, so
        // `animating` was already `false`, no redraw was requested, and
        // the rim jumped to its next value on the following unrelated
        // event instead of settling. A press that stutters unless you
        // wiggle the mouse is the symptom.
        //
        // A zero `dt` is a pure query — `App::tick` guarantees it — so
        // this advances nothing and costs one call that inlines away for
        // an application that animates nothing.
        let animating = self.host.tick(0.0);
        backend.present(&mut self.host, self.config.page, start);
        // After the frame: the hover bit is set by dispatch and the
        // shape is read from the resolved style, both of which the pump
        // above has just brought up to date (T14.6).
        cursor::apply(&window, self.host.cursor(), &mut self.cursor_shown);

        // The journal, fed beside the frame it describes — this is what
        // `ui.lastChange`, `ui.lastDamage` and `ui.waitSettled` read, and
        // without it an agent can drive the window but cannot tell when
        // it has finished moving.
        #[cfg(feature = "agent")]
        {
            let timing = lumen_static_agent::FrameTiming::default();
            let mut tree = lumen_static_agent::TreeMut::of_root(
                self.host.app.tree_mut(),
                &self.host.styles,
                &self.host.states,
            );
            self.agent.frame_done(&mut tree, timing);
        }

        // Another frame only while something is moving. A still UI
        // idles, which is what keeps the idle frame numbers in
        // `docs/results-frame-cost.md` true.
        if animating {
            window.request_redraw();
        }
    }
}

#[cfg(feature = "agent")]
impl<A: App> ApplicationHandler<agent::UserEvent> for Win<A> {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        Win::resumed_impl(self, e);
    }
    fn window_event(&mut self, e: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        Win::window_event_impl(self, e, id, event);
    }

    /// Answer one agent request **on the event loop's own thread**, with
    /// the tree and the backend in hand.
    fn user_event(&mut self, _: &ActiveEventLoop, ev: agent::UserEvent) {
        let agent::UserEvent::Agent(rpc) = ev;
        let Some(backend) = self.backend.as_deref_mut() else {
            let _ = rpc.reply.send(serde_json::json!({
                "jsonrpc": "2.0", "id": rpc.req.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "error": {"code": -32603, "message": "no window yet"}
            }));
            return;
        };
        let mut live = agent::Live {
            host: &mut self.host,
            metrics: &mut self.metrics,
            backend,
            page: self.config.page,
        };
        // **One call.** `dispatch_input` already falls through to
        // `dispatch` for anything that is not an `input.*` method or a
        // screenshot, so the split between "drives" and "only reads"
        // does not have to be made out here.
        let resp = self.agent.dispatch_input(&mut live, &rpc.req);
        let _ = rpc.reply.send(resp);
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

#[cfg(not(feature = "agent"))]
impl<A: App> ApplicationHandler for Win<A> {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        Win::resumed_impl(self, e);
    }
    fn window_event(&mut self, e: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        Win::window_event_impl(self, e, id, event);
    }
}

impl<A: App> Win<A> {
    fn resumed_impl(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title(self.config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width as f64,
                self.config.height as f64,
            ));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));

        // Before the first frame: layout is logical, the surface is
        // physical, and this is the only thing that relates them (M10b).
        let scale = window.scale_factor() as f32;
        self.host.set_scale(scale);
        if let Some(m) = self.metrics.as_mut() {
            m.set_scale(scale);
        }

        let mut backend =
            pick_backend(window.clone(), self.font.clone(), Prefer::from_env(self.config.backend));
        eprintln!("lumen-static: {}", backend.name());
        // Taken, not cloned: `ImageSource` is a trait object and the
        // renderer is where the pixels are wanted.
        if let Some(images) = self.config.images.take() {
            backend.set_images(images);
        }
        self.backend = Some(backend);
        // Tell the platform this window accepts composed input, or an
        // IME never starts and CJK entry is impossible rather than
        // merely awkward (T14.8).
        window.set_ime_allowed(true);
        // Ask for the first frame explicitly: nothing else will.
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event_impl(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
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

/// The damage regions a frame hands the backend, clamped to the
/// surface.
///
/// **A named function because it is the seam the gate asserts.**
/// `just test` excludes the wgpu crate and `just gpu-parity` is in
/// neither `test` nor `check`, so a gate written only there runs on
/// nobody's commit (M26/T26.12). This is where the shell decides what
/// the backend is given, it is in a crate `just test` builds, and
/// `tests/gpu_regions.rs` asserts it.
pub fn regions_for(
    d: lumen_static::retain::Damage,
    viewport: lumen_static::display::Rect,
) -> Vec<lumen_static::display::Rect> {
    d.regions(viewport).collect()
}
