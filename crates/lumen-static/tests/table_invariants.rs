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
//!
//! **And it said yes too often** — M41/T41.6, R2-F5. It checked that a
//! parent precedes its child, not DFS pre-order, and let a last child
//! link to "a later node". The four counterexamples at the end of this
//! file were all accepted although `subtree_range` — which decides what
//! the `Boundaries` default's precise marks cover — is wrong for them.

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
    assert!(table_is_well_formed(<lumen_static_widgets::ops::Label as lumen_static::Widget>::SUB_TABLE));
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

// ---------------------------------------------------------------------
// M41/T41.0 (`docs/plan-boundaries-repair.md` §1.4, R2-F5): tables the
// check accepts although `subtree_range` — which the `Boundaries`
// default's precise marks are computed from — is wrong for them, or a
// child-chain walk runs off them. The review's four counterexamples
// (scratch commit f7d5def), judged against descendants computed from the
// parent links alone, which is an oracle that shares no code with
// either the check or `subtree_range`.
// ---------------------------------------------------------------------

/// Descendants of `root`, itself included, from parent links only.
fn descendants(t: &[NodeDesc], root: usize) -> Vec<usize> {
    let mut out = vec![root];
    for j in 0..t.len() {
        let mut a = j;
        while t[a].parent != NO_PARENT {
            a = t[a].parent as usize;
            if a == root {
                out.push(j);
                break;
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Every node whose `subtree_range` disagrees with the oracle, and every
/// sibling chain that leaves the table or does not visit each child
/// exactly once.
fn mismatches(t: &[NodeDesc]) -> Vec<String> {
    let mut bad = vec![];
    for i in 0..t.len() {
        let r: Vec<usize> = lumen_static::style::subtree_range(t, i).collect();
        let o = descendants(t, i);
        if r != o {
            bad.push(format!("node {i}: subtree_range {:?}, parent links {o:?}", lumen_static::style::subtree_range(t, i)));
        }
    }
    let mut seen = vec![0u32; t.len()];
    for p in 0..t.len() {
        if t[p].child_count == 0 {
            continue;
        }
        let mut c = t[p].first_child as usize;
        for _ in 0..t.len() {
            if c >= t.len() {
                bad.push(format!("the chain of {p} leaves the table at {c}"));
                break;
            }
            seen[c] += 1;
            if t[c].next_sibling == 0 {
                break;
            }
            c = t[c].next_sibling as usize;
        }
    }
    for (i, s) in seen.iter().enumerate().skip(1) {
        if *s != 1 {
            bad.push(format!("node {i} visited {s} times by sibling chains"));
        }
    }
    bad
}

/// A: not pre-order. root { A { a, a' }, B } stored as root, A, a, B, a'
/// — a' (A's child) after its uncle B.
fn cx_a() -> Vec<NodeDesc> {
    vec![n(1, NO_PARENT, 0, 2), n(2, 0, 3, 2), n(0, 1, 4, 0), n(0, 0, 0, 0), n(0, 1, 0, 0)]
}

/// B: a last child whose link points at a later node that is not its
/// parent's end. root { A { a }, B, C }; a.next = C.
fn cx_b() -> Vec<NodeDesc> {
    vec![n(1, NO_PARENT, 0, 3), n(2, 0, 3, 1), n(0, 1, 4, 0), n(0, 0, 4, 0), n(0, 0, 0, 0)]
}

/// C: a last child whose link leaves the table.
fn cx_c() -> Vec<NodeDesc> {
    vec![n(1, NO_PARENT, 0, 2), n(0, 0, 2, 0), n(0, 0, 99, 0)]
}

/// D: the "parent's own next sibling" allowance. The range is right, and
/// a child-chain walk (the observe walk's) runs on into the uncle.
fn cx_d() -> Vec<NodeDesc> {
    vec![n(1, NO_PARENT, 0, 2), n(2, 0, 3, 1), n(0, 1, 3, 0), n(0, 0, 0, 0)]
}

#[test]
fn tables_with_a_wrong_subtree_range_are_rejected() {
    let mut accepted = vec![];
    for (name, t) in [("A", cx_a()), ("B", cx_b()), ("C", cx_c()), ("D", cx_d())] {
        let verdict = table_is_well_formed(&t);
        let bad = mismatches(&t);
        println!("counterexample {name}: table_is_well_formed = {verdict} | oracle mismatches {bad:?}");
        assert!(!bad.is_empty(), "setup: counterexample {name} is not actually malformed");
        if verdict {
            accepted.push(name);
        }
    }
    assert!(accepted.is_empty(), "accepted {} of 4 malformed tables: {accepted:?}", accepted.len());
}

/// The oracle's accept direction: it agrees with `subtree_range` on
/// tables that are well-formed, so a mismatch above is about the table.
#[test]
fn the_parent_link_oracle_agrees_on_real_tables() {
    let tables: [(&str, &[NodeDesc]); 3] = [
        ("good", &good()),
        ("Label", <lumen_static_widgets::ops::Label as lumen_static::Widget>::SUB_TABLE),
        ("SearchField", <lumen_static_widgets::SearchField as lumen_static::Widget>::SUB_TABLE),
    ];
    for (name, t) in tables {
        let bad = mismatches(t);
        println!("{name}: {} nodes, mismatches {}", t.len(), bad.len());
        assert!(bad.is_empty(), "{name}: {bad:?}");
    }
}
