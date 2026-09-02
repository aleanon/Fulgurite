//! `table_is_well_formed` must be able to say no.
//!
//! It is `const`-asserted at every derive expansion, which makes it the
//! guarantee behind "a malformed table does not compile". But its only
//! exercise was one well-formed table asserted `true`: every `return
//! false` in it was unreached, so a mutation making it return `true`
//! unconditionally passed the whole suite while silently disabling the
//! proof-carrying-tables property (`docs/plan-review-fixes.md` F5.2).
//!
//! It was also O(n^2): 20 s of const-eval at 3.2k nodes, and at 10k a
//! hard `long_running_const_eval` failure — the tree could not be
//! compiled at all. F5.1 replaced the nested rescan with a single pass
//! over each node-as-child. These cases pin both the verdicts and the
//! reasons, so the linear form cannot quietly accept something the
//! quadratic one rejected.

use lumen_static::widget::table_is_well_formed;
use lumen_static::{NodeDesc, NO_PARENT};

/// A node, spelled out: (first_child, parent, next_sibling, child_count).
fn n(first_child: u32, parent: u32, next_sibling: u32, child_count: u16) -> NodeDesc {
    NodeDesc::new(first_child, parent, next_sibling, child_count, 0)
}

/// root { a, b } — the smallest table with structure.
fn good() -> Vec<NodeDesc> {
    vec![n(1, NO_PARENT, 0, 2), n(0, 0, 2, 0), n(0, 0, 0, 0)]
}

#[test]
fn the_well_formed_case_is_accepted() {
    assert!(table_is_well_formed(&good()));
    // And every real table in the workspace, via the derive's own const
    // assert — this is the one that was already covered.
    assert!(table_is_well_formed(<lumen_static::ops::Label as lumen_static::Widget>::SUB_TABLE));
}

#[test]
fn an_empty_table_is_rejected() {
    assert!(!table_is_well_formed(&[]));
}

#[test]
fn a_root_with_a_parent_is_rejected() {
    let mut t = good();
    t[0].parent = 0;
    assert!(!table_is_well_formed(&t));
}

#[test]
fn a_child_preceding_its_parent_is_rejected() {
    // Node 1 claims node 2 as its parent, but 2 comes after it — not
    // DFS pre-order.
    let mut t = good();
    t[1].parent = 2;
    assert!(!table_is_well_formed(&t));
}

#[test]
fn a_child_count_that_overstates_is_rejected() {
    let mut t = good();
    t[0].child_count = 3;
    assert!(!table_is_well_formed(&t), "3 declared, 2 present");
}

#[test]
fn a_child_count_that_understates_is_rejected() {
    let mut t = good();
    t[0].child_count = 1;
    assert!(!table_is_well_formed(&t), "1 declared, 2 present");
}

#[test]
fn first_child_not_immediately_after_the_parent_is_rejected() {
    let mut t = good();
    t[0].first_child = 2;
    assert!(!table_is_well_formed(&t));
}

#[test]
fn a_sibling_link_that_leaves_the_table_is_rejected() {
    let mut t = good();
    t[1].next_sibling = 99;
    assert!(!table_is_well_formed(&t));
}

#[test]
fn a_sibling_link_that_goes_backwards_is_rejected() {
    // A cycle: the linear walk must not loop forever on it.
    let mut t = vec![n(1, NO_PARENT, 0, 2), n(0, 0, 1, 0), n(0, 0, 0, 0)];
    t[1].next_sibling = 1;
    assert!(!table_is_well_formed(&t));
}

#[test]
fn an_orphan_is_rejected() {
    // Three nodes, but the root claims only one child, so node 2 belongs
    // to nobody. Caught by the total: declared counts must sum to n - 1.
    let t = vec![n(1, NO_PARENT, 0, 1), n(0, 0, 0, 0), n(0, 0, 0, 0)];
    assert!(!table_is_well_formed(&t));
}

#[test]
fn a_child_claimed_by_the_wrong_parent_is_rejected() {
    // root { a { b } }, but b says its parent is the root.
    let mut t = vec![n(1, NO_PARENT, 0, 1), n(2, 0, 0, 1), n(0, 1, 0, 0)];
    t[2].parent = 0;
    assert!(!table_is_well_formed(&t));
}

#[test]
fn a_deep_chain_is_accepted() {
    // Exercises the walk's forward progress over a long spine, which is
    // the shape the O(n^2) version was slowest on.
    let depth = 200;
    let mut t = Vec::with_capacity(depth);
    for i in 0..depth {
        let last = i + 1 == depth;
        t.push(n(
            if last { 0 } else { i as u32 + 1 },
            if i == 0 { NO_PARENT } else { i as u32 - 1 },
            0,
            u16::from(!last),
        ));
    }
    assert!(table_is_well_formed(&t));
}
