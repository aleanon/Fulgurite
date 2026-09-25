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
use crate::text::{FontMetrics, FontStack};
use crate::Host;

/// The retention a window's host uses (moved here from the window crate
/// by M37, so a hot library, which links no window system, builds the
/// same `Host` type).
pub type Ret = lumen_static::retain::Incremental;

/// Which invalidation strategy a window's host runs.
///
/// **`Boundaries` by default since M34/T34.2.** The `everything` feature
/// selects the whole-tree walk instead — **a diagnostic**: if a stale
/// frame goes away when you build with it, you have found an
/// invalidation defect. 256 marks because `Boundaries<N>` stores ranges,
/// not nodes, and M26/T26.5 fixed it wrapping its length at exactly that
/// count.
#[cfg(not(feature = "everything"))]
pub type Inval = lumen_static::Boundaries<256>;
#[cfg(feature = "everything")]
pub type Inval = lumen_static::Everything;

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
    fn present(&mut self, src: &mut dyn FrameSource, page: Color);
    /// What the renderer drew, as RGBA8.
    fn frame_rgba(&self) -> Option<(u32, u32, &[u8])>;
    /// What the display server shows, when that is checkable.
    fn fetch_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)>;
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
    fn paste(&mut self, text: &str);
    /// An input-method step. Whether anything changed.
    fn compose(&mut self, c: Composition<'_>) -> bool;
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
    fn agent(&mut self, req: &str, screen: &mut dyn Screen) -> String;
    /// Feed the agent's journal the frame just drawn.
    fn agent_frame_done(&mut self);
    /// The application's `#[reload(keep)]` state, as bytes — empty for a
    /// session that cannot reload.
    fn save(&self) -> Vec<u8>;
    /// What restoring saved state onto this session could not keep — one
    /// line per field that fell back to its boot value, naming it and why
    /// (M37/T37.3, `docs/plan-window-reload.md` §2.4). Drained.
    fn take_restore_log(&mut self) -> String;
    /// The surface holds pixels this session did not draw — a window's
    /// own overlay, now gone: repaint all of it, in every buffer
    /// (`Host::repaint_all`).
    fn repaint_all(&mut self);
}

/// Whether this session catches panics in application code — the
/// window crate decides (M36 rule 3), and a hot library always does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recover {
    On,
    Off,
}

/// Run `f` on `host`, guarded when recovery is on (M36).
fn run_guarded<A: App, T>(
    host: &mut Host<A, Ret, Inval>,
    recover: Recover,
    f: impl FnOnce(&mut Host<A, Ret, Inval>) -> T,
) -> Option<T> {
    #[cfg(panic = "unwind")]
    if recover == Recover::On {
        return host.guard(f);
    }
    let _ = recover;
    Some(f(host))
}

/// Save a session's application, for a reload. A function pointer and
/// not a bound, so `Local<A>` does not require `A: Reloadable` of an
/// application that never reloads.
pub type Saver<A> = fn(&A) -> Vec<u8>;

/// A statically linked — or library-resident — application behind
/// [`Session`].
pub struct Local<A: App> {
    pub host: Host<A, Ret, Inval>,
    program: Option<Program<A>>,
    title: &'static str,
    page: Color,
    metrics: Option<FontMetrics>,
    recover: Recover,
    saver: Option<Saver<A>>,
    restore_log: String,
    #[cfg(feature = "live-agent")]
    agent: lumen_static_agent::Agent,
}

impl<A: App + 'static> Local<A> {
    /// A session over a described program: boots it through
    /// [`Program::host`], installs the thread spawner, and measures text
    /// with the system face.
    pub fn from_program(program: Program<A>, recover: Recover) -> Self {
        let host = program.host();
        let page = program.page;
        let mut s = Local::from_host(host, "", page, recover);
        s.program = Some(program);
        s
    }

    /// A session over an already-built host.
    pub fn from_host(mut host: Host<A, Ret, Inval>, title: &'static str, page: Color, recover: Recover) -> Self {
        // **Deferred work, wired up** — M28/T28.12. The waker comes from
        // the window, which owns the loop ([`Session::set_waker`]).
        host.set_spawner(Box::new(lumen_static::spawn::Threads));
        let font = FontStack::system();
        let metrics = font.map(FontMetrics::new);
        Local {
            host,
            program: None,
            title,
            page,
            metrics,
            recover,
            saver: None,
            restore_log: String::new(),
            #[cfg(feature = "live-agent")]
            agent: lumen_static_agent::Agent::new(),
        }
    }

    /// Let [`Session::save`] answer — a hot library sets this.
    pub fn with_saver(mut self, saver: Saver<A>) -> Self {
        self.saver = Some(saver);
        self
    }

    /// Record what a restore could not keep, for
    /// [`Session::take_restore_log`].
    pub fn note_restore(&mut self, log: String) {
        self.restore_log = log;
    }

    /// The program this session was booted from.
    pub fn program(&self) -> Option<&Program<A>> {
        self.program.as_ref()
    }

    /// Run `f` guarded when recovery is on (M36).
    fn guarded<T>(&mut self, f: impl FnOnce(&mut Host<A, Ret, Inval>, &mut Option<FontMetrics>) -> T) -> Option<T> {
        let metrics = &mut self.metrics;
        run_guarded(&mut self.host, self.recover, |h| f(h, metrics))
    }

    /// A `&self` call into application code — the title, the selection —
    /// caught when recovery is on, answering `fallback` if it panicked.
    fn caught<T>(&self, fallback: T, f: impl FnOnce(&Host<A, Ret, Inval>) -> T) -> T {
        #[cfg(panic = "unwind")]
        if self.recover == Recover::On {
            return std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&self.host))).unwrap_or(fallback);
        }
        let _ = &fallback;
        f(&self.host)
    }

    /// Put the panic screen in the host's list, sized to the surface.
    fn show_failure(&mut self, w: u32, h: u32) {
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

    fn pump_now(h: &mut Host<A, Ret, Inval>, m: &mut Option<FontMetrics>) -> bool {
        match m.as_mut() {
            Some(m) => h.pump_with(m),
            None => h.pump(),
        }
    }
}

impl<A: App + 'static> FrameSource for Local<A> {
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

impl<A: App + 'static> Session for Local<A> {
    fn post(&mut self, ev: Event) {
        self.host.post(ev);
    }

    fn copy(&self) -> Option<String> {
        self.caught(None, |h| h.copy().map(str::to_owned))
    }

    fn paste(&mut self, text: &str) {
        // Guarded: paste runs the focused widget's editing code.
        let _ = self.guarded(|h, _| h.paste(text));
    }

    fn compose(&mut self, c: Composition<'_>) -> bool {
        // Guarded: composition runs the focused widget's editing code.
        self.guarded(|h, _| h.compose(c)).unwrap_or(true)
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
        return self.recover == Recover::On && self.host.failure().is_some();
        #[cfg(not(panic = "unwind"))]
        false
    }

    fn retry(&mut self) -> bool {
        #[cfg(panic = "unwind")]
        if self.recover == Recover::On {
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
        let recover = self.recover;
        let mut live = Live { host: &mut self.host, metrics: &mut self.metrics, screen, page, recover };
        // **One call.** `dispatch_input` already falls through to
        // `dispatch` for anything that is not an `input.*` method or a
        // screenshot.
        self.agent.dispatch_input(&mut live, &req).to_string()
    }

    #[cfg(not(feature = "live-agent"))]
    fn agent(&mut self, _req: &str, _screen: &mut dyn Screen) -> String {
        String::from(
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32601,"message":"this build has no agent"}}"#,
        )
    }

    fn agent_frame_done(&mut self) {
        // Guarded: the journal walks the tree through every widget's
        // `observe` (M36).
        #[cfg(feature = "live-agent")]
        {
            let agent = &mut self.agent;
            let metrics = &mut self.metrics;
            let _ = run_guarded(&mut self.host, self.recover, |h| {
                let timing = lumen_static_agent::FrameTiming::default();
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
            Some(save) => self.caught(Vec::new(), |h| save(&h.app)),
            None => Vec::new(),
        }
    }

    fn take_restore_log(&mut self) -> String {
        std::mem::take(&mut self.restore_log)
    }

    fn repaint_all(&mut self) {
        self.host.repaint_all();
    }
}

/// The agent's view of a session: the host and the tree, and the
/// window's renderer for pixels. Moved here from the window crate by
/// M37, because the tree now lives wherever the session does.
#[cfg(feature = "live-agent")]
pub struct Live<'a, A: App> {
    pub host: &'a mut Host<A, Ret, Inval>,
    pub metrics: &'a mut Option<FontMetrics>,
    pub screen: &'a mut dyn Screen,
    pub page: Color,
    pub recover: Recover,
}

#[cfg(feature = "live-agent")]
impl<A: App> lumen_static_agent::AgentHost for Live<'_, A> {
    fn tree(&self) -> lumen_static_agent::TreeRef<'_> {
        lumen_static_agent::TreeRef::of_root(self.host.app.tree(), &self.host.styles, &self.host.states)
            .with_sheet(self.host.stylesheet(), self.host.sheet_findings())
    }

    fn enqueue(&mut self, ev: Event) {
        self.host.post(ev);
    }

    fn set_palette(&mut self, palette: lumen_static::style::Palette) -> bool {
        self.host.set_palette(palette);
        true
    }

    fn pump(&mut self) -> (u64, bool) {
        let metrics = &mut *self.metrics;
        let handled = run_guarded(self.host, self.recover, |h| match metrics.as_mut() {
            Some(m) => h.pump_with(m),
            None => h.pump(),
        })
        .unwrap_or(false);
        #[cfg(panic = "unwind")]
        if let Some(r) = self.host.failure().cloned() {
            let (w, h) = self.screen.frame_rgba().map_or((1, 1), |(w, h, _)| (w, h));
            let list = match self.metrics.as_mut() {
                Some(m) => crate::recover::failure_frame(&r, w, h, m),
                None => crate::recover::failure_frame(&r, w, h, &mut lumen_static::cx::BlockMetrics),
            };
            self.host.show(list);
        }
        self.screen.present(self.host, self.page);
        (self.host.frame_number(), handled)
    }

    fn failure(&self) -> serde_json::Value {
        #[cfg(panic = "unwind")]
        if self.recover == Recover::On {
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
        if self.recover == Recover::On {
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

    #[cfg(feature = "shadow-oracle")]
    fn shadow(&self) -> Option<serde_json::Value> {
        let r = self.host.shadow_report();
        Some(serde_json::json!({
            "supported": true,
            "compared": r.compared,
            "first": r.first.map(|d| serde_json::json!({
                "frame": d.frame, "node": d.node, "field": d.field,
            })),
        }))
    }
}

/// The shell's part of a hot library's ABI stamp (M37/T37.0/T37.1): its
/// version and features, and **this file's own text, hashed** — the
/// [`Session`] trait is defined here, and an edit to it moves vtable
/// slots that no size would show.
pub fn abi_fields() -> Vec<(&'static str, String)> {
    let mut features = Vec::new();
    for (on, name) in [
        (cfg!(feature = "agent"), "agent"),
        (cfg!(feature = "live-agent"), "live-agent"),
        (cfg!(feature = "reload"), "reload"),
        (cfg!(feature = "truetype"), "truetype"),
        (cfg!(feature = "everything"), "everything"),
        (cfg!(feature = "shadow-oracle"), "shadow-oracle"),
        (cfg!(feature = "simd"), "simd"),
        (cfg!(feature = "threads"), "threads"),
    ] {
        if on {
            features.push(name);
        }
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in include_bytes!("session.rs") {
        h = (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
    vec![
        ("lumen-static-shell", String::from(env!("CARGO_PKG_VERSION"))),
        ("lumen-static-shell features", features.join(",")),
        ("session.rs", format!("{h:016x}")),
        ("Box<dyn Session>", format!("{}", core::mem::size_of::<Box<dyn Session>>())),
    ]
}
