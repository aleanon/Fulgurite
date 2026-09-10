//! M18: damage is a set of regions, not a bounding box.
//!
//! The first test is the one the milestone exists for. Everything else
//! is the bookkeeping that has to hold for it to be usable.

use lumen_static::retain::{Damage, Regions, MAX_REGIONS};

fn area(r: [f32; 4]) -> f32 {
    (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0)
}

fn total(d: Damage, viewport: [f32; 4]) -> f32 {
    d.regions(viewport).map(area).sum()
}

const VIEW: [f32; 4] = [0.0, 0.0, 1200.0, 800.0];

/// **The reason M18 exists.** Two small changes in opposite corners have
/// a *bounding box* of very nearly the whole window.
///
/// A scrollbar thumb on the right edge and a status label at the bottom
/// left: 5 000 px of real damage, and a box of 703 280 — **140×**. The
/// first draft of this test used a full-width toolbar instead of the
/// label and only reached 4.6×, because a toolbar already spans x and
/// the box was barely worse than the union. Both numbers are printed
/// below so the bound can be seen to be under the measurement rather
/// than taken on trust.
#[test]
fn two_far_apart_changes_do_not_become_the_whole_window() {
    let thumb = [1190.0, 200.0, 1200.0, 300.0]; //  10 x 100 = 1 000
    let status = [8.0, 770.0, 208.0, 790.0]; // 200 x  20 = 4 000

    let d = Damage::rect(thumb).union(Damage::rect(status));
    let exact = total(d, VIEW);
    let boxed = area(d.clip_to(VIEW).expect("some damage"));
    let want = area(thumb) + area(status);

    println!("regions {exact} px, bounding box {boxed} px ({:.0}x), needed {want} px",
             boxed / exact);
    assert_eq!(exact, want, "the two regions must stay separate and exact");
    assert_eq!(d.regions(VIEW).count(), 2, "and there must be two of them");
    // 50x over a measured 140x.
    assert!(
        boxed > exact * 50.0,
        "the box should be far worse than the set, or this test is not \
         measuring the amplification: box {boxed}, set {exact}"
    );
}

/// Overlapping regions merge — keeping them apart would render their
/// intersection twice for nothing.
#[test]
fn overlapping_regions_merge() {
    let mut g = Regions::one([0.0, 0.0, 100.0, 100.0]);
    g.push([50.0, 50.0, 150.0, 150.0]);
    assert_eq!(g.len(), 1, "overlapping regions should have merged: {:?}", g.as_slice());
    assert_eq!(g.as_slice()[0], [0.0, 0.0, 150.0, 150.0]);
}

/// A region already covered adds nothing, and one that covers an
/// existing region replaces it.
#[test]
fn containment_is_absorbed_both_ways() {
    let mut g = Regions::one([0.0, 0.0, 100.0, 100.0]);
    g.push([10.0, 10.0, 20.0, 20.0]);
    assert_eq!(g.len(), 1);
    assert_eq!(g.as_slice()[0], [0.0, 0.0, 100.0, 100.0], "the inner rect changed nothing");

    g.push([-10.0, -10.0, 200.0, 200.0]);
    assert_eq!(g.len(), 1);
    assert_eq!(g.as_slice()[0], [-10.0, -10.0, 200.0, 200.0], "the outer rect took over");
}

/// Past the cap, precision degrades by merging the cheapest pair —
/// **not** by collapsing to `All`, which is what a naive bound would do
/// and what would make the fifth change cost the whole frame.
#[test]
fn a_full_set_degrades_by_merging_rather_than_collapsing() {
    let mut d = Damage::None;
    // Five small, well-separated squares along a diagonal.
    for k in 0..5 {
        let x = k as f32 * 200.0;
        let y = k as f32 * 100.0;
        d = d.union(Damage::rect([x, y, x + 10.0, y + 10.0]));
    }
    let n = d.regions(VIEW).count();
    assert!(n <= MAX_REGIONS, "the set must stay bounded: {n} regions");
    assert!(matches!(d, Damage::Regions(_)), "it must not have collapsed to All: {d:?}");

    let exact = total(d, VIEW);
    let boxed = area(d.clip_to(VIEW).unwrap());
    println!("5 squares -> {n} regions, {exact} px against a {boxed} px box");
    // 500 px of real damage. A collapse to the bounding box would be
    // 810 * 410 = 332 100.
    assert!(
        exact < boxed / 4.0,
        "merging should still be far better than the box: {exact} vs {boxed}"
    );
}

/// The predicate a backdrop-reading command tests against.
#[test]
fn intersects_is_strict_on_a_shared_edge() {
    let d = Damage::rect([0.0, 0.0, 100.0, 100.0]);
    assert!(d.intersects([50.0, 50.0, 150.0, 150.0]), "overlapping");
    assert!(!d.intersects([100.0, 0.0, 200.0, 100.0]), "a shared edge shares no pixels");
    assert!(!d.intersects([200.0, 200.0, 300.0, 300.0]), "disjoint");

    assert!(!Damage::None.intersects([0.0, 0.0, 10.0, 10.0]), "None touches nothing");
    assert!(Damage::All.intersects([0.0, 0.0, 10.0, 10.0]), "All touches everything");
    assert!(!Damage::All.intersects([0.0, 0.0, 0.0, 10.0]), "…except an empty rect");
}

/// `All` absorbs, `None` is the identity, and an empty rect is not
/// damage at all.
#[test]
fn union_has_the_lattice_it_claims() {
    let r = Damage::rect([0.0, 0.0, 10.0, 10.0]);
    assert_eq!(Damage::All.union(r), Damage::All);
    assert_eq!(r.union(Damage::All), Damage::All);
    assert_eq!(Damage::None.union(r), r);
    assert_eq!(r.union(Damage::None), r);
    assert_eq!(Damage::rect([5.0, 5.0, 5.0, 5.0]), Damage::None, "an empty rect is not damage");
    assert_eq!(r.union(r), r, "union is idempotent");
}

/// Regions are clipped to the viewport, and one entirely outside it
/// yields nothing rather than a rect off the surface.
#[test]
fn regions_are_clipped_to_the_viewport() {
    let d = Damage::rect([-50.0, -50.0, 50.0, 50.0]).union(Damage::rect([5000.0, 0.0, 5100.0, 10.0]));
    let got: Vec<[f32; 4]> = d.regions(VIEW).collect();
    assert_eq!(got.len(), 1, "the off-surface region must not be yielded: {got:?}");
    assert_eq!(got[0], [0.0, 0.0, 50.0, 50.0], "and the other is clamped");
}

/// `All` yields the viewport, so a caller can loop uniformly.
#[test]
fn all_yields_the_viewport_and_none_yields_nothing() {
    assert_eq!(Damage::All.regions(VIEW).collect::<Vec<_>>(), vec![VIEW]);
    assert_eq!(Damage::None.regions(VIEW).count(), 0);
}
