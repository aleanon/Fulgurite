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
//! the one difference is which line [`Backend::name`] prints. A machine
//! whose only adapter is a CPU rasteriser (Mesa's llvmpipe) counts as one
//! with no driver under `Prefer::Auto`, since M51/T51.2. See [`Prefer`]
//! and `docs/results-gpu.md` §6.
#![forbid(unsafe_code)]

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
pub mod keys;

pub use backend::{Backend, Prefer, Software};
pub use clipboard::Clipboard;
pub use dialog::Dialogs;
#[cfg(feature = "gpu")]
pub use gpu::GpuBackend;
pub use windows::{Pane, Panes};
pub use surface::{canvas, sync_size, Frame, Surface};
pub use translate::{Cursor, Translated};

/// The backend half of `resumed`: make a backend and hand it the images —
/// extracted verbatim by M47/T47.0 so a unit test can drive it with a
/// fake backend (R15-F6).
///
/// Taken, not cloned: `ImageSource` is a trait object and the renderer
/// is where the pixels are wanted.
fn resume_backend(
    images: &mut Option<Box<dyn lumen_static::ImageSource>>,
    make: impl FnOnce() -> Box<dyn Backend>,
) -> Box<dyn Backend> {
    let mut backend = make();
    if let Some(images) = images.take() {
        backend.set_images(images);
    }
    backend
}

/// **Whether a dropped present asks for another frame** — M47/T47.4
/// (R7-F7).
///
/// An idle window asks for no frame of its own, so a present the surface
/// refused (outdated, lost, timed out) would leave the window unpainted
/// until input arrived. On a drop this asks again, up to
/// [`MAX_PRESENT_RETRIES`] in a row, so a surface that never comes back
/// cannot spin the loop; a present that lands resets the count. Returns
/// whether to request a redraw.
pub fn present_retry(dropped: bool, streak: &mut u32) -> bool {
    if !dropped {
        *streak = 0;
        return false;
    }
    *streak += 1;
    if *streak == MAX_PRESENT_RETRIES + 1 {
        eprintln!(
            "lumen-static: {} presents dropped in a row; waiting for the next event",
            *streak
        );
    }
    *streak <= MAX_PRESENT_RETRIES
}

/// **Whether a frame is presented** — M53/T53.4
/// (`docs/plan-frame-cost.md` §2.3(b)).
///
/// `own` is whether the window asked for this redraw itself (input, a
/// wake, a deadline, an agent answer) and nothing forced a present — a
/// resize, or the retry of a dropped one. `forced` is a panic screen or
/// a reload bar, which are pixels the session's damage does not see.
/// Only an own, unforced frame whose `damage` is `None` is skipped;
/// every redraw the platform asked for presents, because winit 0.30
/// does not say whether it is an expose (`surface::Surface::present`),
/// and an expose means the server lost the pixels.
///
/// **What it cannot see**: an expose that arrives while an own request
/// is pending is coalesced into one `RedrawRequested`, and is skipped if
/// the frame changed nothing. Under a compositor there are no exposes;
/// without one, the next frame that changes anything repaints.
pub fn present_needed(own: bool, forced: bool, damage: lumen_static::retain::Damage) -> bool {
    !own || forced || damage != lumen_static::retain::Damage::None
}

/// How many dropped presents in a row [`present_retry`] asks again for.
pub const MAX_PRESENT_RETRIES: u32 = 3;

/// The backend half of `suspended`: take the images back, then drop the
/// backend — M47/T47.7 (R15-F6). What [`resume_backend`] took out of
/// `images`, this puts back, so the next resume hands it on again.
fn suspend_backend(images: &mut Option<Box<dyn lumen_static::ImageSource>>, mut backend: Box<dyn Backend>) {
    if let Some(i) = backend.take_images() {
        *images = Some(i);
    }
    drop(backend);
}

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
/// **A machine whose only adapter is a CPU rasteriser is one with no
/// GPU**, under [`Prefer::Auto`] (M51/T51.2, R9-F6). Where Mesa is
/// installed and no driver works, wgpu finds llvmpipe, and until M51
/// `Auto` took it; the software canvas is faster there. [`Prefer::Gpu`]
/// still takes it, because the person asked. [`choose_backend`] is the
/// decision.
///
/// Without the `gpu` feature this is always the software canvas, and
/// `prefer` only decides whether to say so.
fn pick_backend(
    window: Arc<Window>,
    font: Option<FontStack>,
    prefer: Prefer,
) -> Box<dyn Backend> {
    #[cfg(feature = "gpu")]
    {
        choose_backend(
            prefer,
            |accept_cpu| {
                gpu::GpuBackend::new(window.clone(), font.clone(), accept_cpu)
                    .map(|g| Box::new(g) as Box<dyn Backend>)
            },
            || Box::new(Software::new(window.clone(), font.clone())),
        )
    }
    #[cfg(not(feature = "gpu"))]
    {
        if prefer == Prefer::Gpu {
            eprintln!("gpu requested, but this binary was built without the `gpu` feature");
        }
        Box::new(Software::new(window, font))
    }
}

/// Which backend `prefer` yields — [`pick_backend`]'s decision, with the
/// two constructors passed in so it can be tested without a window.
///
/// `gpu` is called with **whether a CPU adapter is acceptable**, which is
/// `true` for [`Prefer::Gpu`] only (M51/T51.2, R9-F6): under
/// [`Prefer::Auto`] a machine whose only adapter is Mesa's llvmpipe keeps
/// the software canvas, which the review measured 3.3× faster at 1080p
/// and 160 MB lighter (`docs/results-gpu.md` §6). It is not called at all
/// for [`Prefer::Software`]. `None` from it falls back, with one line
/// saying so.
#[doc(hidden)]
pub fn choose_backend<B>(
    prefer: Prefer,
    gpu: impl FnOnce(bool) -> Option<B>,
    software: impl FnOnce() -> B,
) -> B {
    if prefer != Prefer::Software {
        match gpu(prefer == Prefer::Gpu) {
            Some(g) => return g,
            None => eprintln!("no usable GPU adapter — falling back to the software canvas"),
        }
    }
    software()
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
    /// **Held across suspend and resume** (M47/T47.7): handed to each
    /// backend `resumed` makes, and taken back from it by `suspended`
    /// (`Backend::take_images`) before it is dropped. It was "taken once",
    /// and a second `resumed` got none (R15-F6). A `None` here is not an
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
    /// `None` is inert **with the feature on as well** (M48/T48.12): no
    /// socket, and no journal or audit on any frame — they ran on every
    /// frame of every agent build until then. Without the feature the
    /// field is inert and nothing of the agent is linked, which is what
    /// keeps a release build free of `observe`.
    ///
    /// **One JSON object per line, or the connection ends** (M39/T39.1):
    /// the endpoint is a local line protocol and refuses HTTP by framing,
    /// so a web page open in a browser on this machine cannot drive the
    /// window. It is not authenticated: any process running as the same
    /// user can connect (`docs/00-handoff-static-tree.md` §9).
    ///
    /// **`LUMEN_AGENT_ADDR` overrides it** (M48/T48.11): the window reads
    /// the variable in one place, and a script sets it to choose a port;
    /// this field is the program's default. With neither, no endpoint.
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
use lumen_static_shell::session::{Local, Session};

/// Whether this window catches panics in application code — M36
/// (`docs/plan-panic-recovery.md` rule 3, as amended by M52/T52.7): a
/// debug build does, a release build does with the `reload` feature, and
/// never where panics abort.
///
/// **A type, not a value** ([`lumen_static_shell::session::RecoverPolicy`]).
/// Until M52 this was a constant handed to a runtime field, so a
/// release window without `reload` linked every `recover::` function
/// though none could run; `just release-window-shape` counts 0 now.
///
/// Decided **here**, in the window crate, and handed to the session:
/// the workspace's dev profile builds the shell with `debug-assertions =
/// false`, so a `cfg(debug_assertions)` evaluated there was false in
/// every debug build.
#[cfg(all(panic = "unwind", any(debug_assertions, feature = "reload")))]
pub use lumen_static_shell::session::Recovering as Policy;
#[cfg(not(all(panic = "unwind", any(debug_assertions, feature = "reload"))))]
pub use lumen_static_shell::session::Crashing as Policy;

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
    let session = Local::from_host(host, config.title, config.page, Policy);
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
    let config = Config { agent: agent_address(config.agent.clone()), ..config };
    #[cfg(feature = "agent")]
    if let Some(addr) = config.agent.clone() {
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || agent::serve(addr, proxy));
    }
    let mut win = Win::new(session, config);
    // M48/T48.12: no endpoint, no journal.
    win.journal = cfg!(feature = "agent") && win.config.agent.is_some();
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

/// **Where the agent endpoint is served** — M48/T48.11 (R13-F10,
/// `docs/plan-agent-truth.md` §2.5), read in this one place.
///
/// `LUMEN_AGENT_ADDR` wins, so a script can choose a port; the program's
/// own address (`Program::agent`, `Config::agent`) is the default; with
/// neither there is no endpoint. Launchers read `LUMEN_AGENT` and the
/// client read `LUMEN_AGENT_ADDR` until then, so `LUMEN_AGENT_ADDR=…
/// showcase` bound one port while the client dialled another.
/// `LUMEN_AGENT` is honoured for one release, with a line on stderr.
#[cfg(feature = "agent")]
fn agent_address(program: Option<String>) -> Option<String> {
    if let Some(a) = std::env::var("LUMEN_AGENT_ADDR").ok().filter(|a| !a.is_empty()) {
        return Some(a);
    }
    if let Some(a) = std::env::var("LUMEN_AGENT").ok().filter(|a| !a.is_empty()) {
        eprintln!(
            "lumen-static: LUMEN_AGENT is deprecated and will stop working next release -- \
             set LUMEN_AGENT_ADDR={a} instead"
        );
        return Some(a);
    }
    program
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
    /// **A test hook, debug builds only** — M47/T47.7. With
    /// `LUMEN_TEST_RESUME=<ms>` set, the window runs its own `suspended`
    /// and `resumed` once, `<ms>` after it starts, so a live drive can
    /// compare the window before and after a lifecycle round trip on a
    /// desktop, where the platform never sends one.
    #[cfg(debug_assertions)]
    test_resume: Option<Instant>,
    /// Dropped presents in a row, for [`present_retry`] (M47/T47.4).
    present_drops: u32,
    /// The pending redraw was this window's own request — input, a wake,
    /// a deadline, an agent answer — and not only the platform's
    /// (M53/T53.4). See [`present_needed`].
    own_redraw: bool,
    /// The next frame presents whatever its damage says: a resize, or
    /// the retry of a dropped present, whose damage was banked by the
    /// present that did not land.
    must_present: bool,
    /// Frames presented, and redraws that presented nothing because
    /// nothing changed — `app.presents` (M53/T53.4).
    presents: (u64, u64),
    /// The reload state, when the window rebuilds its application (M37).
    #[cfg(feature = "hot")]
    hot: Option<hot::Hot>,
    /// Whether an endpoint was configured — M48/T48.12 (R14-F8). The
    /// journal and the audit run only then: an agent build with no
    /// address paid for both on every frame, for answers nothing could
    /// ask for.
    journal: bool,
    /// `app.quit` was answered: leave the loop at the next
    /// `about_to_wait`, which has it in hand (M48/T48.13).
    quit: bool,
    /// Frames drawn and frames journaled, for `LUMEN_FRAME_REPORT`.
    frames: (u64, u64),
    /// `LUMEN_FRAME_REPORT=N`: after N frames, print how many were
    /// journaled and exit 0 — the diagnostic the T48.12 gate reads, since
    /// a window with no endpoint has nothing else to ask.
    report_after: Option<u64>,
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
            #[cfg(debug_assertions)]
            test_resume: std::env::var("LUMEN_TEST_RESUME")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .map(|ms| Instant::now() + std::time::Duration::from_millis(ms)),
            present_drops: 0,
            own_redraw: false,
            must_present: false,
            presents: (0, 0),
            #[cfg(feature = "hot")]
            hot: None,
            journal: false,
            quit: false,
            frames: (0, 0),
            report_after: std::env::var("LUMEN_FRAME_REPORT").ok().and_then(|v| v.parse().ok()),
        }
    }

    /// Ctrl+C, Ctrl+X and Ctrl+V (Cmd on macOS) — T14.5.
    ///
    /// Handled here rather than in a widget because the *clipboard* is
    /// the platform's; what the widget owns is the selection and the
    /// typing, and both of those are session calls. Returns whether the
    /// keystroke was acted on as a clipboard shortcut, so the caller
    /// knows not to route it on — and it is not, since M45/T45.9, when
    /// there is nothing to act on: `keys::clipboard_decision` decides.
    fn clipboard_shortcut(&mut self, event: &WindowEvent) -> bool {
        let WindowEvent::KeyboardInput { event, .. } = event else { return false };
        if event.state != winit::event::ElementState::Pressed {
            return false;
        }
        // The decision is `keys::clipboard_decision`, a function a test
        // can call (M45/T45.0 item 11). Only what it needs is read: the
        // selection for c/x, the clipboard for v.
        let key = translate::key_of(&event.logical_key);
        let copied = match key {
            Some(lumen_static::Key::Char('c' | 'x')) => self.session.copy(),
            _ => None,
        };
        let pasted = match key {
            Some(lumen_static::Key::Char('v')) => self.clipboard.get().filter(|t| !t.is_empty()),
            _ => None,
        };
        let decision = keys::clipboard_decision(keys::Shortcut {
            mods: self.mods,
            key,
            focused: self.session.has_focus(),
            selection: copied.is_some(),
            clipboard: pasted.is_some(),
            os: keys::Os::CURRENT,
        });
        match decision {
            None => false,
            Some(keys::Clip::Swallow) => true,
            Some(keys::Clip::Copy) | Some(keys::Clip::Cut) => {
                if let Some(text) = copied {
                    self.clipboard.set(&text);
                }
                if decision == Some(keys::Clip::Cut) {
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
            Some(keys::Clip::Paste) => {
                if let Some(text) = pasted {
                    self.session.paste(&text);
                }
                true
            }
        }
    }

    /// Ask for a frame of the window's own — M53/T53.4. The frame it
    /// gets presents only if it changed something ([`present_needed`]).
    fn ask_redraw(&mut self) {
        if let Some(w) = &self.window {
            self.own_redraw = true;
            w.request_redraw();
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(backend)) = (self.window.clone(), self.backend.as_mut()) else {
            return;
        };
        // Whose frame this is, taken now: a request made during the
        // frame is for the next one.
        let own = core::mem::take(&mut self.own_redraw) && !core::mem::take(&mut self.must_present);
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
                // M49/T49.10: at least once — a job with a side effect that
                // was running at the swap may perform it twice.
                let r = hot.reloader.reissued;
                if r.spawned + r.unknown > 0 {
                    eprintln!(
                        "lumen-static: {} task(s) in flight re-issued to the new build, {} dropped (no job for them)",
                        r.spawned, r.unknown
                    );
                }
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
        // **What the frame cost is kept** (M48/T48.3): `present` fills it
        // in, and it was dropped here, so `app.perf` read 0 for every
        // stage of every turnkey window (R13-F2).
        //
        // **A frame of our own that changed nothing presents nothing**
        // (M53/T53.4, R7-F6): a pointer crossing a region it changes
        // nothing in cost one pump and one whole-surface present per
        // motion. The pump is already skipped (T53.3); this skips the
        // blit and the present. The damage is not taken, so it stays
        // pending for the next present.
        #[cfg(feature = "hot")]
        let overlay = barred.is_some();
        #[cfg(not(feature = "hot"))]
        let overlay = false;
        let presented = if !present_needed(own, self.session.failed() || overlay, self.session.damage()) {
            self.presents.1 += 1;
            // The pump's own figures still stand (under `Everything` it
            // laid out and painted); nothing was rendered or presented.
            let (layout_ns, paint_ns) = self.session.timings();
            surface::Frame { layout_ns, paint_ns, frame_ns: start.elapsed().as_nanos() as u64, ..Default::default() }
        } else {
            self.presents.0 += 1;
            #[cfg(feature = "hot")]
            let presented = match barred {
                Some(mut b) => backend.present(&mut b, self.config.page, start),
                None => backend.present(&mut *self.session, self.config.page, start),
            };
            #[cfg(not(feature = "hot"))]
            let presented = backend.present(&mut *self.session, self.config.page, start);
            presented
        };
        // **A dropped present asks again** (M47/T47.4): the frame was
        // rendered and did not reach the screen, and nothing else will ask
        // on an idle window. Before the failure check, so a panic screen
        // whose present was dropped is shown too.
        if present_retry(presented.dropped, &mut self.present_drops) {
            // The pending damage went with the dropped present, so the
            // retry's own damage says nothing: it must present
            // (M53/T53.4).
            self.must_present = true;
            window.request_redraw();
        }
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
        //
        // **Only with an endpoint** (M48/T48.12): nothing else can read it.
        self.frames.0 += 1;
        if self.journal {
            self.session.agent_frame_done(frame_times(&presented));
            self.frames.1 += 1;
        }
        if self.report_after.is_some_and(|n| self.frames.0 >= n) {
            eprintln!(
                "lumen-static: {} frames drawn, {} journaled (endpoint: {})",
                self.frames.0,
                self.frames.1,
                self.config.agent.as_deref().unwrap_or("none")
            );
            std::process::exit(0);
        }

        // Another frame only while something is moving, or while the
        // inbox has a backlog (M28/T28.2: the drain is capped per frame,
        // so a backlog asks for another frame instead of the cap being
        // raised), or after a dropped present (above, M47/T47.4). A still
        // UI idles, which is what keeps the idle frame numbers in
        // `docs/results-frame-cost.md` true — and since M53/T53.4 a still
        // UI under a moving pointer idles too: each motion's redraw runs
        // the skipped pump (T53.3) and presents nothing
        // (`present_needed`), 0 presents for 200 motions in that file's
        // Idle section, where it was one per motion.
        if animating || self.session.more_deferred() {
            self.ask_redraw();
        }
    }
}

impl ApplicationHandler<UserEvent> for Win {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        Win::resumed_impl(self, e);
    }

    /// **The images come back before the backend goes** — M47/T47.7.
    ///
    /// `resumed` makes a new window and backend on every call, so a
    /// suspend must drop both — and take the image source back into
    /// `Config` first, or the next backend is handed none and every image
    /// vanishes (R15-F6). This is the desktop half of a lifecycle; surface
    /// recreation on Android stays with `docs/mobile-decision.md`.
    fn suspended(&mut self, _e: &ActiveEventLoop) {
        if let Some(backend) = self.backend.take() {
            suspend_backend(&mut self.config.images, backend);
        }
        self.window = None;
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
        if self.quit {
            event_loop.exit();
            return;
        }
        #[cfg(debug_assertions)]
        if let Some(at) = self.test_resume {
            if Instant::now() >= at {
                self.test_resume = None;
                eprintln!("lumen-static: LUMEN_TEST_RESUME: suspend and resume");
                self.suspended(event_loop);
                self.resumed_impl(event_loop);
            } else {
                // Wake for the hook as well as for the timers.
                let flow = match self.wait {
                    ControlFlow::WaitUntil(t) if t < at => ControlFlow::WaitUntil(t),
                    _ => ControlFlow::WaitUntil(at),
                };
                event_loop.set_control_flow(flow);
                return;
            }
        }
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
            self.ask_redraw();
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
                self.ask_redraw();
            }
            #[cfg(feature = "agent")]
            UserEvent::Agent(rpc) => self.answer_agent(rpc),
            #[cfg(feature = "hot")]
            UserEvent::Building => {
                if let Some(h) = self.hot.as_mut() {
                    h.reloader.building = true;
                }
                self.ask_redraw();
            }
            #[cfg(feature = "hot")]
            UserEvent::Built(outcome) => {
                if let Some(h) = self.hot.as_mut() {
                    h.reloader.outcome(outcome);
                }
                self.ask_redraw();
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
        // **`app.quit` is the window's** — M48/T48.13 (R13-F9). The skill
        // listed it and only counter-window had it; every turnkey window
        // answered -32601, which `agent_client.py quit` reported as
        // success. The reply goes first, then the loop exits at its next
        // `about_to_wait`. Answered whatever the session's state, so a
        // window showing a panic can still be closed by a drive.
        if rpc.req.get("method").and_then(|m| m.as_str()) == Some("app.quit") {
            let _ = rpc.reply.send(serde_json::json!({
                "jsonrpc": "2.0", "id": rpc.req.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": { "ok": true }
            }));
            self.quit = true;
            self.ask_redraw();
            return;
        }
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
                    // M49/T49.11: node indices from before the last swap
                    // address other nodes; resolve by author id again.
                    "shape_changed": r.is_some_and(|r| r.shape_changed),
                    // M49/T49.10: the old build's work in flight, handed on.
                    "reissued": r.map_or(0, |r| r.reissued.spawned),
                    "reissue_dropped": r.map_or(0, |r| r.reissued.unknown),
                }
            }));
            return;
        }
        // `app.presents` is the window's too (M53/T53.4): frames
        // presented, and redraws that presented nothing because nothing
        // changed. Answered without a frame, so asking does not move it.
        if rpc.req.get("method").and_then(|m| m.as_str()) == Some("app.presents") {
            let _ = rpc.reply.send(serde_json::json!({
                "jsonrpc": "2.0", "id": rpc.req.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": {"presents": self.presents.0, "skipped": self.presents.1},
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
        self.ask_redraw();
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

        let (font, prefer) = (self.font.clone(), Prefer::from_env(self.config.backend));
        let backend = resume_backend(&mut self.config.images, || pick_backend(window.clone(), font, prefer));
        eprintln!("lumen-static: {}", backend.name());
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
                    self.ask_redraw();
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
                self.ask_redraw();
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
                    self.ask_redraw();
                }
            }

            // A resize is the platform's: the surface was reallocated and
            // must be presented whatever the damage says (M53/T53.4).
            WindowEvent::Resized(_) => {
                self.must_present = true;
                self.redraw()
            }
            WindowEvent::RedrawRequested => self.redraw(),

            ref event if self.clipboard_shortcut(event) => {
                self.ask_redraw();
            }

            // A key press, through the same function a test drives
            // (`keys::deliver_key`, M45/T45.0 item 10).
            WindowEvent::KeyboardInput { event: ref k, .. } => {
                let down = k.state == winit::event::ElementState::Pressed;
                if keys::deliver_key(&mut *self.session, &k.logical_key, k.text.as_deref(), down, self.mods) {
                    self.ask_redraw();
                }
            }

            ref event => {
                // **An event that goes with a change of stacking presents**
                // (M53/T53.4). A window uncovered under the pointer gets
                // its EnterNotify and its Expose together, and winit
                // folds the expose's redraw into the one the enter asks
                // for — so without this, the frame the enter asked for
                // changed nothing, skipped its present, and left the
                // uncovered area as the server cleared it
                // (`just motion-present`'s expose case, first run).
                if matches!(
                    event,
                    WindowEvent::CursorEntered { .. }
                        | WindowEvent::CursorLeft { .. }
                        | WindowEvent::Focused(_)
                        | WindowEvent::Occluded(_)
                        | WindowEvent::Moved(_)
                ) {
                    self.must_present = true;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
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
                    self.ask_redraw();
                }
            }
        }
    }
}

/// A presented frame's cost as the session's agent wants it — M48/T48.3.
///
/// A frame that presented nothing (a zero-sized surface) measured
/// nothing, so every stage is `None` rather than the zeros `Frame`'s
/// default holds: `app.perf` reports `null` for it, never `0`. Input
/// latency is not measured by this window (`counter-window` measures it
/// itself), so it is `None` too.
pub(crate) fn frame_times(f: &Frame) -> lumen_static_shell::session::FrameTimes {
    if f.frame_ns == 0 {
        return Default::default();
    }
    // **0 is "this frame ran no such pass", not a measurement** —
    // M53/T53.3–T53.4: a pump the idle fast path skipped lays out and
    // paints nothing, and `app.perf` reads a 0 as a stage nobody timed
    // (M48/T48.3). So it is `None`, and the percentiles are over frames
    // that ran the pass.
    let ran = |ns: u64| (ns > 0).then_some(ns);
    lumen_static_shell::session::FrameTimes {
        layout_ns: ran(f.layout_ns),
        paint_ns: ran(f.paint_ns),
        render_ns: Some(f.render_ns),
        present_ns: Some(f.present_ns),
        frame_ns: Some(f.frame_ns),
        damaged_px: Some(f.damaged_px),
        input_latency_ns: None,
        atlas_give_ups: f.atlas_give_ups,
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
        run_program(self, |p| Box::new(Local::from_program(p, Policy)), None);
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
                    Local::from_program(p, Policy).with_saver(lumen_static_reload::hot::save::<A>),
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

    /// M51/T51.1: the GPU backend's atlas give-up count survives the
    /// window's hand-off to the session, which is what `app.perf` reports.
    #[test]
    fn frame_times_carries_the_atlas_give_up_count() {
        let f = surface::Frame { frame_ns: 1, atlas_give_ups: Some(7), ..Default::default() };
        let t = frame_times(&f);
        println!("Frame.atlas_give_ups {:?} -> FrameTimes.atlas_give_ups {:?}", f.atlas_give_ups, t.atlas_give_ups);
        assert_eq!(t.atlas_give_ups, Some(7));
        let sw = surface::Frame { frame_ns: 1, ..Default::default() };
        assert_eq!(frame_times(&sw).atlas_give_ups, None, "the software canvas has no atlas");
    }

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

    /// **The present decision** (M53/T53.4).
    #[test]
    fn only_an_own_unforced_frame_with_no_damage_skips_its_present() {
        use lumen_static::retain::Damage;
        let cases = [
            (true, false, Damage::None, false),
            (true, false, Damage::All, true),
            (false, false, Damage::None, true), // the platform's: maybe an expose
            (true, true, Damage::None, true),   // a panic screen or reload bar
        ];
        for (own, forced, damage, want) in cases {
            println!("own {own}, forced {forced}, damage {damage:?} -> present {want}");
            assert_eq!(present_needed(own, forced, damage), want, "own {own} forced {forced} {damage:?}");
        }
    }

    /// **The retry decision** (M47/T47.4): a drop asks again, up to the
    /// bound in a row; a present that lands resets the count.
    #[test]
    fn a_dropped_present_asks_again_a_bounded_number_of_times() {
        let mut streak = 0;
        let asks: Vec<bool> = (0..6).map(|_| present_retry(true, &mut streak)).collect();
        println!("six drops in a row ask again: {asks:?} (bound {MAX_PRESENT_RETRIES} in a row)");
        assert_eq!(asks, [true, true, true, false, false, false], "retry on a drop, stop after the bound");
        assert!(!present_retry(false, &mut streak), "a present that landed asks for nothing");
        assert_eq!(streak, 0, "and resets the count");
        assert!(present_retry(true, &mut streak), "so the next drop is retried again");
    }

    /// One pixel's worth of image source, so a backend's `set_images`
    /// can be seen to receive *something*.
    struct OnePixel;
    impl lumen_static::ImageSource for OnePixel {
        fn image(&self, _id: lumen_static::ImageId) -> Option<(u32, u32, &[u8])> {
            Some((1, 1, &[255, 0, 0, 255]))
        }
    }

    /// A backend that records, in a log shared with the test, which
    /// backend was handed images; and keeps them.
    struct Fake {
        id: usize,
        log: std::rc::Rc<std::cell::RefCell<Vec<usize>>>,
        images: Option<Box<dyn lumen_static::ImageSource>>,
    }
    impl Backend for Fake {
        fn resize(&mut self, _w: u32, _h: u32) -> bool {
            false
        }
        fn present(&mut self, _s: &mut dyn lumen_static_shell::session::FrameSource, _p: Color, _t: Instant) -> surface::Frame {
            surface::Frame::default()
        }
        fn set_images(&mut self, images: Box<dyn lumen_static::ImageSource>) {
            // Only a source that answers counts as images.
            if images.image(lumen_static::ImageId(0)).is_some() {
                self.log.borrow_mut().push(self.id);
            }
            self.images = Some(images);
        }
        fn take_images(&mut self) -> Option<Box<dyn lumen_static::ImageSource>> {
            self.images.take()
        }
        fn name(&self) -> &str {
            "fake"
        }
    }

    /// **R15-F6: a second `resumed` must still have the images.**
    ///
    /// `resumed` makes a new backend every time and hands it
    /// `config.images.take()`; there is no suspend handler, so the
    /// first backend's images go down with it and the second gets none.
    /// Driven here as resume, suspend, resume, printing what each backend
    /// was given. Until T47.7 the suspend was a bare drop, and backend #2
    /// printed `None`.
    #[test]
    fn a_second_resume_still_has_the_images() {
        let mut images: Option<Box<dyn lumen_static::ImageSource>> = Some(Box::new(OnePixel));
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut made = 0;
        for round in 1..=2 {
            let backend = resume_backend(&mut images, || {
                made += 1;
                Box::new(Fake { id: round, log: log.clone(), images: None })
            });
            let given = log.borrow().contains(&round);
            println!("backend #{round} images: {}", if given { "Some" } else { "None" });
            // What `suspended` does with it.
            suspend_backend(&mut images, backend);
        }
        assert_eq!(made, 2, "setup: two backends were made");
        assert_eq!(*log.borrow(), vec![1, 2], "backend #2 was given no images");
    }
}
