//! M22/T22.5: two panels merging like droplets — a smooth minimum of
//! two signed distance fields.

use fulgurite::display::{glass_union, Color, DisplayList, Material};
use fulgurite_shell::software::Canvas;

const W: u32 = 520;
const H: u32 = 240;
/// The primary blob: 120 × 120, on **fractional** edges.
const PANEL: [f32; 4] = [40.37, 50.61, 160.37, 170.61];

fn scene(m: Material) -> Canvas {
    let mut c = Canvas::new(W, H);
    c.clear(Color(0x10, 0x14, 0x1c, 0xff));
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, W as f32, H as f32], Color(0x30, 0x70, 0xb0, 0xff));
    l.rect([0.0, 120.0, W as f32, H as f32], Color(0xe0, 0xd0, 0x60, 0xff));
    // **The union**, which is what the widget emits: the command's rect
    // is the ink, the clip test and the source, so a second blob outside
    // it would be stale, holed and short of backdrop.
    l.glass(glass_union(PANEL, &m), 34.0, m);
    fulgurite::Renderer::render(&mut c, &l, W, H);
    c
}

fn liquid() -> Material {
    let m = Material::LIQUID;
    assert!(m.rim > 0.0, "a material with no rim has no bevel and no neck scale");
    m
}

/// A panel separated from its twin by `gap` px of clear space.
fn apart(gap: f32) -> Material {
    let w = PANEL[2] - PANEL[0];
    Material {
        morph_x: ((w + gap) * Material::MORPH_UNIT) as i16,
        ..liquid()
    }
}

/// Whether a column is inked, judged **against the backdrop that would
/// be there** rather than against a fixed colour: the panel is
/// see-through, so "inked" cannot mean "a particular value".
fn inked_column(c: &Canvas, bare: &Canvas, x: usize) -> usize {
    let mut n = 0;
    for y in 90..150usize {
        let i = (y * W as usize + x) * 4;
        if c.pixels[i..i + 3] != bare.pixels[i..i + 3] {
            n += 1;
        }
    }
    n
}

/// **Both directions, one test** — `docs/plan-liquid.md` §11 is explicit
/// that the far extreme is a *required negative case* and not a vacuity:
/// beyond the blend radius the blobs must read as separate, or the
/// smooth minimum is fusing panels that should not fuse.
#[test]
fn the_neck_closes_near_and_opens_far() {
    let bare = scene(Material { blur: 0.0, rim: 0.0, tint: Color(0, 0, 0, 0), ..liquid() });
    let rim = liquid().rim;
    // The midpoint of the gap, where the neck is thinnest.
    let mid = |gap: f32| (PANEL[2] + gap / 2.0) as usize;

    let near = scene(apart(rim));
    let far = scene(apart(rim * 3.0));
    let n = inked_column(&near, &bare, mid(rim));
    let f = inked_column(&far, &bare, mid(rim * 3.0));
    println!("gap {rim:.0} px (one rim): {n} px inked between the blobs");
    println!("gap {:.0} px (three rims): {f} px inked", rim * 3.0);

    // The second blob exists at all — a gate that only ever looked
    // between the blobs would pass on a renderer that drew one panel.
    let far_blob = inked_column(&far, &bare, (PANEL[2] + rim * 3.0 + 60.0) as usize);
    println!("inside the far blob: {far_blob} px inked");
    assert!(far_blob > 40, "the second blob was not drawn at all ({far_blob} px)");

    assert!(n > 40, "the neck did not close at one rim width ({n} px inked)");
    assert_eq!(f, 0, "the blobs are still joined at three rim widths ({f} px inked)");
}

/// **A lone panel is byte-identical to before this task.**
///
/// M19 records that glass keeps `fill_rounded_within`'s hard-span
/// coverage precisely so a panel is pixel-identical to a rounded rect of
/// the same geometry, and that a pure SDF ramp disagrees by up to 254/255
/// on a fractional edge (`docs/plan-glass.md` §4.1). The SDF path is
/// therefore taken only when a second blob is present — a branch on the
/// command, not the pixel — and this pins that it stayed that way.
///
/// The digest was taken from the renderer at the commit before T22.5 and
/// is unchanged by it. The fixture's edges are deliberately fractional;
/// a panel on integer boundaries would agree by accident and pin
/// nothing.
#[test]
fn a_lone_panel_is_byte_identical_to_before_the_morph() {
    const BEFORE_T22_5: u64 = 0x2e4f_d518_a418_f744;
    let mut c = Canvas::new(300, 220);
    c.clear(Color(0x10, 0x14, 0x1c, 0xff));
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, 300.0, 220.0], Color(0x30, 0x70, 0xb0, 0xff));
    l.rect([0.0, 110.0, 300.0, 220.0], Color(0xe0, 0xd0, 0x60, 0xff));
    l.glass([40.37, 50.61, 250.37, 170.61], 34.0, Material::LIQUID);
    fulgurite::Renderer::render(&mut c, &l, 300, 220);

    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in &c.pixels {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    println!("lone panel digest 0x{h:016x}, before T22.5 0x{BEFORE_T22_5:016x}");
    assert_eq!(h, BEFORE_T22_5, "the morph moved the M19/M21 baseline for an ordinary panel");
}

/// **The neck has no crease in it.**
///
/// Two separate claims in `docs/plan-liquid.md` §7 meet here, and both
/// are about the same 20 px of pixels:
///
/// - The blended gradient is *exact* — weighting the two by the same `h`
///   the value uses leaves no remainder — but its length falls below 1
///   where they disagree and reaches 0 where they oppose. The bevel is
///   built on the normal being unit **by construction**, and that claim
///   is false exactly at the neck. Using one blob's gradient instead of
///   the blend flips the normal where `h` crosses 0.5, and the profile
///   gains a step.
/// - A smooth minimum is not a distance field. The gradient's length is
///   below 1 in the neck, so `u` reaches 0 after fewer than `rim` pixels
///   and the bevel pinches. Dividing the distance by that length
///   restores it, and shares the square root the normalisation needs.
///
/// Measured along a line through the blobs' shared centre, from well
/// inside the first to well inside the second, as the largest jump
/// between adjacent pixels. The bound is printed beside it.
#[test]
fn the_neck_has_no_crease() {
    // **4, not 14.** The first draft bounded this at 14 because that
    // felt like "a visible step"; the *measurement* is 0.1, and both of
    // the injections this test exists for land at 12.5 — under the
    // bound, green, proving nothing. A smooth surface over a uniform
    // backdrop varies by well under a level per pixel, so 4 is already
    // 40x the signal and a third of the artefact.
    const BOUND: f32 = 4.0;
    let rim = liquid().rim;
    let c = scene(apart(rim));
    let y = ((PANEL[1] + PANEL[3]) / 2.0) as usize;
    let lum = |x: usize| {
        let i = (y * W as usize + x) * 4;
        0.299 * c.pixels[i] as f32 + 0.587 * c.pixels[i + 1] as f32 + 0.114 * c.pixels[i + 2] as f32
    };
    // Well inside the first blob, through the neck, well inside the
    // second — never near either outer silhouette, which has a real
    // edge and would dominate the maximum.
    let (from, to) = ((PANEL[0] + 60.0) as usize, (PANEL[2] + rim + 60.0) as usize);
    let (mut worst, mut at) = (0.0f32, 0);
    for x in from..to {
        let j = (lum(x + 1) - lum(x)).abs();
        if j > worst {
            worst = j;
            at = x;
        }
    }
    println!("largest adjacent jump across the neck: {worst:.1} at x={at} (bound {BOUND})");
    assert!(to - from > 100, "the traverse is too short to say anything");
    assert!(worst < BOUND, "the neck has a step of {worst:.1} levels at x={at}");
}

/// **The recorder's hash must carry the offset, and the widget path
/// cannot prove it.**
///
/// `Glass` emits the *union* of both blobs, and the union determines the
/// offset exactly — so changing `morph_x` through the widget moves the
/// command's rect, the rect is hashed, and damage is reported whether or
/// not the material's own fields are. The injection for the recorder's
/// field list stays green through that route, which is a finding about
/// the route and not about the hash.
///
/// `Painter::glass` is public and takes a rect and a material
/// separately, so a caller may hold one fixed while the other moves.
/// This drives `Recording` directly with **the same rect** both times,
/// which is the only arrangement that can tell the two apart.
#[test]
fn the_recorder_hashes_the_offset_even_with_the_rect_held_still() {
    use fulgurite::display::{DisplayList, Painter};
    use fulgurite::retain::{Damage, Incremental, Recording, Retention};

    const R: [f32; 4] = [20.0, 20.0, 140.0, 140.0];
    let mut ret = Incremental::default();
    let mut out = DisplayList::new();
    let mut frame = |m: Material| {
        out.clear();
        {
            let mut rec = Recording::new(&mut out, &mut ret);
            rec.glass(R, 12.0, m);
        }
        ret.finish()
    };

    let base = liquid();
    frame(base);
    assert_eq!(frame(base), Damage::None, "the recording did not settle");
    let moved = frame(Material { morph_x: 320, ..base });
    println!("same rect, morph_x 0 -> 320: {moved:?}");
    assert_ne!(
        moved,
        Damage::None,
        "the offset changed and no damage was reported; the second blob would be stale"
    );
}
