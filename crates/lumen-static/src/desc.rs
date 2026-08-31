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
/// Purely structural now: who the parent is, where the children start,
/// and who the next sibling is. 16 bytes in a release build.
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
}

/// `parent` value of a tree root.
pub const NO_PARENT: u32 = u32::MAX;

/// Deepest recursion the framework accepts. A tree deeper than this
/// fails to compile (the derive asserts `DEPTH <= MAX_DEPTH`), which is
/// how §5's stack-depth concern is retired: it becomes a fact about a
/// constant rather than a runtime hazard.
pub const MAX_DEPTH: usize = 64;

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
            w: s.w.clamp(self.min_w, self.max_w),
            h: s.h.clamp(self.min_h, self.max_h),
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
}
