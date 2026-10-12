//! Step 6 correctness: a fused subtree is one table entry with layout
//! geometry identical to its unfused twin, composes at the seam, keeps
//! values live, and computes rigidity across fusion.

use benches::{Card, FusedCard, FusedItem, FusedTen, Ten};
use fulgurite_widgets::ops::{Fixed, Label};
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

fn root_size<W: Widget + Default>() -> fulgurite::Size {
    let mut w = W::default();
    let mut st = vec![NodeState::default(); W::NODE_COUNT];
    let sty = pass::default_styles(&w);
    pass::layout(&mut w, &sty, &mut st, ROOT_C);
    st[0].size
}

#[test]
fn fused_shapes_match_their_unfused_twins() {
    // Fusion is now a *call style*: the same generated body with static
    // calls, so a fused struct has the same node count and table as its
    // unfused twin — only the emitted calls differ.
    assert_eq!(FusedTen::NODE_COUNT, Ten::NODE_COUNT);
    assert_eq!(FusedCard::NODE_COUNT, Card::NODE_COUNT);
    assert_eq!(FusedCard::SUB_TABLE.len(), Card::SUB_TABLE.len());
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
    assert_eq!(Mixed::NODE_COUNT, 1 + 1 + FusedItem::NODE_COUNT + 1);

    let mut m = Mixed::default();
    let mut st = [NodeState::default(); Mixed::NODE_COUNT];
    let sty = pass::default_styles(&m);
    pass::layout(&mut m, &sty, &mut st, ROOT_C);

    // The fused cell is placed below the head label like any child.
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
    let mut st = [NodeState::default(); FusedTen::NODE_COUNT];
    let sty = pass::default_styles(&t);
    pass::layout(&mut t, &sty, &mut st, ROOT_C);
    let mut a = fulgurite::DisplayList::new();
    pass::paint(&t, &st, &mut a);

    // Re-laid out first: a fused tree's labels default to empty, and an
    // empty label has no area, so it records no text run at all. Without
    // the layout this compared two frames that both drew nothing.
    t.l1.set("changed");
    pass::layout(&mut t, &sty, &mut st, ROOT_C);
    let mut b = fulgurite::DisplayList::new();
    pass::paint(&t, &st, &mut b);
    assert_ne!(a.hash_from(0), b.hash_from(0), "paint reads fused internals through the fields");

    // And layout responds too: longer text grows the fused row. (Base is
    // re-derived after the write — pre-mutation provenance wouldn't
    // soundly cover it.)
    t.l2.set("a much longer label than before");
    pass::layout(&mut t, &sty, &mut st, ROOT_C);
    let mut ten = Ten::default();
    ten.l1.set("changed");
    ten.l2.set("a much longer label than before");
    let mut tst = [NodeState::default(); Ten::NODE_COUNT];
    let tsty = pass::default_styles(&ten);
    pass::layout(&mut ten, &tsty, &mut tst, ROOT_C);
    assert_eq!(st[0].size, tst[0].size, "still tracks the unfused twin");
}
