//! **Why a small change under a glass panel repaints the panel's whole
//! source** — M50/T50.9, R8-F6, closed on this measurement rather than
//! built.
//!
//! `docs/plan-raster-allocation.md` T50.9 proposed bounding the damage
//! under a panel whose material has `adapt == 0` by the panel's reach:
//! damage ∪ (damage expanded by `glass_source`'s reach), clipped to the
//! source, instead of damage ∪ the whole source. §5's kill rule allowed
//! it (a frosted sheet costs 21.63 ms for a 576 px change,
//! `docs/results-glass.md` §5a). **It is unsound, whatever the reach**,
//! and not because of `adapt`:
//!
//! A partial redraw clears its region and draws every command clipped to
//! it. A panel pixel the region repaints reads the backdrop within the
//! reach of itself — and where that neighbourhood leaves the region, the
//! canvas holds last frame's **finished** pixels. Inside the panel those
//! are the panel's own previous output, not the backdrop, so a region that
//! stops inside the panel redraws its edge from glass instead of from what
//! is behind it. Growing the region to cover those reads puts more panel
//! pixels in it, whose reads leave it again: **every sound region contains
//! the whole panel**. The most the rule could shed is the reach ring
//! around it — and only where nothing drawn after the panel covers that
//! ring. `Diffing::dilate`'s `D ∪= src` is the sound answer; anything
//! smaller than the panel needs the backdrop kept across frames, the cache
//! T19.4 decided against (`docs/results-glass.md` §5).
//!
//! So this pins both directions on a frosted panel with a 24×24 change
//! under it: the shipped region (the whole source) redraws to exactly a
//! full render, and the reach-bounded ones do not — printed, so a future
//! renderer that retains the backdrop turns this red and says the bound
//! has become possible.

use fulgurite::display::{glass_source, Color, DisplayList, Material};
use fulgurite::Renderer;
use fulgurite_shell::software::Canvas;

const PANEL: [f32; 4] = [100.0, 100.0, 700.0, 300.0];
const CHANGE: [f32; 4] = [300.0, 150.0, 324.0, 174.0];

fn scene(k: u32) -> DisplayList {
    let mut l = DisplayList::new();
    for i in 0..24 {
        let y = i as f32 * 20.0;
        let c = if i % 2 == 0 { Color(40, 60, 120, 255) } else { Color(200, 210, 240, 255) };
        l.rect([0.0, y, 800.0, y + 10.0], c);
    }
    let c = if k.is_multiple_of(2) { Color(230, 40, 40, 255) } else { Color(40, 230, 40, 255) };
    l.rect(CHANGE, c);
    l.glass(PANEL, 12.0, Material::FROSTED);
    l
}

/// Pixels by which a partial redraw of `region` differs from a full
/// render of the changed frame.
fn differ(region: [f32; 4]) -> usize {
    let (w, h) = (800u32, 480u32);
    let clear = Color(0, 0, 0, 255);
    let mut inc = Canvas::new(w, h);
    inc.clear(clear);
    inc.render(&scene(0), w, h);
    inc.clear_within(region, clear);
    inc.render_within(&scene(1), region);
    let mut full = Canvas::new(w, h);
    full.clear(clear);
    full.render(&scene(1), w, h);
    inc.pixels.chunks_exact(4).zip(full.pixels.chunks_exact(4)).filter(|(a, b)| a != b).count()
}

#[test]
fn a_region_that_stops_inside_the_panel_redraws_it_wrong() {
    let src = glass_source(PANEL, &Material::FROSTED);
    let reach = PANEL[0] - src[0];
    let grow = |k: f32| {
        [
            (CHANGE[0] - k * reach).max(src[0]),
            (CHANGE[1] - k * reach).max(src[1]),
            (CHANGE[2] + k * reach).min(src[2]),
            (CHANGE[3] + k * reach).min(src[3]),
        ]
    };
    let whole = differ(src);
    let one = differ(grow(1.0));
    let two = differ(grow(2.0));
    // The panel alone: sound here, because nothing drawn after the panel
    // reaches its ring and the change is inside it. The ceiling of what a
    // sound bound could save is the ring, printed as a share of the source.
    let panel = differ(PANEL);
    let area = |r: [f32; 4]| (r[2] - r[0]) * (r[3] - r[1]);
    println!(
        "frosted panel {PANEL:?}, reach {reach} px, 24x24 change: whole source {whole} px differ (bound 0); \
         change + reach {one} px; change + 2 x reach {two} px (the bounded rule would need 0); \
         the panel alone {panel} px, and its ring is {:.1}% of the source",
        100.0 * (1.0 - area(PANEL) / area(src))
    );
    assert_eq!(whole, 0, "the shipped rule, the whole source, left {whole} px wrong");
    assert!(
        one > 0 && two > 0,
        "a reach-bounded region redrew the panel correctly ({one}, {two} px wrong): the backdrop is \
         retained now, and T50.9's bounded damage has become possible -- update docs/results-glass.md §5a"
    );
}
