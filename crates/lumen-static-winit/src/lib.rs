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
/// from the front door; the stylesheet, `Config::images`,
/// `Config::agent` and the overlay sink were the first four.
///
/// **`Boundaries` by default since M34/T34.2.** The evidence the flip
/// waited for is `docs/plan-boundaries-default.md` T34.0–T34.1: every
/// gate that runs a window now runs both strategies, the oracle runs at
/// this exact type, and a shadow oracle compared a live showcase session
/// without a divergence. The `everything` feature selects the whole-tree
/// walk instead — **a diagnostic**: if a stale frame goes away when you
/// build with it, you have found an invalidation defect.
///
/// 256 marks because `Boundaries<N>` stores ranges, not nodes, and M26/
/// T26.5 fixed it wrapping its length at exactly that count.
#[cfg(not(feature = "everything"))]
pub type Inval = lumen_static::Boundaries<256>;
#[cfg(feature = "everything")]
pub type Inval = lumen_static::Everything;

/// The retention this crate's windows use. Named so `Host<A, Ret, Inval>`
/// reads the same at every site.
pub type Ret = lumen_static::retain::Incremental;

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
    start(host, config, None);
}

fn start<A: App + 'static>(host: Host<A, Ret, Inval>, config: Config, program: Option<Program<A>>) {
    // **Always a user-event loop** — M28/T28.1.
    //
    // This was `with_user_event()` under the `agent` feature and
    // `EventLoop::new()` without it. The loop's type is fixed at
    // construction, so the two could not be chosen at runtime, and the
    // consequence was that an ordinary build had no way to be woken from
    // another thread at all. `ControlFlow::Wait` then means a completion
    // posted by a worker waits for the user to move the mouse.
    //
    // The cost of doing it unconditionally is one enum discriminant in
    // the loop's queue type; `UserEvent` carries an `Rpc` only when the
    // agent feature is on.
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    #[cfg(feature = "agent")]
    if let Some(addr) = config.agent.clone() {
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || agent::serve(addr, proxy));
    }
    let mut win = Win::new(host, config, program);
    // **Deferred work, wired up** — M28/T28.12.
    //
    // A windowed shell installs both halves: the waker, so a completion
    // rouses a loop sleeping in `ControlFlow::Wait` (T28.1), and the
    // default spawner, so an application that asks for work gets it
    // (T28.3). Neither is installed by `Host::new`, because a headless
    // host has no loop to wake and a test wants to choose its own
    // spawner.
    //
    // `Threads` is the default because `docs/plan-tasks.md` §0.3 is
    // explicit that no figure in this repository says what an async
    // runtime would cost — so the default is the one whose cost is
    // known and whose crate count is zero.
    win.host.inbox.set_waker(std::sync::Arc::new(ProxyWaker::new(event_loop.create_proxy())));
    win.host.set_spawner(Box::new(lumen_static::spawn::Threads));
    event_loop.run_app(&mut win).expect("run");
}

/// The turnkey handler behind [`run`].
struct Win<A: App> {
    host: Host<A, Ret, Inval>,
    config: Config,
    /// The description the window was started from, when it was — M32.
    /// Kept for its title, which may be a function of the state.
    program: Option<Program<A>>,
    /// The title the window is showing, so an unchanged one is not set
    /// again.
    title_shown: String,
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
    fn new(host: Host<A, Ret, Inval>, config: Config, program: Option<Program<A>>) -> Self {
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
            program,
            title_shown: String::new(),
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
                    // Guarded: paste runs the focused widget's editing code.
                    let _ = guarded(&mut self.host, |h| h.paste(&text));
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
        // **A panic is being shown** (M36): present the screen and run no
        // application code — running it would be an automatic retry.
        if failed(&self.host) {
            show_failure(&mut self.host, &mut self.metrics, size.width.max(1), size.height.max(1));
            backend.present(&mut self.host, self.config.page, start);
            return;
        }
        // Animation, before the pump: a tick changes state, and the
        // bindings the pump runs are what put that state on screen.
        let dt = self.last_frame.replace(start).map_or(0.0, |t| {
            start.duration_since(t).as_secs_f32() * 1000.0
        });
        // **One guarded entry for the frame's application code** (M36):
        // both ticks and the pump. A panic in any of them is caught, the
        // host is reset, and the screen below replaces the frame.
        let metrics = &mut self.metrics;
        let ran = guarded(&mut self.host, |h| {
            h.tick(dt);
            match metrics.as_mut() {
                Some(m) => h.pump_with(m),
                None => h.pump(),
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
            h.tick(0.0)
        });
        let Some(animating) = ran else {
            show_failure(&mut self.host, &mut self.metrics, size.width.max(1), size.height.max(1));
            backend.present(&mut self.host, self.config.page, start);
            return;
        };
        backend.present(&mut self.host, self.config.page, start);
        // A title that is a function of the state is re-asked after every
        // frame and set only when it changed (`docs/plan-program.md`
        // §2.1). A constant one compares equal every time.
        let title = self.title_now();
        if title != self.title_shown {
            window.set_title(&title);
            self.title_shown = title.into_owned();
        }
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
            // Guarded: the journal walks the tree through every widget's
            // `observe` (M36).
            let agent = &mut self.agent;
            let _ = guarded(&mut self.host, |h| {
                let timing = lumen_static_agent::FrameTiming::default();
                let mut tree = lumen_static_agent::TreeMut::of_root(h.app.tree_mut(), &h.styles, &h.states);
                agent.frame_done(&mut tree, timing);
            });
        }

        // Another frame only while something is moving. A still UI
        // idles, which is what keeps the idle frame numbers in
        // `docs/results-frame-cost.md` true.
        // **A deadline is a reason to redraw too** — M28/T28.11.
        //
        // `ControlFlow::Wait` sleeps until input arrives, so a timer on
        // an idle window would never be checked. `WaitUntil` is what
        // makes a deadline a deadline rather than a hope; nothing
        // pending falls back to `Wait`, which is today's behaviour and
        // what keeps the idle frame numbers in
        // `docs/results-frame-cost.md` true.
        //
        // `more_deferred` is the other reason: the inbox drain is capped
        // per frame (T28.2), so a backlog asks for another frame instead
        // of the cap being raised.
        // `more_deferred` joins `animating` here — M28/T28.2. The inbox
        // drain is capped per frame, so a backlog asks for another frame
        // instead of the cap being raised.
        if animating || self.host.more_deferred() {
            window.request_redraw();
        }
    }
}

impl<A: App> ApplicationHandler<UserEvent> for Win<A> {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        Win::resumed_impl(self, e);
    }
    fn window_event(&mut self, e: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        Win::window_event_impl(self, e, id, event);
    }

    /// Anything that reached the loop from off the window's thread.
    ///
    /// **One impl since M28/T28.1**, where there were two: one over
    /// `agent::UserEvent` under that feature and one over winit's
    /// default `()` without it.
    /// **What the loop waits on** — M28/T28.11.
    ///
    /// `ControlFlow::Wait` sleeps until input arrives, so a timer on an
    /// idle window would never be checked: a deadline needs a frame, and
    /// an idle window produces none. `WaitUntil` is what makes a
    /// deadline a deadline rather than a hope.
    ///
    /// Nothing pending falls back to `Wait`, which is today's behaviour
    /// and what keeps the idle frame numbers in
    /// `docs/results-frame-cost.md` true — this changes when a still
    /// window wakes, never how often a busy one draws.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.host.next_deadline_ms() {
            Some(ms) => event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now()
                    + std::time::Duration::from_secs_f32(ms.max(0.0) / 1000.0),
            )),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

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
        }
    }
}

#[cfg(feature = "agent")]
impl<A: App> Win<A> {
    /// Answer one agent request **on the event loop's own thread**, with
    /// the tree and the backend in hand.
    ///
    /// Split out of `user_event` by M28/T28.1: the handler now has two
    /// arms and only one of them exists under this feature, so the body
    /// moved to where the feature gate can sit on a whole `impl` rather
    /// than on statements inside a function.
    fn answer_agent(&mut self, rpc: agent::Rpc) {
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

impl<A: App> Win<A> {
    fn title_now(&self) -> std::borrow::Cow<'static, str> {
        match &self.program {
            Some(p) => p.title_of(&self.host.app),
            None => std::borrow::Cow::Borrowed(self.config.title),
        }
    }

    fn resumed_impl(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title(self.title_now().into_owned())
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
        // **While a panic is shown** (M36 §2.2): close, resize and redraw
        // as ever; **R retries**; everything else is swallowed, so nothing
        // piles up in the queue to replay into the application on retry.
        if failed(&self.host) {
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
                    retry(&mut self.host);
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
                // Guarded: composition runs the focused widget's editing
                // code (M36).
                let changed = guarded(&mut self.host, |h| match ime {
                    Ime::Preedit(text, _) => h.compose(Composition::Preedit(text)),
                    Ime::Commit(text) => h.compose(Composition::Commit(text)),
                    Ime::Disabled => h.compose(Composition::Cancel),
                    Ime::Enabled => false,
                })
                .unwrap_or(true);
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

impl<A: App + 'static> Run for Program<A> {
    fn run(mut self) {
        // `LUMEN_SHOT=frame.ppm`: one headless frame from the same
        // `host()` the window would drive, then exit. Every program gets
        // it without writing a `--shot` of its own, which is what let
        // M32/T32.1 compare an example's frame before and after its
        // migration.
        if let Ok(path) = std::env::var("LUMEN_SHOT") {
            let (w, h, page) = (self.width, self.height, self.page);
            return shot(self.host(), w, h, page, &path);
        }
        // **The same `host()` a `--shot` calls**, so the window and every
        // headless frame are built by one function — the property M32
        // exists for. Nothing here installs a sheet of its own.
        let host: Host<A, Ret, Inval> = self.host();
        let config = Config {
            title: "",
            width: self.width,
            height: self.height,
            page: self.page,
            backend: self.backend,
            palette: self.palette,
            images: self.images.take(),
            agent: self.agent.take(),
        };
        start(host, config, Some(self));
    }
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

// ---- M36: a panic shows where it happened, and the window survives ----

/// Whether this window guards application code — M36
/// (`docs/plan-panic-recovery.md` rule 3): a debug build does, a release
/// build does with the `reload` feature, and never where panics abort.
#[cfg(all(panic = "unwind", any(debug_assertions, feature = "reload")))]
pub(crate) fn guarded<A: App, T>(
    host: &mut Host<A, Ret, Inval>,
    f: impl FnOnce(&mut Host<A, Ret, Inval>) -> T,
) -> Option<T> {
    host.guard(f)
}

#[cfg(not(all(panic = "unwind", any(debug_assertions, feature = "reload"))))]
pub(crate) fn guarded<A: App, T>(
    host: &mut Host<A, Ret, Inval>,
    f: impl FnOnce(&mut Host<A, Ret, Inval>) -> T,
) -> Option<T> {
    Some(f(host))
}

/// Whether a panic is being shown.
pub(crate) fn failed<A: App>(host: &Host<A, Ret, Inval>) -> bool {
    #[cfg(all(panic = "unwind", any(debug_assertions, feature = "reload")))]
    return host.failure().is_some();
    #[cfg(not(all(panic = "unwind", any(debug_assertions, feature = "reload"))))]
    {
        let _ = host;
        false
    }
}

/// Put the panic screen in the host's list, sized to the surface.
pub(crate) fn show_failure<A: App>(host: &mut Host<A, Ret, Inval>, metrics: &mut Option<FontMetrics>, w: u32, h: u32) {
    #[cfg(all(panic = "unwind", any(debug_assertions, feature = "reload")))]
    if let Some(r) = host.failure().cloned() {
        use lumen_static_shell::recover::failure_frame;
        let list = match metrics.as_mut() {
            Some(m) => failure_frame(&r, w, h, m),
            None => failure_frame(&r, w, h, &mut lumen_static::cx::BlockMetrics),
        };
        host.show(list);
    }
    #[cfg(not(all(panic = "unwind", any(debug_assertions, feature = "reload"))))]
    let _ = (host, metrics, w, h);
}

/// Retry after a panic: the next guarded entry runs again.
pub(crate) fn retry<A: App>(host: &mut Host<A, Ret, Inval>) -> bool {
    #[cfg(all(panic = "unwind", any(debug_assertions, feature = "reload")))]
    {
        host.retry();
        true
    }
    #[cfg(not(all(panic = "unwind", any(debug_assertions, feature = "reload"))))]
    {
        let _ = host;
        false
    }
}
