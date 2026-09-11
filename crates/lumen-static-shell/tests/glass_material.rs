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
    // `adapt: 0` — M22/T22.3 gave `LIQUID` an adaptive tint, and this
    // test's whole premise is a *fixed* one it can name. Left on, the
    // material blends toward `tint_over_light` and the test measures a
    // colour it never chose.
    // A strongly coloured tint over a contrasting backdrop, so "pulled
    // toward the tint" is a channel measurement and not a brightness
    // one — the specular adds white and would confound that.
    let m = Material {
        tint: Color(40, 90, 255, 30),
        shadow: 0,
        light: 0.0,
        saturation: Material::NEUTRAL_SATURATION,
        adapt: 0,
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
        ("dispersion", |m: &mut Material| m.dispersion = 200),
        ("adapt", |m: &mut Material| m.adapt = 40),
        ("tint_over_light", |m: &mut Material| m.tint_over_light = Color(9, 9, 9, 99)),
        ("morph_x", |m: &mut Material| m.morph_x = 160),
        ("morph_y", |m: &mut Material| m.morph_y = 80),
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

/// **M22/T22.1: the refracted read is sub-pixel.**
///
/// It truncated to a whole surface pixel until then, and the downsample
/// factor is 1 at the liquid blur — so the four bilinear taps collapsed
/// to one and the refraction was nearest-neighbour, with a staircase on
/// its rim.
///
/// The property a test can hold onto: a **sub-pixel** change in the
/// panel's position must produce a change in the refracted rim. Under
/// the old integer read, moving the panel by less than a pixel moved
/// nothing at all — which is the definition of the staircase.
#[test]
fn a_sub_pixel_move_changes_the_refracted_rim() {
    let m = Material { shadow: 0, specular: 0.0, tint: Color(0, 0, 0, 0), ..liquid() };
    let scene = |dx: f32| {
        let mut c = Canvas::new(W, H);
        c.clear(Color(0x10, 0x10, 0x10, 0xff));
        let mut l = DisplayList::new();
        // A hard edge **where the refracted reads actually land**, not
        // merely under the rim. On the left rim the read is displaced
        // *outward* by up to `refraction` px, so a pixel at x = 45 reads
        // near x = 41: an edge at 60 is never sampled, and the first
        // version of this test put it there and measured 0 of 800.
        l.rect([0.0, 0.0, 45.0, H as f32], Color(0xf0, 0xf0, 0xf0, 0xff));
        l.rect([45.0, 0.0, W as f32, H as f32], Color(0x28, 0x28, 0x28, 0xff));
        l.glass([40.0 + dx, 50.0, 220.0 + dx, 170.0], 34.0, m);
        lumen_static::Renderer::render(&mut c, &l, W, H);
        c
    };
    let a = scene(0.0);
    let b = scene(0.35);
    let (mut differ, mut checked) = (0, 0);
    for y in 90..130usize {
        for x in 40..60usize {
            checked += 1;
            if (luma(px(&a, x, y)) - luma(px(&b, x, y))).abs() > 1.0 {
                differ += 1;
            }
        }
    }
    println!("{differ} of {checked} rim px move under a 0.35 px shift");
    assert!(checked > 500, "not enough sampled: {checked}");
    assert!(
        differ > checked / 4,
        "a sub-pixel move changed almost nothing ({differ}/{checked}) — the read is still quantised"
    );
}

// ---- M22/T22.2: dispersion ------------------------------------------

/// Mean over the **left** rim band, where the outward direction is −x.
fn left_rim(c: &Canvas) -> [f32; 3] {
    mean(c, 41, 50, 90, 130)
}

/// **Blue deviates more than red.** Normal dispersion — the refractive
/// index falls with wavelength — and the *ordering* is the whole claim:
/// a fringe with the channels the other way round is not a weaker lens,
/// it is colour noise on the wrong side of the edge.
///
/// So the assertion is signed. The backdrop steps from white on the
/// outside to dark under the panel, and refraction displaces the read
/// *outward*; blue is displaced furthest, so blue reads the brighter
/// side hardest and the left rim must come out blue-over-red.
#[test]
fn blue_refracts_further_than_red() {
    let base = Material {
        shadow: 0,
        specular: 0.0,
        tint: Color(0, 0, 0, 0),
        saturation: Material::NEUTRAL_SATURATION,
        ..liquid()
    };
    assert!(base.refraction > 0.0, "no refraction means no dispersion to measure");
    let scene = |d: u8| {
        let mut c = Canvas::new(W, H);
        c.clear(Color(0x10, 0x10, 0x10, 0xff));
        let mut l = DisplayList::new();
        // The step sits at the panel's own left edge, so the outward
        // reads straddle it — the same lesson as T22.1's fixture.
        l.rect([0.0, 0.0, 40.0, H as f32], Color(0xff, 0xff, 0xff, 0xff));
        l.rect([40.0, 0.0, W as f32, H as f32], Color(0x20, 0x20, 0x20, 0xff));
        l.glass(PANEL, 34.0, Material { dispersion: d, ..base });
        lumen_static::Renderer::render(&mut c, &l, W, H);
        c
    };

    // The accepting direction: with dispersion off, an achromatic
    // backdrop must stay achromatic. A gate that only refuses is a gate
    // that refuses everything.
    let off = left_rim(&scene(0));
    println!("dispersion off: rim chroma {:.2}", chroma(off));
    assert!(chroma(off) < 1.0, "the rim is not achromatic without dispersion: {off:?}");

    let on = left_rim(&scene(255));
    let split = on[2] - on[0];
    println!("dispersion on: blue − red = {split:+.2} (rim mean {on:?})");
    assert!(
        split > 4.0,
        "no blue-over-red fringe ({split:+.2}) — dispersion is absent, or inverted"
    );
}

/// **At the capture edge the fringe is switched off, not clamped.**
///
/// Three displaced reads clamp at three different distances, so a panel
/// flush with the window edge grows a coloured band out of a backdrop
/// that has no colour in it at all. The guard drops to the shared read
/// the moment any extreme would clamp, which makes that band achromatic
/// — the same degradation the pre-T22.2 renderer had.
///
/// **The property is measured over a grey ramp**, and both halves of it
/// are asserted, because each alone is satisfiable by a renderer that is
/// wrong in the other direction:
///
/// - inside the clamped band the output must carry *no* chroma, and
/// - immediately outside it the fringe must still be there, or the
///   "fix" is just dispersion switched off everywhere.
///
/// Two things the first two drafts of this test got wrong, both worth
/// keeping:
///
/// - A **flat** backdrop cannot show the artefact. A clamped read and an
///   unclamped one differ in *position*, and a flat field gives that
///   difference no value to express itself in — so deleting the guard
///   left the test green. Hence the ramp.
/// - Sampling out to x = 10 at `LIQUID`'s 11 px refraction spans the
///   clamped band *and* the region where dispersion is legitimately
///   working, so the test failed with the guard in place. The band's
///   width scales with the refraction; at 60 px it is 25 columns wide
///   and there is room to sample each side separately.
#[test]
fn a_clamped_read_invents_no_colour() {
    // A deep bevel, so the clamped band is wide enough to sample apart
    // from the fringe beyond it.
    let base = Material {
        shadow: 0,
        specular: 0.0,
        tint: Color(0, 0, 0, 0),
        saturation: Material::NEUTRAL_SATURATION,
        rim: 60.0,
        refraction: 60.0,
        dispersion: 255,
        ..liquid()
    };
    // Flush with the left edge: the source clamps to x = 0 and the left
    // rim's outward reads run off it.
    const FLUSH: [f32; 4] = [0.0, 50.0, 180.0, 170.0];
    const BAND: usize = 25;
    let mut c = Canvas::new(W, H);
    c.clear(Color(0x10, 0x10, 0x10, 0xff));
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, W as f32, H as f32], Color(0x20, 0x20, 0x20, 0xff));
    // An achromatic ramp across the clamp: every source pixel is grey,
    // so any chroma in the result was invented by the renderer.
    for i in 0..40u32 {
        let v = 255u32.saturating_sub(i * 6) as u8;
        l.rect([i as f32, 0.0, i as f32 + 1.0, H as f32], Color(v, v, v, 0xff));
    }
    l.glass(FLUSH, 34.0, base);
    lumen_static::Renderer::render(&mut c, &l, W, H);

    let (mut worst, mut checked) = (0.0f32, 0);
    for y in 90..130usize {
        for x in 0..BAND {
            checked += 1;
            worst = worst.max(chroma(px(&c, x, y)));
        }
    }
    // And the other direction: the fringe survives just outside.
    let mut beyond = 0.0f32;
    for y in 90..130usize {
        for x in BAND + 1..BAND + 9 {
            beyond = beyond.max(chroma(px(&c, x, y)));
        }
    }
    println!("clamped band x<{BAND}: worst chroma {worst:.0}; just beyond: {beyond:.0}");
    assert!(checked > 900, "not enough sampled: {checked}");
    assert_eq!(worst, 0.0, "a clamped read invented {worst:.0} of chroma from a grey ramp");
    assert!(beyond > 10.0, "no fringe outside the band ({beyond:.0}) — dispersion is off everywhere");
}

/// **The captured backdrop has to reach as far as the blue channel
/// does.** `glass_source` bounds both the capture and the damage
/// dependency, so a source that stops short means the outermost fringe
/// is blurred from edge-extended data — identically in a full and a
/// partial render, which is why no other gate in this milestone can see
/// it.
///
/// The formula carries `+ 4` px of slack, and **the slack alone covers
/// the dispersive widening at any ordinary refraction**: at `LIQUID`'s
/// 11 px it is 11 × 0.25 = 2.75 px, well under 4. A fixture built on
/// `LIQUID` therefore measures the slack and not the term — the first
/// version of this test did exactly that and reported 0 of 240.
///
/// This one is deliberately outside the slack. At refraction 100 the
/// widening is 25 px, and the two formulas disagree about a 25 px band
/// that the blue read genuinely lands in:
///
/// ```text
/// with the term:  ceil(9 + 100 × 1.25 + 4) = 138  capture from x = 162
/// without it:     ceil(9 + 100        + 4) = 113  capture from x = 187
/// ```
///
/// The probe stripe lies in that band. Two independent things then go
/// wrong with the short source — the band is uncaptured *and* the blue
/// coordinate clamps, so the guard drops the fringe — and both point the
/// same way, which is why the assertion is one-sided.
#[test]
fn the_source_reaches_as_far_as_the_blue_channel() {
    const WIDE: u32 = 600;
    const PANEL2: [f32; 4] = [300.0, 50.0, 560.0, 170.0];
    // Inside the correct capture (x >= 162), outside the short one.
    const STRIPE: [f32; 4] = [163.0, 0.0, 187.0, 220.0];
    let m = Material {
        shadow: 0,
        specular: 0.0,
        tint: Color(0, 0, 0, 0),
        saturation: Material::NEUTRAL_SATURATION,
        refraction: 100.0,
        dispersion: 255,
        ..liquid()
    };
    let scene = |stripe: Color| {
        let mut c = Canvas::new(WIDE, H);
        c.clear(Color(0x10, 0x10, 0x10, 0xff));
        let mut l = DisplayList::new();
        l.rect([0.0, 0.0, WIDE as f32, H as f32], Color(0x40, 0x40, 0x40, 0xff));
        l.rect(STRIPE, stripe);
        l.glass(PANEL2, 34.0, m);
        lumen_static::Renderer::render(&mut c, &l, WIDE, H);
        c
    };
    let plain = scene(Color(0x40, 0x40, 0x40, 0xff));
    let lit = scene(Color(0xff, 0xff, 0xff, 0xff));
    // Blue only: the stripe is far outside where red and green read, so
    // a change in any other channel would mean something else moved.
    let blue = |c: &Canvas, x: usize, y: usize| c.pixels[(y * WIDE as usize + x) * 4 + 2] as f32;
    // **One column.** The displacement rides u², so it collapses fast:
    // at x = 301 the read has already come back inside the band the
    // short source does capture, and a wider box would dilute a real
    // signal into a minority and make the bound unreachable.
    let (mut worst, mut differ, mut checked) = (0.0f32, 0, 0);
    for y in 86..134usize {
        for x in 300..301usize {
            checked += 1;
            let d = (blue(&plain, x, y) - blue(&lit, x, y)).abs();
            worst = worst.max(d);
            if d > 1.0 {
                differ += 1;
            }
        }
    }
    println!("stripe in the disputed band moves {differ} of {checked} rim px, worst blue delta {worst:.1}");
    assert!(checked > 40, "not enough sampled: {checked}");
    assert!(
        differ > checked * 3 / 4,
        "the outer rim cannot see the stripe ({differ}/{checked}) — the source stops short of the blue read"
    );
}

// ---- M22/T22.3: the adaptive tint -----------------------------------

/// The panel's interior, minus the backdrop it sits on.
///
/// Reading the interior alone would confound the tint with the backdrop
/// — both rise together through the sweep. The *difference* isolates
/// what the material contributed: positive where the tint adds light,
/// negative where it subtracts.
///
/// A mean over a box, not one pixel, because `u8` output quantises a
/// per-step movement of about one level into ties and "strictly
/// monotonic" would then be measuring rounding.
fn lift(c: &Canvas, backdrop: f32) -> f32 {
    luma(mean(c, 100, 160, 95, 125)) - backdrop
}

/// **The tint follows the backdrop, smoothly and all the way across.**
///
/// Three things are asserted, and the plan requires all three because
/// each alone accepts a renderer that is wrong in a way the others
/// catch: monotonic accepts a *constant*, travel accepts a jump, and
/// neither notices that the interesting region was never entered.
#[test]
fn the_tint_sweeps_across_the_backdrop() {
    let m = liquid();
    assert!(m.adapt > 0, "a material that does not adapt has nothing to sweep");
    let mut prev = f32::INFINITY;
    let (mut first, mut last, mut crossed) = (0.0f32, 0.0f32, false);
    let mut worst_step = 0.0f32;
    for i in 0..24u32 {
        let v = (24 + i * 9) as u8;
        let l = lift(&render(m, Color(v, v, v, 0xff)), v as f32);
        println!("  backdrop {v:>3} -> lift {l:+.3}");
        if i == 0 {
            first = l;
        } else {
            assert!(l < prev, "step {i} did not move the tint: {l:.2} vs {prev:.2}");
            worst_step = worst_step.max(prev - l);
            if (prev > 0.0) != (l > 0.0) {
                crossed = true;
            }
        }
        prev = l;
        last = l;
    }
    let travel = first - last;
    println!("tint travel {travel:.1} levels (from {first:+.1} to {last:+.1}), largest step {worst_step:.2}");
    assert!(travel > 20.0, "the tint barely moved across the whole sweep: {travel:.1}");
    assert!(crossed, "the sweep never crossed the threshold — it stayed on one side of neutral");
}

/// **The pre-registered exit — `docs/plan-liquid.md` §5.**
///
/// The tint is a pure function of the captured backdrop, so it changes
/// whenever content scrolls under the panel. The bound was fixed before
/// the measurement, and it is a principled one rather than a round
/// number: **less than one level of `u8` per frame**, because movement
/// below the quantisation of the output cannot be seen to flicker. If a
/// one-pixel scroll had exceeded it, the task stops and the question of
/// state reopens rather than the bound moving.
#[test]
fn a_scroll_moves_the_tint_by_less_than_one_level_a_frame() {
    const BOUND: f32 = 1.0;
    let m = liquid();
    let tinted = |dy: f32, a: u8| {
        let mut c = Canvas::new(W, H);
        c.clear(Color(0x10, 0x10, 0x10, 0xff));
        let mut l = DisplayList::new();
        // A hard light/dark boundary scrolling through the panel: the
        // steepest input the mean can be given.
        l.rect([0.0, 0.0, W as f32, H as f32], Color(0x18, 0x18, 0x18, 0xff));
        l.rect([0.0, 40.0 + dy, W as f32, H as f32], Color(0xf0, 0xf0, 0xf0, 0xff));
        l.glass(PANEL, 34.0, Material { adapt: a, ..m });
        lumen_static::Renderer::render(&mut c, &l, W, H);
        luma(mean(&c, 100, 160, 95, 125))
    };
    // **The adaptive contribution alone**, by differencing against the
    // same frame with `adapt: 0`. Measuring the panel's absolute
    // luminance instead measures the *backdrop* scrolling under the
    // sample box — 6.8 levels a frame, none of it the tint, and the
    // bound fired on a renderer that was behaving.
    let at = |dy: f32| tinted(dy, m.adapt) - tinted(dy, 0);
    let (mut worst, mut prev, mut total) = (0.0f32, at(0.0), 0.0f32);
    for i in 1..90u32 {
        let v = at(i as f32);
        worst = worst.max((v - prev).abs());
        total += (v - prev).abs();
        prev = v;
    }
    // **The bound is per panel size, and that is worth measuring rather
    // than deriving.** The mean is over the panel's own rect, so a
    // one-pixel scroll changes it by (contrast x width) / area — which
    // grows as the panel shrinks. A short panel is the hostile case.
    let short = |dy: f32, a: u8| {
        let mut c = Canvas::new(W, H);
        c.clear(Color(0x10, 0x10, 0x10, 0xff));
        let mut l = DisplayList::new();
        l.rect([0.0, 0.0, W as f32, H as f32], Color(0x18, 0x18, 0x18, 0xff));
        l.rect([0.0, 40.0 + dy, W as f32, H as f32], Color(0xf0, 0xf0, 0xf0, 0xff));
        l.glass([40.0, 90.0, 220.0, 130.0], 18.0, Material { adapt: a, ..m });
        lumen_static::Renderer::render(&mut c, &l, W, H);
        luma(mean(&c, 100, 160, 105, 115))
    };
    let mut short_worst = 0.0f32;
    let mut sp = short(0.0, m.adapt) - short(0.0, 0);
    for i in 1..90u32 {
        let v = short(i as f32, m.adapt) - short(i as f32, 0);
        short_worst = short_worst.max((v - sp).abs());
        sp = v;
    }
    println!("scroll: worst per-frame move {worst:.3} levels (bound {BOUND}), total {total:.1}");
    println!("        same scroll under a 40 px panel: {short_worst:.3} levels");
    assert!(total > 5.0, "the boundary never moved the panel — the fixture scrolls nothing");
    assert!(worst < BOUND, "a one-pixel scroll moved the tint {worst:.3} levels, over the {BOUND} bound");
}

/// **The mean is reduced over the panel, not over the clip.**
///
/// `bounds` is the panel rect intersected with the clip. Reduce over it
/// and a clipped panel computes a different mean from an unclipped one
/// — and then differs on *every pixel they share*, not merely at the
/// edge. Clipping must restrict which pixels are written and never what
/// colour they are.
///
/// **The plan expected this to show up as a damage-oracle failure, and
/// it does not.** `Recording::glass` registers the whole
/// `glass_source` rect as a dependency, so a partial redraw that
/// touches the panel already carries a clip containing all of it, and
/// `clipped(rect, clip)` returns `rect` unchanged — the injection has
/// nothing to bite on. A `PushClip` in the list is the situation that
/// actually distinguishes the two, and it is a real one: a glass panel
/// inside a `ScrollView` is clipped by its viewport every frame.
///
/// Reducing over the dilated capture instead would be simply the wrong
/// colour, equally wrong in every render, and no gate would see it.
#[test]
fn a_clipped_panel_computes_the_same_tint() {
    let m = liquid();
    let list = {
        let mut l = DisplayList::new();
        l.rect([0.0, 0.0, W as f32, H as f32], Color(0x18, 0x18, 0x18, 0xff));
        l.rect([0.0, 110.0, W as f32, H as f32], Color(0xf0, 0xf0, 0xf0, 0xff));
        l.glass(PANEL, 34.0, m);
        l
    };
    let mut full = Canvas::new(W, H);
    full.clear(Color(0x10, 0x10, 0x10, 0xff));
    lumen_static::Renderer::render(&mut full, &list, W, H);

    // The same scene with the panel clipped to a slice — deliberately
    // asymmetric and entirely above the light/dark boundary, so a mean
    // taken over it lands far from the mean over the whole panel.
    const PART: [f32; 4] = [60.0, 60.0, 200.0, 100.0];
    let mut partial = Canvas::new(W, H);
    partial.clear(Color(0x10, 0x10, 0x10, 0xff));
    {
        let mut l = DisplayList::new();
        l.rect([0.0, 0.0, W as f32, H as f32], Color(0x18, 0x18, 0x18, 0xff));
        l.rect([0.0, 110.0, W as f32, H as f32], Color(0xf0, 0xf0, 0xf0, 0xff));
        l.push_clip(PART);
        l.glass(PANEL, 34.0, m);
        l.pop_clip();
        lumen_static::Renderer::render(&mut partial, &l, W, H);
    }

    let (mut differ, mut checked, mut worst) = (0, 0, 0i32);
    for y in 62..98usize {
        for x in 62..198usize {
            checked += 1;
            let (a, b) = (px(&full, x, y), px(&partial, x, y));
            let d = (0..3).map(|k| (a[k] - b[k]).abs() as i32).max().unwrap();
            worst = worst.max(d);
            if d != 0 {
                differ += 1;
            }
        }
    }
    println!("clipped vs unclipped inside the clip: {differ} of {checked} px differ, worst {worst}");
    assert!(checked > 3000, "not enough sampled: {checked}");
    assert_eq!(differ, 0, "the clipped panel painted {differ} of {checked} px differently (worst {worst})");
}

/// **The mean the renderer reduces is a *blurred* mean, and a blurred
/// mean over a sub-region is not the true mean of that sub-region.**
///
/// Every sample inside the panel mixes in backdrop from up to 3σ
/// outside it, so content the panel does not cover still votes on the
/// panel's tint. `docs/plan-liquid.md` §5 owes this number before it
/// trusts its input.
///
/// Measured as the tint shift between two scenes with an **identical**
/// backdrop under the panel and opposite surroundings, which is exactly
/// the contamination and nothing else. The bound was set before the
/// measurement at 20% of the sweep's full 31-level travel: beyond that,
/// the panel's colour would be substantially chosen by content it does
/// not cover.
#[test]
fn the_blurred_mean_leaks_little_from_outside_the_panel() {
    const BOUND: f32 = 6.2;
    let m = liquid();
    let surrounded = |outside: Color| {
        let mut c = Canvas::new(W, H);
        c.clear(Color(0x10, 0x10, 0x10, 0xff));
        let mut l = DisplayList::new();
        l.rect([0.0, 0.0, W as f32, H as f32], outside);
        // Identical under the panel in both scenes.
        l.rect(PANEL, Color(0x80, 0x80, 0x80, 0xff));
        l.glass(PANEL, 34.0, m);
        lumen_static::Renderer::render(&mut c, &l, W, H);
        luma(mean(&c, 100, 160, 95, 125))
    };
    let dark = surrounded(Color(0x00, 0x00, 0x00, 0xff));
    let light = surrounded(Color(0xff, 0xff, 0xff, 0xff));
    let leak = (dark - light).abs();
    println!("blurred-mean leak from outside the panel: {leak:.2} levels (bound {BOUND})");
    assert!(leak < BOUND, "content outside the panel moved its tint {leak:.2} levels");
}
