//! The host: the frame loop, a software renderer, and the surface an
//! agent drives — plan T5.3.
//!
//! This is everything between an application and the operating system
//! *except* the OS binding itself. A winit front end adds one thing:
//! translating window events into [`Host::post`] and blitting
//! [`Canvas::pixels`](software::Canvas). Keeping that seam explicit is what lets the loop be
//! tested headlessly, which is how every other subsystem here got its
//! acceptance test.

use lumen_static::cx::{BlockMetrics, LayoutCx, TextMeasure};
use lumen_static::desc::NodeState;
use lumen_static::display::{Cmd, Color, DisplayList, Renderer};
use lumen_static::event::App;
use lumen_static::invalidate::{Everything, Invalidation};
use lumen_static::retain::{Damage, Incremental, Retention};
use lumen_static::style::{Rule, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{pass, Constraints, Event, Point};

pub mod software;
pub mod svg;
#[cfg(feature = "truetype")]
pub mod text;

/// A running application: the tree, its parallel arrays, the one input
/// queue, and the frame's display list.
pub struct Host<A: App, R: Retention = Incremental, I: Invalidation = Everything> {
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
    /// Spawns refused for want of a spawner. Counted for the same reason
    /// the inbox counts refusals: a silent drop is the failure a gate
    /// exists to make visible.
    unspawned: usize,
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
    last_paint_ns: u64,
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
    /// binary. It defaults to `Everything`, so `Host<A>` is the code
    /// this crate shipped before T16.7.
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
}

impl<A: App, R: Retention, I: Invalidation> Host<A, R, I> {
    pub fn new(app: A, width: u32, height: u32) -> Self {
        let n = <A::Tree as Widget>::NODE_COUNT;
        let styles = pass::default_styles(app.tree());
        Host {
            app,
            inbox: lumen_static::inbox::Inbox::new(),
            more_deferred: false,
            spawner: None,
            unspawned: 0,
            styles,
            states: vec![NodeState::default(); n],
            list: DisplayList::new(),
            input: lumen_static::event::InputState::default(),
            overlays: Default::default(),
            queue: lumen_static::InputQueue::new(),
            size: (width, height),
            scale: 1.0,
            frame: 0,
            last_layout_ns: 0,
            last_paint_ns: 0,
            retain: R::default(),
            last_damage: Damage::All,
            invalidation: I::default(),
            arenas_dirty: true,
            sheet: Vec::new(),
            #[cfg(feature = "agent")]
            sheet_findings: Vec::new(),
            palette: lumen_static::style::Palette::LIGHT,
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

    /// Mark the subtree rooted at the node carrying `#[widget_id(wid)]`
    /// as changed (T16.7).
    ///
    /// Returns whether the id matched — a typo'd id is otherwise a
    /// silent no-op that shows up as a frame which quietly failed to
    /// redraw, which is the worst shape this class of bug takes.
    ///
    /// Under `Everything` this does nothing and everything is redrawn
    /// anyway, so an application can call it unconditionally and switch
    /// the parameter later.
    #[must_use = "a false return means the id matched no node and nothing was invalidated"]
    pub fn invalidate(&mut self, wid: &str) -> bool {
        self.arenas_dirty = true;
        let table = <A::Tree as Widget>::SUB_TABLE;
        let Some(root) = lumen_static::style::index_of_wid(table, wid) else {
            return false;
        };
        self.invalidation.mark(lumen_static::style::subtree_range(table, root));
        true
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

    /// Re-cascade one subtree, by the `#[widget_id]` on its root.
    ///
    /// The operation a class toggle performs — the tally swaps two
    /// rules when the sign flips — and the reason it is worth scoping:
    /// a full `set_stylesheet` re-resolves every node against every
    /// rule, measured at **36 µs for 901 nodes**, four times what a
    /// whole-tree layout and paint cost there
    /// (`docs/results-cascade-cost.md`).
    ///
    /// `sheet` must differ from the last one cascaded only in rules that
    /// match inside the subtree; debug builds check that against a full
    /// cascade (`style::cascade_subtree`). Returns whether the id was
    /// found — a typo'd id is a silent no-op otherwise, which is the
    /// failure mode this whole crate keeps finding.
    #[must_use = "a false return means the id matched no node and nothing was restyled"]
    pub fn restyle_subtree(&mut self, sheet: &[Rule], wid: &str) -> bool {
        let table = <A::Tree as Widget>::SUB_TABLE;
        let Some(root) = lumen_static::style::index_of_wid(table, wid) else {
            return false;
        };
        lumen_static::style::cascade_subtree_with(
            self.app.tree_mut(),
            sheet,
            &self.palette,
            &mut self.styles,
            root,
        );
        // The whole sheet is the one now in force, so it is the one kept
        // and linted.
        self.keep_sheet(sheet);
        // Scoped, so the invalidation is too — the same range the
        // cascade just rewrote.
        self.invalidation.mark(lumen_static::style::subtree_range(table, root));
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
    pub fn post(&mut self, ev: Event) {
        self.queue.push(ev);
    }

    pub fn frame_number(&self) -> u64 {
        self.frame
    }

    pub fn timings(&self) -> (u64, u64) {
        (self.last_layout_ns, self.last_paint_ns)
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
        let tx = self.inbox.sender();
        match self.spawner.as_mut() {
            Some(s) => {
                s.spawn(job, tx);
                true
            }
            None => {
                self.unspawned += 1;
                false
            }
        }
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

    /// How many spawns were refused for want of a spawner.
    pub fn unspawned(&self) -> usize {
        self.unspawned
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

    /// One frame: drain input, lay out, paint. Returns whether any event
    /// was consumed, which a real shell uses to decide whether to
    /// redraw.
    pub fn pump(&mut self) -> bool {
        self.pump_with(&mut BlockMetrics)
    }

    /// Same, with a caller-supplied text stack.
    pub fn pump_with(&mut self, metrics: &mut dyn TextMeasure) -> bool {
        let mut handled = false;
        let before = PointerFocus::of(&self.input);
        while let Some(ev) = self.queue.pop() {
            // `dispatch_overlaid`: an overlay takes pointer hits first,
            // Escape closes the topmost, and a press outside every open
            // one dismisses them and is consumed. `self.overlays` is
            // what the *last* paint deferred, which is deliberate — what
            // can be clicked is what was drawn.
            handled |= pass::dispatch_overlaid(
                &mut self.app,
                &self.styles,
                &mut self.states,
                &mut self.input,
                &self.overlays,
                &ev,
            );
        }

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
            self.invalidation.mark_all();
            self.arenas_dirty = true;
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
                if app.apply(m) {
                    **changed = true;
                }
            },
        );
        if applied_changed {
            self.invalidation.mark_all();
            self.arenas_dirty = true;
        }
        // A full drain may have left more behind; ask for another frame
        // rather than raising the cap, which is what keeps a burst from
        // becoming one long frame.
        self.more_deferred = applied == lumen_static::inbox::Inbox::<A::Msg>::PER_FRAME;

        if self.app.update() {
            self.invalidation.mark_all();
            self.arenas_dirty = true;
        }
        // **The arenas, if any are owed.** Before layout, so a template
        // styled by the previous frame's pass is laid out with those
        // styles rather than one frame behind them.
        if self.arenas_dirty {
            self.arenas_dirty = false;
            if !self.sheet.is_empty() {
                lumen_static::style::cascade_arenas_deep(
                    self.app.tree_mut(),
                    &self.sheet,
                    &self.palette,
                );
            }
        }

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
                .with_scale(self.scale);
            let tree = self.app.tree_mut();
            tree.layout(&mut cx, limits, &self.styles, &mut self.states, 0);
            tree.place(Point::default(), &self.styles, &mut self.states, 0);
        }
        self.last_layout_ns = t0.elapsed().as_nanos() as u64;

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
        // children that cannot be seen (T16.12). **Logical** units: the
        // list is painted through `Scaled`, which converts on the way
        // in and converts this back on the way out.
        self.list.set_bounds(Some([0.0, 0.0, self.size.0 as f32, self.size.1 as f32]));
        {
            // Logical geometry in, physical commands out (M10b), with
            // retention observing the stream on the way past (T13.8).
            // At a scale of 1.0 `Scaled` is a forwarding wrapper, and
            // under `Rebuild` `Recording` forwards to an empty
            // `observe` the optimizer removes.
            let mut scaled = lumen_static::Scaled::new(&mut self.list, self.scale);
            let mut out = lumen_static::Recording::new(&mut scaled, &mut self.retain);
            pass::paint_overlaid(self.app.tree(), &self.states, &mut out, &mut self.overlays);
        }
        self.last_paint_ns = t1.elapsed().as_nanos() as u64;

        // What this frame changed. Retention has already seen every
        // command; this closes the frame. Free under `Rebuild`.
        self.last_damage = self.retain.finish();

        // The set describes what changed *since the last frame*, so it
        // is spent once layout has read it. Marks arriving after this
        // point belong to the next frame, which is what makes
        // `invalidate` callable from anywhere between pumps.
        self.invalidation.clear();

        self.frame += 1;
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
        self.app.tick(dt_ms)
    }

    /// Route one composition step to the focused node — T14.8.
    ///
    /// Returns whether anything changed, so a host knows whether to
    /// redraw. `false` when nothing is focused or the focused thing does
    /// not edit text, which are the same answer from a caller's side.
    ///
    /// Not an `Event`: a composition step carries a string, and `Event`
    /// is `Copy` and 16 bytes. Same reason `paste` is a method.
    pub fn compose(&mut self, c: lumen_static::event::Composition<'_>) -> bool {
        let Some(i) = self.input.focus.map(|f| f as usize) else { return false };
        let table = <A::Tree as lumen_static::Widget>::SUB_TABLE;
        if i >= table.len() {
            return false;
        }
        self.app.tree_mut().node_mut(i).compose(c)
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
    /// The **deepest** hovered node wins. C6 marks every node on the
    /// hover chain, and an ancestor's cursor should not override the
    /// leaf the pointer is actually on — a button inside a text field's
    /// row must read as a button.
    pub fn cursor(&self) -> lumen_static::style::Cursor {
        self.states
            .iter()
            .enumerate()
            .rev()
            .find(|(_, s)| s.hovered)
            .and_then(|(i, _)| self.styles.get(i))
            .map_or(lumen_static::style::Cursor::Default, |s| s.cursor)
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

    /// Type `text` into the focused node, as if a person had.
    ///
    /// **Per character, into the one queue**, so a paste replays as
    /// ordinary typing and a recorded session needs no clipboard to
    /// reproduce (`docs/plan-events.md` E0). It also means a paste
    /// replaces a selection, moves the caret and fires bindings by
    /// exactly the paths typing already uses — no second edit path to
    /// get subtly different.
    ///
    /// **Chunked against [`lumen_static::InputQueue::<64>::CAP`]**, because a
    /// paste is not bounded by it. Filling the queue and pumping, then
    /// filling it again, is what keeps a 10 kB paste from losing its
    /// tail — and this is the first real caller of the `push` refusal
    /// M13/T13.4 added the drop counter for.
    ///
    /// Returns how many characters were delivered, which is every
    /// non-control one unless a widget refused them.
    pub fn paste(&mut self, text: &str) -> usize {
        let mut n = 0;
        for ch in text.chars().filter(|c| !c.is_control() || *c == '\n') {
            if self.queue.len() == lumen_static::InputQueue::<64>::CAP {
                self.pump();
            }
            self.post(Event::Text { ch });
            n += 1;
        }
        self.pump();
        n
    }

    /// What the frame just pumped changed, against the frame before it.
    pub fn damage(&self) -> Damage {
        self.last_damage
    }

    /// What a surface buffer `age` frames old needs redrawn, and bank
    /// the accumulated damage as presented.
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
pub fn for_each_rect(list: &DisplayList, mut f: impl FnMut([f32; 4], Color)) {
    for c in &list.cmds {
        if let Cmd::Rect { rect, color } = c {
            f(*rect, *color);
        }
    }
}
