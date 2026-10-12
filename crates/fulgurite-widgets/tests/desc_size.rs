//! **`NodeDesc`'s size, under the features that decide it** — M54/T54.1
//! (`docs/plan-a11y-platform.md` §2.1).
//!
//! `desc.rs` stated 48 B in a release build and 72 B with `observe` and
//! nothing asserted either. The split of `a11y` out of `observe` rests on
//! the first: a screen-reader build must not pay the agent's `src` stamp.
//!
//! Here rather than in core's own tests because core cannot be *tested*
//! with `a11y` alone: its dev-dependency on this crate would build these
//! widgets without `a11y`, which the seam assertion refuses. This crate
//! carries the feature through, and `just test` runs the file three ways:
//! default (48), `--features a11y` (48) and the workspace's `observe`
//! (72).

use fulgurite::desc::NodeDesc;

#[test]
fn node_desc_pays_for_provenance_only_under_observe() {
    let n = core::mem::size_of::<NodeDesc>();
    let (bound, features) = match (fulgurite::OBSERVE, fulgurite::A11Y) {
        (true, _) => (72, "observe (implies a11y)"),
        (false, true) => (48, "a11y without observe"),
        (false, false) => (48, "neither"),
    };
    println!("NodeDesc = {n} B under {features} (bound {bound})");
    assert_eq!(n, bound, "NodeDesc is {n} B under {features}, not {bound}");
}
