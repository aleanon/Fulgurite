//! **A runtime-assembled table's child count is checked** — M46/T46.3
//! (R2-F9, `docs/plan-hash-identity.md`).
//!
//! `assemble::concat_table` wrote `count as u16` into the synthetic
//! root's child count with no bound. At 65,536 instances the count
//! wrapped to 0 on a node marked `CONTAINER`, so the observe walk listed
//! no children; at 70,000 it read 4,464 and the table was not well
//! formed. Nothing checked either: the derive `const`-asserts
//! `table_is_well_formed` for every table it builds, and runtime
//! assembly, which "calls the same fn" to splice, never called the check.

use fulgurite::assemble::concat_table;
use fulgurite::widget::table_is_well_formed;
use fulgurite::Widget;
use fulgurite_widgets::ops::Label;

fn build(count: usize) -> Vec<fulgurite::NodeDesc> {
    concat_table(count, <Label as Widget>::NODE_COUNT, <Label as Widget>::SUB_TABLE)
}

/// Up to the limit, the root lists every instance and the table is well
/// formed. The accept direction: `u16::MAX` itself must not be refused.
#[test]
fn up_to_u16_max_instances_the_root_lists_every_one() {
    for count in [0usize, 1, 2, 1000, u16::MAX as usize] {
        let t = build(count);
        let children = t[0].child_count as usize;
        let ok = table_is_well_formed(&t);
        println!("count {count}: root child_count {children} (must equal count), well formed {ok}");
        assert_eq!(children, count, "the root lost instances at {count}");
        assert!(ok, "the table at {count} is not well formed");
    }
}

/// One past the limit is refused by name, rather than wrapped to a
/// root that claims no children. `expected` pins the message, so a panic
/// for any other reason does not satisfy this.
#[test]
#[should_panic(expected = "65535")]
fn one_past_u16_max_is_refused_with_the_limit() {
    let t = build(u16::MAX as usize + 1);
    println!(
        "count 65536: root child_count {} (must equal 65536), well formed {}",
        t[0].child_count,
        table_is_well_formed(&t)
    );
}
