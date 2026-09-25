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

/// M40/T40.1: at a device scale the regions a backend is handed are the
/// **physical** rect that changed, not the logical one.
///
/// The GPU path is handed `regions_for(host.take_damage_for_age(..))`
/// against a physical viewport and snaps each region outward
/// (`lumen-static-wgpu`'s `snap_out`), so the only way it could still
/// leave stale pixels after the wrappers were swapped is a region in
/// the wrong space. At 1.5 a bound label's logical box must be covered
/// at 1.5 × that. With the old order (`Recording` outside `Scaled`) this
/// prints `regions [[0.0, 32.0, 48.0, 48.0]]` against the physical
/// `[0.0, 48.0, 72.0, 72.0]` and fails.
#[test]
fn at_a_device_scale_the_regions_cover_the_physical_rect() {
    use lumen_static::{App as DeriveApp, StaticTree};
    use lumen_static_shell::Host;
    use lumen_static_widgets::ops::Label;

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Ui {
        #[widget_id("pad")]
        pad: Label,
        #[widget_id("value")]
        value: Label<String>,
    }
    #[derive(DeriveApp, Default)]
    struct Tally {
        #[tree]
        ui: Ui,
        #[bind(ui.value, "{}")]
        count: i64,
    }

    let scale = 1.5;
    let (w, h) = (450u32, 300u32);
    let mut app = Tally::default();
    app.ui.pad.set("a line above, so the label is not at the origin");
    let mut host: Host<Tally> = Host::new(app, w, h);
    host.set_scale(scale);
    host.set_stylesheet(&[]);
    host.pump();
    host.pump();
    host.take_damage_for_age(1);

    host.app.count = 123_456;
    host.pump();
    let d = host.take_damage_for_age(1);
    assert!(matches!(d, Damage::Regions(_)), "setup: a label change must damage regions, got {d:?}");
    let out = regions_for(d, [0.0, 0.0, w as f32, h as f32]);

    let table = <Ui as lumen_static::Widget>::SUB_TABLE;
    let i = lumen_static::style::index_of_wid(table, "value").expect("id");
    let st = host.states[i];
    let phys = [st.pos.x * scale, st.pos.y * scale, (st.pos.x + st.size.w) * scale, (st.pos.y + st.size.h) * scale];
    let covered = out.iter().any(|r| r[0] <= phys[0] && r[1] <= phys[1] && r[2] >= phys[2] && r[3] >= phys[3]);
    println!("scale {scale}: regions {out:?}; the label's physical rect {phys:?} (must be inside one)");
    assert!(covered, "no region covers the label's physical rect {phys:?}: {out:?}");
}
