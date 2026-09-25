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

/// Whether `r` lies inside the union of `set` — by sampling, since a
/// union of boxes need not be a box: every pixel centre of `r` must fall
/// in some region.
fn covered(set: &[[f32; 4]], r: [f32; 4]) -> bool {
    let mut y = r[1] + 0.5;
    while y < r[3] {
        let mut x = r[0] + 0.5;
        while x < r[2] {
            if !set.iter().any(|s| x >= s[0] && x < s[2] && y >= s[1] && y < s[3]) {
                return false;
            }
            x += 1.0;
        }
        y += 1.0;
    }
    true
}

/// Push the rects one at a time and, **after every push**, report any
/// rect pushed so far that the set no longer covers. After every push and
/// not only at the end, because later merges grow boxes that can cover a
/// dropped rect again by accident — 64 random rects end as one box.
fn uncovered(rects: &[[f32; 4]]) -> (usize, Vec<[f32; 4]>) {
    let mut g = Regions::default();
    let mut lost = Vec::new();
    for (k, r) in rects.iter().enumerate() {
        g.push(*r);
        for p in &rects[..=k] {
            if !covered(g.as_slice(), *p) && !lost.contains(p) {
                lost.push(*p);
            }
        }
    }
    (g.len(), lost)
}

/// M40/T40.0, R18-F5: **a pushed rect is never dropped.** Merging may
/// lose precision — that is the ceiling [`MAX_REGIONS`] trades for — but
/// a region that stops being covered is a stale pixel, because nothing
/// else will redraw it.
///
/// `a_full_set_degrades_by_merging_rather_than_collapsing` bounds the
/// merged *area*, and dropping a region makes that bound easier to meet,
/// so a `push` that discarded the fifth rect on the full-set branch
/// passed the whole suite. This asserts coverage instead, on the
/// review's five disjoint squares and on 64 rects from a fixed LCG.
#[test]
fn every_pushed_rect_stays_covered() {
    let five = [
        [0.0, 0.0, 10.0, 10.0],
        [200.0, 0.0, 210.0, 10.0],
        [400.0, 0.0, 410.0, 10.0],
        [600.0, 400.0, 610.0, 410.0],
        [800.0, 400.0, 810.0, 410.0],
    ];
    let (n, lost) = uncovered(&five);
    println!("five squares: pushed {}, regions {n}, uncovered {} (must be 0): {lost:?}", five.len(), lost.len());
    let five_lost = lost;

    // A fixed LCG, so the sequence is the same on every run.
    let mut s: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (s >> 33) as u32
    };
    let rects: Vec<[f32; 4]> = (0..64)
        .map(|_| {
            let (x, y) = ((next() % 1100) as f32, (next() % 700) as f32);
            let (w, h) = ((next() % 60 + 1) as f32, (next() % 60 + 1) as f32);
            [x, y, x + w, y + h]
        })
        .collect();
    let (n, lost) = uncovered(&rects);
    println!("LCG: pushed {}, regions {n}, uncovered {} (must be 0)", rects.len(), lost.len());
    assert!(five_lost.is_empty(), "five squares: a pushed rect was dropped: {five_lost:?}");
    assert!(lost.is_empty(), "LCG: a pushed rect was dropped: {lost:?}");
}
