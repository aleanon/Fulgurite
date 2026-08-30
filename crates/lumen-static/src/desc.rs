//! The flat node table: descriptors, vtables, and per-node runtime state.
//!
//! `NodeDesc` follows the handoff §5 sketch with two additions, both
//! required by the stack-free passes and recorded in
//! `docs/step2-design-notes.md`:
//! - `parent`, so reverse/forward loops can relate a node to its parent
//!   without a stack;
//! - `next_sibling`, so a container op can walk exactly its own children
//!   (a child's subtree intervenes in DFS order, so `first_child + k` does
//!   not address child `k`).
//!
//! Dispatch granularity (learned from the first-cut benchmark, see
//! `docs/results-step2-crossover.md`): ops are called once per *container*
//! per phase and loop their children inline, not once per child — per-child
//! dispatch tripled the indirect-call count and lost to naive dyn recursion.

/// One node of the tree, in DFS pre-order (parent before children).
///
/// Indices (`first_child`, `parent`, `next_sibling`) index the table the
/// entry lives in; `offset` is the byte offset of the node's *value* struct
/// from the tree base pointer. `kind` indexes the kind list assembled
/// alongside the table. `next_sibling == 0` means "last child" (slot 0 is
/// always the root, which is nobody's sibling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeDesc {
    pub offset: u32,
    pub kind: u16,
    pub first_child: u32,
    pub child_count: u16,
    pub parent: u32,
    pub next_sibling: u32,
    pub flags: u8,
}

impl NodeDesc {
    pub const RIGID: u8 = 1 << 0;
    /// The node's kind participates in the down and place phases. Set for
    /// derived containers with children and for repeaters (which have no
    /// *table* children but place their materialized instances).
    pub const CONTAINER: u8 = 1 << 1;

    /// Placeholder for const table construction; every slot is overwritten.
    pub const EMPTY: NodeDesc = NodeDesc {
        offset: 0,
        kind: 0,
        first_child: 0,
        child_count: 0,
        parent: NO_PARENT,
        next_sibling: 0,
        flags: 0,
    };

    #[inline]
    pub const fn is_rigid(&self) -> bool {
        self.flags & Self::RIGID != 0
    }

    #[inline]
    pub const fn is_container(&self) -> bool {
        self.flags & Self::CONTAINER != 0
    }
}

/// `parent` value of a tree root.
pub const NO_PARENT: u32 = u32::MAX;

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

/// Per-node mutable state, one slot per table entry, in a parallel array —
/// the `[NodeState; NODE_COUNT]` from handoff §3. Kept to 32 bytes: the
/// passes stream this array, so its width is bandwidth.
#[derive(Clone, Copy, Debug, Default)]
pub struct NodeState {
    pub constraints: Constraints,
    pub size: Size,
    pub pos: Point,
}

/// One entry per widget *kind* (handoff §5: `KIND_COUNT`, not tree
/// positions). All fns receive the tree base pointer and locate their value
/// struct at `base + desc.offset`. Container fns walk their children via
/// `first_child`/`next_sibling` — one dispatch per container per phase.
///
/// # Safety contract
/// A `WidgetOps` may only be dispatched against a `(base, desc)` pair
/// produced by the same derive/assembly that produced the ops entry: the
/// fns cast `base + desc.offset` to their concrete widget type.
pub struct WidgetOps {
    pub name: &'static str,
    /// Containers only (`child_count > 0`): write every child's
    /// constraints, derived from `st[i].constraints`.
    pub down: fn(base: *const u8, table: &[NodeDesc], st: &mut [NodeState], i: usize),
    /// The node's size. Containers read their children's already-computed
    /// sizes from `st`; leaves read their value struct.
    pub up: fn(base: *const u8, table: &[NodeDesc], st: &[NodeState], i: usize) -> Size,
    /// Containers only: write every child's position from `st[i].pos`.
    pub place: fn(base: *const u8, table: &[NodeDesc], st: &mut [NodeState], i: usize),
    /// Fold paint-relevant output into a checksum (display-list stand-in).
    pub paint: fn(base: *const u8, d: &NodeDesc, st: &NodeState, out: &mut u64),
}

/// Shared no-op for leaf `down`/`place` slots (never dispatched: the passes
/// gate on `child_count > 0`).
pub fn leaf_nop(_b: *const u8, _t: &[NodeDesc], _st: &mut [NodeState], _i: usize) {}
