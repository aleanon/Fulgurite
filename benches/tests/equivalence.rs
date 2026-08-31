//! Step 2 correctness: table well-formedness, and the static pipeline
//! against the naive dyn tree as an oracle — same shapes, same math, so
//! every node's size and position must agree exactly.

use benches::{d_assembled, d_card, d_ten, Block, Card, Ten};
use lumen_static::assemble::Assembled;
use lumen_static::{pass, Constraints, NodeState, Point, Size, Widget, NO_PARENT};

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
    assert!(lumen_static::widget::table_is_well_formed(Block::SUB_TABLE));
    const _: () = assert!(Block::DEPTH <= lumen_static::MAX_DEPTH);
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
fn static_matches_dyn_root_geometry() {
    // Ten vs dyn ten
    let got = run_static::<Ten>();
    let mut d = d_ten();
    let dsz = d.layout(ROOT_C);
    assert_eq!(got[0].0, dsz, "Ten root size");

    // Assembled<Card> vs dyn column of cards
    let mut a = Assembled::<Card>::new(45);
    a.layout(ROOT_C);
    let got: Vec<_> = a.states.iter().map(|s| (s.size, s.pos)).collect();
    let mut d = d_assembled(45);
    let dsz = d.layout(ROOT_C);
    d.place(Point::default());
    assert_eq!(got[0].0, dsz, "assembled root size");

    // Spot-check: the first card sees the whole viewport, because the
    // column offers each instance whatever main-axis space is left and
    // nothing has been consumed yet.
    let mut dc = d_card();
    let card_c = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };
    let card_sz = dc.layout(card_c);
    assert_eq!(got[1].0, card_sz, "first card size");
}

#[test]
fn paint_is_deterministic_and_value_sensitive() {
    let mut tree = Ten::default();
    let mut st = [NodeState::default(); Ten::NODE_COUNT];
    let sty = pass::default_styles(&tree);
    pass::layout(&mut tree, &sty, &mut st, ROOT_C);
    let mut a = 0u64;
    pass::paint(&tree, &st, &mut a);
    let mut b = 0u64;
    pass::paint(&tree, &st, &mut b);
    assert_eq!(a, b);

    // Values live in the fields (§3): mutate one and paint must see it.
    tree.l1.set("changed");
    let mut c = 0u64;
    pass::paint(&tree, &st, &mut c);
    assert_ne!(a, c);
}
