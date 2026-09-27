//! The host: the frame loop, a software renderer, and the surface an
//! agent drives — plan T5.3.
//!
//! This is everything between an application and the operating system
//! *except* the OS binding itself. A winit front end adds one thing:
//! translating window events into [`Host::post`] and blitting
//! [`Canvas::pixels`](software::Canvas). Keeping that seam explicit is what lets the loop be
//! tested headlessly, which is how every other subsystem here got its
//! acceptance test.
#![forbid(unsafe_code)]

use lumen_static::cx::{BlockMetrics, LayoutCx, TextMeasure};
use lumen_static::desc::NodeState;
use lumen_static::display::{Cmd, Color, DisplayList, Renderer};
use lumen_static::event::App;
use lumen_static::invalidate::Invalidation;
use lumen_static::retain::{Damage, Incremental, Retention};
use lumen_static::style::{Rule, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{pass, Constraints, Event, Point};

/// M53/T53.0: what a pump did, counted.
pub mod frame_cost;
/// M12/T12.2: golden images, with a diff artifact.
pub mod golden;
/// M32: `application(boot)` — the program described once.
pub mod program;
/// M36: a panic's report, and the host's recovery from it.
#[cfg(panic = "unwind")]
pub mod recover;
// A running application with its type erased: what a window drives,
// and what a hot library hands the loader (M37/T37.1). Measures text,
// so it needs the face.
#[cfg(feature = "truetype")]
pub mod session;
pub mod software;
pub mod svg;
#[cfg(feature = "truetype")]
pub mod text;

/// What the shadow oracle has compared, and the first place a bounded
/// frame disagreed with the whole-tree one — M34/T34.1.
#[cfg(feature = "shadow-oracle")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Shadow {
    /// Frames compared. Zero means the oracle never ran — a drive that
    /// asserts "no divergence" must assert this is not zero first.
    pub compared: u64,
    /// Of those, frames the idle fast path skipped — M53/T53.3. Printed
    /// beside `compared` by every drive, so a run in which nothing was
    /// skipped cannot pass as one in which skipping was checked.
    pub skipped: u64,
    pub first: Option<Divergence>,
}

/// Where a bounded frame first differed from the whole-tree one.
#[cfg(feature = "shadow-oracle")]
#[derive(Clone, Copy, Debug)]
pub struct Divergence {
    pub frame: u64,
    pub node: u32,
    /// Which part of the node's state differed: `size`, `pos`,
    /// `colour`, `font` or `shape` from `NodeState`, or `paint` when the
    /// states agreed and the painted lists did not — state a widget keeps
    /// for itself (M41/T41.2).
    pub field: &'static str,
}

/// The oracle's buffers, kept across frames — M41/T41.2.
#[cfg(feature = "shadow-oracle")]
#[derive(Default)]
struct ShadowScratch {
    list: DisplayList,
    live: Vec<bool>,
    before: Vec<u64>,
    after: Vec<u64>,
}

/// `LUMEN_SHADOW_TALLY`'s writer — see `Host::shadow_check`. `None` is
/// a compared frame, `Some` a host's first divergence.
#[cfg(feature = "shadow-oracle")]
fn shadow_tally(d: Option<&Divergence>) {
    use std::io::Write;
    static FILE: std::sync::OnceLock<Option<std::sync::Mutex<std::fs::File>>> =
        std::sync::OnceLock::new();
    let Some(file) = FILE.get_or_init(|| {
        let path = std::env::var_os("LUMEN_SHADOW_TALLY")?;
        let f = std::fs::OpenOptions::new().create(true).append(true).open(path).ok()?;
        Some(std::sync::Mutex::new(f))
    }) else {
        return;
    };
    let line = match d {
        None => "c\n".to_string(),
        Some(d) => {
            let exe = std::env::current_exe().ok();
            let exe = exe.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("?");
            let thread = std::thread::current();
            let thread = thread.name().unwrap_or("?").replace(' ', "_");
            format!("d {exe} {thread} {} {} {}\n", d.frame, d.node, d.field)
        }
    };
    if let Ok(mut f) = file.lock() {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Which table indices `node(i)` answers for with a node of their own,
/// rather than with an enclosing container standing in for a branch that
/// is not laid out. The rule `lumen_static_agent::live_nodes` documents:
/// a stand-in is the same object, and the same kind, as its parent's.
#[cfg(feature = "shadow-oracle")]
fn shadow_live(root: &dyn Node, table: &[lumen_static::desc::NodeDesc], out: &mut Vec<bool>) {
    let ident = |n: &dyn Node| (n as *const dyn Node as *const (), n.kind_name());
    out.clear();
    out.resize(table.len(), true);
    for (i, d) in table.iter().enumerate() {
        if d.parent == lumen_static::NO_PARENT {
            continue;
        }
        let p = d.parent as usize;
        out[i] = out[p] && ident(root.node(i)) != ident(root.node(p));
    }
}

/// Each live leaf's own paint, hashed — the journal's per-node signature
/// (`lumen_static_agent`'s `measure_into`), in logical units. A container
/// signs nothing here: its own paint is resolved into `NodeState`, which
/// the oracle compares field by field.
#[cfg(feature = "shadow-oracle")]
fn sign_leaves(
    root: &dyn Node,
    table: &[lumen_static::desc::NodeDesc],
    live: &[bool],
    states: &[NodeState],
    list: &mut DisplayList,
    out: &mut Vec<u64>,
) {
    out.clear();
    out.resize(table.len(), 0);
    list.clear();
    list.set_bounds(None);
    for (i, d) in table.iter().enumerate() {
        if d.child_count == 0 && live[i] {
            let from = list.len();
            root.node(i).paint(states, i, list);
            out[i] = list.hash_from(from);
        }
    }
}

/// A running application: the tree, its parallel arrays, the one input
/// queue, and the frame's display list.
pub struct Host<A: App, R: Retention = Incremental, I: Invalidation = lumen_static::Boundaries<256>> {
    pub app: A,
    /// Results produced off the frame loop — M28/T28.2.
    ///
    /// Drained in [`Host::pump_with`], immediately before
    /// [`App::update`] runs, so a message and the bindings that present
    /// it settle in the **same** frame. Draining after `update` would
    /// show every deferred result one frame late, which is the class of
    /// bug `docs/layout-and-storage-decisions.md` §1 exists to prevent
    /// for synchronous state.
    ///
    /// An application that defers nothing has `Msg = Never`, so this is
    /// an inbox of an uninhabited type: the channel exists, nothing can
    /// be put in it, and the drain is a `try_recv` that always answers
    /// empty.
    pub inbox: lumen_static::inbox::Inbox<A::Msg>,
    /// The last drain hit its per-frame cap, so more may be waiting —
    /// M28/T28.2. A shell asks for another frame rather than the host
    /// raising the cap.
    more_deferred: bool,
    /// Where deferred work runs — M28/T28.3.
    ///
    /// `dyn` at the shell, which `docs/plan-agent-parity.md`'s appendix
    /// settled: never a widget field, never a fourth type parameter on
    /// `Host`. `None` until a shell installs one, and a spawn without a
    /// spawner is refused and counted rather than silently dropped —
    /// see [`Host::spawn`].
    spawner: Option<Box<dyn lumen_static::spawn::Spawner<A::Msg>>>,
    /// Timer firings coalesced for want of outbox room, over this host's
    /// life — M47/T47.2. [`Host::timer_coalesced`].
    timer_coalesced: u64,
    /// Every job spawned and not yet landed — M47/T47.3. Its length is
    /// the in-flight count ([`Host::in_flight`]).
    ledger: lumen_static::spawn::Ledger,
    /// Spawns refused for want of a spawner. Counted for the same reason
    /// the inbox counts refusals: a silent drop is the failure a gate
    /// exists to make visible.
    unspawned: usize,
    /// Descriptors whose `(kind, step)` the loaded build does not
    /// recognise — M28/T28.4, `docs/plan-tasks.md` §2.1's stability
    /// rule.
    unknown_kind: usize,
    /// What the host has already started — M28/T28.10. Diffed each frame
    /// against what the application declares.
    running: Vec<(u64, u64)>,
    /// Subscriptions started and stopped, since the host was made. A
    /// test reads the pair; a start that repeats every frame is the
    /// defect the diff exists to prevent, and a counter is how it
    /// becomes visible.
    sub_started: usize,
    sub_stopped: usize,
    pub styles: Vec<Style>,
    pub states: Vec<NodeState>,
    pub list: DisplayList,
    /// Focus, pointer capture and modifier state.
    ///
    /// One field since M14/T14.1. It was `focus` and `capture` as
    /// separate fields, and modifier tracking would have made a third —
    /// three things for a session recording to snapshot, and three
    /// places for `dispatch` to disagree with the host.
    ///
    /// `capture` is the node that owns the current pointer gesture
    /// (M11/T11.2): set when a node consumes a `PointerDown`, cleared on
    /// the release. While it is set, moves go straight to that node and
    /// nothing walks — which is what lets a slider keep tracking after
    /// the pointer has left it without reinstating the per-motion
    /// ancestor walk M9 refused.
    pub input: lumen_static::event::InputState,
    /// Subtrees the last paint deferred to the overlay phase.
    ///
    /// **Added M24/T24.2**, and until then `Popup` did not work from
    /// this host at all — which is every example and every window, since
    /// `Host` is the front door. `Painter::defer` answered `false` with
    /// no sink, so a popup painted *inline*, in tree order, and the
    /// backgrounds of its later siblings drew straight over it;
    /// `dispatch` rather than `dispatch_overlaid` meant Escape and
    /// click-outside dismissal never ran either. The machinery had
    /// shipped in M15 with nothing wired to it.
    ///
    /// Sixteen, and a full sink refuses rather than dropping silently —
    /// see [`lumen_static::overlay::Overlays`]. Sixteen simultaneously
    /// open overlays is a menu inside a menu inside a modal, four deep.
    overlays: lumen_static::overlay::Overlays<16>,
    queue: lumen_static::InputQueue,
    /// The touch id that drives the pointer, from its `Begin` until it
    /// lifts or is cancelled — M44/T44.4. See [`Host::pump_with`].
    primary_touch: Option<u32>,
    /// Surface size in **physical** pixels — what a window reports.
    size: (u32, u32),
    /// Physical pixels per logical pixel (M10b).
    ///
    /// Everything above the renderer is logical: constraints, `Style`,
    /// `NodeState`, hit testing and every bound an agent reports. Only
    /// the display list is physical, produced by wrapping the painter in
    /// `Scaled`. Without this the framework laid out against *physical*
    /// pixels, so a window rendered half-size on a 2x display and a
    /// 24-logical-pixel accessibility floor was measured against numbers
    /// twice too large.
    scale: f32,
    frame: u64,
    last_layout_ns: u64,
    last_place_ns: u64,
    last_paint_ns: u64,
    /// Passes run since construction — M53/T53.0. See [`Host::pass_counts`].
    passes: frame_cost::PassCounts,
    /// The last pump's stages and whole — M53/T53.1. See [`Host::stage_timings`].
    stages: frame_cost::StageTimes,
    /// How much of the previous frame is kept, and what changed
    /// (`docs/plan-architecture-v2.md` M8b). A type parameter rather
    /// than a feature so `Rebuild` stays available in the same binary
    /// as the oracle every incremental gate is compared against.
    retain: R,
    last_damage: Damage,
    /// Which subtrees changed since the last frame —
    /// `docs/plan-invalidation.md` T16.7.
    ///
    /// A second type parameter beside `R`, and for the same reason:
    /// `Everything` is the whole-tree walk *and* the oracle a bounded
    /// walk is checked against, so both must be constructible in one
    /// binary. It defaults to `Boundaries<256>` since M34/T34.2 — the
    /// same type the window ships (`lumen_static_winit::Inval`) — so a
    /// headless `Host<A>` and a window draw with one strategy. It was
    /// `Everything` until then, and every headless test ran the
    /// strategy no user did.
    invalidation: I,
    /// Whether an arena still owes the cascade a pass.
    ///
    /// **Set by a change, cleared by the pass that follows it.** A
    /// stylesheet is applied before the first frame and an arena belongs
    /// to a widget the application can swap in afterwards — so a rule
    /// naming a node inside a repeated row was dead until this existed
    /// (`lumen_static::style::cascade_arenas_deep`, M27/T27.11). Run
    /// after `App::update` and before layout, which is when the swapped
    /// subtree exists and before anything measures it.
    ///
    /// Only the arenas: the main table was cascaded when the sheet was
    /// set, and re-running that every frame an application changed is
    /// the per-frame cascade M16 removed.
    ///
    /// **Owed, then paid only inside the frame's marks** (M53/T53.2):
    /// every site that sets this also marks where its change landed, and
    /// the pump asks only the marked nodes for arenas
    /// (`style::cascade_arenas_marked`). A frame that marked everything
    /// walks the whole table.
    arenas_dirty: bool,
    /// The last stylesheet applied — M25/T25.2.
    ///
    /// Kept because the cascade consumes a sheet and keeps only its
    /// answer, so nothing afterwards could say which rule did nothing.
    /// `Rule` is `Copy`, and a sheet is set, not pumped.
    sheet: Vec<Rule>,
    /// What [`lumen_static::style::lint_sheet_deep`] found in `sheet`:
    /// rules whose selector matches nothing, or that set only fields no
    /// matched node reads. The agent reports them as `W1010`.
    #[cfg(feature = "agent")]
    sheet_findings: Vec<lumen_static::style::SheetFinding>,
    /// The palette every look is resolved against — M25/T25.3.
    ///
    /// Data, swapped at runtime by [`Host::set_palette`], and handed to
    /// `layout` through [`LayoutCx::with_palette`]. `Palette::LIGHT` until
    /// set.
    palette: lumen_static::style::Palette,
    /// Where the sheet comes from — M32 (`docs/plan-program.md`).
    ///
    /// Set by [`program::Program::host`], and re-asked by
    /// [`Host::adopt`], so a swapped-in application is styled by the
    /// same description that styled the first one. `None` for a host
    /// built with [`Host::new`] directly, which is styled only by what
    /// its caller sets.
    sheet_source: Option<std::rc::Rc<dyn program::Sheet<A>>>,
    /// The program's boot — M49/T49.4. Set by [`program::Program::host`]
    /// beside the sheet source, and what `Host::reload` rebuilds
    /// through: the one construction path, in process as in a window
    /// swap. `None` for a host built with [`Host::new`] directly.
    boot: Option<program::Boot<A>>,
    /// What the last frame had marked when it cleared — M29/T29.3.
    last_marks: lumen_static::invalidate::Marks,
    /// Whether the last frame owed the arenas a cascade — M29/T29.4.
    last_arenas_owed: bool,
    #[cfg(feature = "shadow-oracle")]
    shadow: Shadow,
    /// The oracle's retained buffers — M41/T41.2.
    #[cfg(feature = "shadow-oracle")]
    shadow_scratch: ShadowScratch,
    /// The panic the last guarded entry caught, until `retry` — M36.
    #[cfg(panic = "unwind")]
    failure: Option<recover::PanicReport>,
    /// The application's source directory, for the report's application
    /// frame — M36. `application(..)` records it; a test sets it.
    source_root: Option<&'static str>,
    /// Jobs a boot function returned before any spawner existed. Started
    /// by [`Host::set_spawner`]; see [`Host::hold_until_spawner`].
    held: Vec<lumen_static::spawn::Job<A::Msg>>,
}

impl<A: App, R: Retention, I: Invalidation> Host<A, R, I> {
    /// This host's invalidation strategy, by name: `type_name::<I>()`,
    /// e.g. `lumen_static::invalidate::Boundaries<256>` — M42/T42.1.
    ///
    /// What the agent's `app.diagnostics` reports as `process.strategy`,
    /// so a drive can check the strategy of the process it drove rather
    /// than trusting a label it set for itself (R18-F1). Read from the
    /// type parameter, not from a feature: the parameter is what the host
    /// actually runs.
    pub fn strategy(&self) -> &'static str {
        core::any::type_name::<I>()
    }

    /// Install the program's sheet, and keep its source so a later
    /// [`Host::adopt`] can re-ask it — M32.
    pub fn set_sheet_source(&mut self, source: std::rc::Rc<dyn program::Sheet<A>>) {
        let sheet = source.sheet(&self.app);
        self.sheet_source = Some(source);
        if !sheet.is_empty() {
            self.set_stylesheet(&sheet);
        }
    }

    /// Keep the program's boot, for `Host::reload` — M49/T49.4.
    pub fn set_boot(&mut self, boot: program::Boot<A>) {
        self.boot = Some(boot);
    }

    /// Keep a job until a spawner is installed — M32.
    ///
    /// Only for what a boot function returned: it runs before the window
    /// has installed its spawner, and a headless host may never install
    /// one. [`Host::spawn`] still *refuses* without a spawner, for the
    /// reason its rustdoc gives; a boot's jobs are the exception because
    /// they are known before the first frame and the spawner is known to
    /// be coming.
    pub fn hold_until_spawner(&mut self, job: lumen_static::spawn::Job<A::Msg>) {
        self.held.push(job);
    }

    /// What the last frame had marked — M29/T29.3. A narrowed frame and
    /// one that fell back to marking everything draw the same pixels;
    /// this is what tells them apart.
    pub fn last_marks(&self) -> lumen_static::invalidate::Marks {
        self.last_marks
    }

    /// Whether the last frame owed the arenas a cascade — M29/T29.4. Set
    /// by any binding write, narrowed or not, because nothing shows a
    /// binding cannot change a template's styles (§2.4).
    pub fn last_arenas_owed(&self) -> bool {
        self.last_arenas_owed
    }

    /// Judge the frame just painted against a whole-tree one — M34/T34.1,
    /// redesigned by M41/T41.2 (`docs/plan-boundaries-repair.md` §2.2).
    ///
    /// **When it runs: after paint and `retain.finish`**, so the frame it
    /// judges is the bounded pass's own output, painted before anything
    /// here touched a widget. T34.1's first version ran *before* paint,
    /// and its second layout re-ran every `layout` on the live widgets —
    /// a `TextInput`'s scroll, a `Repeat`'s binds — so paint then drew
    /// the repaired widgets and the oracle judged a frame it had already
    /// fixed. Every defect it could see, it hid (R1-F6).
    ///
    /// **What it compares, in order:**
    ///
    /// 1. Each live **leaf's own paint** — every node with no table
    ///    children, which includes an arena container such as `Repeat` —
    ///    is signed from the bounded widgets *before* the second layout.
    /// 2. It lays out and places again under [`Changed::ALL`] into a
    ///    scratch copy of the states, applies the focus-ring rule, and
    ///    paints the whole tree into a scratch list through `Scaled` —
    ///    **not** `Recording`, so retention, `last_damage` and the
    ///    journal are untouched — with scratch overlays.
    /// 3. `NodeState`, field by field: the first node that differs is
    ///    named with `size`, `pos`, `colour`, `font` or `shape`.
    /// 4. Otherwise the two lists' hashes. If they differ the leaf whose
    ///    signature moved is named with `field: "paint"` — state a widget
    ///    keeps for itself, which `NodeState` cannot show. A list that
    ///    differs with no leaf's signature moved is a container's own
    ///    internal state, and is reported against the root, still
    ///    `"paint"`.
    ///
    /// **What it cannot avoid: it repairs the frames *after* the one it
    /// judges.** Widgets are not `Clone` and `App` is not either, so the
    /// second layout runs on the live widgets, and the next frame starts
    /// from what it wrote. A defect is one report on its first frame, not
    /// pixels that persist — which is why a drive judging `Boundaries`
    /// keeps a leg without this feature (`plan-boundaries-repair.md` §0
    /// rule 2).
    ///
    /// One host, not two: a second host would run every handler and
    /// spawn every task twice. It reports rather than panics, because a
    /// panic in a debug window becomes a panic screen; set
    /// `LUMEN_SHADOW_ABORT=1` to make it abort instead.
    ///
    /// **`LUMEN_SHADOW_TALLY=<file>`** appends one line per compared
    /// frame to that file — `c`, or `d <exe> <thread> <frame> <node>
    /// <field>` for a host's first divergence — so a sweep over many
    /// processes, such as `cargo test --workspace` with this feature, can
    /// say how many frames it judged and where (M41/T41.7). Every host in
    /// every process appends to the one file; each line is written whole.
    ///
    /// **What it costs:** one whole-tree layout, place and paint, plus a
    /// paint of every leaf twice, on every frame the oracle is compiled
    /// into. A diagnostic behind its own feature; recorded, not gated.
    ///
    /// [`Changed::ALL`]: lumen_static::invalidate::Changed::ALL
    #[cfg(feature = "shadow-oracle")]
    fn shadow_check(&mut self, metrics: &mut dyn TextMeasure, limits: lumen_static::Constraints) {
        let table = <A::Tree as Widget>::SUB_TABLE;
        let mut sc = core::mem::take(&mut self.shadow_scratch);
        // 1. The bounded widgets' own paint, before anything repairs it.
        shadow_live(self.app.tree(), table, &mut sc.live);
        sign_leaves(self.app.tree(), table, &sc.live, &self.states, &mut sc.list, &mut sc.before);

        // 2. The whole-tree frame, into scratch.
        let mut scratch = self.states.clone();
        {
            let mut cx = LayoutCx::with_changed(metrics, lumen_static::invalidate::Changed::ALL)
                .with_palette(&self.palette)
                .with_scale(self.scale);
            let tree = self.app.tree_mut();
            tree.layout(&mut cx, limits, &self.styles, &mut scratch, 0);
            tree.place(Point::default(), &self.styles, &mut scratch, 0);
        }
        if !self.input.focus_visible {
            if let Some(s) = self.input.focus.and_then(|f| scratch.get_mut(f as usize)) {
                s.focus_ring = 0;
            }
        }
        sc.list.clear();
        sc.list.set_bounds(self.list.bounds);
        {
            let mut ov = lumen_static::overlay::Overlays::<16>::default();
            let mut out = lumen_static::Scaled::new(&mut sc.list, self.scale);
            pass::paint_overlaid(self.app.tree(), &scratch, &mut out, &mut ov);
        }
        let lists_differ = sc.list.hash_from(0) != self.list.hash_from(0);
        self.shadow.compared += 1;
        shadow_tally(None);
        if self.shadow.first.is_some() {
            self.shadow_scratch = sc;
            return;
        }

        // 3. `NodeState`, the more specific answer when it has one.
        //
        // The *deepest* differing node on the first chain, not the first
        // in table order: a leaf that grew grows every ancestor, and the
        // root is not what changed. Pre-order makes it one forward scan —
        // a differing node inside the current one's subtree is a
        // descendant of it.
        let differs = |a: &NodeState, b: &NodeState| -> Option<&'static str> {
            Some(if a.size != b.size {
                "size"
            } else if a.pos != b.pos {
                "pos"
            } else if a.bg != b.bg || a.fg != b.fg || a.border_color != b.border_color {
                "colour"
            } else if a.font_size != b.font_size || a.font_weight != b.font_weight {
                "font"
            } else if a.radius != b.radius || a.pad != b.pad || a.focus_ring != b.focus_ring {
                "shape"
            } else {
                return None;
            })
        };
        let mut found: Option<(u32, &'static str)> = None;
        let mut end = self.states.len();
        for (i, (a, b)) in self.states.iter().zip(&scratch).enumerate() {
            if i >= end {
                break;
            }
            if let Some(field) = differs(a, b) {
                found = Some((i as u32, field));
                end = lumen_static::style::subtree_range(table, i).end.min(end);
            }
        }
        // 4. The lists, attributed to the leaf whose own paint moved.
        if found.is_none() && lists_differ {
            sign_leaves(self.app.tree(), table, &sc.live, &scratch, &mut sc.list, &mut sc.after);
            let leaf = sc.before.iter().zip(&sc.after).position(|(a, b)| a != b);
            found = Some((leaf.unwrap_or(0) as u32, "paint"));
        }
        self.shadow_scratch = sc;
        if let Some((node, field)) = found {
            let d = Divergence { frame: self.frame, node, field };
            shadow_tally(Some(&d));
            if std::env::var_os("LUMEN_SHADOW_ABORT").is_some() {
                eprintln!("shadow oracle: {d:?}");
                std::process::abort();
            }
            self.shadow.first = Some(d);
        }
    }

    /// What the shadow oracle has seen — M34/T34.1.
    #[cfg(feature = "shadow-oracle")]
    pub fn shadow_report(&self) -> Shadow {
        self.shadow
    }

    /// Where the application's source lives, relative to where it was
    /// built — `fixtures/panic-app` — so a panic report can name the
    /// application's own line when the panic is inside the framework.
    /// Recorded by `program::application` from its caller; M36.
    pub fn set_source_root(&mut self, root: &'static str) {
        self.source_root = Some(root);
    }

    /// **Run `f` guarded** — M36/T36.1 (`docs/plan-panic-recovery.md`).
    ///
    /// A panic inside `f` — a handler, `apply`, a binding, a widget's
    /// `layout` or `paint`, `tick` — is caught, reported and recovered
    /// from: the host's own invariants are restored and the report is
    /// kept until [`Host::retry`]. `None` when it panicked. **While a
    /// failure is kept, `f` is not run at all**: running it would be an
    /// automatic retry, and a panic in `layout` would then spin.
    #[cfg(panic = "unwind")]
    pub fn guard<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> Option<T> {
        if self.failure.is_some() {
            return None;
        }
        recover::install();
        recover::clear();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut *self))) {
            Ok(t) => Some(t),
            Err(payload) => {
                let report = recover::report(&*payload, self.source_root);
                self.recover_from_panic();
                self.failure = Some(report);
                None
            }
        }
    }

    /// The panic the last guarded entry caught, if it has not been
    /// retried — M36.
    #[cfg(panic = "unwind")]
    pub fn failure(&self) -> Option<&recover::PanicReport> {
        self.failure.as_ref()
    }

    /// Present `list` instead of the application's frame, as full damage
    /// — the panic screen (M36/T36.3). The next real frame is full damage
    /// too, because the retained journal no longer describes the surface.
    #[cfg(panic = "unwind")]
    pub fn show(&mut self, list: DisplayList) {
        self.list = list;
        self.repaint_all();
    }

    /// **The surface holds pixels this host did not draw**: repaint all
    /// of it, in every buffer the swapchain still has. Through the
    /// retained history rather than as one frame's full damage, because
    /// a buffer two presents old holds the foreign pixels too — M37/T37.4,
    /// where a reload's error bar outlived its fix on the older buffers.
    ///
    /// **Marks everything too** (M53/T53.3): the next pump must paint,
    /// or the idle fast path would keep a list nothing re-rendered and
    /// report no damage for a surface that needs all of it.
    pub fn repaint_all(&mut self) {
        self.retain.reset();
        self.last_damage = Damage::All;
        self.invalidation.mark_all();
    }

    /// Clear the failure, so the next guarded entry runs again — M36.
    /// The host was reset when the panic was caught; the application's
    /// own state is as the panic left it (plan §2.2: no rollback).
    #[cfg(panic = "unwind")]
    pub fn retry(&mut self) {
        self.failure = None;
    }

    /// Restore what a mid-frame panic can leave inconsistent — M36 §2.2.
    ///
    /// The display list, the overlays and the retained journal were
    /// possibly half-built, so the next frame is full damage. The marks
    /// may be partial, so everything is marked. Pointer state that
    /// dispatch latched before the panic — capture, the pressed and
    /// hovered node — is released, or a retried frame keeps a hover shade
    /// nothing will clear. Events queued behind the one that panicked
    /// were produced against a frame that never finished, and are
    /// dropped. Focus is kept.
    #[cfg(panic = "unwind")]
    fn recover_from_panic(&mut self) {
        self.list.clear();
        self.overlays = Default::default();
        self.retain.reset();
        self.last_damage = Damage::All;
        self.invalidation.mark_all();
        self.arenas_dirty = true;
        self.input.capture = None;
        self.input.pressed = None;
        self.input.hovered = None;
        self.input.hover_inner = None;
        self.input.swallow_release = false;
        // T44.2's latch goes with the press it identified.
        self.input.press = None;
        for n in self.states.iter_mut() {
            n.pressed = false;
            n.hovered = false;
        }
        while self.queue.pop().is_some() {}
    }

    /// Jobs held for a spawner that has not been installed yet.
    pub fn held_jobs(&self) -> usize {
        self.held.len()
    }

    /// Adopt a newly built application and re-ask for a stylesheet.
    ///
    /// **The primitive half of a hot swap**, and the line that was
    /// missing: the host owns `sheet: Vec<Rule>`, nothing in the reload
    /// sequence touched it, and an edited stylesheet therefore never
    /// reached the frame. `crates/lumen-static-shell/tests/reload_sheet.rs`
    /// demonstrated that before this existed and guards it now.
    ///
    /// Takes the application rather than building it, so the reload
    /// sequence — `save`, boot, `restore` — stays with the caller that
    /// already has the `reload` surface, and this method needs neither
    /// the feature nor the bound. `Host::reload` (behind the
    /// `reload` feature) is the sugar for when it does — a plain code
    /// span, not a link, because a link to a `cfg`-gated item does not
    /// resolve in a default-feature doc build.
    pub fn adopt(&mut self, app: A) {
        self.app = app;
        self.reasked();
    }

    /// What adopting an application re-derives: the sheet, re-asked from
    /// the program's description (M32), the one place a sheet comes from
    /// — a host with no source keeps what it has — and every mark,
    /// because nothing about last frame's describes a new state.
    ///
    /// **For a host without a sheet source this mark is the only one**
    /// (M41/T41.0, R5-F6). With a source, `set_stylesheet` marks
    /// everything too and hides this line: deleting it left every
    /// reload gate green. A host built with [`Host::new`] has no source,
    /// and without this mark an adopted or restored application is
    /// drawn over last frame's layout — `tests/reload_oracle.rs`'s
    /// sourceless arms, which print the pixels a missing mark differs by.
    fn reasked(&mut self) {
        if let Some(source) = self.sheet_source.clone() {
            let sheet = source.sheet(&self.app);
            self.set_stylesheet(&sheet);
        }
        self.invalidation.mark_all();
        self.arenas_dirty = true;
    }

    /// Graft saved `#[reload(keep)]` state onto the application this host
    /// already runs — M37/T37.1. A window's swap boots the **new**
    /// build's program (`docs/plan-window-reload.md` §2.2) and restores
    /// onto it here, so construction never goes through a pointer held
    /// across the swap.
    #[cfg(feature = "reload")]
    pub fn restore(&mut self, saved: &lumen_static::reload::Value)
    where
        A: lumen_static::reload::Reloadable,
    {
        self.app.restore(saved);
        self.reasked();
    }

    /// Rebuild the application through the **program's boot**, keeping
    /// what `save` kept, and re-ask for the stylesheet — M49/T49.4.
    ///
    /// The same construction a hot window's swap performs — it boots the
    /// new library's `program()` and restores onto it — so the two cannot
    /// disagree (`docs/plan-reload-hardening.md` §2.2). Jobs the boot
    /// returns start as a window's do: now if a spawner is installed,
    /// held until one is otherwise.
    ///
    /// **A host not built from a program has nothing to rebuild from**,
    /// and says so: `false`, with the application untouched. It used to
    /// call `Reloadable::fresh`, a second constructor routed to
    /// `Application::build`, which nothing checked against the boot.
    #[cfg(feature = "reload")]
    pub fn reload(&mut self) -> bool
    where
        A: lumen_static::reload::Reloadable,
    {
        let Some(boot) = self.boot.clone() else { return false };
        let saved = self.app.save();
        let (mut next, jobs) = boot();
        next.restore(&saved);
        self.adopt(next);
        for job in jobs {
            if self.spawner.is_some() {
                self.spawn(job);
            } else {
                self.hold_until_spawner(job);
            }
        }
        true
    }

    pub fn new(app: A, width: u32, height: u32) -> Self {
        let n = <A::Tree as Widget>::NODE_COUNT;
        let styles = pass::default_styles(app.tree());
        Host {
            app,
            inbox: lumen_static::inbox::Inbox::new(),
            more_deferred: false,
            spawner: None,
            ledger: lumen_static::spawn::Ledger::new(),
            timer_coalesced: 0,
            unspawned: 0,
            unknown_kind: 0,
            running: Vec::new(),
            sub_started: 0,
            sub_stopped: 0,
            styles,
            states: vec![NodeState::default(); n],
            list: DisplayList::new(),
            input: lumen_static::event::InputState::default(),
            overlays: Default::default(),
            queue: lumen_static::InputQueue::new(),
            primary_touch: None,
            size: (width, height),
            scale: 1.0,
            frame: 0,
            last_layout_ns: 0,
            last_place_ns: 0,
            last_paint_ns: 0,
            passes: frame_cost::PassCounts::default(),
            stages: frame_cost::StageTimes::default(),
            retain: R::default(),
            last_damage: Damage::All,
            invalidation: I::default(),
            arenas_dirty: true,
            sheet: Vec::new(),
            #[cfg(feature = "agent")]
            sheet_findings: Vec::new(),
            palette: lumen_static::style::Palette::LIGHT,
            sheet_source: None,
            boot: None,
            held: Vec::new(),
            last_marks: lumen_static::invalidate::Marks::default(),
            last_arenas_owed: false,
            #[cfg(feature = "shadow-oracle")]
            shadow: Shadow::default(),
            #[cfg(feature = "shadow-oracle")]
            shadow_scratch: ShadowScratch::default(),
            #[cfg(panic = "unwind")]
            failure: None,
            source_root: None,
        }
    }

    /// The palette the stylesheet's roles and every `layout` resolve
    /// against (M25/T25.3, T25.11).
    pub fn palette(&self) -> &lumen_static::style::Palette {
        &self.palette
    }

    /// Swap the palette (M25/T25.3).
    ///
    /// **Re-cascades the kept sheet** (M25/T25.11): a rule that names a
    /// `style::Role` resolved to a colour when it was cascaded, so the new
    /// palette reaches it only through another cascade. The whole sheet,
    /// because [`Host::stylesheet`] is the sheet now in force however it
    /// was applied — by `set_stylesheet` or by `restyle_subtree` — and it
    /// is one cascade on an event a user makes, not per frame.
    ///
    /// **Invalidates rather than damages.** Damage follows by itself:
    /// retention compares every painted command's hash with last frame's,
    /// so a node that paints a new colour is damaged where it paints. What
    /// nothing else supplies is the *layout* that stamps the new style —
    /// under `Boundaries` a clean subtree is skipped before a widget ever
    /// sees it — so this marks everything, as [`Host::set_stylesheet`]
    /// does. An equal palette neither cascades nor marks: a host that sets
    /// its palette every frame pays for none of it.
    pub fn set_palette(&mut self, palette: lumen_static::style::Palette) {
        if palette == self.palette {
            return;
        }
        self.palette = palette;
        lumen_static::style::cascade_deep_with(
            self.app.tree_mut(),
            &self.sheet,
            &self.palette,
            &mut self.styles,
        );
        self.invalidation.mark_all();
    }

    /// The last stylesheet applied (M25/T25.2).
    pub fn stylesheet(&self) -> &[Rule] {
        &self.sheet
    }

    /// Rules in the last stylesheet that cannot change the frame
    /// (M25/T25.2) — empty for a sheet in which every rule does
    /// something. By index into [`Host::stylesheet`].
    /// **Requires the `agent` feature** — M26/T26.18. The lint is a
    /// development diagnostic costing 183 µs a sheet swap
    /// (`docs/results-frame-and-types.md` T26.0), and its only consumer
    /// is the agent's `ui.lint`, so a build without one neither runs it
    /// nor stores its findings.
    #[cfg(feature = "agent")]
    pub fn sheet_findings(&self) -> &[lumen_static::style::SheetFinding] {
        &self.sheet_findings
    }

    /// Keep the sheet in force, and lint it where anything can read the
    /// findings.
    ///
    /// The sheet itself is kept unconditionally: [`Host::set_palette`]
    /// re-cascades it when a palette role resolves differently
    /// (M25/T25.11), which has nothing to do with the agent.
    fn keep_sheet(&mut self, sheet: &[Rule]) {
        self.sheet.clear();
        self.sheet.extend_from_slice(sheet);
        #[cfg(feature = "agent")]
        {
            let mut found = core::mem::take(&mut self.sheet_findings);
            found.clear();
            lumen_static::style::lint_sheet_deep(self.app.tree_mut(), sheet, &mut |f| {
                found.push(f)
            });
            self.sheet_findings = found;
        }
    }

    /// Mark the subtree rooted at **every** node carrying
    /// `#[widget_id(wid)]` as changed (T16.7; every match since
    /// M46/T46.2).
    ///
    /// An id is a set: a composite used twice carries its part ids twice,
    /// and styling already reads `Selector::Id` as every match. Marking
    /// only the first left the second instance's frame stale under
    /// `Boundaries` — the default — while this returned `true`
    /// (`tests/duplicate_id_host.rs`).
    ///
    /// Returns whether the id matched — a typo'd id is otherwise a
    /// silent no-op that shows up as a frame which quietly failed to
    /// redraw, which is the worst shape this class of bug takes.
    ///
    /// Under `Everything` this does nothing and everything is redrawn
    /// anyway, so an application can call it unconditionally and switch
    /// the parameter later.
    #[must_use = "false means the id matched no node and nothing was invalidated; \
                  true means every node carrying it was"]
    pub fn invalidate(&mut self, wid: &str) -> bool {
        // Owed, and paid only inside the subtrees marked below
        // (M53/T53.2): an id whose subtree holds no arena costs no
        // cascade.
        self.arenas_dirty = true;
        let table = <A::Tree as Widget>::SUB_TABLE;
        let mut any = false;
        for root in lumen_static::style::WidMatches::new(table, wid) {
            self.invalidation.mark(lumen_static::style::subtree_range(table, root));
            any = true;
        }
        any
    }

    /// Mark everything changed — the answer whenever the blast radius
    /// is not known.
    pub fn invalidate_all(&mut self) {
        // A subtree may be new, and a new arena has never seen the
        // sheet (M27/T27.11).
        self.arenas_dirty = true;
        self.invalidation.mark_all();
    }

    /// What the frame just pumped considered changed. For tests and for
    /// finding out whether the set is collapsing every frame, which is
    /// the failure mode that makes the machinery pointless.
    pub fn changed(&self) -> lumen_static::invalidate::Changed<'_> {
        self.invalidation.view()
    }

    /// Apply a stylesheet. Re-runs the cascade, which is the only pass
    /// that walks the table linearly and the only one that does not run
    /// every frame.
    pub fn set_stylesheet(&mut self, sheet: &[Rule]) {
        // `cascade_deep`, not `cascade`: a rule must be able to reach a
        // repeater's template row or an `Either` branch, which are not
        // in the table (`docs/plan-review-fixes.md` F4.5).
        // Against the host's palette, so a role in the sheet resolves to
        // the theme in force rather than the light default (M25/T25.11).
        lumen_static::style::cascade_deep_with(
            self.app.tree_mut(),
            sheet,
            &self.palette,
            &mut self.styles,
        );
        self.keep_sheet(sheet);
        // A stylesheet can change any node's size, so nothing is known
        // to be clean any more.
        self.invalidation.mark_all();
    }

    /// Re-cascade the subtree under every node carrying `#[widget_id(wid)]`.
    ///
    /// The operation a class toggle performs — the tally swaps two
    /// rules when the sign flips — and the reason it is worth scoping:
    /// a full `set_stylesheet` re-resolves every node against every
    /// rule, measured at **36 µs for 901 nodes**, four times what a
    /// whole-tree layout and paint cost there
    /// (`docs/results-cascade-cost.md`).
    ///
    /// **Every match, not the first** (M46/T46.2): a rule on
    /// `Selector::Id(wid)` matches all of them, so restyling one left the
    /// rest on the old sheet — and a debug build panicked blaming the
    /// sheet for what was a duplicated id. See
    /// `style::cascade_wid_with`.
    ///
    /// `sheet` must differ from the last one cascaded only in rules that
    /// match inside those subtrees; debug builds check that against a
    /// full cascade, once, over the union. Returns whether the id was
    /// found — a typo'd id is a silent no-op otherwise, which is the
    /// failure mode this whole crate keeps finding.
    #[must_use = "false means the id matched no node and nothing was restyled; \
                  true means every node carrying it was"]
    pub fn restyle_subtree(&mut self, sheet: &[Rule], wid: &str) -> bool {
        let table = <A::Tree as Widget>::SUB_TABLE;
        let n = lumen_static::style::cascade_wid_with(
            self.app.tree_mut(),
            sheet,
            &self.palette,
            &mut self.styles,
            wid,
        );
        if n == 0 {
            return false;
        }
        // The whole sheet is the one now in force, so it is the one kept
        // and linted.
        self.keep_sheet(sheet);
        // Scoped, so the invalidation is too — the same ranges the
        // cascade just rewrote.
        for root in lumen_static::style::WidMatches::new(table, wid) {
            self.invalidation.mark(lumen_static::style::subtree_range(table, root));
        }
        true
    }

    /// A new viewport. Everything is laid out against it, so nothing
    /// retained from the previous size describes the next frame:
    /// retention is reset and the next frame is a full redraw.
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.size == (width, height) {
            return;
        }
        self.size = (width, height);
        self.retain.reset();
        self.last_damage = Damage::All;
        // Every laid-out position changed, so nothing kept from the
        // previous frame describes the next one — including the dirty
        // set, whose ranges are about a tree that no longer has those
        // sizes.
        self.invalidation.mark_all();
    }

    /// Queue one input. OS input, agent-synthesized input and test input
    /// all arrive here — the single queue that makes a session replay.
    ///
    /// **Moves merge and releases are kept** — M44/T44.5, through
    /// [`InputQueue::push_merged`](lumen_static::InputQueue::push_merged).
    /// This was a bare `push` with its refusal discarded, and a window
    /// posts every `CursorMoved`: more than 64 events between redraws
    /// lost the tail, and when the tail held the `PointerUp` the capture
    /// was stranded — a later move with no button held still dragged the
    /// slider (R3-F2). What is still refused is counted, and
    /// [`Host::dropped_inputs`] says how much.
    pub fn post(&mut self, ev: Event) {
        self.queue.push_merged(ev);
    }

    /// How many input events this host has ever lost for want of queue
    /// room — M44/T44.5. Non-zero means input was lost, not that the
    /// queue is full now; the agent reports it in `app.diagnostics`.
    pub fn dropped_inputs(&self) -> u32 {
        self.queue.dropped()
    }

    /// How many more events [`Host::post`] can take before a pump,
    /// counting none that would merge — what a caller feeding a burst
    /// (a paste, the agent's `input.type`) chunks against.
    pub fn input_room(&self) -> usize {
        self.queue.room()
    }

    pub fn frame_number(&self) -> u64 {
        self.frame
    }

    /// `(layout + place, paint)`, in nanoseconds.
    ///
    /// **The first element is both passes**, which is what every figure
    /// taken from this accessor has always meant. It is stated here
    /// because it is not what the name suggests, and a review read the
    /// grid probe's "layout" column as layout alone and compared it with
    /// a layout-only ratio from another milestone — AGENT.md's
    /// "bound computed from the wrong quantity", in a results table.
    /// [`Host::timings3`] separates them; prefer it for anything that
    /// attributes a cost to one pass.
    pub fn timings(&self) -> (u64, u64) {
        (self.last_layout_ns + self.last_place_ns, self.last_paint_ns)
    }

    /// `(layout, place, paint)`, in nanoseconds — the three passes apart.
    ///
    /// **Three of the pump's six stages, not the frame** (M53/T53.1): the
    /// input-and-bindings stage, the arena cascade and `finish` are not
    /// in it. Use [`Host::stage_timings`] for a frame's cost.
    ///
    /// **`place` is unconditional and no invalidation narrows it**: the
    /// derive's `place` takes no `Changed` and walks every node to give
    /// it an absolute position (`docs/results-damage-precision.md`
    /// T29.1). So a probe that narrows a mark and reports one combined
    /// number is reporting a floor it cannot move as if it were residue
    /// it had not yet removed.
    pub fn timings3(&self) -> (u64, u64, u64) {
        (self.last_layout_ns, self.last_place_ns, self.last_paint_ns)
    }

    /// How many times each pass has run since the host was built —
    /// M53/T53.0 (`docs/plan-frame-cost.md`, rule 3: counts gate, clocks
    /// record). Read it before and after, and subtract with
    /// [`frame_cost::PassCounts::since`].
    pub fn pass_counts(&self) -> frame_cost::PassCounts {
        self.passes
    }

    /// The last pump, stage by stage and whole — M53/T53.1.
    ///
    /// **Prefer this to [`Host::timings3`] for "what a frame costs".**
    /// `timings3` is three of the six stages; the arena cascade, the
    /// input-and-bindings stage and `finish` are outside it, and a stage
    /// sum published as the frame overstated a narrowed click's saving
    /// as a share three times (`docs/results-damage-precision.md`,
    /// T53.1). `pump` is measured around the whole call and
    /// [`frame_cost::StageTimes::gap`] is what no stage covers.
    pub fn stage_timings(&self) -> frame_cost::StageTimes {
        self.stages
    }

    /// Logical viewport: the physical surface divided by the scale.
    fn limits(&self) -> Constraints {
        Constraints {
            min_w: 0.0,
            max_w: self.size.0 as f32 / self.scale,
            min_h: 0.0,
            max_h: self.size.1 as f32 / self.scale,
        }
    }

    /// Physical pixels per logical pixel. 1.0 until a shell says
    /// otherwise.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Tell the host what the display's scale factor is — from
    /// `winit`'s `Window::scale_factor`, and again on
    /// `ScaleFactorChanged`.
    ///
    /// Resets retention: every laid-out position changes, so nothing
    /// kept from the previous frame describes the next one.
    pub fn set_scale(&mut self, scale: f32) {
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        if self.scale == scale {
            return;
        }
        self.scale = scale;
        self.retain.reset();
        self.last_damage = Damage::All;
        self.invalidation.mark_all();
    }

    /// Whether the last frame's drain hit its cap and more deferred
    /// results may be waiting — M28/T28.2.
    ///
    /// A shell that sees `true` asks for another frame. The alternative,
    /// raising the cap until the queue empties, is what turns a burst of
    /// completions into one long frame.
    pub fn more_deferred(&self) -> bool {
        self.more_deferred
    }

    /// Install the spawner deferred work runs on — M28/T28.3.
    ///
    /// A shell calls this once. [`Threads`](lumen_static::spawn::Threads)
    /// is the default a windowed shell installs; a test installs
    /// [`Immediate`](lumen_static::spawn::Immediate) or
    /// [`Queued`](lumen_static::spawn::Queued) to control *when*.
    pub fn set_spawner(&mut self, s: Box<dyn lumen_static::spawn::Spawner<A::Msg>>) {
        self.spawner = Some(s);
        for job in std::mem::take(&mut self.held) {
            self.spawn(job);
        }
    }

    /// Start one job. Its result comes back through the inbox and is
    /// applied by a later [`Host::pump`].
    ///
    /// Returns whether it started. **A spawn with no spawner installed
    /// is refused, not queued**: queueing it would mean work that runs
    /// at an unpredictable later moment — when a shell happens to
    /// install one — and a host that never installs one would
    /// accumulate jobs forever. The refusal is counted
    /// ([`Host::unspawned`]) rather than logged, because "how many did
    /// we drop" is a number.
    pub fn spawn(&mut self, job: lumen_static::spawn::Job<A::Msg>) -> bool {
        match self.spawner.as_mut() {
            Some(s) => {
                // **Listed before it can land** — M47/T47.3. The sender
                // is the job's own, so the drain can say which job each
                // result came from, and its drop says the body returned.
                let (tx, track) = self.inbox.tracked_sender();
                self.ledger.record(job.desc, track);
                s.spawn(job, tx);
                true
            }
            None => {
                // **The refusal is reported as a failure too**, not only
                // counted — M28/T28.9. A chain whose next step was never
                // spawned stalls exactly as one whose job panicked does,
                // and the application cannot tell the difference from
                // the outside unless it is told.
                self.unspawned += 1;
                if let Some(f) = job.on_fail {
                    if let Some(m) = f(job.desc, lumen_static::spawn::Failure::NotStarted) {
                        self.inbox.post(m);
                    }
                }
                false
            }
        }
    }

    /// Start what appeared, stop what vanished, leave the rest alone.
    ///
    /// Identity is `(kind, key)` — values — so the diff is a set
    /// difference and nothing hashes any code. A subscription already
    /// running is **left alone**: restarting it every frame is the
    /// defect this exists to prevent, and `sub_started` is how a test
    /// sees the difference.
    fn diff_subscriptions(&mut self) {
        // The application owns the set and mutates it when its state
        // changes, exactly as it owns the outbox. The host only reads —
        // which means there is no "re-declare" hook to forget to call,
        // and no frame in which the set is momentarily empty.
        let declared: Vec<(u64, u64)> = match self.app.subscriptions() {
            Some(s) if !s.as_slice().is_empty() || !self.running.is_empty() => {
                s.as_slice().to_vec()
            }
            // Nothing declared and nothing running: the overwhelmingly
            // common case, and it must cost a `None` check and no more.
            _ => return,
        };

        // Stopped: running and no longer declared.
        let stopped: Vec<(u64, u64)> =
            self.running.iter().copied().filter(|r| !declared.contains(r)).collect();
        for s in &stopped {
            self.running.retain(|r| r != s);
            self.sub_stopped += 1;
        }

        // Started: declared and not already running. A subscription is
        // spawned exactly like any other job — `docs/events-decision.md`
        // is absolute that the inbox is the only way deferred work
        // re-enters the app, and this plan adds no second path.
        for d in &declared {
            if self.running.contains(d) {
                continue;
            }
            self.running.push(*d);
            self.sub_started += 1;
            let desc = lumen_static::spawn::Descriptor { kind: d.0, step: 0, payload: d.1 };
            match A::job_for(desc) {
                Some(job) => {
                    self.spawn(lumen_static::spawn::Job {
                        desc,
                        job,
                        on_fail: Some(A::failed),
                    });
                }
                None => self.unknown_kind += 1,
            }
        }
    }

    /// Subscriptions started and stopped since this host was made.
    pub fn subscription_counts(&self) -> (usize, usize) {
        (self.sub_started, self.sub_stopped)
    }

    /// Hand every descriptor a handler pushed to the spawner, pairing
    /// each with the job `fn` from the **currently loaded** build.
    ///
    /// Copies out of the application's outbox before spawning, because
    /// `App::job_for` and the spawner both need the app borrow released
    /// — and because a job that pushed to the outbox while being spawned
    /// would otherwise extend this drain without bound.
    fn drain_outbox(&mut self) {
        let mut buf = [lumen_static::spawn::Descriptor::default();
            lumen_static::spawn::Outbox::CAPACITY];
        let n = match self.app.outbox() {
            Some(o) if !o.is_empty() => o.drain_into(&mut buf),
            _ => return,
        };
        for d in buf.iter().take(n) {
            match A::job_for(*d) {
                Some(job) => {
                    self.spawn(lumen_static::spawn::Job {
                        desc: *d,
                        job,
                        on_fail: Some(A::failed),
                    });
                }
                // **An unrecognised `(kind, step)` is dropped and
                // counted**, never guessed at — `docs/plan-tasks.md`
                // §2.1. Guessing is what makes a renumbering silently
                // run the wrong step.
                None => self.unknown_kind += 1,
            }
        }
    }

    /// Descriptors dropped because the loaded build does not recognise
    /// their `(kind, step)`.
    pub fn unknown_kind(&self) -> usize {
        self.unknown_kind
    }

    /// Run whatever the installed spawner is holding, and say how many.
    ///
    /// Only [`Queued`](lumen_static::spawn::Queued) holds anything; every
    /// other spawner answers zero. A test drives it through the host
    /// rather than keeping its own handle, because the spawner lives
    /// behind `dyn` here and a test that kept a handle would be
    /// asserting against a different object than the host uses.
    pub fn run_pending_jobs(&mut self) -> usize {
        self.spawner.as_mut().map_or(0, |s| s.run_pending())
    }

    /// How many spawned jobs are in flight — M47/T47.3: spawned, and not
    /// yet both finished and drained. **The ledger's length**, pruned
    /// first, so the answer is as of this call and not of the last frame.
    ///
    /// Jobs held for a spawner that is not installed yet are not in
    /// flight; [`Host::held_jobs`] counts those.
    pub fn in_flight(&mut self) -> usize {
        self.ledger.prune();
        self.ledger.len()
    }

    /// Subscriptions running now — what `ui.waitSettled` reports as
    /// `subscriptions` rather than counting against settling (M48/T48.2).
    pub fn subscriptions_running(&self) -> usize {
        self.running.len()
    }

    /// **What `ui.waitSettled` waits on** — M48/T48.2: [`Host::in_flight`]
    /// less the jobs of running subscriptions. From the one ledger
    /// (`docs/plan-agent-truth.md` rule 2): a subscription's job is listed
    /// for as long as it runs, which would keep a subscribed application
    /// from ever settling, so it is counted apart
    /// ([`Host::subscriptions_running`]). A subscription's job is its
    /// declared `(kind, key)` at step 0.
    pub fn in_flight_unsubscribed(&mut self) -> usize {
        self.ledger.prune();
        let running = &self.running;
        self.ledger.descriptors().filter(|d| !is_subscription_job(running, d)).count()
    }

    /// The ledger itself: what [`Host::in_flight`] counts. As of the last
    /// prune; [`Host::reissuable`] is what a reload re-issues.
    pub fn ledger(&self) -> &lumen_static::spawn::Ledger {
        &self.ledger
    }

    /// **What a hot swap hands the new build** — M49/T49.10 (R4-F3): every
    /// job in flight, oldest first, less the jobs of running
    /// subscriptions.
    ///
    /// In flight is the ledger's definition — spawned, and not yet both
    /// returned and drained ([`Ledger::unsettled`](lumen_static::spawn::Ledger::unsettled),
    /// read without a prune because a session saves through `&self`), so
    /// a result still sitting in this host's inbox is included: the inbox
    /// goes with this host, and the job must run again to be delivered.
    /// A subscription's job is left out for the reason
    /// [`Host::in_flight_unsubscribed`] leaves it out: the new build
    /// starts what its application declares, and re-issuing it too would
    /// run the stream twice.
    ///
    /// **Delivery is at least once** across the swap: the old build's
    /// worker is not stopped (a thread cannot be), so a job that was
    /// running still finishes there — into an inbox nobody drains — and
    /// runs again under the new build. A job with a side effect may
    /// perform it twice. `book/src/events.md` says so beside the chains.
    pub fn reissuable(&self) -> Vec<lumen_static::spawn::Descriptor> {
        let running = &self.running;
        self.ledger.unsettled().filter(|d| !is_subscription_job(running, d)).collect()
    }

    /// Spawn what an old build had in flight, through **this** build's
    /// `App::job_for` — M49/T49.10. The other half of
    /// [`Host::reissuable`], called by a hot library's boot after the
    /// restore.
    ///
    /// A descriptor already in flight here is **not spawned twice**: the
    /// new build's boot has already started its own boot jobs, and an old
    /// build's copy of the same job, still running at the swap, is that
    /// job. Matched one for one, so two identical requests from the old
    /// build and one boot job spawn one more. A descriptor this build has
    /// no job for is dropped and counted in [`Host::unknown_kind`], as
    /// `drain_outbox` drops one — never guessed at.
    pub fn reissue(&mut self, descs: &[lumen_static::spawn::Descriptor]) -> Reissued {
        self.ledger.prune();
        let mut here: Vec<lumen_static::spawn::Descriptor> = self.ledger.descriptors().collect();
        let mut out = Reissued::default();
        for d in descs {
            if let Some(i) = here.iter().position(|h| h == d) {
                here.swap_remove(i);
                out.already += 1;
                continue;
            }
            match A::job_for(*d) {
                Some(job) => {
                    if self.spawn(lumen_static::spawn::Job { desc: *d, job, on_fail: Some(A::failed) }) {
                        out.spawned += 1;
                    } else {
                        out.refused += 1;
                    }
                }
                None => {
                    self.unknown_kind += 1;
                    out.unknown += 1;
                }
            }
        }
        out
    }

    /// Timer firings that came due with no room in the outbox and were
    /// counted rather than pushed — M47/T47.2, R7-F4. Monotonic.
    ///
    /// Delivered plus this is every period that elapsed. Per timer, the
    /// same figure is `Timers::coalesced`.
    pub fn timer_coalesced(&self) -> u64 {
        self.timer_coalesced
    }

    /// How many spawns were refused for want of a spawner.
    pub fn unspawned(&self) -> usize {
        self.unspawned
    }

    /// Job failures no application message reported — M47/T47.0, R7-F9.
    ///
    /// `FailFn`'s `None` means "this application does not model that
    /// failure", and this is where such a drop is counted (M47/T47.6): a
    /// panicking job in an application that leaves `App::failed` at its
    /// default used to leave a trace on stderr only. Read from the inbox's
    /// shared counter, because the failure is seen on a worker.
    pub fn unmodelled_failures(&self) -> usize {
        self.inbox.unmodelled()
    }

    /// A sender for deferred work, already carrying this host's waker.
    ///
    /// The one way an application gets one, so a sender minted before
    /// the waker was installed — which `inbox.rs` documents and
    /// `tests/inbox_wake.rs` pins — cannot happen by accident here.
    pub fn sender(&self) -> lumen_static::inbox::Sender<A::Msg> {
        self.inbox.sender()
    }

    /// A physical point (what an OS reports) in the logical coordinates
    /// everything above the renderer uses.
    ///
    /// A shell must convert before `post`ing a pointer event, or clicks
    /// land at twice their coordinates on a 2x display.
    pub fn to_logical(&self, x: f32, y: f32) -> (f32, f32) {
        (x / self.scale, y / self.scale)
    }

    /// Dispatch one event now, outside the queue: what `pump_with` does
    /// with each event it drains, and what a paste that falls back to
    /// typing does with each character (M45/T45.7). Whether a node took
    /// it.
    fn dispatch_now(&mut self, ev: &Event, consumed: &mut pass::Consumed) -> bool {
        // `dispatch_overlaid`: an overlay takes pointer hits first,
        // Escape closes the topmost, and a press outside every open
        // one dismisses them and is consumed. `self.overlays` is
        // what the *last* paint deferred, which is deliberate — what
        // can be clicked is what was drawn.
        pass::dispatch_overlaid_into(
            &mut self.app,
            &self.styles,
            &mut self.states,
            &mut self.input,
            &self.overlays,
            ev,
            consumed,
        )
    }

    /// Mark what a handled event may have changed — the rule below, used
    /// by the pump and by a paste's typing fallback alike.
    fn mark_handled(&mut self, consumed: &pass::Consumed) {
        // **Narrowed for a split application** — M29/T29.6. Its
        // handlers cannot reach the tree (`App::STATE_IS_SPLIT`), so
        // what an event changed is the nodes that consumed it — marked
        // here — plus whatever the bindings wrote, which
        // `update_marked` marks below. An unsplit application's
        // handler may write any widget, so it keeps the whole tree.
        if A::STATE_IS_SPLIT && !consumed.overflowed() {
            let table = <A::Tree as Widget>::SUB_TABLE;
            for &n in consumed.nodes() {
                self.invalidation.mark(lumen_static::style::subtree_range(table, n as usize));
            }
        } else {
            self.invalidation.mark_all();
        }
        // Owed either way, and paid only inside the marks just made
        // (M53/T53.2): a consumed wheel marks its repeater, so the
        // repeater is asked; a click on a box marks the box, which owns
        // no arena, so nothing is cascaded.
        self.arenas_dirty = true;
    }

    /// One frame: drain input, lay out, paint. Returns whether any event
    /// was consumed, which a real shell uses to decide whether to
    /// redraw.
    ///
    /// **A pump with nothing to do does nothing** (M53/T53.3): under a
    /// strategy that can skip, a frame with nothing marked, handled,
    /// applied or owed keeps the list it has and reports
    /// [`Retention::still`]'s
    /// damage — `None` for a tracking strategy. So a write to the tree
    /// must be marked (`Host::invalidate`) to be drawn at all.
    pub fn pump(&mut self) -> bool {
        self.pump_with(&mut BlockMetrics)
    }

    /// Same, with a caller-supplied text stack.
    pub fn pump_with(&mut self, metrics: &mut dyn TextMeasure) -> bool {
        let t_pump = std::time::Instant::now();
        let mut handled = false;
        let mut consumed = pass::Consumed::default();
        let before = PointerFocus::of(&self.input);
        // **Focus back from an overlay that closed without input** —
        // M44/T44.9. A `Veil` is closed by its application, after the
        // Escape that asked; the frame that dropped it from the overlay
        // list asked for this one (below, after paint), and here the
        // focus it opened over comes back — inside `before`/`after`, so
        // T25.0's marks redraw both nodes.
        pass::settle_overlays(&self.app, &self.styles, &mut self.states, &mut self.input, &self.overlays);
        while let Some(ev) = self.queue.pop() {
            handled |= self.dispatch_now(&ev, &mut consumed);
            // **The primary touch drives the pointer** — M44/T44.4
            // (`docs/plan-input-semantics.md` §2.4). The raw `Touch` went
            // first, so a recogniser in `lumen_static::gesture` still sees
            // every finger; then the first finger down, until it lifts,
            // is also a mouse. Here in the intake rather than in the
            // winit translation, so an embedded touch panel that feeds
            // `Event::Touch` into a `Host` gets the same behaviour.
            for synth in promote_touch(&mut self.primary_touch, &ev).into_iter().flatten() {
                handled |= self.dispatch_now(&synth, &mut consumed);
            }
        }

        // **A cancel the application asked for** — M28/T28.12.
        //
        // Before the drains, so a result that arrived this frame from a
        // task the application has just cancelled is dropped rather
        // than applied. After them it would be applied and *then*
        // cancelled, which is the ordering that makes a cancel look
        // unreliable under load.
        if self.app.take_cancel() {
            self.inbox.cancel();
        }

        // **Subscriptions, diffed** — M28/T28.10.
        //
        // Before the outbox drain, so a subscription that starts this
        // frame is spawned by the same drain that handles a handler's
        // request — one spawn path, not two.
        self.diff_subscriptions();

        // **The outbox, drained after dispatch** — M28/T28.4.
        //
        // After, not before, and that ordering is the whole claim: a
        // descriptor a handler pushed *this* frame is spawned on this
        // frame. Drained before dispatch, every request would wait for
        // the next event to arrive — a click's work starting one frame
        // late, and only under load would anyone notice.
        //
        // Input has a queue into the frame; this is the queue out of it.
        self.drain_outbox();

        // **A handler that ran invalidates everything** (T16.7).
        //
        // Conservative on purpose, and the reason is in the signature: a
        // handler is `fn(&mut App, &EventCtx)`, so it may touch any
        // field of the application, and narrowing the mark to the node
        // that was clicked would be narrowing it to where the *event*
        // landed rather than to where the *change* did. Those are
        // different nodes as soon as a button edits a label elsewhere,
        // which is the ordinary case.
        //
        // So this degrades to today's whole-tree walk rather than to a
        // wrong frame. The precise entry point is `#[bind]`, which knows
        // statically which tree field it writes; narrowing beyond that
        // is OPEN (`docs/plan-invalidation.md` T16.7).
        if handled {
            self.mark_handled(&consumed);
        }

        // **State no handler reports** (M25/T25.0).
        //
        // `dispatch` writes `hovered`, `pressed`, `focused` and
        // `focus_visible` whether or not anything handled the event, and
        // the paint attributes that depend on them — the hover and press
        // shade, the focus ring — are resolved in `layout`. Until T25.0
        // only the two marks above existed, so under `Boundaries` a
        // hover or a press skipped the node and painted last frame's
        // shade; `Everything` lays out every node anyway and hid it.
        //
        // Marked precisely, old node and new, because unlike a handler
        // the blast radius here *is* known: a pointer-state bit changes
        // what exactly one node paints.
        let after = PointerFocus::of(&self.input);
        if after != before {
            let table = <A::Tree as Widget>::SUB_TABLE;
            let inv = &mut self.invalidation;
            if after.hovered != before.hovered {
                mark_node(inv, table, before.hovered);
                mark_node(inv, table, after.hovered);
            }
            if after.pressed != before.pressed {
                mark_node(inv, table, before.pressed);
                mark_node(inv, table, after.pressed);
            }
            if after.focus != before.focus || after.focus_visible != before.focus_visible {
                mark_node(inv, table, before.focus);
                mark_node(inv, table, after.focus);
            }
        }

        // Derived presentation is refreshed here, between input and
        // layout: a handler has just changed state, and layout is about
        // to measure what that state renders as. Doing it in the loop
        // rather than at every call site is what makes a binding
        // impossible to forget.
        // A binding that wrote invalidates everything (T16.10).
        //
        // Conservative in *where*, precise in *whether*: the derive
        // knows which tree field each binding targets but not which node
        // index that is, so the mark cannot be narrowed here — while
        // `format_into` compares before it writes, so a frame in which
        // no binding changed anything is known to be still, and that is
        // the case worth catching. An idle window, an animation between
        // ticks and a repeat key that changed nothing all land here.
        // **Deferred results, before `update`** — M28/T28.2.
        //
        // Before rather than after, so a message and the bindings that
        // present it settle in one frame: `apply` writes state, and the
        // `update` below is what refreshes the presentation copies from
        // it. Reversed, every deferred result would appear one frame
        // late.
        //
        // `apply` reports whether it changed anything, for the same
        // reason `update` does — under `Boundaries` a `false` is the
        // difference between skipping the tree and walking it. An
        // application that defers nothing has `Msg = Never`, and this is
        // a `try_recv` on a queue nothing can write to.
        let mut applied_changed = false;
        let applied = self.inbox.drain_at_most(
            lumen_static::inbox::Inbox::<A::Msg>::PER_FRAME,
            &mut (&mut self.app, &mut applied_changed),
            |(app, changed), m| {
                // The state, not the application (M33/T33.2): a message is
                // handler-shaped work and gets what a handler gets.
                if A::apply(app.state_mut(), m) {
                    **changed = true;
                }
            },
        );
        if applied_changed {
            // A split application's `apply` received only the state
            // (M33/T33.2), so the bindings will say where it landed —
            // M29/T29.6. An unsplit one may have written anywhere.
            if !A::STATE_IS_SPLIT {
                self.invalidation.mark_all();
            }
            self.arenas_dirty = true;
        }
        // A full drain may have left more behind; ask for another frame
        // rather than raising the cap, which is what keeps a burst from
        // becoming one long frame.
        self.more_deferred = applied == lumen_static::inbox::Inbox::<A::Msg>::PER_FRAME;
        // What landed leaves the ledger (M47/T47.3). An empty ledger — every
        // application that spawns nothing — is a length check.
        if !self.ledger.is_empty() {
            self.ledger.prune();
        }

        // **What the bindings wrote, and where** — M29/T29.3. One range
        // per binding that wrote; `Everything` for a `with =` hook, an
        // `after_update` that changed something, or a hand-written
        // `update` that cannot say. The arena flag is kept on any write:
        // §2.4 asks a narrowed mark to show that a binding cannot change
        // a template's styles, and nothing here shows it. Since
        // M53/T53.2 it is paid only over the ranges the binding marked,
        // which is where a binding that swapped an arena in wrote.
        let inv = &mut self.invalidation;
        match self.app.update_marked(&mut |r| inv.mark(r)) {
            lumen_static::event::Wrote::Nothing => {}
            lumen_static::event::Wrote::Marked => self.arenas_dirty = true,
            lumen_static::event::Wrote::Everything => {
                self.invalidation.mark_all();
                self.arenas_dirty = true;
            }
        }
        // **An idle pump does nothing** — M53/T53.3 (R6-F1,
        // `docs/plan-frame-cost.md` §2.3(a)). Nothing marked, nothing
        // handled, no pointer or focus state moved, nothing applied, no
        // binding wrote and no arena owed: every one of those marks the
        // tree when it happens, and so do a resize, a scale or palette
        // change, a new sheet, `adopt` and a caught panic — and a fresh
        // host starts with everything marked. So this frame would lay
        // out nothing, and paint exactly the list already held. Keep the
        // list, report no damage, and skip place, paint and `finish`,
        // which ran on every pump until T53.3 whatever `Boundaries`
        // skipped. **Only under a strategy that can skip**: `Everything`
        // is the diagnostic that always walks (M34).
        //
        // The risk is paint that changes without a mark: until T53.3 a
        // pump always painted, and hid it. The shadow oracle judges a
        // skipped frame like any other (below), so it is what finds one.
        if I::CAN_SKIP
            && !handled
            && after == before
            && !applied_changed
            && !self.arenas_dirty
            && self.invalidation.marks() == lumen_static::invalidate::Marks::default()
        {
            self.stages.input = t_pump.elapsed().as_nanos() as u64;
            self.last_damage = self.retain.still();
            self.last_marks = lumen_static::invalidate::Marks::default();
            self.last_arenas_owed = false;
            (self.last_layout_ns, self.last_place_ns, self.last_paint_ns) = (0, 0, 0);
            self.passes.skipped += 1;
            // Judged like a painted frame: the list held is what is on
            // screen, and the oracle's whole-tree frame must still equal
            // it. Without this a skipped frame is the one frame nothing
            // checks (§5's condition for shipping this at all).
            #[cfg(feature = "shadow-oracle")]
            {
                let limits = self.limits();
                self.shadow_check(metrics, limits);
                self.shadow.skipped += 1;
            }
            self.frame += 1;
            self.passes.pumps += 1;
            let pump = t_pump.elapsed().as_nanos() as u64;
            self.stages = frame_cost::StageTimes { input: self.stages.input, pump, ..Default::default() };
            return handled;
        }

        // **The arenas, if any are owed.** Before layout, so a template
        // styled by the previous frame's pass is laid out with those
        // styles rather than one frame behind them.
        //
        // **Only the marked nodes are asked** — M53/T53.2 (R6-F3). An
        // arena is owed the cascade only when it is new, a new arena is
        // a write, and a write this frame knows about is in the marks
        // (`style::cascade_arenas_marked` has the argument). So a click
        // on a box that owns no arena walks the box's range and cascades
        // nothing, where it walked every node in the table until T53.2;
        // a frame that marked everything walks everything, as before.
        self.stages.input = t_pump.elapsed().as_nanos() as u64;
        let t_cascade = std::time::Instant::now();
        self.last_arenas_owed = self.arenas_dirty;
        if self.arenas_dirty {
            self.arenas_dirty = false;
            if !self.sheet.is_empty() {
                let styled = lumen_static::style::cascade_arenas_marked(
                    self.app.tree_mut(),
                    &self.sheet,
                    &self.palette,
                    self.invalidation.view(),
                );
                if styled > 0 {
                    self.passes.cascades += 1;
                }
            }
        }

        self.stages.cascade = t_cascade.elapsed().as_nanos() as u64;
        let limits = self.limits();
        let t0 = std::time::Instant::now();
        {
            // Disjoint field borrows: the view borrows `invalidation`,
            // the palette `palette`, the tree borrows `app`.
            let mut cx = LayoutCx::with_changed(metrics, self.invalidation.view())
                .with_palette(&self.palette)
                // M26/T26.17 Phase D #12: the measurer's answer depends
                // on the scale, so the memo's key must carry it. Without
                // this line a `ScaleFactorChanged` that holds the logical
                // size constant — the ordinary monitor-to-monitor drag —
                // leaves every text box measured for the old display.
                // `tests/scale_memo.rs` is this line's gate (M40/T40.0):
                // `BlockMetrics` is linear in the scale, so no other host
                // test can tell whether the scale arrived.
                .with_scale(self.scale);
            let tree = self.app.tree_mut();
            self.passes.layouts += 1;
            tree.layout(&mut cx, limits, &self.styles, &mut self.states, 0);
            self.last_layout_ns = t0.elapsed().as_nanos() as u64;
            // Timed apart from layout because **invalidation cannot
            // narrow it**: `place` takes no `Changed`, so it is the same
            // whole-tree walk however clean the frame is. Folded into
            // one number it reads as layout that a better mark would
            // remove (M29/T29.1's review).
            let t1 = std::time::Instant::now();
            self.passes.places += 1;
            tree.place(Point::default(), &self.styles, &mut self.states, 0);
            self.last_place_ns = t1.elapsed().as_nanos() as u64;
            // M55/T55.5's net: a length that did not come through the
            // cascade's repair stops a debug build here, by node.
            lumen_static::pass::debug_assert_finite(&self.states);
        }

        // A focus ring only for keyboard focus (T15.6). **After**
        // layout, which resolves the ring from the stylesheet and would
        // otherwise overwrite this — the first version zeroed it before
        // and had no effect at all.
        //
        // One write per frame rather than a per-node flag: there is one
        // focus, and `NodeState` would have grown from 76 bytes to 80
        // for every node of every tree to say something about one of
        // them, which the M15 gating policy forbids.
        if !self.input.focus_visible {
            if let Some(f) = self.input.focus {
                if let Some(s) = self.states.get_mut(f as usize) {
                    s.focus_ring = 0;
                }
            }
        }

        let t1 = std::time::Instant::now();
        self.list.clear();
        // Tell paint where the frame is going, so a container can cull
        // children that cannot be seen (T16.12). **Physical** units —
        // `self.size` is the surface's — and a container still culls in
        // its own logical space, because `Scaled::clip_bounds` divides
        // this back by the scale on the way out.
        self.list.set_bounds(Some([0.0, 0.0, self.size.0 as f32, self.size.1 as f32]));
        {
            // Logical geometry in, physical commands out (M10b), with
            // retention observing the stream (T13.8) **after** `Scaled`
            // has multiplied it: `paint → Scaled(Recording(list))`.
            //
            // So every region in `Damage` is in physical pixels, the
            // space both surfaces draw in. The other order — `Recording`
            // outside `Scaled` — hashed and bounded each command before
            // it was scaled, and at any scale but 1.0 a partial frame
            // redrew a logical-sized rect at the logical position and
            // left the physical one stale (R1-F1, M40/T40.1;
            // `tests/damage_scale.rs` is the gate).
            //
            // At a scale of 1.0 `Scaled` is a forwarding wrapper, and
            // under `Rebuild` `Recording` forwards to an empty
            // `observe` the optimizer removes.
            let mut rec = lumen_static::Recording::new(&mut self.list, &mut self.retain);
            let mut out = lumen_static::Scaled::new(&mut rec, self.scale);
            self.passes.paints += 1;
            pass::paint_overlaid(self.app.tree(), &self.states, &mut out, &mut self.overlays);
        }
        self.last_paint_ns = t1.elapsed().as_nanos() as u64;

        // What this frame changed. Retention has already seen every
        // command; this closes the frame. Free under `Rebuild`.
        let t_finish = std::time::Instant::now();
        self.last_damage = self.retain.finish();

        // **The shadow oracle** — M34/T34.1, moved here by M41/T41.2.
        // Only a strategy that can skip has anything to check. **After
        // paint and `finish`**: its second layout runs on the live
        // widgets, and before paint it repaired the frame it judged. It
        // applies the focus-ring rule above to its scratch too: the rule
        // rewrites a field after layout, so a node the bounded frame
        // skipped keeps last frame's zero while a re-layout restores the
        // sheet's value — equal pixels, unequal state. Compared before
        // the rule, the first live run reported exactly that as a
        // divergence (showcase, node 5, "shape").
        #[cfg(feature = "shadow-oracle")]
        if I::CAN_SKIP {
            self.shadow_check(metrics, limits);
        }

        // The set describes what changed *since the last frame*, so it
        // is spent once layout has read it. Marks arriving after this
        // point belong to the next frame, which is what makes
        // `invalidate` callable from anywhere between pumps.
        self.last_marks = self.invalidation.marks();
        self.invalidation.clear();

        // **An overlay opened or closed this frame** — M44/T44.9/T44.10.
        // Settled now, after the clear, so the focus it records or hands
        // back is right the moment this pump returns (an agent asking
        // "where is focus" after the click that closed a menu gets the
        // trigger, not the dead list); and the nodes it moved between are
        // marked for the next frame, which it asks for, so their rings
        // are redrawn under `Boundaries`.
        if pass::overlays_unsettled(&self.input, &self.overlays) {
            let was = self.input.focus;
            if pass::settle_overlays(&self.app, &self.styles, &mut self.states, &mut self.input, &self.overlays) {
                let table = <A::Tree as Widget>::SUB_TABLE;
                mark_node(&mut self.invalidation, table, was);
                mark_node(&mut self.invalidation, table, self.input.focus);
            }
            self.more_deferred = true;
        }

        self.frame += 1;
        self.passes.pumps += 1;
        self.stages.finish = t_finish.elapsed().as_nanos() as u64;
        self.stages.layout = self.last_layout_ns;
        self.stages.place = self.last_place_ns;
        self.stages.paint = self.last_paint_ns;
        self.stages.pump = t_pump.elapsed().as_nanos() as u64;
        handled
    }

    /// Advance animations by `dt` milliseconds; returns whether
    /// anything moved (T15.4).
    ///
    /// A host calls this once a frame with the interval it measured,
    /// and asks for another frame while it keeps returning `true`.
    /// Costs nothing for an application that animates nothing: the
    /// default `App::tick` returns `false` and inlines away.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        // **Timers first** — M28/T28.11. Each one that came due pushes a
        // descriptor into the outbox, so a tick is spawned by exactly
        // the path a handler's request is: **no timer thread** exists,
        // and each firing is one job on the installed spawner. Under the
        // window's `Threads` that was one OS thread per firing until
        // M47/T47.3 (R7-F5); it is a job on a pool of at most
        // `Threads::cap` long-lived workers now. `docs/plan-tasks.md`
        // §2.4's rule that the inbox is the only way deferred work
        // re-enters the app is why there is no second path here either.
        //
        // **Straight into the application's outbox** (M47/T47.2, R7-F4).
        // `advance` pushes only as many as the outbox has room for and
        // counts the rest, so nothing it fires can be refused. It went
        // through a scratch outbox here, whose refusals were dropped: a
        // 10 ms timer handed `tick(1000)` fired 100, delivered 32 and
        // counted 68 nowhere. The timers are taken out for the call,
        // because the two borrows of `self.app` cannot overlap.
        let fired = match self.app.timers() {
            Some(t) if !t.is_empty() => {
                let mut timers = core::mem::take(t);
                let done = match self.app.outbox() {
                    Some(o) => timers.advance(dt_ms, o),
                    // Timers and no outbox: nothing could ever spawn what
                    // they fire, so every firing is coalesced, and counted.
                    None => {
                        let mut nowhere = lumen_static::spawn::Outbox::new();
                        let d = timers.advance(dt_ms, &mut nowhere);
                        lumen_static::spawn::Advanced { delivered: 0, coalesced: d.fired() }
                    }
                };
                if let Some(t) = self.app.timers() {
                    *t = timers;
                }
                self.timer_coalesced += done.coalesced;
                done.fired()
            }
            _ => 0,
        };
        // **A tick that advanced and changed something marks the tree** —
        // M34/T34.0. `App::tick` takes `&mut self` and may write any
        // widget (showcase's writes the readout and reseeds the stage),
        // and it reports a `bool`, not ranges. Until this, nothing marked
        // it, so under `Boundaries` a tick-written label kept its old box
        // (`tests/tick_marks.rs`). A zero `dt` is a pure query by
        // `App::tick`'s contract and changes nothing, so it marks nothing.
        let ticked = self.app.tick(dt_ms);
        if ticked && dt_ms > 0.0 {
            self.invalidation.mark_all();
            self.arenas_dirty = true;
        }
        ticked || fired > 0
    }

    /// How long until this host's soonest timer is due, in milliseconds
    /// — M28/T28.11.
    ///
    /// **This is what a windowed loop waits on.** `ControlFlow::Wait`
    /// sleeps until input arrives, and an idle window produces no
    /// frames, so a deadline with nothing to check it on never fires.
    /// A shell reads this and waits *until* that instant instead;
    /// `None` means nothing is pending and the loop may wait
    /// indefinitely, which is today's behaviour.
    ///
    /// **Waiting is half of it** (M47/T47.1, R7-F1): the timers are
    /// checked by [`Host::tick`], so a shell that wakes at the instant
    /// must also draw a frame there, or the deadline passes unchecked.
    /// The figure is measured from the last tick, so the instant is that
    /// frame's start plus this, not "now" plus this.
    pub fn next_deadline_ms(&mut self) -> Option<f32> {
        self.app.timers().and_then(|t| t.earliest())
    }

    /// Route one composition step to the focused node — T14.8.
    ///
    /// Returns whether anything changed, so a host knows whether to
    /// redraw. `false` when nothing is focused or the focused thing does
    /// not edit text, which are the same answer from a caller's side.
    ///
    /// **A change marks the focused node's subtree** — M41/T41.1
    /// (`docs/plan-boundaries-repair.md` §1.1, R1-F2). The window routes
    /// `Ime::Preedit`, `Ime::Commit` and `Ime::Disabled` here and then
    /// pumps an empty queue, so none of the pump's own mark sites sees
    /// the edit. Until this mark, `Boundaries` skipped the field's
    /// `layout` — its scroll, its box and its clusters — and painted the
    /// commit against last frame's; `Everything` hid it. The same
    /// precise mark the pointer-state block uses, plus the arena flag the
    /// consumed path sets. `false` marks nothing.
    ///
    /// Not an `Event`: a composition step carries a string, and `Event`
    /// is `Copy` and 16 bytes. Same reason `paste` is a method.
    pub fn compose(&mut self, c: lumen_static::event::Composition<'_>) -> bool {
        let Some(i) = self.input.focus.map(|f| f as usize) else { return false };
        let table = <A::Tree as lumen_static::Widget>::SUB_TABLE;
        if i >= table.len() {
            return false;
        }
        let changed = self.app.tree_mut().node_mut(i).compose(c);
        if changed {
            mark_node(&mut self.invalidation, table, Some(i as u32));
            self.arenas_dirty = true;
        }
        changed
    }

    /// The pointer shape for whatever the pointer is currently over —
    /// `docs/plan-desktop.md` T14.6.
    ///
    /// Read from the *resolved style*, not from `NodeState`: `Style` had
    /// room for a `Cursor` in its tail padding and `NodeState` did not,
    /// and `NodeState` is asserted at 76 B by `fixtures/embedded`
    /// against the RAM budget in `docs/results-embedded.md`. The host
    /// has both arrays, so nothing needed to grow.
    ///
    /// The node the last pointer move **hit** wins, which is the deepest
    /// one — an ancestor's cursor must not override the leaf the pointer
    /// is actually on.
    ///
    /// **Read from the hit, not by scanning the hovered bits** — M44/T44.8.
    /// The scan walked the main table only, so a `Button` inside a
    /// `Repeat` row read as the repeater's own cursor, `Default` — the
    /// case T14.6 names (R15-F7). Dispatch records the hit's row part in
    /// `InputState::hover_inner`, and a virtualizer answers for its rows
    /// through `Node::inner_cursor`. It is also O(1) now, and the window
    /// calls this per event.
    pub fn cursor(&self) -> lumen_static::style::Cursor {
        let Some(i) = self.input.hovered.map(|i| i as usize) else {
            return lumen_static::style::Cursor::Default;
        };
        if i >= self.styles.len() {
            return lumen_static::style::Cursor::Default;
        }
        if let Some(inner) = self.input.hover_inner {
            if let Some(c) = self.app.tree().node(i).inner_cursor(inner) {
                return c;
            }
        }
        self.styles[i].cursor
    }

    /// The focused node's selected text — what a copy sends to the
    /// clipboard (`docs/plan-desktop.md` T14.5).
    ///
    /// Borrowed, so this allocates nothing. `None` when nothing is
    /// focused, or the focused thing has no selection: there is nothing
    /// to copy either way, and a caller does not need to tell them
    /// apart.
    ///
    /// The *clipboard* is the windowing layer's business. A clipboard is
    /// a platform, and neither this crate nor the core has one.
    pub fn copy(&self) -> Option<&str> {
        let i = self.input.focus? as usize;
        let table = <A::Tree as lumen_static::Widget>::SUB_TABLE;
        (i < table.len()).then_some(())?;
        self.app.tree().node(i).selection()
    }

    /// Paste `text` into the focused node **as one edit** — M45/T45.7.
    ///
    /// The focused node's [`lumen_static::Node::paste`] takes
    /// the whole string and answers how many characters it **kept**: a
    /// `TextArea` normalises line endings and keeps them, a `TextInput`
    /// strips them, a `NumberInput` keeps what its number rule allows.
    /// That count is what this returns. A node with no paste of its own
    /// gets the string as typing instead, one event per character with
    /// each line ending (`\r\n`, `\r`, `\n`) as `Key::Enter` and other
    /// control characters dropped, dispatched straight away rather than
    /// queued; the count is then the events a node took. Nothing focused
    /// keeps nothing, and answers 0.
    ///
    /// **One pump, whatever the length.** This used to post one
    /// `Event::Text` per character and pump every 64 of them, each pump
    /// linear in the text — a 256 KB paste was 4,096 frames and quadratic
    /// (R8-F5) — and a `TextArea`, which takes a newline only as Enter,
    /// dropped every pasted line break while the count said it was
    /// delivered (R8-F4, R10-F5; `docs/results-text.md` T45.0 item 7).
    ///
    /// **The paste is marked**, the focused node's subtree, as
    /// [`Host::compose`] marks: the edit happens outside dispatch, so no
    /// pump mark site sees it. It is still an edit of the node that owns
    /// it, by the method typing uses, so a selection is replaced and the
    /// caret moves exactly as typing would.
    ///
    /// **Pumps with the `BlockMetrics` stand-in**, so it is for a headless
    /// caller whose frames use the stand-in too. A caller that lays out
    /// with a real text stack uses [`Host::paste_with`] and passes it —
    /// a window's session does.
    pub fn paste(&mut self, text: &str) -> usize {
        self.paste_with(text, &mut BlockMetrics)
    }

    /// [`Host::paste`], pumping with the caller's text stack — M41/T41.5.
    ///
    /// The one pump a paste makes lays the field out, and it must measure
    /// with the stack every other frame uses. `paste` pumped with
    /// `BlockMetrics` while the window that called it owned the real
    /// measurer, and the next real frame was served the stand-in's box
    /// and clusters: eight pasted `m`s 80 px wide against 131.2 typed,
    /// and a click after a paste landing on the wrong byte
    /// (`tests/paste_measurer.rs`; R2-F1, R10-F4). And since M45/T45.2
    /// the memo keys on the measurer as well (`TextMeasure::generation`),
    /// so a stand-in's box is not served to a real frame even from a
    /// caller that pumps with the wrong one.
    pub fn paste_with(&mut self, text: &str, metrics: &mut dyn TextMeasure) -> usize {
        let table = <A::Tree as lumen_static::Widget>::SUB_TABLE;
        let focus = self.input.focus.map(|f| f as usize).filter(|&i| i < table.len());
        let hooked = focus.and_then(|i| self.app.tree_mut().node_mut(i).paste(text).map(|n| (i, n)));
        let kept = match hooked {
            Some((i, n)) => {
                if n > 0 {
                    mark_node(&mut self.invalidation, table, Some(i as u32));
                    self.arenas_dirty = true;
                }
                n
            }
            None => self.paste_as_typing(text),
        };
        self.pump_with(metrics);
        kept
    }

    /// A paste into a node with no paste of its own: typing, dispatched
    /// now rather than queued, so no pump happens mid-paste.
    fn paste_as_typing(&mut self, text: &str) -> usize {
        let mut consumed = pass::Consumed::default();
        let (mut handled, mut kept) = (false, 0);
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let ev = match c {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    Event::Key { key: lumen_static::Key::Enter, down: true }
                }
                '\n' => Event::Key { key: lumen_static::Key::Enter, down: true },
                c if c.is_control() => continue,
                ch => Event::Text { ch },
            };
            let took = self.dispatch_now(&ev, &mut consumed);
            handled |= took;
            kept += took as usize;
        }
        if handled {
            self.mark_handled(&consumed);
        }
        kept
    }

    /// What the frame just pumped changed, against the frame before it,
    /// in **physical** pixels — the surface's space, not layout's.
    /// Retention observes the paint stream after `Scaled` has multiplied
    /// it (M40/T40.1), so a region here is what a rasteriser must redraw
    /// as it stands; nothing downstream converts it.
    pub fn damage(&self) -> Damage {
        self.last_damage
    }

    /// What a surface buffer `age` frames old needs redrawn, and bank
    /// the accumulated damage as presented. In **physical** pixels, like
    /// [`Host::damage`].
    ///
    /// The question a shell presenting into a multi-buffered surface
    /// must ask instead of [`Host::damage`]: `age` comes from the
    /// surface — softbuffer reports it per buffer — 0 meaning its contents are
    /// unspecified. Call it once per presented frame — it counts
    /// presents, and a shell that pumps twice for one frame (a
    /// stylesheet swap between passes) must not call it twice.
    pub fn take_damage_for_age(&mut self, age: u8) -> Damage {
        self.retain.take_damage_for_age(age)
    }

    /// Hand the frame to a renderer. The renderer is chosen by the
    /// caller and never appears in a widget signature.
    ///
    /// The list is already in physical pixels — `pump` painted it
    /// through `Scaled` — so a renderer needs to know nothing about the
    /// scale factor.
    pub fn render(&self, r: &mut dyn Renderer) {
        r.render(&self.list, self.size.0, self.size.1);
    }

    /// Convenience: click at a point, as two ordinary queued events.
    pub fn click(&mut self, x: f32, y: f32) {
        self.post(Event::down(x, y));
        self.post(Event::up(x, y));
    }

    /// The node under a point, for tests and tooling.
    pub fn probe(&self, x: f32, y: f32) -> Option<lumen_static::Hit> {
        pass::hit_test(self.app.tree(), &self.states, x, y)
    }
}

/// The pointer events the **primary touch** stands for — M44/T44.4.
///
/// `Begin` of a finger when none is primary makes it primary and is a
/// move and a left press at its point; the primary's `Move` is a move,
/// its `End` a release and its `Cancel` a `PointerCancel`, and either of
/// the last two ends it. Every other finger, and every other event,
/// stands for nothing. On X11 winit also synthesises a `CursorMoved` for
/// the first touch; that arrives as one more move and never as a second
/// press (`tests/touch_as_pointer.rs`).
/// Whether `d` is the job of a running subscription: its declared
/// `(kind, key)` at step 0 (M48/T48.2). One predicate for the two readers
/// that leave such jobs out — settling, and a swap's re-issue.
fn is_subscription_job(running: &[(u64, u64)], d: &lumen_static::spawn::Descriptor) -> bool {
    d.step == 0 && running.contains(&(d.kind, d.payload))
}

/// What [`Host::reissue`] did with an old build's work in flight —
/// M49/T49.10. Plain counts, so it can cross a library seam as integers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reissued {
    /// Spawned under this build's `App::job_for`.
    pub spawned: usize,
    /// Already in flight here (a boot job this build started itself), so
    /// not spawned again.
    pub already: usize,
    /// This build has no job for its `(kind, step)`: dropped, and counted
    /// in [`Host::unknown_kind`] too.
    pub unknown: usize,
    /// Refused for want of a spawner ([`Host::unspawned`]).
    pub refused: usize,
}

fn promote_touch(primary: &mut Option<u32>, ev: &Event) -> [Option<Event>; 2] {
    use lumen_static::event::TouchPhase;
    let Event::Touch { id, x, y, phase } = *ev else { return [None, None] };
    match phase {
        TouchPhase::Begin if primary.is_none() => {
            *primary = Some(id);
            [Some(Event::PointerMove { x, y }), Some(Event::down(x, y))]
        }
        TouchPhase::Move if *primary == Some(id) => [Some(Event::PointerMove { x, y }), None],
        TouchPhase::End if *primary == Some(id) => {
            *primary = None;
            [Some(Event::up(x, y)), None]
        }
        TouchPhase::Cancel if *primary == Some(id) => {
            *primary = None;
            [Some(Event::PointerCancel { x, y }), None]
        }
        _ => [None, None],
    }
}

/// The input state whose changes `pump` marks without a handler having
/// run — M25/T25.0.
#[derive(Clone, Copy, PartialEq)]
struct PointerFocus {
    hovered: Option<u32>,
    pressed: Option<u32>,
    focus: Option<u32>,
    focus_visible: bool,
}

impl PointerFocus {
    fn of(input: &lumen_static::event::InputState) -> Self {
        PointerFocus {
            hovered: input.hovered,
            pressed: input.pressed,
            focus: input.focus,
            focus_visible: input.focus_visible,
        }
    }
}

/// Mark the subtree rooted at node `i`, if there is one and it is still
/// in the table — a hot reload can shrink the tree under an index.
fn mark_node<I: Invalidation>(inv: &mut I, table: &[lumen_static::desc::NodeDesc], i: Option<u32>) {
    if let Some(i) = i {
        let i = i as usize;
        if i < table.len() {
            inv.mark(lumen_static::style::subtree_range(table, i));
        }
    }
}

/// Background colour a shell clears to before rendering.
pub const CLEAR: Color = Color(255, 255, 255, 255);

/// Everything a command list needs to be drawn, for renderers that want
/// to walk it without matching on `Cmd` themselves.
/// The features this shell was compiled with that change what a live
/// drive observes — M42/T42.1, `process.features` in `app.diagnostics`.
///
/// `agent` (the lint, and with `live-agent` the endpoint), `everything`,
/// `shadow-oracle` and `reload`. A drive leg declares which of these it
/// expects, and `scripts/live/launch.py` refuses a process that reports
/// otherwise: `shadow-oracle` and `reload` decide whether `app.shadow` and
/// `app.failure` can say anything at all, and "cannot say" must not be
/// read as "no" (`docs/plan-drive-identity.md` §0).
pub fn process_features() -> Vec<&'static str> {
    let mut f = Vec::new();
    for (on, name) in [
        (cfg!(feature = "agent"), "agent"),
        (cfg!(feature = "everything"), "everything"),
        (cfg!(feature = "shadow-oracle"), "shadow-oracle"),
        (cfg!(feature = "reload"), "reload"),
    ] {
        if on {
            f.push(name);
        }
    }
    f
}

pub fn for_each_rect(list: &DisplayList, mut f: impl FnMut([f32; 4], Color)) {
    for c in &list.cmds {
        if let Cmd::Rect { rect, color } = c {
            f(*rect, *color);
        }
    }
}
