//! **A running application with its type erased** — M37/T37.1
//! (`docs/plan-window-reload.md` §1).
//!
//! A window drives a [`Host`], and a `Host<A>` is generic over the
//! application. For a window that reloads that cannot work: the tree
//! holds code (handler `fn`s, every widget's `layout` and `paint`
//! monomorphised over the application's types), so a `Host<A>` compiled
//! into the loader would be the loader's code forever. **The `Host`
//! lives in the library**, and the window drives it through [`Session`]:
//! input in, a frame out, the title, the cursor, the agent's requests.
//!
//! The same trait carries a statically linked application — [`Local`] —
//! so the window has one path, not a static one and a hot one that can
//! drift. A hot library hands the loader a `Box<dyn Session>` it built
//! itself, behind the ABI stamp (`lumen_static_reload::abi`), because
//! the loader borrows its values in place: the boundary is pointers, not
//! bytes, by measurement (`docs/step4-hot-reload.md`, T37.0).
//!
//! **Every method that runs application code catches a panic inside the
//! library** when recovery is on (M36): a hot library has its own std,
//! and a panic must not unwind across into the loader's.

use std::sync::Arc;

use lumen_static::display::{Color, DisplayList};
use lumen_static::event::{App, Composition};
use lumen_static::retain::Damage;
use lumen_static::style::Cursor;
use lumen_static::Event;

use crate::program::Program;
use crate::text::{FontMetrics, FontStack, Shaping};
use crate::Host;
use lumen_static::invalidate::Invalidation;

/// The retention a window's host uses (moved here from the window crate
/// by M37, so a hot library, which links no window system, builds the
/// same `Host` type).
pub type Ret = lumen_static::retain::Incremental;

/// The strategy a session runs when its program does not choose one
/// (M55/T55.4) — defined beside [`Program`], which carries the choice.
pub use crate::program::DefaultInval;

/// What a renderer reads from a frame: the list, how much of it is
/// stale for a surface of a given age, and what it cost to build.
pub trait FrameSource {
    fn list(&self) -> &DisplayList;
    fn take_damage_for_age(&mut self, age: u8) -> Damage;
    /// `(layout_ns, paint_ns)` of the last frame.
    fn timings(&self) -> (u64, u64);
}

impl<A: App, R: lumen_static::retain::Retention, I: lumen_static::invalidate::Invalidation> FrameSource
    for Host<A, R, I>
{
    fn list(&self) -> &DisplayList {
        &self.list
    }
    fn take_damage_for_age(&mut self, age: u8) -> Damage {
        Host::take_damage_for_age(self, age)
    }
    fn timings(&self) -> (u64, u64) {
        Host::timings(self)
    }
}

/// What the window's renderer offers a session: presenting a frame, and
/// its pixels back — for the agent, which lives with the tree.
pub trait Screen {
    /// Present the frame, and say what it cost — `damaged_px` above all,
    /// because a frame an agent's input pumped is presented **here**,
    /// inside the request, and the redraw after it usually finds nothing
    /// left to draw (M48/T48.4). `FrameTimes::default()` from a screen
    /// that measures nothing.
    fn present(&mut self, src: &mut dyn FrameSource, page: Color) -> FrameTimes;
    /// What the renderer drew, as RGBA8.
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])>;
    /// What the display server shows, when that is checkable.
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)>;
}

/// **A [`Screen`] with no window behind it** — M60/T60.0: a software
/// canvas the session's frames are drawn into, for a headless drive
/// through [`Session::agent`] (`input.*` presents here, `ui.screenshot`
/// reads it back). What a window's backend does, minus the window: the
/// damaged regions are cleared and replayed, as `Surface::present` does,
/// and `source: "surface"` has nothing to read.
pub struct Offscreen {
    pub canvas: crate::software::Canvas,
    /// The age of the canvas's content, for `take_damage_for_age`: 0
    /// until the first present, then 1 — one buffer, always last frame's.
    age: u8,
}

impl Offscreen {
    /// A `width × height` canvas, drawing text with the system face
    /// when there is one (the block stand-in otherwise, as `LUMEN_SHOT`
    /// does), laid out `Shaping::Basic`. For a session's frames use
    /// [`Offscreen::with_shaping`] with its [`Session::shaping`], or a
    /// `Complex` session is measured shaped and drawn unshaped.
    pub fn new(width: u32, height: u32) -> Self {
        Self::with_shaping(width, height, Shaping::Basic)
    }

    /// [`Offscreen::new`], with the system face laid out by `shaping` —
    /// M81/T81.4: pass the session's [`Session::shaping`], which is its
    /// program's (`Program::shaping`), so the canvas paints with the mode
    /// the host measured with, as the window's backend does.
    pub fn with_shaping(width: u32, height: u32, shaping: Shaping) -> Self {
        let canvas = match FontStack::system() {
            Some(f) => crate::software::Canvas::with_font(width, height, f.with_shaping(shaping)),
            None => crate::software::Canvas::new(width, height),
        };
        Offscreen { canvas, age: 0 }
    }
}

impl Screen for Offscreen {
    fn present(&mut self, src: &mut dyn FrameSource, page: Color) -> FrameTimes {
        let t = std::time::Instant::now();
        let viewport = [0.0, 0.0, self.canvas.width as f32, self.canvas.height as f32];
        let damage = src.take_damage_for_age(self.age);
        self.age = 1;
        let mut damaged_px = 0u64;
        for area in damage.regions(viewport) {
            self.canvas.clear_within(area, page);
            self.canvas.render_within(src.list(), area);
            damaged_px += ((area[2] - area[0]) as f64 * (area[3] - area[1]) as f64).max(0.0) as u64;
        }
        let (layout_ns, paint_ns) = src.timings();
        let render_ns = t.elapsed().as_nanos() as u64;
        FrameTimes {
            layout_ns: Some(layout_ns),
            paint_ns: Some(paint_ns),
            render_ns: Some(render_ns),
            present_ns: Some(0),
            frame_ns: Some(render_ns),
            damaged_px: Some(damaged_px),
            input_latency_ns: None,
            atlas_give_ups: None,
        }
    }
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])> {
        Some((self.canvas.width, self.canvas.height, &self.canvas.pixels))
    }
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
}

/// A running application, as a window drives it. Object-safe, and every
/// value it passes is either `Copy` or owned by the caller's side of the
/// call — except the display list, which a renderer borrows in place.
///
/// **Its shape is part of the ABI stamp** ([`abi_fields`]): a method
/// added here moves every slot of the vtable after it.
pub trait Session: FrameSource {
    /// Queue an input event.
    fn post(&mut self, ev: Event);
    /// The focused field's selected text.
    fn copy(&self) -> Option<String>;
    /// Whether any node has focus — the clipboard shortcuts route a key
    /// on when nothing does (M45/T45.9).
    fn has_focus(&self) -> bool;
    /// Paste into the focused node as one edit; the characters it kept
    /// (`Host::paste_with`, M45/T45.7). 0 when a panic was caught.
    fn paste(&mut self, text: &str) -> usize;
    /// An input-method step. Whether anything changed.
    fn compose(&mut self, c: Composition<'_>) -> bool;
    /// **Where the focused node's caret is**, in physical pixels
    /// (`Host::caret_rect`) — M80/T80.5: what the window passes to
    /// `set_ime_cursor_area` after a frame. `None` without focus, from a
    /// node with no caret, while a panic is shown, and when asking
    /// panicked.
    ///
    /// **Guarded, and `&mut`**: the answer is a widget's
    /// `Node::caret_rect`, application code, so a panic in it raises M36's
    /// screen through the host's guard instead of unwinding across the
    /// seam. The `&self` catch `copy` and `title` use answers a fallback
    /// and records nothing, so a panicking caret would have been a silent
    /// `None` (`docs/plan-text-fit.md`, "What building found", T80.5).
    fn caret_rect(&mut self) -> Option<[f32; 4]>;
    fn set_scale(&mut self, scale: f32);
    fn scale(&self) -> f32;
    fn to_logical(&self, x: f32, y: f32) -> (f32, f32);
    fn resize(&mut self, width: u32, height: u32);
    /// **One frame's application code** — tick, pump, and a zero-`dt`
    /// tick that asks whether anything is still moving — or, when a
    /// panic is being shown, the panic screen at `width × height`.
    /// Whether to draw again.
    fn frame(&mut self, dt_ms: f32, width: u32, height: u32) -> bool;
    /// Whether a panic is being shown (M36).
    fn failed(&self) -> bool;
    /// Clear a shown panic so the next frame runs again. Whether this
    /// build can recover at all.
    fn retry(&mut self) -> bool;
    fn title(&self) -> String;
    fn cursor(&self) -> Cursor;
    fn next_deadline_ms(&mut self) -> Option<f32>;
    fn more_deferred(&self) -> bool;
    /// Wake the window's loop from a worker (M28/T28.1).
    fn set_waker(&mut self, waker: Arc<dyn lumen_static::inbox::Waker>);
    /// The page colour the frame is cleared to.
    fn page(&self) -> Color;
    /// Answer one agent request, as JSON text. A session built without
    /// the agent answers with an error, so the vtable does not depend on
    /// the feature.
    ///
    /// **A panic in the agent itself is a `-32603` reply** naming the
    /// method and where it happened, and the session keeps answering
    /// (M39/T39.5). **A panic in application code the agent reached is
    /// not**: it unwinds out of this call under [`Crashing`], as it
    /// would from a frame (M36 rule 3), and under [`Recovering`] it is
    /// M36's to show.
    fn agent(&mut self, req: &str, screen: &mut dyn Screen) -> String;
    /// Feed the agent's journal the frame just drawn, with what it cost
    /// as the window measured it (M48/T48.3).
    fn agent_frame_done(&mut self, timing: FrameTimes);
    /// What a swap carries, as bytes: the application's `#[reload(keep)]`
    /// state and, beside it, the host's work in flight (M49/T49.10) —
    /// whatever the installed [`Saver`] writes. Empty for a session that
    /// cannot reload.
    fn save(&self) -> Vec<u8>;
    /// What restoring saved state onto this session could not keep — one
    /// line per field that fell back to its boot value, naming it and why
    /// (M37/T37.3, `docs/plan-window-reload.md` §2.4). Drained.
    fn take_restore_log(&mut self) -> String;
    /// What the boot did with the previous build's work in flight —
    /// M49/T49.10 (`Host::reissue`). All zeros for a session that was not
    /// booted from a swap.
    fn reissued(&self) -> crate::Reissued;
    /// The surface holds pixels this session did not draw — a window's
    /// own overlay, now gone: repaint all of it, in every buffer
    /// (`Host::repaint_all`).
    fn repaint_all(&mut self);
    /// What the last frame changed, in physical pixels (`Host::damage`)
    /// — M53/T53.4. A window that asked for a frame because of input
    /// presents nothing when this is `Damage::None`.
    fn damage(&self) -> Damage;
    /// **The accessibility tree of the frame last drawn** — M54/T54.2
    /// (`docs/plan-a11y-platform.md` §2.2): what the window's platform
    /// adapter sends on `InitialTreeRequested` and after every frame, in
    /// physical pixels. `None` from a build without `a11y`.
    ///
    /// **Unconditional, with an opaque type**, so the vtable is the same
    /// shape in every build and a wrapper (the hot session) forwards it
    /// without a `cfg` of its own; the stamp's feature list tells two
    /// builds apart ([`abi_fields`]).
    fn a11y_tree(&self) -> Option<A11yTree>;
    /// **An assistive technology's action on node `target`** (an accesskit
    /// node id) — M54/T54.4. A main-table node takes it as
    /// `Host::action`; a repeater row's synthesised node understands only
    /// `Default`, as a click at its centre, because a row holds no state
    /// of its own to step (`docs/plan-a11y-platform.md` §2.4). Whether
    /// anything was queued.
    fn a11y_action(&mut self, target: u64, action: lumen_static::Action) -> bool;
    /// **The tree's shape**, hashed — M49/T49.11 (R4-F7): the node count
    /// and every table entry's links, flags and author id. A node's
    /// identity is its offset in the table, so an agent holding node
    /// indices across a swap must resolve them again when this changes;
    /// the author id and a `#a > #b` path are the names that survive.
    fn shape(&self) -> u64;
    /// **The application asked to close** — M60/T60.1: its key hook
    /// answered `KeyOutcome::Quit` (`Host::quit_requested`). The window
    /// exits at its next `about_to_wait`, as it does for `app.quit`.
    fn wants_quit(&self) -> bool;
    /// **How this session lays text out** — M81/T81.4
    /// (`docs/plan-shaped-paint.md`, "Choosing is a `Program` setting"):
    /// the mode of the stack its measurer and the agent's audit were built
    /// with, which is its program's `Program::shaping`. The window builds
    /// its renderer's stack with it **before the first backend exists**,
    /// and compares it again after every swap, calling
    /// `Backend::set_font` when it changed — otherwise a `Complex` program
    /// is measured shaped and drawn unshaped.
    ///
    /// **Required**, like every method here: `impl Session for Hot` must
    /// forward it or not compile, which is the whole forwarding gate
    /// (`docs/plan-iced-roadmap.md` §2.3). Not guarded: it reads a field
    /// and runs no application code.
    fn shaping(&self) -> Shaping;
}

/// An accessibility tree crossing the session seam — M54/T54.2. Holds
/// an `accesskit::TreeUpdate` with `a11y`, and nothing without it.
pub struct A11yTree {
    #[cfg(feature = "a11y")]
    pub update: lumen_static::observe::accesskit::TreeUpdate,
}

/// What one presented frame cost, as the window measured it — the
/// argument to [`Session::agent_frame_done`] (M48/T48.3).
///
/// **Plain integers, defined here**, so no agent type crosses the library
/// seam: a hot library's session is on the far side of the loader, and
/// the loader links no agent. `None` is a stage this window did not
/// measure, which `app.perf` reports as `null` — never `0`. Until T48.3
/// the session built `FrameTiming::default()` itself, so every turnkey
/// window reported 0 ns for every stage and a budget check passed on it
/// (R13-F2). Nanoseconds; `damaged_px` in physical pixels (M40/T40.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameTimes {
    pub layout_ns: Option<u64>,
    pub paint_ns: Option<u64>,
    pub render_ns: Option<u64>,
    pub present_ns: Option<u64>,
    pub frame_ns: Option<u64>,
    pub damaged_px: Option<u64>,
    pub input_latency_ns: Option<u64>,
    /// The GPU renderer's running count of frames drawn with glyphs or
    /// images missing because an atlas could not hold them (M51/T51.1,
    /// R9-F1); `None` from a renderer with no atlas.
    pub atlas_give_ups: Option<u64>,
}

/// **Whether a session catches panics in application code, as a type** —
/// M52/T52.7 (`docs/plan-embedded-footprint.md` §2.2, F-NEW-3), amending
/// M36's rule 3 (`docs/plan-panic-recovery.md`).
///
/// The window crate decides (debug builds, and release builds with
/// `lumen-static-winit/reload`), and a hot library always recovers. Until
/// M52 the decision was a `const` value handed to a runtime field, and
/// every guarded arm tested the field — so every `recover::` function,
/// the failure screen and the panic hook included, was linked into every
/// release window, 14 symbols and 13,964 B that could never run. As a
/// type, each guarded arm tests [`RecoverPolicy::ON`], a constant per
/// instantiation, and under [`Crashing`] the arms and everything they
/// reach are compiled away (`just release-window-shape`).
pub trait RecoverPolicy: Copy + Default + 'static {
    /// Whether a panic in application code is caught and shown.
    const ON: bool;
}

/// Catch a panic in application code, show it, and wait for a retry
/// (M36): what a debug window, a `reload` window and a hot library use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recovering;

impl RecoverPolicy for Recovering {
    const ON: bool = cfg!(panic = "unwind");
}

/// Let a panic in application code unwind, as it would without this
/// crate — a shipped release window, so a crash reaches whatever would
/// have reported it (M36 rule 3). Links no `recover::` code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crashing;

impl RecoverPolicy for Crashing {
    const ON: bool = false;
}

/// Run `f` on `host`, guarded when `P` recovers (M36).
fn run_guarded<A: App, P: RecoverPolicy, I: Invalidation, T>(
    host: &mut Host<A, Ret, I>,
    f: impl FnOnce(&mut Host<A, Ret, I>) -> T,
) -> Option<T> {
    #[cfg(panic = "unwind")]
    if P::ON {
        return host.guard(f);
    }
    Some(f(host))
}

/// Save a session, for a reload. A function pointer and not a bound, so
/// `Local<A>` does not require `A: Reloadable` of an application that
/// never reloads.
///
/// **It is handed the host, not the application** — M49/T49.10: what a
/// swap carries is the application's kept state *and* the host's work in
/// flight ([`Host::reissuable`]), and the second is not the
/// application's. `lumen_static_reload::hot::save` is the one a window
/// uses.
pub type Saver<A, I = DefaultInval> = fn(&Host<A, Ret, I>) -> Vec<u8>;

/// A statically linked — or library-resident — application behind
/// [`Session`]. `P` says whether it catches application panics
/// ([`RecoverPolicy`]); the trait object hides it, so the dylib seam does
/// not see it.
///
/// `I` is the invalidation strategy (M55/T55.4), after M52's `P`:
/// [`DefaultInval`] unless the program chose one.
pub struct Local<A: App, P: RecoverPolicy, I: Invalidation = DefaultInval> {
    pub host: Host<A, Ret, I>,
    program: Option<Program<A, I>>,
    title: &'static str,
    page: Color,
    metrics: Option<FontMetrics>,
    /// The mode of the stack `metrics` and the agent's audit measure with
    /// — [`Session::shaping`]'s answer (M81/T81.4). `Basic` for a session
    /// built around a host ([`Local::from_host`]); its program's for
    /// [`Local::from_program`].
    shaping: Shaping,
    /// The policy, as a type only: every guarded arm reads `P::ON`.
    recover: core::marker::PhantomData<P>,
    saver: Option<Saver<A, I>>,
    restore_log: String,
    reissued: crate::Reissued,
    /// What the last [`Session::frame`] said: whether anything is still
    /// moving. `ui.waitSettled` reads it (M48/T48.2).
    animating: bool,
    /// Pixels the agent's own pumps presented since the last journal
    /// frame — M48/T48.4. Folded into the next `agent_frame_done`, so a
    /// journal frame's `damaged_px` is everything the surface presented
    /// since the previous one, and an input's damage is not lost to the
    /// present inside its request.
    unjournaled_px: Option<u64>,
    #[cfg(feature = "live-agent")]
    agent: lumen_static_agent::Agent,
}

impl<A: App + 'static, P: RecoverPolicy, I: Invalidation> Local<A, P, I> {
    /// A session over a described program: boots it through
    /// [`Program::host`], installs the thread spawner, and measures text
    /// with the system face, laid out by the program's `Program::shaping`
    /// (M81/T81.4) — the frame's measurer and the agent's audit both, from
    /// one stack, so they cannot shape differently.
    pub fn from_program(program: Program<A, I>, recover: P) -> Self {
        let host = program.host();
        let page = program.page;
        let mut s = Local::with_stack(host, "", page, program.shaping, recover);
        s.program = Some(program);
        s
    }

    /// A session over an already-built host, measuring `Shaping::Basic`.
    pub fn from_host(host: Host<A, Ret, I>, title: &'static str, page: Color, policy: P) -> Self {
        Local::with_stack(host, title, page, Shaping::Basic, policy)
    }

    fn with_stack(mut host: Host<A, Ret, I>, title: &'static str, page: Color, shaping: Shaping, _policy: P) -> Self {
        // **Deferred work, wired up** — M28/T28.12. The waker comes from
        // the window, which owns the loop ([`Session::set_waker`]).
        host.set_spawner(Box::new(lumen_static::spawn::Threads::new()));
        let font = FontStack::system().map(|f| f.with_shaping(shaping));
        // What the stack lays out by, which is what the window must render
        // by: the answer `Session::shaping` gives.
        let shaping = font.as_ref().map_or(Shaping::Basic, FontStack::shaping);
        // **The audit measures with the face the host lays out with** —
        // M60/T60.3. Left at the block stand-in, `ui.lint` reported the
        // block's opinion of every text box (`tests/agent_audit_face.rs`);
        // `examples/counter-window` set this itself until M60 moved it
        // onto this session.
        #[cfg(feature = "live-agent")]
        let agent = {
            let mut agent = lumen_static_agent::Agent::new();
            if let Some(f) = font.clone() {
                agent.set_text_measure(Box::new(FontMetrics::new(f)));
            }
            agent
        };
        let metrics = font.map(FontMetrics::new);
        Local {
            host,
            program: None,
            title,
            page,
            metrics,
            shaping,
            recover: core::marker::PhantomData,
            saver: None,
            restore_log: String::new(),
            reissued: crate::Reissued::default(),
            animating: false,
            unjournaled_px: None,
            #[cfg(feature = "live-agent")]
            agent,
        }
    }

    /// Let [`Session::save`] answer — a hot library sets this.
    pub fn with_saver(mut self, saver: Saver<A, I>) -> Self {
        self.saver = Some(saver);
        self
    }

    /// Record what a restore could not keep, for
    /// [`Session::take_restore_log`].
    pub fn note_restore(&mut self, log: String) {
        self.restore_log = log;
    }

    /// Record what re-issuing the previous build's work did, for
    /// [`Session::reissued`] (M49/T49.10).
    pub fn note_reissue(&mut self, r: crate::Reissued) {
        self.reissued = r;
    }

    /// The program this session was booted from.
    pub fn program(&self) -> Option<&Program<A, I>> {
        self.program.as_ref()
    }

    /// Run `f` guarded when recovery is on (M36).
    fn guarded<T>(&mut self, f: impl FnOnce(&mut Host<A, Ret, I>, &mut Option<FontMetrics>) -> T) -> Option<T> {
        let metrics = &mut self.metrics;
        run_guarded::<A, P, I, T>(&mut self.host, |h| f(h, metrics))
    }

    /// A `&self` call into application code — the title, the selection —
    /// caught when recovery is on, answering `fallback` if it panicked.
    fn caught<T>(&self, fallback: T, f: impl FnOnce(&Host<A, Ret, I>) -> T) -> T {
        #[cfg(panic = "unwind")]
        if P::ON {
            return std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&self.host))).unwrap_or(fallback);
        }
        let _ = &fallback;
        f(&self.host)
    }

    /// Put the panic screen in the host's list, sized to the surface.
    fn show_failure(&mut self, w: u32, h: u32) {
        #[cfg(panic = "unwind")]
        if !P::ON {
            return;
        }
        #[cfg(panic = "unwind")]
        if let Some(r) = self.host.failure().cloned() {
            use crate::recover::failure_frame;
            let list = match self.metrics.as_mut() {
                Some(m) => failure_frame(&r, w, h, m),
                None => failure_frame(&r, w, h, &mut lumen_static::cx::BlockMetrics),
            };
            self.host.show(list);
        }
        #[cfg(not(panic = "unwind"))]
        let _ = (w, h);
    }

    fn pump_now(h: &mut Host<A, Ret, I>, m: &mut Option<FontMetrics>) -> bool {
        match m.as_mut() {
            Some(m) => h.pump_with(m),
            None => h.pump(),
        }
    }
}

#[cfg(feature = "live-agent")]
impl<A: App + 'static, P: RecoverPolicy, I: Invalidation> Local<A, P, I> {
    /// **The agent's view of this session**, over `screen` — the [`Live`]
    /// that [`Session::agent`] drives, for a caller that drives it some
    /// other way: `lumen_static_agent::harness::TestApp` above all
    /// (M60/T60.4). So a harness session and an agent request go through
    /// one adapter, and `examples/counter-window --drive2` needs none of
    /// its own.
    pub fn live<'a>(&'a mut self, screen: &'a mut dyn Screen) -> Live<'a, A, P, I> {
        Live {
            host: &mut self.host,
            metrics: &mut self.metrics,
            screen,
            page: self.page,
            recover: P::default(),
            animating: self.animating,
            presented_px: &mut self.unjournaled_px,
        }
    }
}

/// The accessibility half of [`Session`] for a [`Local`] — M54/T54.2,
/// one body per build so neither has a dead branch.
impl<A: App + 'static, P: RecoverPolicy, I: Invalidation> Local<A, P, I> {
    #[cfg(feature = "a11y")]
    fn tree_for_platform(&self) -> Option<A11yTree> {
        let title = Session::title(self);
        self.caught(None, |h| Some(A11yTree { update: h.accessibility_tree(&title) }))
    }

    #[cfg(not(feature = "a11y"))]
    fn tree_for_platform(&self) -> Option<A11yTree> {
        None
    }

    /// A click at the centre of the node `target` names in the tree the
    /// platform was sent — physical there, logical in the queue.
    #[cfg(feature = "a11y")]
    fn click_for_platform(&mut self, target: u64) -> bool {
        let Some(tree) = self.tree_for_platform() else { return false };
        let Some(b) = tree.update.nodes.iter().find(|(id, _)| id.0 == target).and_then(|(_, n)| n.bounds()) else {
            return false;
        };
        if b.x1 <= b.x0 || b.y1 <= b.y0 {
            return false;
        }
        let (x, y) = self.host.to_logical(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
        self.host.post(Event::down(x, y));
        self.host.post(Event::up(x, y));
        true
    }

    #[cfg(not(feature = "a11y"))]
    fn click_for_platform(&mut self, _target: u64) -> bool {
        false
    }
}

impl<A: App + 'static, P: RecoverPolicy, I: Invalidation> FrameSource for Local<A, P, I> {
    fn list(&self) -> &DisplayList {
        &self.host.list
    }
    fn take_damage_for_age(&mut self, age: u8) -> Damage {
        self.host.take_damage_for_age(age)
    }
    fn timings(&self) -> (u64, u64) {
        self.host.timings()
    }
}

impl<A: App + 'static, P: RecoverPolicy, I: Invalidation> Session for Local<A, P, I> {
    fn post(&mut self, ev: Event) {
        self.host.post(ev);
    }

    fn copy(&self) -> Option<String> {
        self.caught(None, |h| h.copy().map(str::to_owned))
    }

    fn has_focus(&self) -> bool {
        self.host.input.focus.is_some()
    }

    fn paste(&mut self, text: &str) -> usize {
        // Guarded: paste runs the focused widget's editing code. With the
        // session's own measurer, which every other frame of this session
        // lays out with — M41/T41.5 (R2-F1, R10-F4). A hot library's
        // session is one of these on the far side of the loader, and the
        // reload crate's wrapper forwards to it, so it inherits this.
        self.guarded(|h, m| match m.as_mut() {
            Some(m) => h.paste_with(text, m),
            None => h.paste(text),
        })
        .unwrap_or(0)
    }

    fn compose(&mut self, c: Composition<'_>) -> bool {
        // Guarded: composition runs the focused widget's editing code.
        self.guarded(|h, _| h.compose(c)).unwrap_or(true)
    }

    fn caret_rect(&mut self) -> Option<[f32; 4]> {
        // Guarded (M80/T80.5): a widget's `caret_rect` is application
        // code. A failed host answers `None` without asking, so no IME
        // area is passed while M36's screen is up.
        self.guarded(|h, _| h.caret_rect()).flatten()
    }

    fn set_scale(&mut self, scale: f32) {
        self.host.set_scale(scale);
        if let Some(m) = self.metrics.as_mut() {
            m.set_scale(scale);
        }
    }

    fn scale(&self) -> f32 {
        self.host.scale()
    }

    fn to_logical(&self, x: f32, y: f32) -> (f32, f32) {
        self.host.to_logical(x, y)
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.host.resize(width, height);
    }

    fn frame(&mut self, dt_ms: f32, width: u32, height: u32) -> bool {
        // **A panic is being shown** (M36): the screen, and no
        // application code — running it would be an automatic retry.
        if self.failed() {
            self.show_failure(width, height);
            return false;
        }
        // **One guarded entry for the frame's application code** (M36):
        // both ticks and the pump.
        let ran = self.guarded(|h, m| {
            // Animation, before the pump: a tick changes state, and the
            // bindings the pump runs are what put that state on screen.
            h.tick(dt_ms);
            Self::pump_now(h, m);
            // **Ask again, after the pump — M22/T22.4.** A press
            // dispatched during the pump starts an animation the first
            // tick could not have seen; a zero `dt` is a pure query.
            h.tick(0.0)
        });
        self.animating = ran.unwrap_or(false);
        match ran {
            Some(animating) => animating,
            None => {
                self.show_failure(width, height);
                false
            }
        }
    }

    fn failed(&self) -> bool {
        #[cfg(panic = "unwind")]
        return P::ON && self.host.failure().is_some();
        #[cfg(not(panic = "unwind"))]
        false
    }

    fn retry(&mut self) -> bool {
        #[cfg(panic = "unwind")]
        if P::ON {
            self.host.retry();
            return true;
        }
        false
    }

    fn title(&self) -> String {
        match &self.program {
            Some(p) => self.caught(String::from("(the title panicked)"), |h| p.title_of(&h.app).into_owned()),
            None => self.title.to_string(),
        }
    }

    fn cursor(&self) -> Cursor {
        self.host.cursor()
    }

    fn next_deadline_ms(&mut self) -> Option<f32> {
        self.host.next_deadline_ms()
    }

    fn more_deferred(&self) -> bool {
        self.host.more_deferred()
    }

    fn set_waker(&mut self, waker: Arc<dyn lumen_static::inbox::Waker>) {
        self.host.inbox.set_waker(waker);
    }

    fn page(&self) -> Color {
        self.page
    }

    #[cfg(feature = "live-agent")]
    fn agent(&mut self, req: &str, screen: &mut dyn Screen) -> String {
        let req: serde_json::Value = match serde_json::from_str(req) {
            Ok(v) => v,
            Err(e) => {
                return serde_json::json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": format!("parse error: {e}")}
                })
                .to_string()
            }
        };
        let page = self.page;
        let recover = P::default();
        let animating = self.animating;
        let mut live = Live {
            host: &mut self.host,
            metrics: &mut self.metrics,
            screen,
            page,
            recover,
            animating,
            presented_px: &mut self.unjournaled_px,
        };
        // **One call.** `dispatch_input` already falls through to
        // `dispatch` for anything that is not an `input.*` method or a
        // screenshot.
        #[cfg(not(panic = "unwind"))]
        return self.agent.dispatch_input(&mut live, &req).to_string();
        // **The agent's own panic is a reply** — M39/T39.5 (R14-F5).
        // Selector parsing, `ui.getTree`, `ui.explain` and every widget's
        // `observe` ran bare here, so a panic in any of them ended the
        // window. Caught whatever the `RecoverPolicy` says, because none of
        // it is the application's — and **the application's is re-raised**:
        // `Live::pump` marks it, so under `Crashing` a shipped
        // application still crashes where a crash reporter can see it
        // (M36 rule 3). Under `Recovering` the pump's own guard has
        // already caught it, so nothing marked arrives here. No journal
        // reset: `dispatch_input` takes `&self`.
        #[cfg(panic = "unwind")]
        {
            crate::recover::install();
            crate::recover::clear();
            let agent = &self.agent;
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| agent.dispatch_input(&mut live, &req))) {
                Ok(reply) => reply.to_string(),
                Err(payload) => match payload.downcast::<AppPanic>() {
                    Ok(app) => std::panic::resume_unwind(app.0),
                    Err(payload) => agent_panicked(&req, &*payload),
                },
            }
        }
    }

    #[cfg(not(feature = "live-agent"))]
    fn agent(&mut self, _req: &str, _screen: &mut dyn Screen) -> String {
        String::from(
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32601,"message":"this build has no agent"}}"#,
        )
    }

    fn agent_frame_done(&mut self, mut t: FrameTimes) {
        if let Some(px) = self.unjournaled_px.take() {
            t.damaged_px = Some(t.damaged_px.unwrap_or(0) + px);
        }
        #[cfg(not(feature = "live-agent"))]
        let _ = t;
        // Guarded: the journal walks the tree through every widget's
        // `observe` (M36).
        #[cfg(feature = "live-agent")]
        {
            let agent = &mut self.agent;
            let metrics = &mut self.metrics;
            let _ = run_guarded::<A, P, I, _>(&mut self.host, |h| {
                let timing = lumen_static_agent::FrameTiming {
                    layout_ns: t.layout_ns,
                    paint_ns: t.paint_ns,
                    render_ns: t.render_ns,
                    present_ns: t.present_ns,
                    frame_ns: t.frame_ns,
                    input_latency_ns: t.input_latency_ns,
                    damaged_px: t.damaged_px,
                    atlas_give_ups: t.atlas_give_ups,
                };
                let scale = h.scale();
                let mut tree = lumen_static_agent::TreeMut::of_root(h.app.tree_mut(), &h.styles, &h.states);
                // The measure the host laid out with, so the audit's
                // re-layout leaves every widget as the frame did (M57/A3).
                match metrics {
                    Some(m) => agent.frame_done_measured(&mut tree, timing, m, scale),
                    None => agent.frame_done(&mut tree, timing),
                }
            });
        }
    }

    fn save(&self) -> Vec<u8> {
        match self.saver {
            Some(save) => self.caught(Vec::new(), save),
            None => Vec::new(),
        }
    }

    fn take_restore_log(&mut self) -> String {
        std::mem::take(&mut self.restore_log)
    }

    fn reissued(&self) -> crate::Reissued {
        self.reissued
    }

    fn repaint_all(&mut self) {
        self.host.repaint_all();
    }

    fn damage(&self) -> Damage {
        self.host.damage()
    }

    fn a11y_tree(&self) -> Option<A11yTree> {
        self.tree_for_platform()
    }

    fn a11y_action(&mut self, target: u64, action: lumen_static::Action) -> bool {
        let table = <A::Tree as lumen_static::widget::Widget>::SUB_TABLE.len();
        if target >= 1 && (target as usize) <= table {
            self.host.action((target - 1) as u32, action);
            return true;
        }
        action == lumen_static::Action::Default && self.click_for_platform(target)
    }

    fn shape(&self) -> u64 {
        table_shape(<A::Tree as lumen_static::widget::Widget>::SUB_TABLE)
    }

    fn wants_quit(&self) -> bool {
        self.host.quit_requested()
    }

    fn shaping(&self) -> Shaping {
        self.shaping
    }
}

/// FNV-1a over a table's structure: its length, and each entry's links,
/// flags and author id — what an index means (M49/T49.11).
pub fn table_shape(table: &[lumen_static::desc::NodeDesc]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h = (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(&(table.len() as u64).to_le_bytes());
    for d in table {
        eat(&d.first_child.to_le_bytes());
        eat(&d.parent.to_le_bytes());
        eat(&d.next_sibling.to_le_bytes());
        eat(&d.child_count.to_le_bytes());
        eat(&[d.flags]);
        eat(d.wid.as_bytes());
        eat(&[0]);
    }
    h
}

/// A panic raised by **application** code the agent reached — M39/T39.5.
/// [`Live`]'s pump wraps whatever escapes it in this, so
/// `Session::agent`'s catch can tell the application's panic (re-raised)
/// from the agent's own (answered). **Only `Session::agent` unwraps it**;
/// a caller that drives [`Live`] some other way would see the wrapper, not
/// the application's payload — T39.5's first injection showed exactly that
/// when the catch was removed.
#[cfg(all(feature = "live-agent", panic = "unwind"))]
struct AppPanic(Box<dyn std::any::Any + Send>);

/// The `-32603` reply to a panic in the agent itself, with the method and
/// the location M36's hook recorded.
#[cfg(all(feature = "live-agent", panic = "unwind"))]
fn agent_panicked(req: &serde_json::Value, payload: &(dyn std::any::Any + Send)) -> String {
    let r = crate::recover::report(payload, None);
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("?");
    let id = match req.get("id") {
        Some(v @ (serde_json::Value::String(_) | serde_json::Value::Number(_))) => v.clone(),
        _ => serde_json::Value::Null,
    };
    let at = r.location.as_ref().map(|(f, l, c)| format!(" at {f}:{l}:{c}")).unwrap_or_default();
    serde_json::json!({
        "jsonrpc": "2.0", "id": id,
        "error": {
            "code": -32603,
            "message": format!("the agent panicked answering `{method}`{at}: {}", r.message),
            "data": {
                "location": r.location.as_ref().map(|(f, l, c)| serde_json::json!({"file": f, "line": l, "column": c})),
                "note": r.note,
            },
        },
    })
    .to_string()
}

/// The agent's view of a session: the host and the tree, and the
/// window's renderer for pixels. Moved here from the window crate by
/// M37, because the tree now lives wherever the session does.
#[cfg(feature = "live-agent")]
pub struct Live<'a, A: App, P: RecoverPolicy, I: Invalidation = DefaultInval> {
    pub host: &'a mut Host<A, Ret, I>,
    pub metrics: &'a mut Option<FontMetrics>,
    pub screen: &'a mut dyn Screen,
    pub page: Color,
    pub recover: P,
    /// Whether the session's last frame was still animating — its
    /// `ui.waitSettled` term (M48/T48.2).
    pub animating: bool,
    /// Where this request's presents add their damaged pixels, for the
    /// next journal frame (M48/T48.4).
    pub presented_px: &'a mut Option<u64>,
}

/// One frame at 60 Hz, in milliseconds: a timer due sooner than this is
/// work the next frame does, so a window waiting on it is not settled.
#[cfg(feature = "live-agent")]
const FRAME_MS: f32 = 1000.0 / 60.0;

#[cfg(feature = "live-agent")]
impl<A: App, P: RecoverPolicy, I: Invalidation> Live<'_, A, P, I> {
    /// Present the host's frame, adding what it damaged to the next
    /// journal frame's `damaged_px` (M48/T48.4). **Every present inside a
    /// request goes through here**: M45's paste presented on its own and
    /// its damage was lost, which `just widget-drive` read as "the journal
    /// damaged the widget but the surface presented 0 px".
    fn present_counted(&mut self) {
        if let Some(px) = self.screen.present(self.host, self.page).damaged_px {
            *self.presented_px = Some(self.presented_px.unwrap_or(0) + px);
        }
    }

    /// One pump and present — what the agent's `pump` runs, marked as
    /// application code there (M39/T39.5).
    fn pump_app(&mut self) -> (u64, bool) {
        let metrics = &mut *self.metrics;
        let handled = run_guarded::<A, P, I, _>(self.host, |h| match metrics.as_mut() {
            Some(m) => h.pump_with(m),
            None => h.pump(),
        })
        .unwrap_or(false);
        #[cfg(panic = "unwind")]
        if let Some(r) = self.host.failure().cloned().filter(|_| P::ON) {
            let (w, h) = self.screen.frame_rgba().map_or((1, 1), |(w, h, _)| (w, h));
            let list = match self.metrics.as_mut() {
                Some(m) => crate::recover::failure_frame(&r, w, h, m),
                None => crate::recover::failure_frame(&r, w, h, &mut lumen_static::cx::BlockMetrics),
            };
            self.host.show(list);
        }
        self.present_counted();
        (self.host.frame_number(), handled)
    }
}

#[cfg(feature = "live-agent")]
impl<A: App, P: RecoverPolicy, I: Invalidation> lumen_static_agent::AgentHost for Live<'_, A, P, I> {
    fn tree(&self) -> lumen_static_agent::TreeRef<'_> {
        lumen_static_agent::TreeRef::of_root(self.host.app.tree(), &self.host.styles, &self.host.states)
            .with_sheet(self.host.stylesheet(), self.host.sheet_findings())
    }

    fn enqueue(&mut self, ev: Event) {
        self.host.post(ev);
    }

    /// Application code, marked as `pump` marks it: a paste runs the
    /// focused widget's editing code.
    fn paste(&mut self, text: &str) -> Option<(u64, usize)> {
        let go = || {
            let kept = match self.metrics.as_mut() {
                Some(m) => self.host.paste_with(text, m),
                None => self.host.paste(text),
            };
            self.present_counted();
            (self.host.frame_number(), kept)
        };
        #[cfg(panic = "unwind")]
        return Some(match std::panic::catch_unwind(std::panic::AssertUnwindSafe(go)) {
            Ok(v) => v,
            Err(p) => std::panic::resume_unwind(Box::new(AppPanic(p))),
        });
        #[cfg(not(panic = "unwind"))]
        Some(go())
    }

    fn dropped_inputs(&self) -> Option<u32> {
        Some(self.host.dropped_inputs())
    }

    fn set_palette(&mut self, palette: lumen_static::style::Palette) -> bool {
        self.host.set_palette(palette);
        true
    }

    /// The window's own route (`Session::compose`): `Host::compose`,
    /// guarded, with the session's measurer untouched — M48/T48.13.
    fn compose(&mut self, c: lumen_static::event::Composition<'_>) -> Option<bool> {
        Some(run_guarded::<A, P, I, _>(self.host, |h| h.compose(c)).unwrap_or(true))
    }

    /// **Application code**, marked as such (M39/T39.5): anything that
    /// escapes is re-raised wrapped in a private `AppPanic` marker, so `Session::agent`
    /// hands it on instead of answering it.
    fn pump(&mut self) -> (u64, bool) {
        #[cfg(panic = "unwind")]
        return match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.pump_app())) {
            Ok(v) => v,
            Err(p) => std::panic::resume_unwind(Box::new(AppPanic(p))),
        };
        #[cfg(not(panic = "unwind"))]
        self.pump_app()
    }

    /// Settled when nothing is animating, the inbox has no backlog, no
    /// job is in flight or undrained (M47/T47.3's ledger,
    /// `Host::in_flight_unsubscribed`), and no timer is due within a
    /// frame (`docs/plan-agent-truth.md` §2.2). Subscriptions are
    /// excluded and counted: a recurring timer would never settle.
    fn settled(&mut self) -> Option<serde_json::Value> {
        let deadline = self.host.next_deadline_ms();
        let more = self.host.more_deferred();
        // M47/T47.3's ledger: a job is listed from spawn until its body
        // returned and every result it sent was drained -- so a result
        // waiting in the inbox is pending too. Subscriptions apart.
        let pending = self.host.in_flight_unsubscribed();
        let due = deadline.is_some_and(|ms| ms <= FRAME_MS);
        Some(serde_json::json!({
            "settled": !self.animating && !more && pending == 0 && !due,
            "animating": self.animating,
            "more_deferred": more,
            "pending_tasks": pending,
            "next_deadline_ms": deadline,
            "subscriptions": self.host.subscriptions_running(),
        }))
    }

    fn failure(&self) -> serde_json::Value {
        #[cfg(panic = "unwind")]
        if P::ON {
            return serde_json::json!({
                "supported": true,
                "failure": self.host.failure().map(|r| serde_json::json!({
                    "message": r.message,
                    "location": r.location.as_ref().map(|(f, l, c)| serde_json::json!({"file": f, "line": l, "column": c})),
                    "app_frame": r.app_frame.as_ref().map(|(f, l)| serde_json::json!({"file": f, "line": l})),
                    "note": r.note,
                })),
            });
        }
        serde_json::json!({ "supported": false })
    }

    fn retry(&mut self) -> bool {
        #[cfg(panic = "unwind")]
        if P::ON {
            self.host.retry();
            return true;
        }
        false
    }

    fn frame(&self) -> Option<(u32, u32, &[u8])> {
        self.screen.frame_rgba()
    }

    fn scale(&self) -> f32 {
        self.host.scale()
    }

    fn presented(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.screen.fetch_rgba()
    }

    /// M42/T42.1: the strategy is the host's own type parameter — the
    /// session's `I`, chosen by the application since M55/T55.4 — and the
    /// features are the shell's `cfg!` set.
    fn process_info(&self) -> serde_json::Value {
        lumen_static_agent::process_block(self.host.strategy(), &crate::process_features())
    }

    /// Measured on the frame's own display list (M54/T54.5): each text
    /// run whose centre lies in `rect` against the opaque fill under it,
    /// with the page behind everything.
    fn contrast(&self, rect: Option<[f32; 4]>) -> Option<serde_json::Value> {
        let r = crate::contrast::text_contrast(&self.host.list, self.page);
        let inside = |b: &[f32; 4]| {
            let (x, y) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
            rect.is_none_or(|q| x >= q[0] && x < q[2] && y >= q[1] && y < q[3])
        };
        let runs: Vec<&crate::contrast::Run> = r.measured.iter().filter(|m| inside(&m.rect)).collect();
        let min = runs.iter().min_by(|a, b| a.ratio.total_cmp(&b.ratio));
        let run_json = |m: &crate::contrast::Run| {
            serde_json::json!({
                "text": m.text, "ratio": m.ratio,
                "fg": [m.fg.0, m.fg.1, m.fg.2], "ground": [m.ground.0, m.ground.1, m.ground.2],
            })
        };
        Some(serde_json::json!({
            "supported": true,
            "measured": runs.len(),
            "unmeasured": r
                .unmeasured
                .iter()
                .filter(|(k, _, _)| match self.host.list.cmds.get(*k) {
                    Some(lumen_static::display::Cmd::Text { rect, .. }) => inside(rect),
                    _ => false,
                })
                .count(),
            "clipped": r.clipped,
            "min": min.map(|m| run_json(m)),
        }))
    }

    /// Everything marked, every buffer's history dropped, then one frame:
    /// the whole canvas repainted from the display list (M57/D6).
    fn repaint_all(&mut self) -> bool {
        self.host.invalidate_all();
        self.host.repaint_all();
        lumen_static_agent::AgentHost::pump(self);
        true
    }

    #[cfg(feature = "shadow-oracle")]
    fn shadow(&self) -> Option<serde_json::Value> {
        let r = self.host.shadow_report();
        Some(serde_json::json!({
            "supported": true,
            "compared": r.compared,
            "skipped": r.skipped,
            "first": r.first.map(|d| serde_json::json!({
                "frame": d.frame, "node": d.node, "field": d.field,
            })),
        }))
    }
}

/// The shell's part of a hot library's ABI stamp (M37/T37.0/T37.1): its
/// version and features, and the size of the boxed [`Session`].
///
/// An edit to this crate — the `Session` trait's vtable included — is
/// caught by the source hash `lumen_static_reload`'s build script takes
/// of the whole crate (M49/T49.1), which replaced a hash of this one file.
pub fn abi_fields() -> Vec<(&'static str, String)> {
    let mut features = Vec::new();
    for (on, name) in [
        (cfg!(feature = "agent"), "agent"),
        (cfg!(feature = "live-agent"), "live-agent"),
        (cfg!(feature = "reload"), "reload"),
        (cfg!(feature = "truetype"), "truetype"),
        (cfg!(feature = "shadow-oracle"), "shadow-oracle"),
        (cfg!(feature = "simd"), "simd"),
        (cfg!(feature = "threads"), "threads"),
        (cfg!(feature = "a11y"), "a11y"),
        // M81/T81.1: from the task that adds the feature
        // (docs/plan-iced-roadmap.md §2.3). Under `shaping` a stack can be
        // Complex (T81.3), a discriminant a loader built without it cannot
        // name, so the field refuses the pair (`just abi-stamp`).
        (cfg!(feature = "shaping"), "shaping"),
    ] {
        if on {
            features.push(name);
        }
    }
    vec![
        ("lumen-static-shell", String::from(env!("CARGO_PKG_VERSION"))),
        ("lumen-static-shell features", features.join(",")),
        ("Box<dyn Session>", format!("{}", core::mem::size_of::<Box<dyn Session>>())),
    ]
}
