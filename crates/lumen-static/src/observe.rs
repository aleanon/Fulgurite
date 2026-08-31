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

pub use accesskit;

use accesskit::{Node as AkNode, NodeId, Rect, Role, Tree, TreeId, TreeUpdate};

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
pub fn emit_subtree<W: Widget>(
    tree: &W,
    st: &[NodeState],
    id_of: &dyn Fn(usize) -> u64,
    out: &mut Vec<(NodeId, AkNode)>,
) {
    emit_table(W::SUB_TABLE, tree, st, id_of, out)
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
    for (i, d) in table.iter().enumerate() {
        let mut n = AkNode::new(Role::GenericContainer);
        n.set_bounds(bounds_of(&st[i]));
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
        tree.node(i).observe(st, i, id_of(i), &mut n, out);
        out.push((NodeId(id_of(i)), n));
    }
}

/// Full accessibility snapshot, ready for an AccessKit adapter or agent.
pub fn snapshot<W: Widget>(tree: &W, st: &[NodeState]) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(W::SUB_TABLE.len());
    emit_subtree(tree, st, &|i| (i + 1) as u64, &mut nodes);
    TreeUpdate { nodes, tree: Some(Tree::new(NodeId(1))), tree_id: TreeId::ROOT, focus: NodeId(1) }
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
