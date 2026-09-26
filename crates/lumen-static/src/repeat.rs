//! `Repeat<T>` — the repeater from handoff §6: one main-table entry, a
//! const template, a runtime instance count, only the visible window
//! materialized.
//!
//! > `NODE_COUNT` grows with how much UI you **wrote**, not how much
//! > data you **display**.
//!
//! The window is plain `Vec` fields, not `RefCell`: with recursive
//! traversal `layout` takes `&mut self`, so a repeater mutates its own
//! window directly. Interior mutability existed only because the old
//! flat passes handed ops a *shared* base pointer.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::cx::LayoutCx;
use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::display::Painter;
use crate::hit::{contains, Hit, Inner};
use crate::style::{Length, Seed, Style};
use crate::widget::{Node, Widget};

pub fn noop_bind<T>(_: &mut T, _: usize) {}

/// A virtualized list: `count` rows of `T`, of which only the visible
/// window is materialized.
///
/// **Requires the `alloc` feature** (default-on). One of the escape
/// hatches M13/T13.4 moved behind it, so that "what does dynamism cost
/// me" is answerable in the type system rather than in prose.
///
/// T13.3 considered giving it caller-supplied storage instead — the
/// shape `TextBuffer::Clusters` uses — and decided against: a
/// virtualized million-row list is not an embedded use case, and the
/// dividing line stays exactly at "runtime content count".
pub struct Repeat<T: Widget + Default + 'static> {
    /// Logical row count — the data size. Never reflected in any table.
    pub count: usize,
    /// Scroll offset in px along the main (vertical) axis.
    pub scroll: f32,
    /// Uniform per-row extent along the main axis.
    pub item_extent: f32,
    /// Configure a recycled template instance for a row. A plain `fn`
    /// pointer: it cannot capture, so a hot reload re-supplies it.
    ///
    /// **This runs for every materialized row on every frame the
    /// repeater is laid out**, so it is on the steady-state path and
    /// **must not allocate**.
    ///
    /// **A data change must be reported** — M41/T41.3
    /// (`docs/plan-boundaries-repair.md` §2.3). This said the per-frame
    /// bind kept a data change visible "without an invalidation call",
    /// which was true only while every frame laid out every node. Under
    /// the `Boundaries` default a repeater nothing marked is not laid out
    /// at all, and one laid out because a sibling changed keeps its
    /// container rows as they were. Report a change in one of two ways:
    /// bump [`Repeat::generation`] (from the application's state, through
    /// a value `#[bind]`, which marks exactly this repeater), or call
    /// `Host::invalidate` on its id.
    ///
    /// The obvious spelling does:
    ///
    /// ```text
    /// fn bind(row: &mut Row, i: usize) {
    ///     row.name.set(format!("row {i}"));   // one heap allocation,
    /// }                                       // per row, per frame
    /// ```
    ///
    /// `set` compares before it stores, so the *write* is skipped — but
    /// `format!` has already allocated by the time `set` is called, and
    /// no comparison can undo that. Use
    /// [`format_into`](crate::bind::format_into), which is the same
    /// machinery the `#[bind]` derive generates for a table node: it
    /// formats through a reused scratch buffer and compares before
    /// storing.
    ///
    /// ```text
    /// fn bind(row: &mut Row, i: usize) {
    ///     lumen_static::bind::format_into(&mut row.name, format_args!("row {i}"));
    /// }
    /// ```
    ///
    /// Both blocks are `text` rather than doctests on purpose: `Label`
    /// lives in `lumen-static-widgets`, which is a *dev*-dependency of
    /// this crate, so no doctest here can name one — and the difference
    /// being shown is an allocation count, which a doctest cannot
    /// observe anyway. The gate named above is what checks it.
    ///
    /// Measured on an 18-row window (M29/T29.5,
    /// `docs/results-damage-precision.md`): the allocating form costs
    /// **10.8 µs a frame** against the framework's own ~1.5 µs for
    /// re-binding the same window, and
    /// `a_repeater_whose_bind_writes_text_allocates_nothing` (in
    /// `crates/lumen-static-shell/tests/alloc_gate.rs`) is the gate that
    /// now refuses it — every other repeater arm in
    /// that file binds with [`noop_bind`], so until M29 nothing had ever
    /// asked what a bind that *writes* costs.
    pub bind: fn(&mut T, usize),
    window: Vec<T>,
    /// Materialized rows' state and the template style row they share.
    /// Instances are off-table, so the cascade cannot reach them; the
    /// row style is their source until per-row styling exists.
    arena: crate::arena::Arena,
    first: usize,
    /// The viewport height `layout` last measured against.
    ///
    /// Kept because `on_event` has no state array and must still tell
    /// "the offset moved" from "we are at an end and the parent should
    /// have the rest of the gesture" — `ScrollView` keeps its overflow
    /// for the same reason. It is last frame's, which is this frame's
    /// for any gesture that did not also resize the window.
    viewport: f32,
    /// The data's version — M41/T41.3.
    ///
    /// **Bump it when what [`Repeat::bind`] reads changes**, or the rows
    /// may not show it (see `bind`). Bound from the application's state
    /// with the value form, `#[bind(ui.list.generation, version)]`, it
    /// is written only when it moves, and the binding marks exactly this
    /// repeater — T29.3's per-site mark, found by address. The repeater
    /// also compares it with the version it last laid out, so a bump that
    /// arrives by another route lays every row out again the next time
    /// the repeater is reached — which, unmarked, is only when something
    /// beside it was.
    ///
    /// M44/T44.2's click latch reads the same counter.
    pub generation: u64,
    /// The `generation` the last layout saw.
    seen_generation: u64,
    /// **The keyboard's row** — M44/T44.10 (`docs/plan-input-semantics.md`).
    ///
    /// Rows are not Tab stops: focus is a main-table index and a row
    /// lives in an arena. So a focused repeater keeps an *active* row
    /// instead — Up, Down, Home, End, PageUp and PageDown move it and
    /// scroll it into view, it is drawn with the focus ring while the
    /// repeater has focus, and Enter or Space activates its handler
    /// (`Node::active_row`). `None` until a key sets it; an application
    /// may set it to its current selection when it opens a list.
    pub active: Option<usize>,
    /// Bumped by `layout` whenever `count` differs from the last
    /// layout's — M44/T44.2. Added to `generation` in `row_generation`,
    /// so an insertion or a removal the application did not report still
    /// cancels a click that spans it. Kept apart from `generation`
    /// because that field is the application's: a `#[bind]` writes it.
    count_epoch: u64,
    /// The `count` the last layout saw.
    seen_count: usize,
    /// Which row each arena slot was last laid out as; `usize::MAX` for a
    /// slot that has not been. A slot whose row changed — the ring moved
    /// — is laid out again even on a frame that did not mark the
    /// repeater; one whose row did not is left to its own boundary.
    slot_rows: Vec<usize>,
}

impl<T: Widget + Default> Default for Repeat<T> {
    fn default() -> Self {
        Self::new(0, 1.0, noop_bind::<T>)
    }
}

impl<T: Widget + Default> Repeat<T> {
    pub fn new(count: usize, item_extent: f32, bind: fn(&mut T, usize)) -> Self {
        Repeat {
            count,
            scroll: 0.0,
            item_extent,
            bind,
            window: Vec::new(),
            arena: crate::arena::Arena::new(T::NODE_COUNT),
            first: 0,
            viewport: 0.0,
            generation: 0,
            seen_generation: 0,
            count_epoch: 0,
            seen_count: count,
            active: None,
            slot_rows: Vec::new(),
        }
    }

    /// Materialized instances (visible window), for tests/inspection.
    /// Which arena slot holds window position `k`.
    ///
    /// The window is a **ring**: position `k` is row `first + k`, and
    /// that row lives in slot `(first + k) % len`. Slot-indexing it
    /// instead — position `k` in slot `k` — meant every slot's *row*
    /// changed when `first` advanced by one, so a one-row scroll rebound
    /// and re-measured all 31 rows of a 31-row window instead of the one
    /// that actually entered. Measured 31 of 31 before, 1 of 31 after
    /// (`docs/plan-review-fixes.md` F4.6).
    ///
    /// Rows are rebound on every layout; the ring is what makes almost
    /// all of those binds write identical text, which `Label::set_str`
    /// then skips. A layout is not every frame under `Boundaries`, which
    /// is why a data change must be reported (see [`Repeat::bind`]).
    #[inline]
    fn slot(&self, k: usize) -> usize {
        let len = self.window.len();
        if len == 0 {
            0
        } else {
            (self.first + k) % len
        }
    }

    pub fn window_len(&self) -> usize {
        self.window.len()
    }

    /// First materialized row.
    pub fn first_row(&self) -> usize {
        self.first
    }

    /// Copy of instance `k`'s state for template node `node`.
    pub fn instance_state(&self, k: usize, node: usize) -> NodeState {
        self.arena.node_state(self.slot(k), node)
    }

    /// Instance `k`'s widget, for routing and observability.
    pub fn instance(&self, k: usize) -> &T {
        &self.window[self.slot(k)]
    }

    /// The smallest item extent a repeater will act on.
    ///
    /// `visible_range` divides the viewport by the extent, so a subnormal
    /// like `1e-40` — which clears the `<= 0.0` guard — saturates the
    /// `as usize` cast to `usize::MAX` and then overflows the `+ 1`. A
    /// value this small cannot describe a row anyone can see; treating it
    /// as "no rows" is the honest reading. `docs/plan-review-fixes.md`
    /// F1.4.
    pub const MIN_ITEM_EXTENT: f32 = 1.0 / 1024.0;

    /// The most instances a repeater will materialize in one frame.
    ///
    /// A window is bounded by the viewport in normal use; this bounds it
    /// when the inputs are not normal. `count` and `item_extent` both
    /// come back from saved state across a hot reload, so both ends of
    /// the division are attacker-controlled in the reload path — 40 bytes
    /// of JSON asking for 600 million rows is not a frame anyone wants
    /// attempted.
    pub const MAX_WINDOW: usize = 4096;

    /// The largest `count * item_extent` this can address to the row.
    ///
    /// `visible_range` picks the first row with `scroll / item_extent`,
    /// in `f32`. `f32` represents consecutive integers exactly only up
    /// to **2^24 = 16,777,216**; above that the gap between
    /// representable values is 2, then 4, then 8. Once one ulp of
    /// `scroll` exceeds `item_extent`, some rows have no offset that
    /// selects them and scrolling *skips* them — silently, and worse the
    /// further down the list you are.
    ///
    /// The number, since this had been deferred twice without one
    /// (`docs/step3-repeaters.md`): at the common 24 px row, **699,050
    /// rows** is the ceiling; at 1 px it is 16.7 million. Below the
    /// ceiling every row is reachable.
    ///
    /// **Above it, the rows past the ceiling do not exist** — M44/T44.7.
    /// Every geometric use of `count` (the content height, the scroll
    /// range, the window) goes through [`Repeat::reachable_rows`], which
    /// clamps it, so every row that is laid out is reachable, is placed
    /// one extent below the one before it, and is hit where it is
    /// painted. This rustdoc used to say `layout` debug-asserts and a
    /// release build "scrolls with gaps"; neither was true. No assertion
    /// existed, and the failure was **overlapping rows**: `place`
    /// computes `row * extent - scroll` in `f32`, and at a billion pixels
    /// one ulp is 64 px, so at 50 million rows 13 of 18 visible rows were
    /// hit somewhere other than where they were painted
    /// (`crates/lumen-static/tests/scroll_view.rs`). The clamp is in
    /// every build, not a debug panic, because the count is the
    /// application's *data* — a debug build that crashed when a user
    /// opened a large file would be worse than one that shows the first
    /// sixteen million pixels of it. The agent's audit reports the cut as
    /// `W1009` (`crates/lumen-static-agent/src/audit.rs`), and the
    /// accessibility tree still reports the true `count`.
    ///
    /// Raising it means an `f64` offset, which is a change to
    /// `NodeState`'s width and therefore to the memory budget §2 cares
    /// about — a decision, not a fix.
    pub const MAX_ADDRESSABLE_EXTENT: f32 = MAX_ADDRESSABLE_EXTENT;

    /// Whether `item_extent` can be divided into.
    ///
    /// Spelled positively — `is_finite() &&` rather than a negated
    /// comparison — so NaN lands in the reject arm by construction
    /// rather than by implication.
    #[inline]
    fn extent_is_usable(&self) -> bool {
        self.item_extent.is_finite() && self.item_extent >= Self::MIN_ITEM_EXTENT
    }

    /// The rows this repeater lays out: `count`, clamped to
    /// [`Repeat::max_addressable_rows`] (M44/T44.7). See
    /// [`Repeat::MAX_ADDRESSABLE_EXTENT`] for why rows past the ceiling
    /// are cut rather than drawn on top of each other.
    pub fn reachable_rows(&self) -> usize {
        self.count.min(self.max_addressable_rows())
    }

    /// Rows this repeater can address at its current `item_extent`.
    pub fn max_addressable_rows(&self) -> usize {
        if !self.extent_is_usable() {
            return 0;
        }
        (Self::MAX_ADDRESSABLE_EXTENT / self.item_extent) as usize
    }

    /// The height this repeater was last **placed** at — what a wheel is
    /// clamped against.
    ///
    /// Published because it is the difference between a list that
    /// scrolls and one that silently will not, and because a test that
    /// cannot read it can only assert the symptom (T27.10).
    pub fn viewport(&self) -> f32 {
        self.viewport
    }

    /// The largest useful scroll offset: content past the viewport, or
    /// zero when everything fits.
    pub fn max_scroll(&self, viewport_h: f32) -> f32 {
        if self.count == 0 || !self.extent_is_usable() || !viewport_h.is_finite() {
            return 0.0;
        }
        (self.reachable_rows() as f32 * self.item_extent - viewport_h).max(0.0)
    }

    /// The offset actually used, clamped to what there is to scroll.
    ///
    /// **Clamped at use, never written back** (M27/T27.10). `layout`
    /// used to assign `self.scroll = clamp(self.scroll, 0, max_scroll)`,
    /// which is correct for every *real* layout and destructive for a
    /// speculative one: the agent's ambient audit measures each leaf
    /// twice under 1e6 and 2e6 px boxes to tell an intrinsic node from a
    /// filler (`crates/lumen-static-agent/src/audit.rs`), and against a
    /// 2 000 000 px viewport `max_scroll` is 0 — so every frame the
    /// audit ran reset the offset to zero. **A grid in the showcase
    /// could not be scrolled at all**, while the same wheel on the same
    /// widget moved it in a unit test and in a headless `Host`: found by
    /// `just widget-drive`, which is the only thing that runs the audit
    /// and the wheel in one process.
    ///
    /// A measurement must not change what it measures. The offset is the
    /// user's, `first` and the window are derived from it, and anything
    /// derived may be recomputed by any pass without harm.
    fn offset(&self, viewport_h: f32) -> f32 {
        crate::num::clamp(self.scroll, 0.0, self.max_scroll(viewport_h))
    }

    fn visible_range(&self, viewport_h: f32) -> (usize, usize) {
        // Spelled out rather than `!(extent >= MIN)`: NaN must land in
        // the reject arm, and `is_finite` says so where a negated
        // comparison would only imply it.
        let count = self.reachable_rows();
        if count == 0 || !self.extent_is_usable() {
            return (0, 0);
        }
        let first = (crate::num::floor(self.offset(viewport_h) / self.item_extent).max(0.0)
            as usize)
            .min(count - 1);
        // saturating_add, not `+ 1`: the ceil can saturate the cast.
        let vis = (crate::num::ceil(viewport_h / self.item_extent) as usize)
            .saturating_add(1)
            .min(count - first)
            .min(Self::MAX_WINDOW);
        (first, vis)
    }

    /// Geometry of window position `k`'s root node — for tests.
    pub fn row_state(&self, k: usize) -> NodeState {
        self.arena.node_state(self.slot(k), 0)
    }

    fn slice(&self, k: usize) -> &[NodeState] {
        self.arena.slice(self.slot(k))
    }
}

impl<T: Widget + Default + 'static> Widget for Repeat<T> {
    const NODE_COUNT: usize = 1;
    /// The template's depth counts: a repeater recurses into instances.
    const DEPTH: usize = 1 + T::DEPTH;
    const IS_RIGID: bool = false;
    /// One entry: no *table* children (the template is not spliced in),
    /// but CONTAINER, because it places off-table content.
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::CONTAINER)];

    const KIND: &'static str = "repeat";

    /// **One seed, for the repeater's own node.** Its rows are off-table
    /// and styled by `cascade_arenas` from the template — see
    /// [`crate::hatch::List`], which has the same shape and the same
    /// reason.
    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::CURSOR,
            crate::style::Fields::VISIBLE,
            crate::style::Fields::BACKGROUND,
            crate::style::Fields::BORDER,
            crate::style::Fields::RADIUS,
            crate::style::Fields::SHADOW,
            crate::style::Fields::PRESSED_DIM,
            crate::style::Fields::HOVER_DIM,
        ]))];

    fn type_defaults() -> Style {
        // A list fills its container and scrolls inside it.
        Style { width: Length::FILL, height: Length::FILL, ..Style::default() }
    }
}

/// See [`Repeat::MAX_ADDRESSABLE_EXTENT`]. Free-standing as well as
/// associated so the audit can cite it without naming a type parameter.
pub const MAX_ADDRESSABLE_EXTENT: f32 = 16_777_216.0;

impl<T: Widget + Default + 'static> Node for Repeat<T> {
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn arena_count(&self) -> usize {
        1
    }

    fn arena_mut(&mut self, a: usize) -> Option<crate::arena::ArenaMut<'_>> {
        if a != 0 {
            return None;
        }
        // **No template instance.** This built one with
        // `get_or_insert_with(T::default)` so the walk had a live node
        // to ask for kinds and defaults; every one of those questions
        // is answered by `T::SEEDS` (M26/T26.10).
        self.arena.ensure_style(T::SEEDS);
        Some(crate::arena::ArenaMut {
            table: T::SUB_TABLE,
            style: self.arena.style_mut(),
            seeds: T::SEEDS,
        })
    }

    /// Yes — this is the construct A7 forbids inside a scroller.
    fn virtualizes(&self) -> bool {
        true
    }

    /// The application's `generation` plus a bump for every change of
    /// `count` a layout has seen (M44/T44.2): a press and a release on
    /// one row index are the same item only if neither moved.
    fn row_generation(&self) -> u64 {
        self.generation.wrapping_add(self.count_epoch)
    }

    /// The cursor the rows' shared style row resolved for that template
    /// node (M44/T44.8) — every row is styled alike, so the row index
    /// does not matter.
    fn inner_cursor(&self, inner: crate::hit::Inner) -> Option<crate::style::Cursor> {
        self.arena.style().get(inner.node as usize).map(|s| s.cursor)
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        // The repeater's own style is read via `resolve_self` below;
        // instances use the template's defaults, since the cascade is
        // keyed by table index and instances are off-table.
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // **The repeater's own box, resolved before anything reads it —
        // both axes, M27/T27.4.**
        //
        // M24/T24.2 fixed the width half of this and the comment below
        // still describes it: a row must be laid out against the box the
        // repeater *settled on*, not the one it was offered. The height
        // half was left, and it is the same defect with a different
        // symptom: `visible_range` and the scroll clamp were measured
        // against `limits.max_h`, which for a `Popup`'s content is the
        // whole window. So a 630 px menu inside a 560 px panel computed
        // `max_scroll = 630 - 720 = 0` and **could not be scrolled at
        // all** — the wheel arrived, `on_event` ran, and the clamp
        // pinned the offset to zero. Found by `just widget-drive`, which
        // could not reach the last three widgets in the showcase's own
        // menu.
        //
        // Neither `content` nor `s` depends on a row: `content.h` is
        // `count * item_extent` and `content.w` is the limit. So the box
        // can be resolved first, and everything below measured against
        // it.
        if self.count != self.seen_count {
            self.seen_count = self.count;
            self.count_epoch = self.count_epoch.wrapping_add(1);
        }
        let content = Size { w: limits.max_w, h: self.reachable_rows() as f32 * self.item_extent };
        let s = crate::ops::resolve_self(limits, &style[i], content);
        let (first, vis) = self.visible_range(s.h);
        self.first = first;
        self.window.resize_with(vis, T::default);
        // **The seeds style every row**, and building a `T::default()`
        // per frame to be asked for them is what M26/T26.10 deleted.
        // `Node::defaults` depends on the type and not the instance
        // (F3.2), so the type is what to ask — `Seed::defaults` is the
        // same `fn` the probe would have dispatched to.
        self.arena.ensure(vis, T::SEEDS);
        // Bound on every layout — the ring is what makes almost every one
        // of these write identical text, which `Label::set_str` then
        // skips. **Not every frame**: under `Boundaries` a repeater
        // nothing marked is not reached, so a data change is reported
        // through `generation` or `Host::invalidate` (M41/T41.3).
        let bind = self.bind;
        let len = self.window.len();
        for k in 0..len {
            let row = first + k;
            bind(&mut self.window[row % len], row);
        }

        // **The repeater's own width, resolved first.** A row must be
        // laid out against the box the repeater *settled on*, not the
        // one it was offered — and it can be, because the width this
        // resolves to never depends on a row: `content.w` below is the
        // limit, not a measurement. Using `limits.max_w` here put an
        // 18-row menu inside a 300 px panel out at 932 px wide, with
        // every label centred in a box three times the panel
        // (M24/T24.2). It is the same asymmetry `ScrollView` has and
        // `examples/showcase` recorded against it; this half is now
        // fixed.
        let row_limits =
            Constraints { min_w: 0.0, max_w: s.w, min_h: 0.0, max_h: self.item_extent };
        let n = T::NODE_COUNT;
        // **What a row is told changed — M41/T41.3.** A row's indices are
        // its arena slot's, `0..T::NODE_COUNT`, and the frame's `Changed`
        // ranges are *main-table* indices, so handing a row `cx` as it
        // came asked a derived row whether an unrelated main-table range
        // was clean. It usually answered yes and kept last frame's box —
        // after `Host::invalidate` on this list (`repeat_data.rs` case
        // 3), and on showcase's DataGrid after a wheel, which the shadow
        // oracle named on its first sweep. So a row gets a view in its own
        // index space: everything, when this repeater was marked, its
        // `generation` moved, or the row now in the slot is not the one
        // last laid out there; nothing otherwise, which leaves a derived
        // row to its constraint check. Under `Everything` every repeater
        // is marked, so nothing changes there.
        let marked = !cx.changed.is_clean(i..i + 1) || self.generation != self.seen_generation;
        self.seen_generation = self.generation;
        let self_active = self.active.filter(|&a| a < self.reachable_rows());
        // Instances use the template's own default styles: the cascade
        // is keyed by *table* index, and instances are not in the table
        // (`docs/layout-and-storage-decisions.md` §3's recorded limit).
        let Repeat { window, arena, slot_rows, .. } = self;
        let (row_style, states) = arena.parts();
        let len = window.len();
        if slot_rows.len() != len {
            // A new length re-deals every row to a new slot.
            slot_rows.clear();
            slot_rows.resize(len, usize::MAX);
        }
        let frame = cx.changed;
        // The active row wears the focus ring, and only while this
        // repeater holds focus (T44.10). Written before the row lays out,
        // since a row resolves its look there.
        let (lit, active) = (st[i].focused, self_active);
        for k in 0..len {
            let sl = (first + k) % len;
            let row = first + k;
            if n > 0 {
                states[sl * n].focused = lit && active == Some(row);
            }
            cx.changed = if marked || slot_rows[sl] != row {
                crate::invalidate::Changed::ALL
            } else {
                crate::invalidate::Changed::NONE
            };
            window[sl].layout(cx, row_limits, row_style, &mut states[sl * n..(sl + 1) * n], 0);
            slot_rows[sl] = row;
        }
        cx.changed = frame;

        // **`record_painted`, not two field writes.** This wrote only
        // `constraints` and `size`, so a repeater's `background`,
        // `radius` and `shadow` resolved in the cascade and then reached
        // no `NodeState` — `paint` has no `&[Style]` and reads them from
        // there. A menu panel with a background came out transparent
        // (M24/T24.2). Every other container records through here.
        crate::ops::record_painted(st, i, limits, s, &style[i])
    }

    /// **Measured without being rebuilt** (M27/T27.10).
    ///
    /// The provided body is `self.layout(..)`, which for a repeater
    /// picks a window: `first` and `window` are recomputed against the
    /// probe's box, and the audit's box is 2 000 000 px — so every
    /// frame the audit ran left the *agent's* view of the list showing
    /// row 0 whatever the window had actually painted. The frame on
    /// screen was right and the tree the agent read was wrong, which is
    /// the worst of the two to debug: `just widget-drive` saw a grid
    /// scroll by one row and then jump back.
    ///
    /// `Label` overrides this for the same reason one layer down (its
    /// memo). The answer a probe wants — how tall the content would be
    /// given this box — is the same arithmetic `layout` starts with, and
    /// none of what it then does.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        _cx: &mut crate::LayoutCx,
        limits: Constraints,
        style: &[Style],
        _st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let content = Size { w: limits.max_w, h: self.reachable_rows() as f32 * self.item_extent };
        crate::ops::resolve_self(limits, &style[i], content)
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        // **The viewport is what was placed, not what was offered.**
        // `layout` runs speculatively — twice per frame from the audit's
        // probes, and again from any parent that measures before it
        // commits — and the last of those calls is not the frame's
        // geometry. `place` runs once, on the real box, which is the
        // number `on_event` needs to clamp a wheel against (T27.10).
        self.viewport = st[i].size.h;
        // Instance geometry lives in this repeater's own arena, so the
        // window is positioned here rather than by the caller.
        let n = T::NODE_COUNT;
        let (first, extent) = (self.first, self.item_extent);
        let scroll = self.offset(st[i].size.h);
        let Repeat { window, arena, .. } = self;
        let (row_style, states) = arena.parts();
        let len = window.len();
        for k in 0..len {
            let sl = (first + k) % len;
            let dy = (first + k) as f32 * extent - scroll;
            let origin = Point { x: at.x, y: at.y + dy };
            window[sl].place(origin, row_style, &mut states[sl * n..(sl + 1) * n], 0);
        }
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        // A repeater is a viewport, and its window deliberately overruns
        // it in both directions: `place` puts the first row at
        // `first * extent - scroll`, negative for any scroll off an item
        // boundary, and `visible_range` materializes one row past the
        // bottom because a viewport is rarely an exact multiple of the
        // item extent. Both are required by virtualization rather than
        // incidental to it.
        //
        // `hit` has always gated on this same rect before examining a
        // row, so without this clip paint and hit disagree: a row drawn
        // up to a full item outside the list is visible but not
        // clickable. Clipping here is what makes the two agree.
        //
        // When `Style.overflow` lands (A2 of
        // `docs/text-and-scrolling-decisions.md`) this becomes the
        // generic mechanism and `Repeat` keeps it only as its
        // `defaults()`.
        // **Its own shadow and fill, before the rows** — which every
        // other container does (the derive, `ScrollView`, `Glass`) and
        // this one did not, so a `background` on a repeater resolved
        // and then painted nothing. Found when a menu panel came out
        // transparent (M24/T24.2).
        let r = crate::ops::rect_of(&st[i]);
        if st[i].shadow > 0.0 {
            out.shadow(r, st[i].radius, st[i].shadow);
        }
        out.rounded_rect(r, st[i].bg, st[i].radius);
        crate::ops::border(st, i, out);
        out.push_clip(r);
        for k in 0..self.window.len() {
            self.window[self.slot(k)].paint(self.slice(k), 0, out);
        }
        out.pop_clip();
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !contains(&st[i], x, y) {
            return None;
        }
        for k in 0..self.window.len() {
            if let Some(h) = self.window[self.slot(k)].hit(self.slice(k), 0, x, y) {
                return Some(Hit {
                    node: i as u32,
                    inner: Some(Inner { row: (self.first + k) as u64, node: h.node }),
                    local: h.local,
                });
            }
        }
        Some(Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    /// **A repeater scrolls itself — M27/T27.4.**
    ///
    /// Nothing did before, and nothing noticed, because every repeater
    /// in the tree until M24 sat inside a `ScrollView`. `Select`'s menu
    /// is a bare `Popup<Repeat<Row>>`, so **a dropdown with more options
    /// than fit on screen could not be scrolled at all** — not by a
    /// wheel, not by a drag, not by anything. Found by
    /// `just widget-drive` the moment the showcase's catalogue grew past
    /// what its own menu could show, at twenty-one entries.
    ///
    /// The offset moves *against* `dy`, and declines at an end so the
    /// parent gets the rest of the gesture. Both are
    /// `ScrollView`'s conventions, because a list that scrolled
    /// the other way from every other list would be its own defect.
    fn on_event(&mut self, ev: &crate::event::Event, _cx: &crate::event::EventCtx) -> bool {
        use crate::event::{Event, Key};
        match *ev {
            Event::Scroll { dy, .. } => {
                let want = crate::num::clamp(self.scroll - dy, 0.0, self.max_scroll(self.viewport));
                if want == self.scroll {
                    return false;
                }
                self.scroll = want;
                true
            }
            // **The keyboard moves the active row** (M44/T44.10), reached
            // through the pass's Pass 1 when this repeater is focused.
            Event::Key { key, down: true } => {
                let count = self.reachable_rows();
                if count == 0 || !self.extent_is_usable() {
                    return false;
                }
                let page = ((self.viewport / self.item_extent) as usize).max(1);
                let last = count - 1;
                let cur = self.active.filter(|&a| a < count);
                let next = match key {
                    Key::Down => cur.map_or(0, |a| (a + 1).min(last)),
                    Key::Up => cur.map_or(0, |a| a.saturating_sub(1)),
                    Key::Home => 0,
                    Key::End => last,
                    Key::PageDown => cur.map_or(0, |a| a.saturating_add(page).min(last)),
                    Key::PageUp => cur.map_or(0, |a| a.saturating_sub(page)),
                    _ => return false,
                };
                if Some(next) == cur {
                    return false;
                }
                self.active = Some(next);
                // Into view: the least scroll that shows the whole row.
                let (top, bottom) = (next as f32 * self.item_extent, (next + 1) as f32 * self.item_extent);
                let off = self.offset(self.viewport);
                if top < off {
                    self.scroll = top;
                } else if bottom > off + self.viewport {
                    self.scroll = bottom - self.viewport;
                }
                true
            }
            _ => false,
        }
    }

    /// Arrows move the active row, so they never leave a focused
    /// repeater (M44/T44.1). A repeater is focusable only where its
    /// owner's table says so — a `Select`'s list is — so this answers for
    /// no other.
    fn owns_arrows(&self) -> bool {
        true
    }

    /// The active row, for Enter and Space (M44/T44.10).
    fn active_row(&self) -> Option<crate::hit::Inner> {
        self.active
            .filter(|&a| a < self.reachable_rows())
            .map(|a| crate::hit::Inner { row: a as u64, node: 0 })
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        id: u64,
        node: &mut accesskit::Node,
        extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::List);
        node.set_size_of_set(self.count);
        // Only rows that intersect the viewport are announced.
        //
        // The window deliberately overruns its viewport in both
        // directions — `place` puts the first row at a negative offset
        // for any scroll off an item boundary, and `visible_range`
        // materializes one row past the bottom — and those rows are
        // clipped from paint and rejected by hit. Announcing them anyway
        // put nodes in the AccessKit tree and the agent's view at bounds
        // where nothing is drawn and no click can land: the
        // probe-versus-click divergence, one surface over from where it
        // was fixed (`docs/plan-review-fixes.md` F4.7).
        let viewport = crate::hit::contains_rect(&_st[_i]);
        for k in 0..self.window.len() {
            let rs = self.arena.node_state(self.slot(k), 0);
            if !crate::hit::rects_overlap(viewport, crate::ops::rect_of(&rs)) {
                continue;
            }
            let row = (self.first + k) as u64;
            // Row-stable synthetic ids: `1<<63 | node<<44 | row<<12 | template`.
            let id_of = |t: usize| crate::observe::instance_id(id, row, t);
            crate::observe::emit_subtree(&self.window[self.slot(k)], self.slice(k), &id_of, extra);
            node.push_child(accesskit::NodeId(id_of(0)));
        }
    }

    #[cfg(feature = "observe")]
    fn stats(&self, out: &mut crate::desc::NodeStats) {
        out.items = Some(self.count as u64);
        out.materialized = Some(self.window.len() as u32);
        out.item_extent = Some(self.item_extent);
    }
}

/// Routes to the template's handlers with the hit row attached, so a
/// handler declared on a template widget receives `ctx.row` as an
/// argument — never as a capture (`docs/events-decision.md`).
impl<T, S> crate::event::Handlers<S> for Repeat<T>
where
    T: Widget + Default + 'static + crate::event::Handlers<S>,
{
    fn handler(&self, sel: crate::event::NodeSel) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        let (row, node) = sel.inner?;
        let k = (row as usize).checked_sub(self.first)?;
        // **Through the ring, like paint, hit and `instance`** —
        // M44/T44.7, R3-F5. Window position `k` lives in slot
        // `(first + k) % len`; this indexed `window` raw, so once the ring
        // had rotated a click ran another row's handler, which nothing
        // saw while every repeater bound the same handler to every row
        // (`crates/lumen-static/tests/events.rs`). The explicit bound
        // replaces the one the raw `get` gave, since `slot` wraps.
        if k >= self.window.len() {
            return None;
        }
        let item = self.window.get(self.slot(k))?;
        item.handler(crate::event::NodeSel { i: node as usize, inner: None })
    }
}
