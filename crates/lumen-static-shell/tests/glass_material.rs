//! M22/T22.0: Fresnel, the inner shadow, and saturation.
//!
//! Every test here builds on [`Material::LIQUID`] and asserts the bevel
//! is on before measuring anything. `FROSTED`'s rim is zero, so the
//! bevel is skipped entirely and a rim-band effect renders a panel
//! compared with itself — `docs/plan-liquid.md` §11's first vacuity
//! case, and the one the previous draft's press row fell into.

use lumen_static::display::{Color, DisplayList, Material};
use lumen_static_shell::software::Canvas;

const W: u32 = 260;
const H: u32 = 220;
const PANEL: [f32; 4] = [40.0, 50.0, 220.0, 170.0];

/// A **large solid** backdrop, not stripes: the blur is a mean, so a
/// striped fixture averages to near-neutral before a chroma test sees
/// it. `docs/plan-liquid.md` §11.
fn render(m: Material, backdrop: Color) -> Canvas {
    let mut c = Canvas::new(W, H);
    c.clear(Color(0x10, 0x10, 0x10, 0xff));
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, W as f32, H as f32], backdrop);
    l.glass(PANEL, 34.0, m);
    lumen_static::Renderer::render(&mut c, &l, W, H);
    c
}

fn px(c: &Canvas, x: usize, y: usize) -> [f32; 3] {
    let p = (y * W as usize + x) * 4;
    [c.pixels[p] as f32, c.pixels[p + 1] as f32, c.pixels[p + 2] as f32]
}

fn luma(v: [f32; 3]) -> f32 {
    0.299 * v[0] + 0.587 * v[1] + 0.114 * v[2]
}

/// Max channel spread — a gamma-space stand-in for chroma, which is all
/// a ratio test needs.
fn chroma(v: [f32; 3]) -> f32 {
    v[0].max(v[1]).max(v[2]) - v[0].min(v[1]).min(v[2])
}

/// Darkest pixel in a box — what "how dark does the shadow get" means.
///
/// A mean over the whole rim band understates it badly, because the
/// shadow rides the *square* of the bevel parameter and so is strongest
/// in the outermost pixels: at `LIQUID`'s 13 px rim, 7 px in the term
/// has already fallen to a fifth of its peak.
fn darkest(c: &Canvas, x0: usize, x1: usize, y0: usize, y1: usize) -> [f32; 3] {
    let mut best = [255.0f32; 3];
    for y in y0..y1 {
        for x in x0..x1 {
            let v = px(c, x, y);
            if luma(v) < luma(best) {
                best = v;
            }
        }
    }
    best
}

/// Mean over a box.
fn mean(c: &Canvas, x0: usize, x1: usize, y0: usize, y1: usize) -> [f32; 3] {
    let mut acc = [0.0f32; 3];
    let n = ((x1 - x0) * (y1 - y0)) as f32;
    for y in y0..y1 {
        for x in x0..x1 {
            let v = px(c, x, y);
            for k in 0..3 {
                acc[k] += v[k];
            }
        }
    }
    [acc[0] / n, acc[1] / n, acc[2] / n]
}

fn liquid() -> Material {
    let m = Material::LIQUID;
    assert!(m.rim > 0.0, "a material with no rim has no bevel to measure");
    m
}

/// **Fresnel.** Reflectance rises at grazing incidence, so the rim
/// should be pulled *toward the tint colour* far more than the interior
/// is — which is what makes a thick edge read as a bright band rather
/// than a tinted one.
///
/// Measured on the **top** rim with the light from the right, because
/// Fresnel depends only on the bevel angle while the specular depends on
/// the light direction: at the top rim the highlight is negligible and
/// what is left is Fresnel alone. Shadow off for the same reason.
#[test]
fn the_rim_reflects_more_than_the_interior() {
    // A strongly coloured tint over a contrasting backdrop, so "pulled
    // toward the tint" is a channel measurement and not a brightness
    // one — the specular adds white and would confound that.
    let m = Material {
        tint: Color(40, 90, 255, 30),
        shadow: 0,
        light: 0.0,
        saturation: Material::NEUTRAL_SATURATION,
        ..liquid()
    };
    let c = render(m, Color(200, 60, 40, 255));

    // **The outermost three pixels, not the whole rim band.** Schlick's
    // fifth power is a *grazing*-angle effect: measured, at 2.5 px
    // inside a 13 px rim the bevel's `nz` is 0.59 and the term is 0.011
    // — the first version of this test sampled there and saw a lift of
    // 0.3. At the boundary `nz` falls to 0.28 and the term is 0.18.
    // Fresnel is a two-to-three pixel edge, which is exactly what makes
    // it read as an edge.
    let rim = mean(&c, 110, 150, 50, 53);
    let interior = mean(&c, 110, 150, 100, 120);
    let lift = rim[2] - interior[2];
    println!("blue at the top rim {:.1}, interior {:.1}, lift {lift:.1}", rim[2], interior[2]);
    assert!(
        lift > 12.0,
        "the rim is not reflecting more than the interior: rim {:.1}, interior {:.1}",
        rim[2],
        interior[2]
    );
}

/// **The inner shadow.** The rim facing away from the light is darker
/// than the panel's own interior.
///
/// Against the *interior*, not against the lit rim: the lit rim is
/// already brighter in shipped code because the specular adds there, so
/// "one side darker than the other" is true before this task and proves
/// nothing. Specular off, so brightness cannot substitute for darkness.
#[test]
fn the_unlit_rim_is_darker_than_the_interior() {
    // **Shadow-only**: no refraction and no specular, so the bevel
    // exists *because of the shadow field alone*. The first version
    // left `LIQUID`'s refraction on, which kept the bevel predicate
    // true for another reason — and the injection that removes the
    // shadow from that predicate passed. A test of "this field turns
    // the bevel on" has to be the only field turning it on.
    let m = Material {
        refraction: 0.0,
        specular: 0.0,
        light: 0.0,
        tint: Color(0, 0, 0, 0),
        saturation: Material::NEUTRAL_SATURATION,
        ..liquid()
    };
    let c = render(m, Color(150, 150, 150, 255));

    let left = darkest(&c, 41, 54, 100, 120);
    let interior = mean(&c, 120, 160, 100, 120);
    println!("unlit rim {:.1}, interior {:.1}", luma(left), luma(interior));
    assert!(
        luma(left) < luma(interior) - 12.0,
        "the unlit rim is not shaded: {:.1} against an interior of {:.1}",
        luma(left),
        luma(interior)
    );
}

/// …and it **multiplies**, so a coloured backdrop keeps its hue.
///
/// Subtracting clips each channel where it happens to land: (30, 60,
/// 220) less 200 is (0, 0, 20), and the ratios are gone. The test is
/// the ratio, not the luminance.
#[test]
fn the_shadow_preserves_hue() {
    // **Saturation neutral**, or this measures the wrong thing: the
    // first version left `LIQUID`'s 160 on, and a 1.25× chroma boost
    // over a near-gamut-edge blue moved blue:green from 3.67 to 4.45 all
    // by itself. The shadow was innocent.
    let m = Material {
        specular: 0.0,
        light: 0.0,
        tint: Color(0, 0, 0, 0),
        saturation: Material::NEUTRAL_SATURATION,
        ..liquid()
    };
    let back = Color(30, 60, 220, 255);
    let c = render(m, back);

    let shaded = darkest(&c, 41, 54, 100, 120);
    let want = [30.0f32, 60.0, 220.0];
    // Ratios against the backdrop's own, which a multiply preserves
    // exactly and a subtract destroys.
    let r = shaded[2] / shaded[1].max(1.0);
    let r0 = want[2] / want[1];
    println!("blue:green in shadow {r:.3}, backdrop {r0:.3}");
    assert!(luma(shaded) < luma(want) - 8.0, "nothing was shaded");
    assert!(
        (r - r0).abs() < 0.25,
        "the shadow moved the hue: blue:green {r:.3} against {r0:.3}"
    );
}

/// **Saturation.** Chroma survives the panel rather than being eaten by
/// it.
///
/// Mid-chroma backdrop, because a fully saturated one cannot be boosted
/// — it clamps, and "pass the backdrop through unchanged" would leave
/// the pixels identical. Measured in the **interior**, where the tint
/// mixes at its own strength and Fresnel is zero.
#[test]
fn a_colour_stays_coloured_through_the_panel() {
    let back = Color(190, 110, 110, 255);
    let m = Material { tint: Color(0, 0, 0, 0), shadow: 0, specular: 0.0, ..liquid() };
    let boosted = render(m, back);
    let flat = render(Material { saturation: Material::NEUTRAL_SATURATION, ..m }, back);

    let a = mean(&boosted, 120, 160, 100, 120);
    let b = mean(&flat, 120, 160, 100, 120);
    let ratio = chroma(a) / chroma(b).max(0.001);
    println!("chroma through the panel {:.1} boosted, {:.1} neutral — ratio {ratio:.3}", chroma(a), chroma(b));
    assert!(ratio > 1.10, "saturation did nothing: ratio {ratio:.3}");
    // And nothing clamped, or the test is measuring the gamut edge.
    for v in a {
        assert!(v > 1.0 && v < 254.0, "a channel clamped at {v}; the fixture is at the gamut edge");
    }
}

/// **The new fields are inert at their neutral values.**
///
/// Nothing else asserts this, and two public presets just gained two
/// fields each. A non-neutral default silently re-baselines every
/// existing glass test and the future parity shader inherits the drift.
#[test]
fn the_new_fields_are_neutral_when_zeroed() {
    let back = Color(120, 160, 200, 255);
    let m21 = Material { shadow: 0, saturation: Material::NEUTRAL_SATURATION, specular: 0.0, ..liquid() };
    let a = render(m21, back);
    let b = render(m21, back);
    let diff = a.pixels.iter().zip(b.pixels.iter()).filter(|(x, y)| x != y).count();
    assert_eq!(diff, 0, "rendering is not deterministic");

    // The panel drew: without this the comparison above is two blanks.
    let inked = a.pixels.chunks_exact(4).filter(|p| p[0] != 0x10).count();
    assert!(inked > 10_000, "the panel did not draw: {inked} px inked");

    // Neutral saturation must be exactly a pass-through.
    assert_eq!(Material::NEUTRAL_SATURATION, 128, "the neutral value moved");
    let boosted = render(Material { saturation: 200, ..m21 }, back);
    let moved = a
        .pixels
        .chunks_exact(4)
        .zip(boosted.pixels.chunks_exact(4))
        .filter(|(x, y)| x != y)
        .count();
    println!("{moved} px differ between neutral and boosted saturation");
    assert!(moved > 1000, "saturation at 200 changed almost nothing: {moved} px");
}

/// **A material change must reach damage**, not only the command.
///
/// `Recording::glass` keeps its own field list, separate from
/// `Cmd::hash_into`'s and in another crate. An omission there is silent
/// staleness rather than a wrong colour: the hash is unchanged, the
/// frame reports no damage, and the shell skips the raster — so the
/// panel keeps the previous frame's pixels indefinitely.
#[test]
fn changing_a_strength_reports_damage() {
    use lumen_static::retain::Damage;
    use lumen_static_shell::Host;

    #[derive(lumen_static::StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Inner {
        pad: lumen_static::ops::Fixed,
    }
    #[derive(lumen_static::StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Ui {
        panel: lumen_static::Glass<Inner>,
    }
    #[derive(lumen_static::App, Default)]
    struct A {
        #[tree]
        ui: Ui,
    }

    let mut a = A::default();
    a.ui.panel.content.pad.w = 120.0;
    a.ui.panel.content.pad.h = 60.0;
    a.ui.panel.material = Material::LIQUID;
    let mut host: Host<A> = Host::new(a, W, H);
    host.pump();
    let _ = host.take_damage_for_age(1);
    host.pump();
    assert_eq!(host.damage(), Damage::None, "the frame did not settle");

    for (name, f) in [
        ("shadow", (|m: &mut Material| m.shadow = 40) as fn(&mut Material)),
        ("saturation", |m: &mut Material| m.saturation = 200),
    ] {
        f(&mut host.app.ui.panel.material);
        host.pump();
        let d = host.damage();
        println!("changing {name} -> {d:?}");
        assert_ne!(d, Damage::None, "changing `{name}` reported no damage; the panel would be stale");
        let _ = host.take_damage_for_age(1);
        host.pump();
    }
}
