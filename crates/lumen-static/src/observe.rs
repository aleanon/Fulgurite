//! The §9 observability surface: the tree exposed as an AccessKit tree,
//! plus a JSON dump carrying source provenance for agents and tooling.
//!
//! One implementation serves screen readers and coding agents alike — no
//! bespoke agent channel. Node identity is positional and survives hot
//! reload: main-table nodes are `table index + 1`; repeater instances get
//! row-stable synthetic ids, so "the third row" keeps its id across
//! scrolls *and* dylib swaps.
//!
//! Action synthesis (§9's "synthesize events") arrived with the event
//! system: interactive kinds advertise their `accesskit::Action`s here,
//! and the agent synthesizes them as ordinary queued input.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub use accesskit;

pub use accesskit::{Node as AkNode, NodeId};
/// For derive- and macro-emitted walks (M35/T35.1): they expand in the
/// user's crate, which may be `no_std` even when `observe` is on in a
/// unified workspace build, so they name `Vec` through this crate.
pub use alloc::vec::Vec as ObsVec;
use accesskit::{Rect, Role, TreeId, TreeInfo, TreeUpdate};

use crate::desc::{NodeDesc, NodeState};
use crate::widget::{Node, Widget};

fn bounds_of(st: &NodeState) -> Rect {
    Rect {
        x0: st.pos.x as f64,
        y0: st.pos.y as f64,
        x1: (st.pos.x + st.size.w) as f64,
        y1: (st.pos.y + st.size.h) as f64,
    }
}

/// Emit a subtree's nodes into `out`, with ids assigned by `id_of`.
///
/// The walker pre-fills role `GenericContainer`, absolute bounds and
/// table children; each widget's `observe` then overrides what it knows
/// better and may push extra subtrees (a repeater's window).
///
/// **Statically dispatched where the widget has a walk of its own** —
/// M35/T35.1 (`docs/plan-observe-cost.md`). `List` and `Repeat` call this
/// once per row, and the table walk below reaches each row as a
/// `&dyn Node`: the coercion builds the row widget's vtable, the vtable
/// takes the addresses of its `layout` and `paint`, and LLVM stops
/// inlining them into the container's per-row loop — the whole of
/// `observe`'s compiled-in cost (`docs/results-competitive.md` §7). A
/// widget that answers [`Node::observe_walk`] is never coerced.
pub fn emit_subtree<W: Widget>(
    tree: &W,
    st: &[NodeState],
    id_of: &dyn Fn(usize) -> u64,
    out: &mut Vec<(NodeId, AkNode)>,
) {
    // **One unconditional call.** The first version fell back to the
    // table walk when `observe_walk` answered `false`, and the fallback's
    // `&dyn Node` coercion — in a branch that is dead for any widget with
    // a walk — was enough to keep `Label::paint` address-taken in the
    // `List` probe (`just observe-inlining`). The fallback now lives in
    // `observe_walk`'s default body, which is never compiled for a type
    // that overrides it.
    tree.observe_walk(W::SUB_TABLE, st, id_of, out);
}

/// The whole tree, from its root: the static walk if the root has one,
/// the table walk otherwise — M35/T35.1. What the agent calls, so the
/// agent and `snapshot` walk one way.
pub fn emit_root(
    table: &[NodeDesc],
    root: &dyn Node,
    st: &[NodeState],
    id_of: &dyn Fn(usize) -> u64,
    out: &mut Vec<(NodeId, AkNode)>,
) {
    root.observe_walk(table, st, id_of, out);
}

/// One node's entry: bounds, the hidden flag, its table children, then
/// `observe`, which may push extra subtrees first. Shared by the table
/// walk and every static walk, so the two cannot drift apart.
pub fn emit_node(
    table: &[NodeDesc],
    st: &[NodeState],
    i: usize,
    id_of: &dyn Fn(usize) -> u64,
    out: &mut Vec<(NodeId, AkNode)>,
    observe: impl FnOnce(&mut AkNode, &mut Vec<(NodeId, AkNode)>),
) {
    let d = &table[i];
    let mut n = AkNode::new(Role::GenericContainer);
    n.set_bounds(bounds_of(&st[i]));
    // A node with no area has nothing to announce, and a subtree that is
    // *absent* — an empty `Option`, the hidden branch of an `Either`, a
    // node styled `visible: false` — is expressed exactly that way:
    // `hide_subtree` zeroes it every frame. Left unmarked, a screen
    // reader walks into the branch the user is not looking at and reads
    // it out. `set_hidden` keeps the node in the tree, so nothing is
    // orphaned, and tells the reader to skip it. Added 2026-09-04 with
    // `Either`'s merged table (T8.0), which put a second branch on the
    // table for the first time — but `Option` had the same hole since M4.
    if st[i].size.w <= 0.0 || st[i].size.h <= 0.0 {
        n.set_hidden();
    }
    if d.child_count > 0 {
        let mut c = d.first_child as usize;
        loop {
            n.push_child(NodeId(id_of(c)));
            if table[c].next_sibling == 0 {
                break;
            }
            c = table[c].next_sibling as usize;
        }
    }
    observe(&mut n, out);
    out.push((NodeId(id_of(i)), n));
}

/// Same, for a table that is not a widget's own `SUB_TABLE` (runtime
/// assembly concatenates one).
pub fn emit_table(
    table: &[NodeDesc],
    tree: &dyn Node,
    st: &[NodeState],
    id_of: &dyn Fn(usize) -> u64,
    out: &mut Vec<(NodeId, AkNode)>,
) {
    for i in 0..table.len() {
        emit_node(table, st, i, id_of, out, |n, o| tree.node(i).observe(st, i, id_of(i), n, o));
    }
}

/// Full accessibility snapshot, ready for an AccessKit adapter or agent.
///
/// Focus is reported as the root. A shell that tracks focus should call
/// [`snapshot_focused`] instead — a screen reader follows the `focus`
/// field, so reporting the root means "nothing is focused" no matter
/// what the app believes.
pub fn snapshot<W: Widget>(tree: &W, st: &[NodeState]) -> TreeUpdate {
    snapshot_focused(tree, st, None)
}

/// [`snapshot`], with the focused node's main-table index.
///
/// `None`, or an index past the table, reports the root — the same
/// "nothing focused" a shell without focus tracking gives. Node ids are
/// `index + 1` because AccessKit reserves no id but 0 is a poor
/// sentinel; the root of the emitted tree is id 1.
pub fn snapshot_focused<W: Widget>(
    tree: &W,
    st: &[NodeState],
    focus: Option<u32>,
) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(W::SUB_TABLE.len());
    emit_subtree(tree, st, &|i| (i + 1) as u64, &mut nodes);
    let focus = match focus {
        Some(i) if (i as usize) < W::SUB_TABLE.len() => NodeId(i as u64 + 1),
        _ => NodeId(1),
    };
    TreeUpdate { nodes, tree: Some(TreeInfo::new(NodeId(1))), tree_id: TreeId::ROOT, focus }
}

/// The main-table index an AccessKit [`NodeId`] refers to, if it names a
/// main-table node. The inverse of the `index + 1` above; an id an
/// adapter invented, or one naming a synthesized instance, gives `None`.
pub fn index_of_node<W: Widget>(id: NodeId) -> Option<usize> {
    let i = id.0.checked_sub(1)? as usize;
    (i < W::SUB_TABLE.len()).then_some(i)
}

/// Agent-facing JSON view of the same snapshot, joined with provenance,
/// author ids and kind names — so the agent view and the screen-reader
/// view cannot drift, one being derived from the other.
pub fn dump<W: Widget>(tree: &W, st: &[NodeState]) -> serde_json::Value {
    let update = snapshot(tree, st);
    let table = W::SUB_TABLE;
    let nodes: Vec<serde_json::Value> = update
        .nodes
        .iter()
        .map(|(id, n)| {
            let mut o = serde_json::Map::new();
            o.insert("id".into(), (id.0).into());
            o.insert("role".into(), format!("{:?}", n.role()).into());
            if let Some(v) = n.value() {
                o.insert("value".into(), v.into());
            }
            if let Some(l) = n.label() {
                o.insert("label".into(), l.into());
            }
            if let Some(b) = n.bounds() {
                o.insert("bounds".into(), serde_json::json!([b.x0, b.y0, b.x1, b.y1]));
            }
            let children: Vec<u64> = n.children().iter().map(|c| c.0).collect();
            if !children.is_empty() {
                o.insert("children".into(), children.into());
            }
            let idx = (id.0 as usize).wrapping_sub(1);
            if id.0 & (1 << 63) == 0 && idx < table.len() {
                let src = table[idx].src;
                if !src.file.is_empty() {
                    o.insert("src".into(), format!("{}:{}", src.file, src.line).into());
                }
                if !table[idx].wid.is_empty() {
                    o.insert("wid".into(), table[idx].wid.into());
                }
            }
            serde_json::Value::Object(o)
        })
        .collect();
    serde_json::json!({ "root": 1, "nodes": nodes })
}

/// The synthetic AccessKit id for one node of one instance of an
/// off-table container.
///
/// `1<<63 | node<<44 | row<<12 | template`. The high bit marks the id as
/// synthetic; `node` is the owning container's table index (plus one,
/// since AccessKit reserves 0); `row` is the instance the container
/// chose to expose — a repeater's row, a list's item index, an
/// `Either`'s side.
///
/// **Defined once, here.** Three containers had written this bit layout
/// out by hand and `lumen-static-agent` had a fourth copy, under a
/// comment citing a function that no longer existed. Nothing checked the
/// four agreed, and when they disagree `ui.probe` and `ui.getTree`
/// return different ids for the same node — the exact failure the agent
/// crate's docs claim is impossible
/// (`docs/plan-review-fixes.md` F4.4).
#[inline]
pub fn instance_id(container_node: u64, row: u64, template_node: usize) -> u64 {
    (1u64 << 63) | (container_node << 44) | (row << 12) | template_node as u64
}
