//! Step 6 correctness: a fused subtree is one table entry with layout
//! geometry identical to its unfused twin, composes at the seam, keeps
//! values live, and computes rigidity across fusion.

use benches::{Card, FusedCard, FusedItem, FusedTen, Ten};
use lumen_static::ops::{Fixed, Label};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

fn root_size<W: Widget + Default>() -> lumen_static::Size {
    let w = W::default();
    let base = &w as *const W as *const u8;
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    pass::layout(base, W::SUB_TABLE, W::KIND_LIST, &mut st, ROOT_C);
    st[0].size
}

#[test]
fn fused_is_one_node_one_kind() {
    const _: () = assert!(FusedTen::NODE_COUNT == 1);
    const _: () = assert!(FusedTen::KIND_COUNT == 1);
    const _: () = assert!(FusedCard::NODE_COUNT == 1);
    assert_eq!(FusedCard::SUB_TABLE.len(), 1);
    assert!(!FusedCard::SUB_TABLE[0].is_container(), "no table children");
}

#[test]
fn fused_geometry_matches_unfused() {
    assert_eq!(root_size::<FusedTen>(), root_size::<Ten>());
    assert_eq!(root_size::<FusedCard>(), root_size::<Card>());
}

#[test]
fn fused_composes_at_the_seam() {
    // A fused widget as an ordinary field of a table container: the
    // outer schedule stays dynamic, the inner kernel is fused (§5/§9).
    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Mixed {
        head: Label,
        cell: FusedItem,
        tail: Fixed,
    }
    assert_eq!(Mixed::NODE_COUNT, 4); // root + head + fused cell + tail

    let m = Mixed::default();
    let base = &m as *const Mixed as *const u8;
    let mut st = [NodeState::default(); Mixed::NODE_COUNT];
    pass::layout(base, Mixed::SUB_TABLE, Mixed::KIND_LIST, &mut st, ROOT_C);

    // The fused cell is placed like any leaf: below the head label.
    assert_eq!(st[2].pos.y, st[1].size.h);
    assert!(st[2].size.w > 0.0);
}

#[test]
fn fused_rigidity_composes() {
    #[derive(StaticTree, Default)]
    #[widget(row, spacing = 0.0, fuse)]
    struct RigidFused {
        a: Fixed,
        b: Fixed,
    }
    const _: () = assert!(RigidFused::IS_RIGID);
    assert!(RigidFused::SUB_TABLE[0].is_rigid());
    // …and a fused struct with a flexible child is flexible.
    const _: () = assert!(!FusedItem::IS_RIGID);
}

#[test]
fn fused_values_stay_live() {
    let mut t = FusedTen::default();
    let base = &t as *const FusedTen as *const u8;
    let mut st = [NodeState::default(); 1];
    pass::layout(base, FusedTen::SUB_TABLE, FusedTen::KIND_LIST, &mut st, ROOT_C);
    let mut a = 0u64;
    pass::paint(base, FusedTen::SUB_TABLE, FusedTen::KIND_LIST, &st, &mut a);

    t.l1.text = "changed";
    let base = &t as *const FusedTen as *const u8;
    let mut b = 0u64;
    pass::paint(base, FusedTen::SUB_TABLE, FusedTen::KIND_LIST, &st, &mut b);
    assert_ne!(a, b, "paint reads fused internals through the fields");

    // And layout responds too: longer text grows the fused row. (Base is
    // re-derived after the write — pre-mutation provenance wouldn't
    // soundly cover it.)
    t.l2.text = "a much longer label than before";
    let base = &t as *const FusedTen as *const u8;
    pass::layout(base, FusedTen::SUB_TABLE, FusedTen::KIND_LIST, &mut st, ROOT_C);
    let mut ten = Ten::default();
    ten.l1.text = "changed";
    ten.l2.text = "a much longer label than before";
    let tbase = &ten as *const Ten as *const u8;
    let mut tst = [NodeState::default(); Ten::NODE_COUNT];
    pass::layout(tbase, Ten::SUB_TABLE, Ten::KIND_LIST, &mut tst, ROOT_C);
    assert_eq!(st[0].size, tst[0].size, "still tracks the unfused twin");
}
