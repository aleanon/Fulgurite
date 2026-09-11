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

/// `blur` is part of the identity: it sizes the region the command
/// reads, so a change to it must be visible to retention.
#[test]
fn the_hash_covers_blur_and_tint() {
    let base = Cmd::Glass {
        rect: [0.0; 4],
        radius: 2.0,
        material: Material { blur: 4.0, tint: Color(1, 1, 1, 40), ..Material::FROSTED },
    };
    let hash = |c: &Cmd| {
        let mut h = 0u64;
        c.hash_into(&mut h);
        h
    };
    let mut wider = base;
    if let Cmd::Glass { material, .. } = &mut wider {
        material.blur = 8.0;
    }
    let mut tinted = base;
    if let Cmd::Glass { material, .. } = &mut tinted {
        material.tint = Color(1, 1, 1, 80);
    }
    // `refraction` widens the source region too, so it is as
    // damage-relevant as `blur` and must be in the identity.
    let mut bent = base;
    if let Cmd::Glass { material, .. } = &mut bent {
        material.refraction = 6.0;
    }
    assert_ne!(hash(&base), hash(&wider), "a widened blur must change the hash");
    assert_ne!(hash(&base), hash(&tinted), "a changed tint must change the hash");
    assert_ne!(hash(&base), hash(&bent), "a widened refraction must change the hash");
}

/// Alpha 0 is a real value — a pure frosted panel — and must survive the
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
        fn text_weighted(&mut self, _: [f32; 4], _: &str, _: f32, _: f32, _: Color) {}
        fn rounded_rect(&mut self, _: [f32; 4], color: Color, radius: f32) {
            self.0.push((color, radius));
        }
        fn shadow(&mut self, _: [f32; 4], _: f32, _: f32) {}
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
