//! Step 2 correctness: table well-formedness, and the static pipeline
//! against the naive dyn tree as an oracle — same shapes, same math, so
//! every node's size and position must agree exactly.

use benches::{d_assembled, d_ten, Block, Card, Ten};
use fulgurite::assemble::Assembled;
use fulgurite::{pass, Constraints, NodeState, Point, Size, Widget, NO_PARENT};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

#[test]
fn tables_are_well_formed() {
    for (name, table) in [
        ("Ten", Ten::SUB_TABLE),
        ("Card", Card::SUB_TABLE),
        ("Block", Block::SUB_TABLE),
    ] {
        assert_eq!(table[0].parent, NO_PARENT, "{name}: root parent");
        for (i, d) in table.iter().enumerate() {
            if i > 0 {
                assert!((d.parent as usize) < i, "{name}[{i}]: parent precedes (DFS)");
            }
            if d.child_count > 0 {
                assert_eq!(d.first_child as usize, i + 1, "{name}[{i}]: first child adjacent");
            }
        }
        // Child counts consistent: every non-root node is counted by its parent.
        let mut counted = vec![0u16; table.len()];
        for d in table.iter().skip(1) {
            counted[d.parent as usize] += 1;
        }
        for (i, d) in table.iter().enumerate() {
            assert_eq!(counted[i], d.child_count, "{name}[{i}]: child_count");
        }
        // Sibling chains: walking first_child/next_sibling visits exactly
        // child_count nodes, each parented to the walked node.
        for (i, d) in table.iter().enumerate() {
            if d.child_count == 0 {
                continue;
            }
            let mut seen = 0;
            let mut c = d.first_child as usize;
            loop {
                assert_eq!(table[c].parent as usize, i, "{name}[{i}]: sibling chain parent");
                seen += 1;
                if table[c].next_sibling == 0 {
                    break;
                }
                c = table[c].next_sibling as usize;
            }
            assert_eq!(seen, d.child_count, "{name}[{i}]: sibling chain length");
        }
    }
    assert_eq!(Ten::NODE_COUNT, 10);
    assert_eq!(Card::NODE_COUNT, 22);
    assert_eq!(Block::NODE_COUNT, 90);
    // Block: itself + title + 4 * (card + title + 4 * (item + 4 leaves))
    // The derive asserts well-formedness and the depth cap at compile
    // time for every table it emits; this re-checks at runtime that the
    // const fn agrees with the hand-written invariants above.
    assert!(fulgurite::widget::table_is_well_formed(Block::SUB_TABLE));
    const _: () = assert!(Block::DEPTH <= fulgurite::MAX_DEPTH);
}

/// Run the framework pipeline; return (size, pos) per node in table order.
fn run_static<W: Widget + Default>() -> Vec<(Size, Point)> {
    let mut w = W::default();
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    let sty = pass::default_styles(&w);
    pass::layout(&mut w, &sty, &mut st, ROOT_C);
    st.iter().map(|s| (s.size, s.pos)).collect()
}

#[test]
fn static_matches_dyn_node_for_node() {
    // Every node's size *and position*, not four scalars from a vector
    // built for the purpose. The module doc claimed this; the code
    // compared `got[0].0`, `got[1].0` and two more, and the dyn arm's
    // `place` call read nothing back (`docs/plan-review-fixes.md` F6.10).
    let got = run_static::<Ten>();
    let mut d = d_ten();
    d.layout(ROOT_C);
    d.place(Point::default());
    let mut want = Vec::new();
    d.collect(&mut want);
    assert_eq!(got.len(), want.len(), "same node count");
    for (i, (a, b)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(a, b, "node {i} disagrees: static {a:?} vs dyn {b:?}");
    }
}

#[test]
fn assembled_matches_dyn_node_for_node() {
    let mut a = Assembled::<Card>::new(45);
    //  places as it goes — it owns the whole
    // column, so there is no caller to position it.
    a.layout(ROOT_C);
    let got: Vec<_> = a.states.iter().map(|s| (s.size, s.pos)).collect();

    let mut d = d_assembled(45);
    d.layout(ROOT_C);
    d.place(Point::default());
    let mut want = Vec::new();
    d.collect(&mut want);

    assert_eq!(got.len(), want.len(), "same node count");
    for (i, (x, y)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(x, y, "node {i} disagrees: static {x:?} vs dyn {y:?}");
    }
}

#[test]
fn paint_is_deterministic_and_value_sensitive() {
    let mut tree = Ten::default();
    let mut st = [NodeState::default(); Ten::NODE_COUNT];
    let sty = pass::default_styles(&tree);
    pass::layout(&mut tree, &sty, &mut st, ROOT_C);
    let mut a = fulgurite::DisplayList::new();
    pass::paint(&tree, &st, &mut a);
    let mut b = fulgurite::DisplayList::new();
    pass::paint(&tree, &st, &mut b);
    assert_eq!(a.hash_from(0), b.hash_from(0));

    // Values live in the fields (§3): mutate one and paint must see it.
    // Re-laid out first, as a real frame does — `Ten`'s labels default to
    // empty text, so without a layout the label's box has no area and
    // draws nothing whatever its contents. (Before 2026-09-02 a
    // zero-area run was still recorded, so this passed while asserting
    // that a command nobody can see had changed.)
    tree.l1.set("changed");
    pass::layout(&mut tree, &sty, &mut st, ROOT_C);
    let mut c = fulgurite::DisplayList::new();
    pass::paint(&tree, &st, &mut c);
    assert_ne!(a.hash_from(0), c.hash_from(0));

    // And the converse: an empty label has no area, so it contributes
    // nothing to paint at all.
    let mut empty = Ten::default();
    let mut est = [NodeState::default(); Ten::NODE_COUNT];
    let esty = pass::default_styles(&empty);
    pass::layout(&mut empty, &esty, &mut est, ROOT_C);
    let mut e = fulgurite::DisplayList::new();
    pass::paint(&empty, &est, &mut e);
    assert!(
        !e.cmds.iter().any(|cmd| matches!(cmd, fulgurite::Cmd::Text { .. })),
        "an empty label records no text run"
    );
}
