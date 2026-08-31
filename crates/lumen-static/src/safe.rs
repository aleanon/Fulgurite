//! A fully-safe variant of the tree and passes — prototype for
//! `docs/verification-notes.md`'s central question: is the `base +
//! offset` unsafe needed, or is it a performance choice?
//!
//! The idea: keep everything that makes this architecture distinctive —
//! the const table, const `NODE_COUNT`, zero allocation, flat iterative
//! passes — and replace only the *erasure mechanism*. Instead of erasing
//! a widget to `*const u8` and recovering it with a cast, the derive
//! emits a safe accessor that returns `&dyn Node`. A `&dyn` is a fat
//! pointer into storage that already exists: no `Box`, no arena, no
//! allocation.
//!
//! What this deletes, beyond the `unsafe` itself:
//! - **`KIND_LIST`**: the vtable arrives with the `&dyn` coercion, and
//!   rustc already dedups vtables per type — so §5's "one body per
//!   widget *kind*, not per tree *position*" is guaranteed by the
//!   language rather than by our splicing.
//! - **`kind: u16` from `NodeDesc`** (this prototype keeps it, to reuse
//!   the identical table and isolate one variable in the benchmark).
//! - **`RefCell` in `Repeat`**: interior mutability exists only because
//!   ops receive a *shared* base pointer; a `node_mut` accessor lets
//!   layout mutate a window directly. Not exercised here — `Repeat` is
//!   out of scope for the prototype.
//!
//! Fusion is evidence this style works: `fuse.rs` already contains zero
//! `unsafe`, because a fused subtree is monomorphized and calls its
//! fields directly. This module generalizes that to every seam.

use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size};

/// A widget's own behavior — the safe counterpart of `WidgetOps`, except
/// there is no table of these: the vtable rides along with `&dyn Node`.
pub trait Node {
    /// Containers only: write every child's constraints.
    fn down(&self, _table: &[NodeDesc], _st: &mut [NodeState], _i: usize) {}
    /// This node's size.
    fn up(&self, table: &[NodeDesc], st: &[NodeState], i: usize) -> Size;
    /// Containers only: write every child's position.
    fn place(&self, _table: &[NodeDesc], _st: &mut [NodeState], _i: usize) {}
    /// Fold paint output (display-list stand-in, as in the unsafe path).
    fn paint(&self, d: &NodeDesc, st: &NodeState, out: &mut u64);
}

/// Addressing: map a table index to the widget living at it.
///
/// The derive generates `node` as a range-delegating match over fields —
/// the same information it uses for `offset_of!` in the unsafe path,
/// spent on a jump table instead of pointer arithmetic.
pub trait Tree {
    const TABLE: &'static [NodeDesc];
    const COUNT: usize;
    fn node(&self, i: usize) -> &dyn Node;
}

/// Layout, identical in structure to `pass::layout` — three stack-free
/// loops over the same const table. Only the accessor differs.
pub fn layout<T: Tree + ?Sized>(tree: &T, st: &mut [NodeState], root_c: Constraints) {
    let table = T::TABLE;
    debug_assert_eq!(table.len(), st.len());

    st[0].constraints = root_c;
    for (i, d) in table.iter().enumerate() {
        if d.is_container() {
            tree.node(i).down(table, st, i);
        }
    }
    for i in (0..table.len()).rev() {
        let s = tree.node(i).up(table, st, i);
        st[i].size = s;
    }
    st[0].pos = Point::default();
    for (i, d) in table.iter().enumerate() {
        if d.is_container() {
            tree.node(i).place(table, st, i);
        }
    }
}

pub fn paint<T: Tree + ?Sized>(tree: &T, st: &[NodeState], out: &mut u64) {
    for (i, d) in T::TABLE.iter().enumerate() {
        tree.node(i).paint(d, &st[i], out);
    }
}

// ---- leaf impls (compare with the unsafe versions in `ops.rs`) --------

use crate::ops::{fold_geom_pub as fold_geom, fold_pub as fold, Fixed, Label, Spacer};

impl Node for Label {
    fn up(&self, _t: &[NodeDesc], st: &[NodeState], i: usize) -> Size {
        st[i].constraints.clamp(Size { w: 8.0 * self.text.len() as f32, h: 16.0 })
    }
    fn paint(&self, d: &NodeDesc, st: &NodeState, out: &mut u64) {
        fold_geom(out, d, st);
        for &b in self.text.as_bytes() {
            fold(out, b as u64);
        }
    }
}

impl Tree for Label {
    const TABLE: &'static [NodeDesc] = <Self as crate::Widget>::SUB_TABLE;
    const COUNT: usize = 1;
    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
}

impl Node for Fixed {
    fn up(&self, _t: &[NodeDesc], st: &[NodeState], i: usize) -> Size {
        st[i].constraints.clamp(Size { w: self.w, h: self.h })
    }
    fn paint(&self, d: &NodeDesc, st: &NodeState, out: &mut u64) {
        fold_geom(out, d, st);
        fold(out, self.w.to_bits() as u64 ^ self.h.to_bits() as u64);
    }
}

impl Tree for Fixed {
    const TABLE: &'static [NodeDesc] = <Self as crate::Widget>::SUB_TABLE;
    const COUNT: usize = 1;
    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
}

impl Node for Spacer {
    fn up(&self, _t: &[NodeDesc], st: &[NodeState], i: usize) -> Size {
        Size { w: st[i].constraints.max_w, h: st[i].constraints.max_h }
    }
    fn paint(&self, d: &NodeDesc, st: &NodeState, out: &mut u64) {
        fold_geom(out, d, st);
    }
}

impl Tree for Spacer {
    const TABLE: &'static [NodeDesc] = <Self as crate::Widget>::SUB_TABLE;
    const COUNT: usize = 1;
    fn node(&self, _i: usize) -> &dyn Node {
        self
    }
}

// ---- runtime assembly, safely (§8's concat-at-startup path) ----------

/// The synthetic column root that `Assembled` uses, as an ordinary value.
#[derive(Default)]
pub struct VirtualColumn;

impl Node for VirtualColumn {
    fn down(&self, t: &[NodeDesc], st: &mut [NodeState], i: usize) {
        crate::ops::col_down(t, st, i, 0.0)
    }
    fn up(&self, t: &[NodeDesc], st: &[NodeState], i: usize) -> Size {
        crate::ops::col_up(t, st, i, 0.0)
    }
    fn place(&self, t: &[NodeDesc], st: &mut [NodeState], i: usize) {
        crate::ops::col_place(t, st, i, 0.0)
    }
    fn paint(&self, d: &NodeDesc, st: &NodeState, out: &mut u64) {
        fold_geom(out, d, st);
    }
}

/// A homogeneous slab of components under one virtual root — the safe
/// counterpart of `Assembled<C>`. Instance addressing is a division by a
/// *constant* (`C::COUNT`), which compiles to a multiply-shift, where the
/// unsafe path uses a precomputed byte offset.
pub struct Slab<C: Tree + 'static> {
    pub root: VirtualColumn,
    pub items: Vec<C>,
    table: Vec<NodeDesc>,
}

impl<C: Tree + Default + 'static> Slab<C> {
    /// Builds the same table `Assembled::new` builds, so the two arms
    /// iterate byte-identical structure.
    pub fn new(count: usize) -> Self {
        // Same builder the unsafe path uses, with stride 0: instances are
        // addressed by index here, so byte offsets are unused.
        let table = crate::assemble::concat_table(count, C::COUNT, C::TABLE, 0);
        Slab { root: VirtualColumn, items: (0..count).map(|_| C::default()).collect(), table }
    }

    pub fn table(&self) -> &[NodeDesc] {
        &self.table
    }

    pub fn node_at(&self, i: usize) -> &dyn Node {
        if i == 0 {
            return &self.root;
        }
        let j = i - 1;
        self.items[j / C::COUNT].node(j % C::COUNT)
    }
}

/// `Slab` cannot implement [`Tree`] (its table is runtime-built, not a
/// const), so the passes are duplicated for it — the same three loops.
pub fn layout_slab<C: Tree + Default + 'static>(
    slab: &Slab<C>,
    st: &mut [NodeState],
    root_c: Constraints,
) {
    let table = slab.table();
    st[0].constraints = root_c;
    for (i, d) in table.iter().enumerate() {
        if d.is_container() {
            slab.node_at(i).down(table, st, i);
        }
    }
    for i in (0..table.len()).rev() {
        let s = slab.node_at(i).up(table, st, i);
        st[i].size = s;
    }
    st[0].pos = Point::default();
    for (i, d) in table.iter().enumerate() {
        if d.is_container() {
            slab.node_at(i).place(table, st, i);
        }
    }
}

pub fn paint_slab<C: Tree + Default + 'static>(slab: &Slab<C>, st: &[NodeState], out: &mut u64) {
    for (i, d) in slab.table().iter().enumerate() {
        slab.node_at(i).paint(d, &st[i], out);
    }
}

// ---- resolved index: pay the accessor once, not once per pass --------

/// The naive safe passes call `node(i)` in every phase — four walks of
/// the delegation chain per frame. Resolving the tree into a slice of
/// `&dyn Node` once makes every subsequent lookup O(1), at the cost of
/// one fat pointer (16 bytes) per node.
///
/// For a static tree the resolved index is only invalidated when the
/// tree *moves* in memory, so a host can build it once rather than per
/// frame — the benchmark rebuilds it per frame anyway, which is the
/// pessimistic reading.
pub fn resolve<T: Tree + ?Sized>(tree: &T) -> Vec<&dyn Node> {
    (0..T::TABLE.len()).map(|i| tree.node(i)).collect()
}

pub fn resolve_slab<C: Tree + Default + 'static>(slab: &Slab<C>) -> Vec<&dyn Node> {
    (0..slab.table().len()).map(|i| slab.node_at(i)).collect()
}

pub fn layout_indexed(
    nodes: &[&dyn Node],
    table: &[NodeDesc],
    st: &mut [NodeState],
    root_c: Constraints,
) {
    st[0].constraints = root_c;
    for (i, d) in table.iter().enumerate() {
        if d.is_container() {
            nodes[i].down(table, st, i);
        }
    }
    for i in (0..table.len()).rev() {
        let s = nodes[i].up(table, st, i);
        st[i].size = s;
    }
    st[0].pos = Point::default();
    for (i, d) in table.iter().enumerate() {
        if d.is_container() {
            nodes[i].place(table, st, i);
        }
    }
}

pub fn paint_indexed(nodes: &[&dyn Node], table: &[NodeDesc], st: &[NodeState], out: &mut u64) {
    for (i, d) in table.iter().enumerate() {
        nodes[i].paint(d, &st[i], out);
    }
}

// ---- recursive descent: iced's traversal, for comparison -------------

/// The alternative traversal: a container lays out its children by
/// **calling them**, the way iced and Flutter do, instead of the flat
/// loops above. Each node is visited exactly once — the parent gets the
/// child's true final size back from the call, so nothing has to be
/// estimated and corrected by a later pass.
///
/// Children are reached through `&dyn RNode`, deliberately: a *static*
/// call to a concrete field would monomorphize the whole subtree, which
/// is `#[fuse]` and already measured. This arm keeps code O(kinds).
///
/// Note what is absent: layout reads **no `NodeDesc` at all**. The
/// structure lives in the types and the child state offsets are consts,
/// so the table is only needed by the agent surface, not by the passes.
pub trait RNode {
    /// Lay out this subtree under `limits`, writing into `st` starting at
    /// index `i`, and return this node's size.
    fn layout(&self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size;
    /// Position this subtree with its origin at `at`.
    fn place(&self, at: Point, st: &mut [NodeState], i: usize);
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64);
}

#[inline]
fn r_leaf_paint(st: &[NodeState], i: usize, out: &mut u64, extra: u64) {
    let s = &st[i];
    fold(out, extra);
    fold(out, s.size.w.to_bits() as u64 ^ ((s.size.h.to_bits() as u64) << 32));
    fold(out, s.pos.x.to_bits() as u64 ^ ((s.pos.y.to_bits() as u64) << 32));
}

impl RNode for Label {
    fn layout(&self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size {
        let s = limits.clamp(Size { w: 8.0 * self.text.len() as f32, h: 16.0 });
        st[i].constraints = limits;
        st[i].size = s;
        s
    }
    fn place(&self, at: Point, st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
        r_leaf_paint(st, i, out, 1);
        for &b in self.text.as_bytes() {
            fold(out, b as u64);
        }
    }
}

impl RNode for Fixed {
    fn layout(&self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size {
        let s = limits.clamp(Size { w: self.w, h: self.h });
        st[i].constraints = limits;
        st[i].size = s;
        s
    }
    fn place(&self, at: Point, st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
        r_leaf_paint(st, i, out, 2);
        fold(out, self.w.to_bits() as u64 ^ self.h.to_bits() as u64);
    }
}

impl RNode for Spacer {
    fn layout(&self, limits: Constraints, st: &mut [NodeState], i: usize) -> Size {
        let s = Size { w: limits.max_w, h: limits.max_h };
        st[i].constraints = limits;
        st[i].size = s;
        s
    }
    fn place(&self, at: Point, st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut u64) {
        r_leaf_paint(st, i, out, 3);
    }
}

/// Recursive counterpart of [`Slab`]: a virtual column over instances.
pub struct RSlab<C> {
    pub items: Vec<C>,
    pub count: usize,
}

impl<C: RNode + Tree + Default> RSlab<C> {
    pub fn new(count: usize) -> Self {
        RSlab { items: (0..count).map(|_| C::default()).collect(), count }
    }

    pub fn layout(&self, limits: Constraints, st: &mut [NodeState]) -> Size {
        let n = self.items.len().max(1) as f32;
        let cc = Constraints {
            min_w: 0.0,
            max_w: limits.max_w,
            min_h: 0.0,
            max_h: (limits.max_h / n).max(0.0),
        };
        let mut sum = 0.0f32;
        let mut mx = 0.0f32;
        let mut at = 1usize;
        for item in &self.items {
            let d: &dyn RNode = item;
            let s = d.layout(cc, st, at);
            sum += s.h;
            mx = mx.max(s.w);
            at += C::COUNT;
        }
        let s = limits.clamp(Size { w: mx, h: sum });
        st[0].size = s;
        s
    }

    pub fn place(&self, st: &mut [NodeState]) {
        st[0].pos = Point::default();
        let mut cur = Point::default();
        let mut at = 1usize;
        for item in &self.items {
            let d: &dyn RNode = item;
            d.place(cur, st, at);
            cur.y += st[at].size.h;
            at += C::COUNT;
        }
    }

    pub fn paint(&self, st: &[NodeState], out: &mut u64) {
        r_leaf_paint(st, 0, out, 0);
        let mut at = 1usize;
        for item in &self.items {
            let d: &dyn RNode = item;
            d.paint(st, at, out);
            at += C::COUNT;
        }
    }
}
