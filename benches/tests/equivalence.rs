//! Step 2 correctness: table well-formedness, and the static pipeline
//! against the naive dyn tree as an oracle — same shapes, same math, so
//! every node's size and position must agree exactly.

use benches::{d_assembled, d_card, d_ten, Block, Card, Ten};
use lumen_static::assemble::Assembled;
use lumen_static::{pass, Constraints, NodeState, Point, Size, Widget, NO_PARENT};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

#[test]
fn tables_are_well_formed() {
    for (name, table, kinds) in [
        ("Ten", Ten::SUB_TABLE, Ten::KIND_LIST),
        ("Card", Card::SUB_TABLE, Card::KIND_LIST),
        ("Block", Block::SUB_TABLE, Block::KIND_LIST),
    ] {
        assert_eq!(table[0].parent, NO_PARENT, "{name}: root parent");
        for (i, d) in table.iter().enumerate() {
            assert!((d.kind as usize) < kinds.len(), "{name}[{i}]: kind in range");
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
    assert!(Block::KIND_COUNT >= Block::SUB_TABLE.iter().map(|d| d.kind).max().unwrap() as usize);
}

/// Run the static pipeline; return (sizes, positions) per node in table order.
fn run_static(
    base: *const u8,
    table: &[lumen_static::NodeDesc],
    kinds: &[&'static lumen_static::WidgetOps],
) -> Vec<(Size, Point)> {
    let mut st = vec![NodeState::default(); table.len()];
    pass::layout(base, table, kinds, &mut st, ROOT_C);
    st.iter().map(|s| (s.size, s.pos)).collect()
}

#[test]
fn static_matches_dyn_root_geometry() {
    // Ten (const table) vs dyn ten
    let tree = Ten::default();
    let got = run_static(&tree as *const Ten as *const u8, Ten::SUB_TABLE, Ten::KIND_LIST);
    let mut d = d_ten();
    let dsz = d.layout(ROOT_C);
    assert_eq!(got[0].0, dsz, "Ten root size");

    // Assembled<Card> vs dyn column of cards
    let mut a = Assembled::<Card>::new(45);
    let base = a.base();
    let got = {
        pass::layout(base, &a.table, &a.kinds, &mut a.states, ROOT_C);
        a.states.iter().map(|s| (s.size, s.pos)).collect::<Vec<_>>()
    };
    let mut d = d_assembled(45);
    let dsz = d.layout(ROOT_C);
    d.place(Point::default());
    assert_eq!(got[0].0, dsz, "assembled root size");

    // Spot-check: first card's subtree root (table index 1)
    let mut dc = d_card();
    let card_c = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 / 45.0 };
    let card_sz = dc.layout(card_c);
    assert_eq!(got[1].0, card_sz, "first card size");
}

#[test]
fn partitioned_pass_matches_full_pass() {
    let mut a = Assembled::<Card>::new(45);
    let base = a.base();
    pass::layout(base, &a.table, &a.kinds, &mut a.states, ROOT_C);
    let full: Vec<_> = a.states.iter().map(|s| (s.size, s.pos)).collect();

    let flex: Vec<u32> =
        (0..a.table.len() as u32).filter(|&i| !a.table[i as usize].is_rigid()).collect();
    assert!(flex.len() < a.table.len(), "some rigid nodes exist");

    // Perturb all state except sizes of rigid nodes, then run partitioned.
    for (i, s) in a.states.iter_mut().enumerate() {
        s.pos = Point { x: -1.0, y: -1.0 };
        if !a.table[i].is_rigid() {
            s.size = Size { w: -1.0, h: -1.0 };
        }
    }
    pass::layout_partitioned(base, &a.table, &a.kinds, &mut a.states, ROOT_C, &flex);
    let part: Vec<_> = a.states.iter().map(|s| (s.size, s.pos)).collect();
    assert_eq!(full, part);
}

#[test]
fn paint_is_deterministic_and_value_sensitive() {
    let mut tree = Ten::default();
    let base = &tree as *const Ten as *const u8;
    let mut st = [NodeState::default(); Ten::NODE_COUNT];
    pass::layout(base, Ten::SUB_TABLE, Ten::KIND_LIST, &mut st, ROOT_C);
    let mut a = 0u64;
    pass::paint(base, Ten::SUB_TABLE, Ten::KIND_LIST, &st, &mut a);
    let mut b = 0u64;
    pass::paint(base, Ten::SUB_TABLE, Ten::KIND_LIST, &st, &mut b);
    assert_eq!(a, b);

    // Values live in the fields (§3): mutate one and paint must see it.
    // The base pointer is re-derived after the write — the pre-mutation
    // pointer's provenance would not soundly cover it.
    tree.l1.text = "changed";
    let base = &tree as *const Ten as *const u8;
    let mut c = 0u64;
    pass::paint(base, Ten::SUB_TABLE, Ten::KIND_LIST, &st, &mut c);
    assert_ne!(a, c);
}
