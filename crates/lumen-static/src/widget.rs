//! The `Widget` trait and the const splicing helpers the derive builds
//! tables with.
//!
//! Everything here is implemented on *concrete* types only (leaves by hand,
//! containers via `#[derive(StaticTree)]`), so all associated consts have
//! concrete values and no unstable `generic_const_exprs` is needed. Generic
//! container sugar (`Row<(A, B)>`) is deferred DSL work — see
//! `docs/step2-design-notes.md`.

use crate::desc::{NodeDesc, WidgetOps};

pub trait Widget {
    /// Nodes in this widget's subtree, itself included (handoff §3).
    const NODE_COUNT: usize;
    /// Entries this subtree contributes to the kind list (with duplicates;
    /// deduplication is a later size optimization, correctness never needs it).
    const KIND_COUNT: usize;
    /// Size provably independent of content (handoff §2's relayout boundary).
    const IS_RIGID: bool;
    /// This subtree's descriptors: indices relative to the subtree (self at
    /// 0, `parent == NO_PARENT` on self), offsets relative to `&self`,
    /// kinds relative to `KIND_LIST`.
    const SUB_TABLE: &'static [NodeDesc];
    /// The kinds `SUB_TABLE` indexes into, self's own ops first.
    const KIND_LIST: &'static [&'static WidgetOps];
}

/// Copy a child's `SUB_TABLE` into a parent table being built, rebasing
/// node indices by `write_at`, byte offsets by `byte_base`, and kind
/// indices by `kind_base`. The child's root is re-parented to
/// `parent_index`. Const, so derives run it at compile time; runtime
/// assembly (handoff §8's concat-at-startup path) calls the same fn.
pub const fn splice_child(
    dst: &mut [NodeDesc],
    write_at: usize,
    src: &[NodeDesc],
    parent_index: u32,
    byte_base: u32,
    kind_base: u16,
) {
    let mut j = 0;
    while j < src.len() {
        let mut d = src[j];
        d.offset += byte_base;
        d.kind += kind_base;
        if d.child_count > 0 {
            d.first_child += write_at as u32;
        }
        if d.next_sibling != 0 {
            d.next_sibling += write_at as u32;
        }
        d.parent = if j == 0 {
            parent_index
        } else {
            d.parent + write_at as u32
        };
        dst[write_at + j] = d;
        j += 1;
    }
    // The spliced root's sibling link (to the parent's next field) is the
    // caller's to set — it is 0 (last child) until then.
    dst[write_at].next_sibling = 0;
}

/// Copy a child's `KIND_LIST` into a parent kind list being built.
pub const fn splice_kinds(
    dst: &mut [&'static WidgetOps],
    write_at: usize,
    src: &[&'static WidgetOps],
) {
    let mut j = 0;
    while j < src.len() {
        dst[write_at + j] = src[j];
        j += 1;
    }
}

/// Number of table entries with (or without) the rigid flag — sizes the
/// partition arrays below.
pub const fn count_partition(table: &[NodeDesc], rigid: bool) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < table.len() {
        if table[i].is_rigid() == rigid {
            n += 1;
        }
        i += 1;
    }
    n
}

/// The rigid/flexible index partition (handoff §5: "a compile-time
/// partition beats a runtime branch"). `N` comes from [`count_partition`]
/// at the concrete call site. Indices stay in table (DFS) order, which the
/// partitioned passes rely on.
pub const fn collect_partition<const N: usize>(table: &[NodeDesc], rigid: bool) -> [u32; N] {
    let mut out = [0u32; N];
    let mut n = 0;
    let mut i = 0;
    while i < table.len() {
        if table[i].is_rigid() == rigid {
            out[n] = i as u32;
            n += 1;
        }
        i += 1;
    }
    assert!(n == N, "partition size mismatch");
    out
}
