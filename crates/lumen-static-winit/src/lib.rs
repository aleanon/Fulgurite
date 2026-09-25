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
use lumen_static::Color;
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[cfg(feature = "agent")]
pub mod agent;

/// What reaches the event loop from somewhere other than the window.
///
/// **Unconditional since M28/T28.1.** It lived in `agent.rs` and existed
/// only under that feature, which meant a build without the agent had no
/// user-event channel at all: `EventLoop::new()` rather than
/// `with_user_event()`. Since the loop runs in `ControlFlow::Wait`, an
/// idle window slept until input arrived and **a completion posted from
/// a worker could not reach a frame** — the whole premise of M28.
///
/// The agent's request is now one variant among others rather than the
/// only reason a loop has a proxy.
pub enum UserEvent {
    /// One agent request, answered on the loop's own thread.
    #[cfg(feature = "agent")]
    Agent(agent::Rpc),
    /// Somebody off the frame thread has queued work and wants a frame —
    /// see [`ProxyWaker`].
    ///
    /// Carries nothing. The message itself already went through the
    /// inbox's channel; duplicating it here would put a second copy in
    /// flight and reintroduce the ordering question the inbox exists to
    /// answer.
    Wake,
    /// The hot-reload builder started a build (M37/T37.2).
    #[cfg(feature = "hot")]
    Building,
    /// …and finished one.
    #[cfg(feature = "hot")]
    Built(lumen_static_reload::hot::Outcome),
}

/// A [`Waker`](lumen_static::inbox::Waker) over the event loop's proxy —
/// M28/T28.1.
///
/// This is the only place winit and the core's deferred-work seam meet.
/// The core cannot name winit, and winit cannot name the application's
/// message type, so the trait carries no payload: it says *wake up*, and
/// the queue the worker already wrote to says what happened.
pub struct ProxyWaker(winit::event_loop::EventLoopProxy<UserEvent>);

impl ProxyWaker {
    /// Wrap a proxy. `EventLoop::create_proxy` is the only source.
    pub fn new(proxy: winit::event_loop::EventLoopProxy<UserEvent>) -> Self {
        ProxyWaker(proxy)
    }
}

impl lumen_static::inbox::Waker for ProxyWaker {
    /// **A closed loop is not an error here.** A worker outliving its
    /// window is ordinary — the user closed it while a download was in
    /// flight — and `send_event` answers that with `Err`. Dropping the
    /// request is the correct response: there is no loop to wake and no
    /// frame anyone will see.
    fn wake(&self) {
        let _ = self.0.send_event(UserEvent::Wake);
    }
}
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
fn pick_backend(
    window: Arc<Window>,
    font: Option<FontStack>,
    prefer: Prefer,
) -> Box<dyn Backend> {
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
    /// The palette the stylesheet's roles resolve against (M25/T25.11),
    /// set before the sheet is applied so the first frame is already in
    /// it. `Palette::LIGHT` by default; a running window swaps it with
    /// `app.setPalette` when the agent is compiled in.
    pub palette: lumen_static::style::Palette,
    /// Where `Cmd::Image` gets its pixels.
    ///
    /// Added 2026-09-13 for `examples/glasswall`, and for the same
    /// reason the stylesheet was: without it a turnkey window cannot draw
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
    ///
    /// **One JSON object per line, or the connection ends** (M39/T39.1):
    /// the endpoint is a local line protocol and refuses HTTP by framing,
    /// so a web page open in a browser on this machine cannot drive the
    /// window. It is not authenticated: any process running as the same
    /// user can connect (`docs/00-handoff-static-tree.md` §9).
    ///
    /// **Loopback only** (M39/T39.2): `127.0.0.1`, `[::1]` and `localhost`
    /// bind; `0.0.0.0` or any other non-loopback address is refused, with
    /// a line on stderr naming the override, `LUMEN_AGENT_ALLOW_REMOTE=1`.
    /// The override binds it and warns that the endpoint has no
    /// authentication.
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
            palette: lumen_static::style::Palette::LIGHT,
            images: None,
            agent: None,
        }
    }
}

/// Which invalidation strategy a turnkey window runs, and the retention
/// its host uses — defined by the shell since M37
/// (`lumen_static_shell::session`), because a hot library builds the same
/// `Host` type and links no window system. Re-exported so
/// `Host<A, Ret, Inval>` reads the same at every site.
///
/// **A build-time switch, and it had to be one.** `Host`'s invalidation
/// is a type parameter, so the choice is fixed at construction.
///
/// **Why it exists at all (M27/T27.3).** Nothing outside `lumen-static`'s
/// own tests had ever instantiated [`lumen_static::Boundaries`], so the
/// strategy M26 spent five defects on was unreachable from every turnkey
/// application. **`Boundaries` by default since M34/T34.2**; this
/// crate's `everything` feature selects the whole-tree walk instead —
/// **a diagnostic**: if a stale frame goes away when you build with it,
/// you have found an invalidation defect.
pub use lumen_static_shell::session::{Inval, Ret};
use lumen_static_shell::session::{Local, Recover, Session};

/// Whether this window catches panics in application code — M36
/// (`docs/plan-panic-recovery.md` rule 3): a debug build does, a release
/// build does with the `reload` feature, and never where panics abort.
///
/// Decided **here**, in the window crate, and handed to the session:
/// the workspace's dev profile builds the shell with `debug-assertions =
/// false`, so a `cfg(debug_assertions)` evaluated there was false in
/// every debug build.
#[cfg(all(panic = "unwind", any(debug_assertions, feature = "reload")))]
pub const RECOVER: Recover = Recover::On;
#[cfg(not(all(panic = "unwind", any(debug_assertions, feature = "reload"))))]
pub const RECOVER: Recover = Recover::Off;

/// Open a window on an already-built application, with no stylesheet.
///
/// The lower-level door, kept for an application that is not described
/// by a [`Program`]. Since M32 it cannot be styled: a stylesheet comes
/// from a program's description, and a `Config` field that could style
/// one window while the headless host was styled by something else is
/// the two-path defect `docs/plan-program.md` removed.
pub fn run<A: App + 'static>(app: A, config: Config) {
    let mut host: Host<A, Ret, Inval> = Host::new(app, config.width, config.height);
    host.set_palette(config.palette);
    let session = Local::from_host(host, config.title, config.page, RECOVER);
    start(Box::new(session), config, None);
}

/// Run a window around a session until it closes. `hot` is the
/// application crate's directory when the window rebuilds it (M37).
fn start(session: Box<dyn Session>, config: Config, hot: Option<std::path::PathBuf>) {
    // **Always a user-event loop** — M28/T28.1.
    //
    // This was `with_user_event()` under the `agent` feature and
    // `EventLoop::new()` without it. The loop's type is fixed at
    // construction, so the two could not be chosen at runtime, and the
    // consequence was that an ordinary build had no way to be woken from
    // another thread at all. `ControlFlow::Wait` then means a completion
    // posted by a worker waits for the user to move the mouse.
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    #[cfg(feature = "agent")]
    if let Some(addr) = config.agent.clone() {
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || agent::serve(addr, proxy));
    }
    let mut win = Win::new(session, config);
    // **Deferred work, wired up** — M28/T28.12. The waker is the
    // window's, because the window owns the loop; the spawner is the
    // session's own (`Local::from_host` installs `Threads`), because a
    // job runs the application's code, which is wherever the session is.
    let waker: std::sync::Arc<dyn lumen_static::inbox::Waker> =
        std::sync::Arc::new(ProxyWaker::new(event_loop.create_proxy()));
    win.session.set_waker(waker.clone());
    #[cfg(feature = "hot")]
    if let Some(dir) = hot {
        match hot::crate_at(&dir) {
            Ok(krate) => {
                eprintln!(
                    "lumen-static: hot reload of `{}` (features `{}`) into {}",
                    krate.name,
                    krate.features,
                    krate.target_dir.display()
                );
                let build = hot::builder(krate, event_loop.create_proxy());
                let watcher = hot::watch(&dir, build.clone());
                if watcher.is_none() {
                    eprintln!("lumen-static: could not watch {}/src — reload by restarting", dir.display());
                }
                // The first build starts at once: the static application
                // runs until it lands (§2.1), so there is no start-up wait.
                let _ = build.send(());
                win.hot = Some(hot::Hot {
                    reloader: lumen_static_reload::hot::Reloader::new(),
                    build,
                    waker,
                    metrics: win.font.clone().map(FontMetrics::new),
                    _watcher: watcher,
                    bar_shown: false,
                });
            }
            Err(e) => eprintln!("lumen-static: no hot reload: {e}"),
        }
    }
    #[cfg(not(feature = "hot"))]
    let _ = (hot, waker);
    event_loop.run_app(&mut win).expect("run");
}

/// What the loop waits on after a frame that started at `from`, when the
/// host's soonest timer was `ms` away at the end of it — M47/T47.1.
///
/// **An instant, not a remainder.** `ms` is what the timers said after
/// the frame ticked them by the time since the previous frame's start,
/// so it is measured from *this* frame's start, and the deadline is
/// fixed there. Until T47.1 the loop re-armed `now + ms` on every wake,
/// so each wake that produced no frame pushed the deadline later.
///
/// **Fallible, because `Timers::every` accepts any finite positive
/// period** (R7-F10): the panicking float-to-`Duration` conversion took
/// the loop down on a period above about 1.8e22 ms. A deadline no
/// `Instant` can hold waits without one, which is what a timer that far
/// away means in practice.
fn control_flow_for(from: Instant, ms: Option<f32>) -> ControlFlow {
    ms.and_then(|ms| std::time::Duration::try_from_secs_f32(ms.max(0.0) / 1000.0).ok())
        .and_then(|d| from.checked_add(d))
        .map_or(ControlFlow::Wait, ControlFlow::WaitUntil)
}

/// The turnkey handler behind [`run`] — **not generic over the
/// application** since M37: everything it asks of one goes through
/// [`Session`], which is what lets the same handler drive an application
/// linked into the binary and one living in a hot library.
struct Win {
    session: Box<dyn Session>,
    config: Config,
    /// The title the window is showing, so an unchanged one is not set
    /// again.
    title_shown: String,
    /// The face the renderer rasterises with. The session measures with
    /// the same system face; the first window measured with the block
    /// stand-in and painted with fontdue, so every box was too narrow and
    /// "TALLY" wrapped to "TAL / LY".
    font: Option<FontStack>,
    /// `None` until `resumed`; winit owns window creation.
    window: Option<Arc<Window>>,
    /// The renderer, chosen once the window exists — a GPU surface
    /// cannot be made without one, and neither can a softbuffer one.
    backend: Option<Box<dyn Backend>>,
    cursor: Cursor,
    /// Single/double/triple click counting (T14.3): it needs a clock,
    /// which the core does not have.
    clicks: translate::ClickCounter,
    /// Modifiers as of the last `ModifiersChanged`. Kept here because
    /// translation needs it *before* the event is queued: it decides
    /// whether a keystroke types (T14.2).
    mods: lumen_static::Mods,
    clipboard: Clipboard,
    /// The cursor the window is currently showing, so an unchanged
    /// frame does not ask the platform to set it again.
    cursor_shown: lumen_static::style::Cursor,
    /// When the last frame was drawn, for the animation tick (T15.4).
    ///
    /// **Cleared while a panic is shown** (M47/T47.1), so the first frame
    /// after a retry — by R or by `app.retry` — ticks by 0 rather than by
    /// the whole time the screen was up (`docs/plan-panic-recovery.md`
    /// §2.2).
    last_frame: Option<Instant>,
    /// What the loop waits on, fixed after each frame by
    /// [`control_flow_for`]: `WaitUntil` the soonest timer's instant, or
    /// `Wait` when there is none or a panic is shown (M47/T47.1).
    wait: ControlFlow,
    /// The reload state, when the window rebuilds its application (M37).
    #[cfg(feature = "hot")]
    hot: Option<hot::Hot>,
}

impl Win {
    fn new(session: Box<dyn Session>, config: Config) -> Self {
        let font = FontStack::system();
        if font.is_none() {
            // A machine with no font we recognise still gets a window;
            // it just gets the block stand-in in it, which is honest
            // about what happened.
            eprintln!("no system font found — falling back to block glyphs");
        }
        Win {
            session,
            config,
            title_shown: String::new(),
            font,
            window: None,
            backend: None,
            cursor: Cursor::default(),
            clicks: Default::default(),
            mods: lumen_static::Mods::NONE,
            clipboard: Clipboard::new(),
            cursor_shown: lumen_static::style::Cursor::Default,
            last_frame: None,
            wait: ControlFlow::Wait,
            #[cfg(feature = "hot")]
            hot: None,
        }
    }

    /// Ctrl+C, Ctrl+X and Ctrl+V (or Cmd on macOS) — T14.5.
    ///
    /// Handled here rather than in a widget because the *clipboard* is
    /// the platform's; what the widget owns is the selection and the
    /// typing, and both of those are session calls. Returns whether the
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
                let Some(text) = self.session.copy() else { return true };
                self.clipboard.set(&text);
                if c == 'x' {
                    // Cut is copy plus the delete the field already
                    // does: one edit path, so the caret and the shaped
                    // cache cannot disagree with a bespoke one.
                    self.session.post(lumen_static::Event::Key {
                        key: lumen_static::Key::Backspace,
                        down: true,
                    });
                }
                true
            }
            'v' => {
                if let Some(text) = self.clipboard.get() {
                    self.session.paste(&text);
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
        let (w, h) = (size.width.max(1), size.height.max(1));
        if backend.resize(w, h) {
            self.session.resize(w, h);
        }
        // **The swap, between frames** (M37 rule 3): a library that
        // landed since the last frame takes over now, with the running
        // session's kept state, the window's waker, scale and size.
        #[cfg(feature = "hot")]
        if let Some(hot) = self.hot.as_mut() {
            let (waker, scale) = (hot.waker.clone(), window.scale_factor() as f32);
            if hot.reloader.swap(&mut self.session, |s| {
                s.set_waker(waker);
                s.set_scale(scale);
                s.resize(w, h);
            }) {
                eprintln!("lumen-static: reloaded ({})", hot.reloader.reloads);
                self.last_frame = None;
            }
        }

        let start = Instant::now();
        // A panic being shown runs no application code (M36), so it does
        // not advance the animation clock either.
        let dt = if self.session.failed() {
            0.0
        } else {
            self.last_frame.replace(start).map_or(0.0, |t| start.duration_since(t).as_secs_f32() * 1000.0)
        };
        // **One guarded entry for the frame's application code** (M36),
        // inside the session: both ticks and the pump, or the panic
        // screen.
        let animating = self.session.frame(dt, w, h);
        // **The deadline, fixed at this frame** (M47/T47.1). While a panic
        // is shown, none: the timers do not run (their clock is frozen
        // above), so a deadline would only redraw the screen at the
        // timer's rate. And the clock is dropped, so the retry's first
        // frame starts from `dt` 0 whichever way the retry came.
        self.wait = if self.session.failed() {
            self.last_frame = None;
            ControlFlow::Wait
        } else {
            control_flow_for(start, self.session.next_deadline_ms())
        };
        #[cfg(feature = "hot")]
        let barred = match self.hot.as_mut() {
            Some(hot) => {
                let r = &hot.reloader;
                let shown = match (&r.error, &r.warning) {
                    (Some(e), _) => Some((hot::FAILED, e.clone())),
                    (None, Some(w)) => Some((hot::RESET, w.clone())),
                    _ => None,
                };
                shown.map(|(head, body)| hot::Barred::new(&mut *self.session, head, &body, w, h, &mut hot.metrics))
            }
            None => None,
        };
        // **The bar is the window's pixels, not the session's**: on the
        // frame it goes away the session must repaint everything, in every
        // buffer, or the bar outlives its fix wherever the session's own
        // damage does not reach (found by `just hot-window-drive`).
        #[cfg(feature = "hot")]
        if let Some(hot) = self.hot.as_mut() {
            if barred.is_none() && hot.bar_shown {
                self.session.repaint_all();
            }
            hot.bar_shown = barred.is_some();
        }
        #[cfg(feature = "hot")]
        match barred {
            Some(mut b) => backend.present(&mut b, self.config.page, start),
            None => backend.present(&mut *self.session, self.config.page, start),
        };
        #[cfg(not(feature = "hot"))]
        backend.present(&mut *self.session, self.config.page, start);
        if self.session.failed() {
            return;
        }
        // A title that is a function of the state is re-asked after every
        // frame and set only when it changed (`docs/plan-program.md`
        // §2.1). A constant one compares equal every time.
        let title = self.title_now();
        if title != self.title_shown {
            window.set_title(&title);
            self.title_shown = title;
        }
        // After the frame: the hover bit is set by dispatch and the
        // shape is read from the resolved style, both of which the pump
        // above has just brought up to date (T14.6).
        cursor::apply(&window, self.session.cursor(), &mut self.cursor_shown);

        // The journal, fed beside the frame it describes — this is what
        // `ui.lastChange`, `ui.lastDamage` and `ui.waitSettled` read, and
        // without it an agent can drive the window but cannot tell when
        // it has finished moving.
        self.session.agent_frame_done();

        // Another frame only while something is moving, or while the
        // inbox has a backlog (M28/T28.2: the drain is capped per frame,
        // so a backlog asks for another frame instead of the cap being
        // raised). A still UI idles, which is what keeps the idle frame
        // numbers in `docs/results-frame-cost.md` true.
        if animating || self.session.more_deferred() {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler<UserEvent> for Win {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        Win::resumed_impl(self, e);
    }
    fn window_event(&mut self, e: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        Win::window_event_impl(self, e, id, event);
    }

    /// **What the loop waits on** — M28/T28.11, made true by M47/T47.1.
    ///
    /// `ControlFlow::Wait` sleeps until input arrives, so a timer on an
    /// idle window would never be checked: a deadline needs a frame, and
    /// an idle window produces none. `WaitUntil` wakes the loop at the
    /// deadline, and **waking is not a frame**: until T47.1 nothing asked
    /// for one at that instant, so an idle window's 120 ms timer ticked
    /// once in three seconds (R7-F1, `docs/results-tasks.md` T28.12).
    /// [`new_events`](Self::new_events) is what turns the wake into a
    /// frame, and the instant is fixed after each frame rather than
    /// re-armed from now on every wake.
    ///
    /// Nothing pending falls back to `Wait`, which is today's behaviour
    /// and what keeps the idle frame numbers in
    /// `docs/results-frame-cost.md` true — this changes when a still
    /// window wakes, never how often a busy one draws.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(self.wait);
    }

    /// **A deadline that has passed asks for a frame** — M47/T47.1.
    ///
    /// On `ResumeTimeReached`, and on any wake at or after the instant
    /// (an event that arrived just as the timer came due), request a
    /// redraw: the frame is where the timers are ticked, so this is the
    /// only thing between a deadline and the timer it belongs to.
    fn new_events(&mut self, _e: &ActiveEventLoop, cause: winit::event::StartCause) {
        let due = match self.wait {
            ControlFlow::WaitUntil(t) => {
                matches!(cause, winit::event::StartCause::ResumeTimeReached { .. }) || Instant::now() >= t
            }
            ControlFlow::Wait | ControlFlow::Poll => false,
        };
        if due {
            // Consumed: the frame this asks for fixes the next instant. A
            // past `WaitUntil` left armed would spin the loop wherever the
            // platform holds the redraw back (a hidden window).
            self.wait = ControlFlow::Wait;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    /// Anything that reached the loop from off the window's thread.
    ///
    /// **One impl since M28/T28.1**, where there were two: one over
    /// `agent::UserEvent` under that feature and one over winit's
    /// default `()` without it.
    fn user_event(&mut self, _e: &ActiveEventLoop, ev: UserEvent) {
        match ev {
            // A worker queued something. Ask for a frame and let the
            // ordinary redraw path handle it — this arm deliberately
            // does **not** pump, because `about_to_wait` and the redraw
            // handler already own that sequence, and a second entry
            // point into it is how two frame loops come to disagree.
            UserEvent::Wake => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            #[cfg(feature = "agent")]
            UserEvent::Agent(rpc) => self.answer_agent(rpc),
            #[cfg(feature = "hot")]
            UserEvent::Building => {
                if let Some(h) = self.hot.as_mut() {
                    h.reloader.building = true;
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            #[cfg(feature = "hot")]
            UserEvent::Built(outcome) => {
                if let Some(h) = self.hot.as_mut() {
                    h.reloader.outcome(outcome);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
        }
    }
}

#[cfg(feature = "agent")]
impl Win {
    /// Answer one agent request **on the event loop's own thread**, with
    /// the tree and the backend in hand.
    ///
    /// The session answers it — the tree is wherever the session is — as
    /// JSON text, so no `serde_json::Value` crosses into a hot library
    /// (M37). The backend goes with it as the [`backend::AsScreen`] the
    /// agent presents to and takes screenshots from.
    fn answer_agent(&mut self, rpc: agent::Rpc) {
        let Some(backend) = self.backend.as_deref_mut() else {
            let _ = rpc.reply.send(serde_json::json!({
                "jsonrpc": "2.0", "id": rpc.req.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "error": {"code": -32603, "message": "no window yet"}
            }));
            return;
        };
        // `app.reloads` is the window's, not the session's: the count
        // outlives every session it counts (M37/T37.4 waits on it).
        #[cfg(feature = "hot")]
        if rpc.req.get("method").and_then(|m| m.as_str()) == Some("app.reloads") {
            let r = self.hot.as_ref().map(|h| &h.reloader);
            let _ = rpc.reply.send(serde_json::json!({
                "jsonrpc": "2.0", "id": rpc.req.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": {
                    "supported": r.is_some(),
                    "reloads": r.map_or(0, |r| r.reloads),
                    "building": r.is_some_and(|r| r.building),
                    "pending": r.is_some_and(|r| r.pending().is_some()),
                    "error": r.and_then(|r| r.error.clone()),
                    "warning": r.and_then(|r| r.warning.clone()),
                }
            }));
            return;
        }
        let text = self.session.agent(&rpc.req.to_string(), &mut backend::AsScreen(backend));
        let resp = serde_json::from_str(&text).unwrap_or_else(|e| {
            serde_json::json!({
                "jsonrpc": "2.0", "id": rpc.req.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "error": {"code": -32603, "message": format!("the session answered malformed JSON: {e}")}
            })
        });
        let _ = rpc.reply.send(resp);
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl Win {
    fn title_now(&self) -> String {
        let t = self.session.title();
        let t = if t.is_empty() { self.config.title.to_string() } else { t };
        // hot_ice's convention: the title says a build is running.
        #[cfg(feature = "hot")]
        if self.hot.as_ref().is_some_and(|h| h.reloader.building) {
            return format!("Reloading… {t}");
        }
        t
    }

    fn resumed_impl(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title(self.title_now())
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width as f64,
                self.config.height as f64,
            ));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));

        // Before the first frame: layout is logical, the surface is
        // physical, and this is the only thing that relates them (M10b).
        self.session.set_scale(window.scale_factor() as f32);

        let mut backend = pick_backend(window.clone(), self.font.clone(), Prefer::from_env(self.config.backend));
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
        // **While a panic is shown** (M36 §2.2): close, resize and redraw
        // as ever; **R retries**; everything else is swallowed, so nothing
        // piles up in the queue to replay into the application on retry.
        if self.session.failed() {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(_) | WindowEvent::RedrawRequested => self.redraw(),
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == winit::event::ElementState::Pressed
                        && matches!(
                            translate::key_of(&event.logical_key),
                            Some(lumen_static::Key::Char('r' | 'R'))
                        ) =>
                {
                    self.session.retry();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                _ => {}
            }
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // A window can be dragged between monitors.
                self.session.set_scale(scale_factor as f32);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            // Input-method composition (T14.8). Not routed through
            // `translate`, because a composition step carries a string
            // and `Event` is `Copy` and 16 bytes — it reaches the
            // focused node through `Session::compose` instead.
            WindowEvent::Ime(ref ime) => {
                use lumen_static::event::Composition;
                use winit::event::Ime;
                let changed = match ime {
                    Ime::Preedit(text, _) => self.session.compose(Composition::Preedit(text)),
                    Ime::Commit(text) => self.session.compose(Composition::Commit(text)),
                    Ime::Disabled => self.session.compose(Composition::Cancel),
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
                let session = &self.session;
                let t = translate::window_event(event, &mut self.cursor, &mut self.clicks, self.mods, |x, y| {
                    session.to_logical(x, y)
                });
                if let winit::event::WindowEvent::ModifiersChanged(m) = event {
                    self.mods = translate::mods(m.state());
                }
                let any = t.events[0].is_some();
                for ev in t.iter() {
                    self.session.post(ev);
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

// ---- M32: running a described program ------------------------------

pub use lumen_static_shell::program::{application, Program};

/// Open a window on a described program and run until it closes — M32.
///
/// An extension trait because the description lives in the shell
/// (`docs/plan-program.md` §2.4): a hot library must be able to build
/// one without linking a window system, and a shell test must be able to
/// call [`Program::host`] without one either.
pub trait Run {
    fn run(self);
}

#[cfg(not(feature = "hot"))]
impl<A: App + 'static> Run for Program<A> {
    fn run(self) {
        run_program(self, |p| Box::new(Local::from_program(p, RECOVER)), None);
    }
}

/// With `hot`, a program's application must be `Reloadable`: the static
/// application runs at once and its kept state carries into the first
/// hot build when it lands (`docs/plan-window-reload.md` §2.1).
#[cfg(feature = "hot")]
impl<A: App + lumen_static::reload::Reloadable + 'static> Run for Program<A> {
    fn run(self) {
        let dir = self.source_root().and_then(hot::locate);
        if dir.is_none() {
            eprintln!("lumen-static: cannot find the application's crate — running without hot reload");
        }
        run_program(
            self,
            |p| {
                Box::new(
                    Local::from_program(p, RECOVER)
                        .with_saver(|a| lumen_static::reload::to_vec(&lumen_static::reload::Reloadable::save(a))),
                )
            },
            dir,
        );
    }
}

fn run_program<A: App + 'static>(
    mut program: Program<A>,
    session: impl FnOnce(Program<A>) -> Box<dyn Session>,
    hot: Option<std::path::PathBuf>,
) {
    // `LUMEN_SHOT=frame.ppm`: one headless frame from the same `host()`
    // the window would drive, then exit. Every program gets it without
    // writing a `--shot` of its own, which is what let M32/T32.1 compare
    // an example's frame before and after its migration.
    if let Ok(path) = std::env::var("LUMEN_SHOT") {
        let (w, h, page) = (program.width, program.height, program.page);
        return shot(program.host(), w, h, page, &path);
    }
    // **The same `host()` a `--shot` calls** — `Local::from_program` boots
    // through it — so the window and every headless frame are built by
    // one function, the property M32 exists for.
    let config = Config {
        title: "",
        width: program.width,
        height: program.height,
        page: program.page,
        backend: program.backend,
        palette: program.palette,
        images: program.images.take(),
        agent: program.agent.take(),
    };
    start(session(program), config, hot);
}

/// Render one frame headlessly and write it as a PPM — `LUMEN_SHOT`.
///
/// Two pumps, as every example's own `--shot` does: the first teaches a
/// scroller how far its content goes, the second is the frame.
fn shot<A: App>(mut host: Host<A, Ret, Inval>, w: u32, h: u32, page: Color, path: &str) {
    use lumen_static_shell::software::Canvas;
    let font = FontStack::system();
    let mut canvas = match font.clone() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    let mut metrics = font.map(FontMetrics::new);
    for _ in 0..2 {
        match metrics.as_mut() {
            Some(m) => {
                host.pump_with(m);
            }
            None => {
                host.pump();
            }
        }
    }
    canvas.clear(page);
    host.render(&mut canvas);
    std::fs::write(path, canvas.to_ppm()).expect("LUMEN_SHOT: write frame");
}

// ---- M37: hot reload inside the window ------------------------------

/// **A window that rebuilds its application and swaps to it** — M37/T37.2
/// (`docs/plan-window-reload.md` §2.6).
///
/// A builder thread runs `cargo rustc --lib --crate-type cdylib` for the
/// application's crate whenever a watcher sees one of its sources change;
/// its outcome reaches the loop as a [`UserEvent`]; and the swap happens
/// at the next frame boundary, through [`lumen_static_reload::hot::Reloader`],
/// which keeps the running library when a build fails, is refused, or
/// panics while booting — and shows why, in a bar over the application.
#[cfg(feature = "hot")]
pub mod hot {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;

    use lumen_static::display::{Color, DisplayList};
    use lumen_static::retain::Damage;
    use lumen_static_reload::hot::{Crate, Reloader};
    use lumen_static_shell::session::{FrameSource, Session};
    use lumen_static_shell::text::FontMetrics;

    use crate::UserEvent;

    /// The bar's colours: a dark red that no application page is, and
    /// text on it that reads (contrast ≥ 7:1 on `BAR`).
    pub const BAR: Color = Color(0x5c, 0x10, 0x14, 0xf0);
    pub const BAR_TEXT: Color = Color(0xff, 0xe4, 0xe1, 0xff);

    /// The bar's first line when a build or swap did not take.
    pub const FAILED: &str = "reload failed — the last good build is still running";
    /// …and when a swap took, but reset kept state that no longer fits
    /// (§2.4).
    pub const RESET: &str = "reloaded — kept state that no longer fits has its boot value";

    /// Find the application's crate from the program's `source_root` —
    /// the compiler's spelling of its directory, relative to where cargo
    /// ran — by looking under the current directory and above the
    /// running binary.
    pub fn locate(source_root: &str) -> Option<PathBuf> {
        let root = Path::new(source_root);
        if root.is_absolute() {
            return root.join("Cargo.toml").exists().then(|| root.to_path_buf());
        }
        let mut bases: Vec<PathBuf> = Vec::new();
        if let Ok(d) = std::env::current_dir() {
            bases.push(d);
        }
        if let Ok(exe) = std::env::current_exe() {
            bases.extend(exe.ancestors().map(Path::to_path_buf));
        }
        bases.into_iter().map(|b| b.join(root)).find(|d| d.join("Cargo.toml").exists())
    }

    /// The crate to rebuild: features from `LUMEN_HOT_FEATURES` (default
    /// `hot`), the profile this binary was built with, and a target
    /// directory of its own beside the binary's (`target/reload`), so a
    /// library build never invalidates the running binary's artifacts.
    pub fn crate_at(dir: &Path) -> Result<Crate, String> {
        let features = std::env::var("LUMEN_HOT_FEATURES").unwrap_or_else(|_| "hot".into());
        let target = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().and_then(Path::parent).map(|t| t.join("reload")))
            .unwrap_or_else(|| dir.join("target").join("reload"));
        Crate::at(dir, &features, !cfg!(debug_assertions), target)
    }

    /// Start the builder thread. Each message on the returned sender asks
    /// for a build; **messages that arrive during a build coalesce into
    /// exactly one more**, because the thread drains the queue before it
    /// starts the next.
    pub fn builder(krate: Crate, proxy: winit::event_loop::EventLoopProxy<UserEvent>) -> mpsc::Sender<()> {
        let (tx, rx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            while rx.recv().is_ok() {
                while rx.try_recv().is_ok() {}
                if proxy.send_event(UserEvent::Building).is_err() {
                    return;
                }
                let outcome = krate.build();
                if proxy.send_event(UserEvent::Built(outcome)).is_err() {
                    return;
                }
            }
        });
        tx
    }

    /// Watch the crate's `src/` and ask for a build when a `.rs` file
    /// changes. Kept alive by the caller; dropping it stops the watch.
    pub fn watch(dir: &Path, build: mpsc::Sender<()>) -> Option<notify::RecommendedWatcher> {
        use notify::{EventKind, RecursiveMode, Watcher};
        let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            let relevant = matches!(ev.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_))
                && ev.paths.iter().any(|p| p.extension().is_some_and(|e| e == "rs"));
            if relevant {
                let _ = build.send(());
            }
        })
        .ok()?;
        w.watch(&dir.join("src"), RecursiveMode::Recursive).ok()?;
        Some(w)
    }

    /// The window's reload state.
    pub struct Hot {
        pub reloader: Reloader,
        pub build: mpsc::Sender<()>,
        pub waker: std::sync::Arc<dyn lumen_static::inbox::Waker>,
        pub metrics: Option<FontMetrics>,
        pub _watcher: Option<notify::RecommendedWatcher>,
        /// Whether the last frame drew the bar.
        pub bar_shown: bool,
    }

    /// The session's frame with the error bar drawn over its foot — the
    /// last good build keeps running under it (rule 2). Copies the list,
    /// which is only while a failure is being shown.
    pub struct Barred {
        list: DisplayList,
    }

    impl Barred {
        pub fn new(
            session: &mut dyn Session,
            head: &str,
            error: &str,
            w: u32,
            h: u32,
            metrics: &mut Option<FontMetrics>,
        ) -> Barred {
            let src = session.list();
            let mut list = DisplayList { cmds: src.cmds.clone(), text: src.text.clone(), bounds: src.bounds };
            // The bar's own damage is the whole surface: the session's
            // damage was computed without it.
            let _ = session.take_damage_for_age(1);
            let (wf, hf) = (w as f32, h as f32);
            let px = 14.0 * session.scale();
            let lh = px * 1.4;
            let pad = 10.0 * session.scale();
            let avail = (wf - 2.0 * pad).max(1.0);
            let mut lines = Vec::new();
            lines.push(head.to_string());
            for l in error.lines().filter(|l| !l.trim().is_empty()).take(8) {
                let wrapped = match metrics.as_mut() {
                    Some(m) => lumen_static_shell::recover::wrap(l, avail, px, m),
                    None => lumen_static_shell::recover::wrap(l, avail, px, &mut lumen_static::cx::BlockMetrics),
                };
                lines.extend(wrapped);
            }
            lines.truncate(10);
            let bar_h = pad * 2.0 + lh * lines.len() as f32;
            let top = (hf - bar_h).max(0.0);
            list.rect([0.0, top, wf, hf], BAR);
            let mut y = top + pad;
            for (i, l) in lines.iter().enumerate() {
                let weight = if i == 0 { 700.0 } else { 400.0 };
                list.text_weighted([pad, y, pad + avail, y + lh], l, px, weight, BAR_TEXT);
                y += lh;
            }
            Barred { list }
        }
    }

    impl FrameSource for Barred {
        fn list(&self) -> &DisplayList {
            &self.list
        }
        fn take_damage_for_age(&mut self, _age: u8) -> Damage {
            Damage::All
        }
        fn timings(&self) -> (u64, u64) {
            (0, 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R7-F10: every period `Timers::every` accepts must give a control
    /// flow, not a panic, and an ordinary one must give its instant.
    #[test]
    fn control_flow_for_takes_every_accepted_deadline() {
        let now = Instant::now();
        for ms in [f32::MAX, 1e30, 1e22, 1e20] {
            let flow = control_flow_for(now, Some(ms));
            println!("deadline {ms:e} ms -> {flow:?}");
        }
        match control_flow_for(now, Some(120.0)) {
            ControlFlow::WaitUntil(t) => {
                let got = t.duration_since(now).as_secs_f64() * 1000.0;
                println!("120 ms -> WaitUntil(now + {got:.3} ms) (bound 120 +- 1)");
                assert!((got - 120.0).abs() <= 1.0, "120 ms armed {got:.3} ms");
            }
            other => panic!("120 ms gave {other:?}, not WaitUntil"),
        }
        assert_eq!(control_flow_for(now, None), ControlFlow::Wait, "no timer must wait without a deadline");
    }
}
