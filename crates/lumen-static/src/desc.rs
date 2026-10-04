//! Node descriptors, per-node state, and the value types the passes use.
//!
//! **The table is no longer the traversal mechanism** (handoff §5,
//! superseded — see `docs/results-traversal.md`). Layout, paint and hit
//! testing recurse over the widget struct itself; the const table
//! survives for *index addressing*, which the agent surface, the change
//! journal and the ambient audit are all keyed on. That is why `kind`
//! and `offset` are gone: nothing dispatches through the table any more,
//! and nothing recovers a widget from a byte offset.

/// Where a node was declared (handoff §9: source provenance, dev-only).
#[cfg(feature = "observe")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SrcLoc {
    pub file: &'static str,
    pub line: u32,
}

#[cfg(feature = "observe")]
impl SrcLoc {
    pub const UNKNOWN: SrcLoc = SrcLoc { file: "", line: 0 };
}

/// One node of the tree, in DFS pre-order (parent before children).
///
/// Structural, plus the two static selector inputs the cascade needs.
/// 48 bytes in a release build and with `a11y` alone, 72 with `observe`
/// (the `src` stamp): 16 of tree links and flags, 32 of `wid`/`classes`
/// fat pointers. Asserted under all three since M54/T54.1
/// (`crates/lumen-static-widgets/tests/desc_size.rs`). The "16 bytes"
/// this comment used to claim predates M1 and was never updated —
/// measured and corrected 2026-09-02.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeDesc {
    pub first_child: u32,
    pub parent: u32,
    pub next_sibling: u32,
    pub child_count: u16,
    pub flags: u8,
    #[cfg(feature = "observe")]
    pub src: SrcLoc,
    /// Author id (`#[widget_id("save")]`). Two jobs: the agent-facing
    /// stable address (table indices shift under source edits), and an
    /// `#id` selector for the cascade — which is why it is *not* gated
    /// behind `observe` any more.
    pub wid: &'static str,
    /// Style classes (`#[widget_class("row", "striped")]`), matched by
    /// the cascade. Static per node, like `wid`.
    pub classes: &'static [&'static str],
}

impl NodeDesc {
    pub const RIGID: u8 = 1 << 0;
    /// Has children (table children, or off-table content it places).
    pub const CONTAINER: u8 = 1 << 1;
    /// Can take keyboard focus.
    pub const FOCUSABLE: u8 = 1 << 2;
    /// **A press here leaves focus where it is.** A press normally moves
    /// focus to the deepest focusable node on its hit chain, or clears it
    /// — what clicking the background does. A popup list that belongs to
    /// a field is neither: a combobox's field keeps focus while its list
    /// is open, as a browser's input does when its suggestion list is
    /// clicked. Without this the press blurred the field, the field's
    /// "abandoned query" revert cleared the filter, and the release chose
    /// by position in the *unfiltered* list (a combobox filtered to
    /// Apple, apricot chose Banana).
    pub const KEEPS_FOCUS: u8 = 1 << 3;

    /// Placeholder for const table construction; every slot is overwritten.
    pub const EMPTY: NodeDesc = NodeDesc::new(0, NO_PARENT, 0, 0, 0);

    pub const fn new(
        first_child: u32,
        parent: u32,
        next_sibling: u32,
        child_count: u16,
        flags: u8,
    ) -> NodeDesc {
        NodeDesc {
            first_child,
            parent,
            next_sibling,
            child_count,
            flags,
            #[cfg(feature = "observe")]
            src: SrcLoc::UNKNOWN,
            wid: "",
            classes: &[],
        }
    }

    /// Stamp source provenance. A no-op unless `feature = "observe"`, so
    /// derive-emitted code calls it unconditionally at zero release cost.
    #[allow(unused_mut, unused_variables)]
    pub const fn with_src(mut self, file: &'static str, line: u32) -> Self {
        #[cfg(feature = "observe")]
        {
            self.src = SrcLoc { file, line };
        }
        self
    }

    /// Stamp an author id.
    pub const fn with_id(mut self, id: &'static str) -> Self {
        self.wid = id;
        self
    }

    /// Stamp style classes.
    pub const fn with_classes(mut self, classes: &'static [&'static str]) -> Self {
        self.classes = classes;
        self
    }

    #[inline]
    pub const fn is_rigid(&self) -> bool {
        self.flags & Self::RIGID != 0
    }

    #[inline]
    pub const fn is_container(&self) -> bool {
        self.flags & Self::CONTAINER != 0
    }

    #[inline]
    pub const fn is_focusable(&self) -> bool {
        self.flags & Self::FOCUSABLE != 0
    }

    /// Whether a press on this node leaves focus alone —
    /// [`NodeDesc::KEEPS_FOCUS`].
    pub const fn keeps_focus(&self) -> bool {
        self.flags & Self::KEEPS_FOCUS != 0
    }
}

/// `parent` value of a tree root.
pub const NO_PARENT: u32 = u32::MAX;

/// Deepest recursion the framework accepts. A tree deeper than this
/// fails to compile (the derive asserts `DEPTH <= MAX_DEPTH`) — with
/// [`crate::hatch::Dyn`] the documented exception, since type erasure
/// hides its content's depth from that arithmetic.
///
/// **This is a runaway-recursion backstop, not a stack budget**, and
/// the two were conflated until 2026-09-07. It exists so a tree that
/// could overflow the stack does not compile; it says nothing about how
/// much stack a *given* tree uses, because that depends on the tree's
/// actual `DEPTH` and not on this cap. At [`stack_bytes`]'s rate a
/// 64-deep tree would want ~18 KB, which is nothing on a desktop and
/// twice `docs/results-embedded.md`'s whole 8 KB budget — so reading
/// this constant as the embedded budget would be wrong in both
/// directions.
///
/// The budget is enforced separately, by [`stack_bytes`] against a
/// tree's own `DEPTH`. `fixtures/embedded` does that at compile time.
pub const MAX_DEPTH: usize = 64;

/// Stack a single level of recursive descent costs, in bytes — the
/// model's **slope**.
///
/// **256–272 B measured at T13.0, and this is the upper end**, recorded
/// in `docs/results-embedded.md`. The worst end deliberately: a budget
/// checked against the optimistic figure is not a budget. M52/T52.6's
/// depth sweep under qemu measured a smaller slope for the fixture's
/// tree, so 272 still bounds it (the figures are in that doc's T52.6
/// section, not here).
///
/// **A level's frame does vary with the tree.** It is the widget's own
/// locals plus the call, and a widget with a large local — the review's
/// 4 KB array in `TextInput::layout` — costs what it costs at whatever
/// depth it sits. So this is a model to assert a *shape* against, not a
/// measurement; `just stack` measures.
pub const STACK_BYTES_PER_LEVEL: usize = 272;

/// Stack a frame costs before any level of the tree — the model's
/// **intercept**, M52/T52.6 (R11-F3).
///
/// The model was `depth × STACK_BYTES_PER_LEVEL` alone, and the real
/// stack exceeded it at every depth: a frame also pays for the layout
/// and paint entry points, the measurer and the painter's own calls,
/// none of which is a level. Fitted from the depth sweep in
/// `docs/results-embedded.md` (M52/T52.6), whose largest residual over
/// the slope is rounded up for toolchain headroom; `just stack` fails if
/// a release frame stops fitting under the model.
pub const STACK_BASE_BYTES: usize = 512;

/// Stack a tree of `depth` levels costs, in bytes: a base plus a slope
/// per level, **in a release build** (the dev profile's frames are
/// several times larger; `docs/results-embedded.md` records both).
///
/// `const`, so an application can assert its own budget at compile time
/// rather than discovering an overflow on a device:
///
/// ```
/// use lumen_static::desc::stack_bytes;
/// # struct Ui;
/// # impl Ui { const DEPTH: usize = 5; }
/// // A real screen is 4-5 deep; 4 KB is the embedded budget.
/// const _: () = assert!(stack_bytes(Ui::DEPTH) <= 4096);
/// ```
///
/// **A backstop on depth, not a measurement of stack.** It cannot see a
/// widget whose own frame grew, because `DEPTH` does not change; that is
/// what `just stack`'s qemu run is for. The number that matters here is
/// a tree's **actual** `DEPTH`, not [`MAX_DEPTH`] — which is why this
/// takes one.
pub const fn stack_bytes(depth: usize) -> usize {
    STACK_BASE_BYTES + depth * STACK_BYTES_PER_LEVEL
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Constraints {
    pub min_w: f32,
    pub max_w: f32,
    pub min_h: f32,
    pub max_h: f32,
}

impl Constraints {
    pub const fn tight(w: f32, h: f32) -> Self {
        Constraints { min_w: w, max_w: w, min_h: h, max_h: h }
    }

    pub const fn loose(w: f32, h: f32) -> Self {
        Constraints { min_w: 0.0, max_w: w, min_h: 0.0, max_h: h }
    }

    #[inline]
    pub fn clamp(&self, s: Size) -> Size {
        Size {
            w: crate::num::clamp(s.w, self.min_w, self.max_w),
            h: crate::num::clamp(s.h, self.min_h, self.max_h),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Per-node layout output, one slot per table entry — the
/// `[NodeState; NODE_COUNT]` from handoff §3.
#[derive(Clone, Copy, Debug, Default)]
pub struct NodeState {
    pub constraints: Constraints,
    pub size: Size,
    pub pos: Point,
    /// Resolved paint attributes, stamped from `Style` during layout.
    ///
    /// `paint` and `hit` do not take `&[Style]` — only `layout` and
    /// `place` do — so a widget cannot read its own colour at paint time.
    /// Rather than thread a fourth parameter through every `Node` method
    /// and every hatch (the change A2 of
    /// `docs/text-and-scrolling-decisions.md` first proposed and then
    /// rejected), the cascade's answer is copied into the state array
    /// the paint pass already carries.
    pub fg: crate::display::Color,
    pub bg: crate::display::Color,
    pub radius: f32,
    pub font_size: f32,
    pub font_weight: f32,
    pub shadow: f32,
    /// Resolved padding as (left, top, right, bottom), so `paint` can
    /// inset content without `&[Style]`.
    pub pad: (f32, f32, f32, f32),
    /// The pointer is down on this node (C6).
    ///
    /// Set by `dispatch`, read by `record_painted` when it resolves the
    /// paint attributes, so a pressed control looks different without a
    /// second style array or a per-frame cascade. It lives in the state
    /// array rather than in `Style` because it is per *node* and per
    /// frame, where a style row is shared by every instance of a
    /// template.
    pub pressed: bool,
    /// The pointer is over this node (C6).
    pub hovered: bool,
    /// This node holds keyboard focus.
    ///
    /// Set by `dispatch` beside `pressed`/`hovered`, for the same
    /// reason: `paint` has no access to the host's focus, and a text
    /// field that cannot tell whether it is focused cannot decide
    /// whether to draw a caret.
    pub focused: bool,
    /// Ring width when focused, resolved from `Style::focus_ring`
    /// (T15.6).
    ///
    /// Here as well as in `Style` because `paint` has no `&[Style]` —
    /// the same reason `fg`, `bg` and `radius` are resolved here (C6).
    /// A `u8`, so it lands in the padding beside the three flags and
    /// `NodeState` stays 76 bytes; `fixtures/embedded` asserts that
    /// against the RAM budget in `docs/results-embedded.md`.
    pub focus_ring: u8,
    /// The resolved border, carried here for the same reason `bg` and
    /// `radius` are: `paint` has no `&[Style]` and must not re-cascade
    /// (M27/T27.1).
    ///
    /// **This is the second eight bytes the border costs**, and it was
    /// not free either: the tail above — three `bool`s and `focus_ring`
    /// — fills one 4-byte slot exactly, so `border_width` cost four and
    /// `border_color` four more. `docs/results-embedded.md` has the
    /// measurement; packing the three `bool`s into a flags byte would
    /// give half of it back and is a named follow-up rather than
    /// something to do quietly inside a widget milestone.
    pub border_color: crate::display::Color,
    pub border_width: u8,
    /// `Style::disabled`, resolved — M57/A16. The pass reads it (with
    /// every ancestor's) to refuse hits, focus and activation. In the
    /// struct's tail padding, so `NodeState` stays 84 bytes.
    pub disabled: bool,
    /// `Style::line_height`, resolved — M80/T80.2: sixteenths of
    /// `font_size`, 0 for the measurer's own spacing. Here for the reason
    /// `font_size` is: `paint` has no `&[Style]`, and a `Label` hands this
    /// byte to `Painter::text_spaced`. One of the two spare tail bytes, so
    /// `NodeState` stays 84 (`tests/cursor.rs`); one is left.
    pub line_height: u8,
}

/// Scalar facts a kind knows about itself that geometry cannot show —
/// the collection vocabulary, mirroring AccessKit's `size_of_set`.
#[cfg(feature = "observe")]
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct NodeStats {
    /// Logical item count for collection-like widgets (a repeater's rows).
    pub items: Option<u64>,
    /// Materialized instances, for virtualized widgets.
    pub materialized: Option<u32>,
    /// Px per item, for virtualized widgets that place by extent.
    ///
    /// Reported because the offset arithmetic that selects a row is
    /// `f32`, and whether every row is *reachable* depends on this
    /// against the row count (`Repeat::MAX_ADDRESSABLE_EXTENT`, lint
    /// `W1009`). Geometry alone cannot answer it.
    pub item_extent: Option<f32>,
}
