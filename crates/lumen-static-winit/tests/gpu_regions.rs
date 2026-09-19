//! M26/T26.12: the GPU backend is handed **one rect per damage region**,
//! not their union.
//!
//! `Prefer::Auto` picks the GPU first, and the shell handed it
//! `Damage::clip_to` — whose own doc says it is "the bounding box of
//! every region" and is "kept for callers that can only take one rect".
//! The GPU can take several. Two small changes at opposite corners of a
//! window bound to the whole window, which is the merging cost M18
//! removed on the software path and which this quietly re-introduced.
//!
//! **This gate lives here, not beside the pixels.** `just test`
//! excludes the wgpu crate and `just gpu-parity` runs in neither `test`
//! nor `check`, so a gate written there runs on nobody's commit. Pixel
//! comparison stays in the adapter-gated run; what is asserted here is
//! what the shell *hands over*, which is where the bug was.

use lumen_static::display::Rect;
use lumen_static::retain::{Damage, Regions};
use lumen_static_winit::regions_for;

const VIEW: Rect = [0.0, 0.0, 800.0, 600.0];

fn area(rs: &[Rect]) -> f32 {
    rs.iter().map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum()
}

/// Two small changes at opposite corners: two rects, and an area far
/// under their bounding box's.
#[test]
fn two_far_apart_regions_are_handed_over_as_two() {
    let mut g = Regions::default();
    g.push([10.0, 10.0, 60.0, 60.0]);
    g.push([740.0, 540.0, 790.0, 590.0]);
    let out = regions_for(Damage::Regions(g), VIEW);

    let bbox = (790.0 - 10.0) * (590.0 - 10.0);
    println!("{} regions, {} px² against a {bbox} px² bounding box", out.len(), area(&out));
    assert_eq!(out.len(), 2, "the two regions were collapsed into {}", out.len());
    assert!(
        area(&out) < bbox * 0.02,
        "{} px² handed over against a {bbox} px² bounding box — the set is \
         the box, not the regions",
        area(&out)
    );
}

/// `Damage::All` is one region: the viewport. A backend that got an
/// empty set for it would draw nothing on the frame that needs
/// everything.
#[test]
fn all_is_the_viewport() {
    let out = regions_for(Damage::All, VIEW);
    assert_eq!(out, vec![VIEW], "`All` must hand over the whole surface");
}

/// And nothing is nothing — the shell skips the draw entirely.
#[test]
fn none_hands_over_no_regions() {
    assert!(regions_for(Damage::None, VIEW).is_empty(), "`None` must hand over nothing");
}

/// Every region is clamped to the surface: a backend cannot be given a
/// rect off the edge of the texture it is drawing into.
#[test]
fn regions_are_clamped_to_the_surface() {
    let mut g = Regions::default();
    g.push([-50.0, -50.0, 40.0, 40.0]);
    g.push([760.0, 560.0, 900.0, 700.0]);
    for r in regions_for(Damage::Regions(g), VIEW) {
        assert!(
            r[0] >= VIEW[0] && r[1] >= VIEW[1] && r[2] <= VIEW[2] && r[3] <= VIEW[3],
            "{r:?} is outside the {VIEW:?} surface"
        );
    }
}
