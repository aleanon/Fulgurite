//! M19/T19.0: the `Glass` command and its plumbing.
//!
//! The filter itself is T19.2's. What is checked here is that the
//! command exists, costs nothing, survives every seam between a widget
//! and a renderer, and draws *something* everywhere.

use lumen_static::display::{Color, Material, GLASS_FALLBACK_ALPHA};
use lumen_static::{Cmd, DisplayList, Painter};

/// `Cmd` must not have grown, **and `Glass` must not be what sets its
/// size** — two assertions, because the first alone is a bound 32 bytes
/// from anything this variant does.
#[test]
fn the_command_is_free() {
    // M80/T80.1: `Text` gained `line_height: u8` (payload 36 -> 37 B),
    // still under the 56 B that sets the size.
    println!("Cmd is {} B (bound 60)", core::mem::size_of::<Cmd>());
    assert_eq!(core::mem::size_of::<Cmd>(), 60, "Cmd grew; every command in every list grew");

    // The payload, mirrored: `size_of` cannot see into a variant.
    struct GlassPayload {
        _rect: [f32; 4],
        _radius: f32,
        _material: Material,
    }
    let payload = core::mem::size_of::<GlassPayload>();
    // `Gradient`'s 56 is what sets `Cmd`'s size. Printed beside the
    // bound so a reader can see the headroom rather than trust it.
    println!("Glass payload {payload} B against Gradient's 56 B, which sets Cmd's 60");
    assert!(
        payload <= 56,
        "Glass ({payload} B) would set Cmd's size; that is a re-budget, not an edit"
    );
}

/// No two command kinds may fold to the same value for the same
/// geometry, or retention cannot tell one becoming the other apart.
///
/// **This is a weaker claim than "the tags are distinct", and the
/// difference is the point.** `Shadow` and `Image` both hashed as tag 6
/// until 2026-09-10 and still never collided, because they fold a
/// different *number* of values after it. So the tag reuse was an
/// inconsistency worth fixing — `Recording`'s parallel numbering already
/// had `Image` at 7 — and not a live defect, and this test says which:
/// giving `Glass` tag 6 alone leaves it green. It goes red only when a
/// kind folds the same tag *and* the same sequence, which is the
/// condition that actually loses information.
#[test]
fn no_two_command_kinds_fold_alike() {
    let r = [0.0, 0.0, 10.0, 10.0];
    let c = Color(1, 2, 3, 255);
    let cmds = [
        Cmd::Rect { rect: r, color: c },
        Cmd::RoundedRect { rect: r, color: c, radius: 0.0 },
        Cmd::Shadow { rect: r, color: c, radius: 0.0, spread: 0.0 },
        Cmd::Image { rect: r, id: lumen_static::display::ImageId(0), tint: c },
        Cmd::Glass { rect: r, radius: 0.0, material: Material { tint: c, ..Material::FROSTED } },
        Cmd::PushClip { rect: r },
        Cmd::PopClip,
    ];
    let mut seen: Vec<u64> = Vec::new();
    for cmd in &cmds {
        let mut h = 0u64;
        cmd.hash_into(&mut h);
        assert!(!seen.contains(&h), "two kinds hash alike: {cmd:?} collides");
        seen.push(h);
    }
}

/// **Every `Material` field is part of the command's identity**, and
/// the test is table-driven so a new field cannot be added without one.
///
/// Omitting a field from the recorder's hash is **silent staleness**,
/// not a wrong colour: the hash is unchanged, damage reports nothing,
/// the shell skips the raster, and the panel keeps the previous frame's
/// pixels indefinitely. The two hand-written field lists live in
/// different crates — the same shape that let `Shadow` and `Image` both
/// hash as tag 6 for a year.
///
/// The exhaustive destructure below is what makes it table-driven
/// rather than hand-enumerated: **adding a field to `Material` stops
/// this test compiling**, which is a better signal than any count —
/// the first version compared the mutator count against
/// `size_of::<Material>()` and tripped over two bytes of alignment
/// padding, which is arithmetic about the wrong thing.
#[test]
fn every_material_field_is_in_the_hash() {
    let base = Cmd::Glass { rect: [0.0; 4], radius: 2.0, material: Material::LIQUID };
    let hash = |c: &Cmd| {
        let mut h = 0u64;
        c.hash_into(&mut h);
        h
    };
    type Mut = (&'static str, fn(&mut Material));
    let mutators: &[Mut] = &[
        ("blur", |m| m.blur += 1.0),
        ("tint", |m| m.tint = Color(m.tint.0, m.tint.1, m.tint.2, m.tint.3.wrapping_add(7))),
        ("rim", |m| m.rim += 1.0),
        ("refraction", |m| m.refraction += 1.0),
        ("specular", |m| m.specular += 0.1),
        ("light", |m| m.light += 0.3),
        ("shadow", |m| m.shadow = m.shadow.wrapping_add(9)),
        ("saturation", |m| m.saturation = m.saturation.wrapping_add(9)),
        ("dispersion", |m| m.dispersion = m.dispersion.wrapping_add(9)),
        ("adapt", |m| m.adapt = m.adapt.wrapping_add(9)),
        ("tint_over_light", |m| {
            let t = m.tint_over_light;
            m.tint_over_light = Color(t.0, t.1, t.2, t.3.wrapping_add(7));
        }),
        ("morph_x", |m| m.morph_x = m.morph_x.wrapping_add(9)),
        ("morph_y", |m| m.morph_y = m.morph_y.wrapping_add(9)),
    ];

    // No `..`: a new field is a compile error here, and the message
    // points at the test that has to gain a mutator.
    let Material {
        blur,
        tint,
        rim,
        refraction,
        specular,
        light,
        shadow,
        saturation,
        dispersion,
        adapt,
        tint_over_light,
        morph_x,
        morph_y,
    } =
        Material::LIQUID;
    let _ =
        (blur, tint, rim, refraction, specular, light, shadow, saturation, dispersion, adapt);
    let _ = (tint_over_light, morph_x, morph_y);
    assert_eq!(mutators.len(), 13, "a field was added to Material without a mutator here");

    for (name, f) in mutators {
        let mut changed = base;
        if let Cmd::Glass { material, .. } = &mut changed {
            f(material);
        }
        assert_ne!(hash(&base), hash(&changed), "changing `{name}` did not change the hash");
    }
}

/// Alpha 0 is a real value/// Alpha 0 is a real value — a pure frosted panel — and must survive the
/// collector, which drops a transparent `rounded_rect`.
#[test]
fn a_fully_transparent_tint_is_not_dropped() {
    let mut list = DisplayList::new();
    list.glass(
        [0.0, 0.0, 10.0, 10.0],
        0.0,
        Material { blur: 6.0, tint: Color(0, 0, 0, 0), ..Material::FROSTED },
    );
    assert_eq!(list.cmds.len(), 1, "a zero-alpha tint with a blur is a frosted panel");

    let mut empty = DisplayList::new();
    empty.glass([5.0, 5.0, 5.0, 5.0], 0.0, Material::FROSTED);
    assert!(empty.cmds.is_empty(), "no area is still no command");
}

/// The degraded fallback has to be *visible*, or every test of it is a
/// test that nothing happened.
#[test]
fn the_painter_fallback_paints_something_you_can_see() {
    /// A painter with no glass support — every leaf renderer that has
    /// not implemented it yet.
    #[derive(Default)]
    struct Plain(Vec<(Color, f32)>);
    impl Painter for Plain {
        fn rect(&mut self, _: [f32; 4], _: Color) {}
        fn text_spaced(&mut self, _: [f32; 4], _: &str, _: f32, _: f32, _: u8, _: Color) {}
        fn rounded_rect(&mut self, _: [f32; 4], color: Color, radius: f32) {
            self.0.push((color, radius));
        }
        fn shadow(&mut self, _: [f32; 4], _: f32, _: f32) {}
        fn stroked_rect(&mut self, _: [f32; 4], _: Color, _: f32, _: f32) {}
        fn push_clip(&mut self, _: [f32; 4]) {}
        fn pop_clip(&mut self) {}
    }

    let mut p = Plain::default();
    // A realistic glass tint: 16% white. Painted flat that is invisible.
    p.glass(
        [0.0, 0.0, 10.0, 10.0],
        4.0,
        Material { tint: Color(255, 255, 255, 40), ..Material::FROSTED },
    );
    assert_eq!(p.0.len(), 1, "the fallback must draw, not skip");
    let (c, radius) = p.0[0];
    println!("fallback alpha {} from a tint of 40", c.3);
    assert_eq!(c.3, GLASS_FALLBACK_ALPHA, "the fallback raises alpha to a floor you can see");
    assert!(c.3 > 40 * 2, "and the floor is well above a realistic tint");
    assert_eq!(radius, 4.0, "the panel keeps its corners");
}
