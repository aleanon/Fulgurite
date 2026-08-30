//! The §9 observability surface: the node table exposed as an AccessKit
//! tree, plus a JSON dump carrying source provenance for agents/tooling.
//!
//! One implementation serves screen readers and coding agents alike — no
//! bespoke agent channel. Node identity is positional and survives hot
//! reload: main-table nodes are `table index + 1` (AccessKit reserves
//! nothing, but 0 is left unused for clarity); repeater instances get
//! row-stable synthetic ids (see `Repeat::observe_op`), so "the third
//! row" keeps its id across scrolls *and* dylib swaps.
//!
//! Action synthesis (§9's "synthesize events") requires an event system,
//! which does not exist yet — deferred, recorded in
//! `docs/step5-observability.md`.

use accesskit::{Node, NodeId, Rect, Role, Tree, TreeId, TreeUpdate};

use crate::desc::{NodeDesc, NodeState, WidgetOps};

fn bounds_of(st: &NodeState) -> Rect {
    Rect {
        x0: st.pos.x as f64,
        y0: st.pos.y as f64,
        x1: (st.pos.x + st.size.w) as f64,
        y1: (st.pos.y + st.size.h) as f64,
    }
}

/// Emit one table's nodes into `out` with ids assigned by `id_of`
/// (index → id). The walker pre-fills role `GenericContainer`, absolute
/// bounds, and table children; each kind's `observe` op then overrides
/// what it knows better and may push extra subtrees (repeater windows).
pub fn emit_subtree(
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    st: &[NodeState],
    id_of: &dyn Fn(usize) -> u64,
    out: &mut Vec<(NodeId, Node)>,
) {
    for i in 0..table.len() {
        let d = &table[i];
        let mut n = Node::new(Role::GenericContainer);
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
        (kinds[d.kind as usize].observe)(base, d, &st[i], id_of(i), &mut n, out);
        out.push((NodeId(id_of(i)), n));
    }
}

/// Full accessibility snapshot of a laid-out tree, ready to hand to an
/// AccessKit platform adapter (or an agent).
pub fn snapshot(
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    st: &[NodeState],
) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(table.len());
    emit_subtree(base, table, kinds, st, &|i| (i + 1) as u64, &mut nodes);
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(1),
    }
}

/// Agent-facing JSON view of the same snapshot: role, value, bounds,
/// children — and, for main-table nodes, the §9 source provenance, so a
/// finding reads "the label declared at src/app.rs:42 has zero size".
pub fn dump(
    base: *const u8,
    table: &[NodeDesc],
    kinds: &[&'static WidgetOps],
    st: &[NodeState],
) -> serde_json::Value {
    let update = snapshot(base, table, kinds, st);
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
            if let Some(b) = n.bounds() {
                o.insert("bounds".into(), serde_json::json!([b.x0, b.y0, b.x1, b.y1]));
            }
            let children: Vec<u64> = n.children().iter().map(|c| c.0).collect();
            if !children.is_empty() {
                o.insert("children".into(), children.into());
            }
            // Provenance exists for main-table nodes (instances inherit
            // their template's declaration site implicitly).
            let idx = (id.0 as usize).wrapping_sub(1);
            if id.0 & (1 << 63) == 0 && idx < table.len() {
                let src = table[idx].src;
                if !src.file.is_empty() {
                    o.insert("src".into(), format!("{}:{}", src.file, src.line).into());
                }
                o.insert("kind".into(), kinds[table[idx].kind as usize].name.into());
            }
            serde_json::Value::Object(o)
        })
        .collect();
    serde_json::json!({ "root": 1, "nodes": nodes })
}
