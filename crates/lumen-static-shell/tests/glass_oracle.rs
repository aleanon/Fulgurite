//! M19/T19.2: a glass panel is pixel-identical under a partial redraw.
//!
//! The property, and the only one that matters: **a panel over a
//! backdrop that moves must look the same whether the frame was redrawn
//! wholly or only where damage said.** A backdrop filter is the one
//! command that reads what is already in the buffer, so the rule every
//! other command obeys for free — replay the list, get the same pixels —
//! does not hold for it without `observe_dep`.
//!
//! # The three ways this test could prove nothing
//!
//! All three are `AGENT.md`'s documented shapes, and all three are
//! guarded below rather than hoped about.
//!
//! 1. **A static backdrop cannot exhibit a stale panel.** If nothing
//!    moves behind the glass, both renders agree about a frame in which
//!    nothing happened — T16.7's invalidation oracle, three milestones
//!    later. The label under the panel changes every step, and the test
//!    asserts the pixels under it actually differ between steps.
//! 2. **A backdrop that changes the command *count* degrades to
//!    `Damage::All`**, and the oracle then compares two identical full
//!    renders. So the backdrop changes a label's *text*, never how many
//!    commands there are, and the test asserts it saw partial damage.
//! 3. **An opaque tint hides everything.** At `tint.3 = 255` the blur
//!    and the backdrop are both invisible and every injection still
//!    produces correct pixels. The tint is translucent and the test
//!    asserts the panel is not a flat colour.

use lumen_static_widgets::ops::{Fixed, Label};
use lumen_static::retain::Damage;
use lumen_static::{App as DeriveApp, Color, StaticTree};
use lumen_static_widgets::{Glass};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::Host;

const W: u32 = 240;
const H: u32 = 160;
const BG: Color = Color(18, 22, 32, 255);

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Content {
    #[widget_id("a")]
    a: Label<String>,
    #[widget_id("b")]
    b: Label<String>,
    #[widget_id("c")]
    c: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Bar {
    #[widget_id("title")]
    title: Label,
}

/// Something painted **after** the panel, just below it — inside the
/// region the blur reads and outside the rect it writes.
///
/// It never changes, which is the point. In a full render it does not
/// exist yet when the panel captures its backdrop; in a partial render
/// whose damage stops at the panel's own rect, the ring around the
/// panel still holds it from last frame and the capture picks it up.
/// That divergence is the whole reason the dependency is the *source*
/// region and not the panel's rect, and without this widget nothing in
/// the scene can tell the two apart.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Tail {
    pad: Fixed,
    #[widget_id("badge")]
    badge: Label,
}

/// Content and a panel over it, sharing one rect — the composition
/// `Glass` needs and the one a `column` cannot express.
#[derive(StaticTree, Default)]
#[widget(stack)]
struct Screen {
    content: Content,
    bar: Glass<Bar>,
    tail: Tail,
}

#[derive(DeriveApp, Default)]
struct Moving {
    #[tree]
    ui: Screen,
    #[bind(ui.content.a, "row a {}")]
    #[bind(ui.content.b, "row b {}")]
    #[bind(ui.content.c, "row c {}")]
    step: i64,
}

fn app() -> Moving {
    let mut m = Moving::default();
    m.ui.bar.content.title.set("Files");
    m.ui.tail.pad.w = 1.0;
    m.ui.tail.pad.h = 18.0;
    m.ui.tail.badge.set("static badge");
    m.ui.bar.material.blur = 6.0;
    // Translucent, deliberately: an opaque tint would hide the blur and
    // the backdrop, and every injection below would still pass.
    m.ui.bar.material.tint = Color(255, 255, 255, 40);
    m
}

fn full(host: &Host<Moving>, canvas: &mut Canvas) {
    canvas.clear(BG);
    host.render(canvas);
}

/// Pixels **inside** the panel, so a test can say the backdrop moved.
///
/// The panel is the top 16 px; this samples inside it and across the
/// width the first label occupies, which is what the blur reads.
fn under_panel(c: &Canvas, scale: f32) -> Vec<u8> {
    let mut out = Vec::new();
    let k = |v: f32| (v * scale) as usize;
    for y in k(2.0)..k(14.0) {
        for x in k(2.0)..k(120.0) {
            let p = (y * c.width as usize + x) * 4;
            out.extend_from_slice(&c.pixels[p..p + 3]);
        }
    }
    out
}

/// Steps whose bound text differs in **length**, one digit at a time.
///
/// Not `1..=8`. Without a system face the text stack falls back to
/// block metrics, where every character is the same box — so "row a 0"
/// and "row a 7" rasterise to **identical pixels**, and the first
/// version of this test drove eight steps in which the frame never
/// changed at all (whole-frame diff: 0 px). The `moved` guard below is
/// what caught it. Length is the one thing block metrics do represent.
const STEPS: [i64; 8] = [1, 10, 100, 1_000, 10_000, 100_000, 1_000_000, 10_000_000];

/// The comparison at `scale`, on a surface of the logical size times it.
/// Returns (partial steps, steps where the panel's pixels moved).
fn drive(scale: f32) -> (usize, usize) {
    let (w, h) = ((W as f32 * scale).round() as u32, (H as f32 * scale).round() as u32);
    let mut host: Host<Moving> = Host::new(app(), w, h);
    host.set_scale(scale);
    host.pump();

    let mut oracle = Canvas::new(w, h);
    let mut incremental = Canvas::new(w, h);
    full(&host, &mut oracle);
    full(&host, &mut incremental);

    let mut partial = 0;
    let mut moved = 0;
    let mut before = under_panel(&oracle, scale);

    for step in STEPS {
        host.app.step = step;
        host.pump();
        full(&host, &mut oracle);

        match host.take_damage_for_age(1) {
            Damage::None => {}
            Damage::All => {
                incremental.clear(BG);
                host.render(&mut incremental);
            }
            d @ Damage::Regions(_) => {
                partial += 1;
                for r in d.regions([0.0, 0.0, w as f32, h as f32]) {
                    incremental.clear_within(r, BG);
                    incremental.render_within(&host.list, r);
                }
            }
        }

        let now = under_panel(&oracle, scale);
        if now != before {
            moved += 1;
        }
        before = now;

        let diff = oracle
            .pixels
            .chunks_exact(4)
            .zip(incremental.pixels.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(diff, 0, "scale {scale}, step {step}: {diff} px differ between a partial and a full redraw");
    }
    println!("scale {scale}: {partial} partial steps, {moved} steps where the panel's pixels moved");
    (partial, moved)
}

#[test]
fn a_panel_over_a_moving_backdrop_is_identical_both_ways() {
    let (partial, moved) = drive(1.0);
    // Guard 1 and 2: the setup happened, and it happened partially.
    assert!(
        moved >= 6,
        "the backdrop under the panel barely moved ({moved} of 8): nothing to go stale"
    );
    assert!(partial >= 5, "only {partial} of 8 steps had partial damage; the rest proved nothing");
}

/// M40/T40.1's glass risk (`docs/plan-physical-damage.md` §5): with
/// `Recording` inside `Scaled`, the backdrop dependency is computed from
/// the **scaled** `blur`, so the region the panel reads is physical like
/// everything else. At 1.0 the two orders agree; this is where they
/// would not.
#[test]
fn a_panel_over_a_moving_backdrop_is_identical_both_ways_at_device_scales() {
    for scale in [1.25, 1.5, 2.0] {
        let (partial, moved) = drive(scale);
        assert!(moved >= 6, "scale {scale}: the backdrop barely moved ({moved} of 8)");
        assert!(partial >= 5, "scale {scale}: only {partial} of 8 steps were partial");
    }
}

/// Guard 3: the panel is showing a blurred backdrop, not a flat fill.
#[test]
fn the_panel_is_not_a_flat_colour() {
    let mut host: Host<Moving> = Host::new(app(), W, H);
    host.app.step = 1_000;
    host.pump();
    let mut c = Canvas::new(W, H);
    full(&host, &mut c);

    let px = under_panel(&c, 1.0);
    let first = &px[..3];
    let distinct = px.chunks_exact(3).filter(|p| *p != first).count();
    println!("{distinct} of {} sampled pixels differ from the first", px.len() / 3);
    assert!(
        distinct > px.len() / 30,
        "the panel is nearly uniform ({distinct} differing) — the backdrop is not showing through"
    );
}

/// The dependency must widen damage past the panel's own rect: the ring
/// the blur reads but does not write is what goes stale otherwise.
#[test]
fn damage_covers_more_than_the_panel_when_the_backdrop_changes() {
    let mut host: Host<Moving> = Host::new(app(), W, H);
    host.pump();
    let _ = host.take_damage_for_age(1);

    host.app.step = 1_000_000;
    host.pump();
    let d = host.take_damage_for_age(1);
    let total: f32 = d
        .regions([0.0, 0.0, W as f32, H as f32])
        .map(|r| (r[2] - r[0]) * (r[3] - r[1]))
        .sum();
    println!("damage {total} px of {}", W * H);
    assert!(total > 0.0, "a changed backdrop damaged nothing");
    assert!(total < (W * H) as f32, "damage is the whole frame; the dependency over-dilated");
}

/// A still frame with a panel in it must still cost nothing.
///
/// The dependency widens damage only when damage *touches* the panel's
/// source region. Made unconditional it would be correct and useless: a
/// window with a glass toolbar would repaint the toolbar every frame
/// forever, which is the opposite of what damage tracking is for.
#[test]
fn a_still_frame_with_a_panel_damages_nothing() {
    let mut host: Host<Moving> = Host::new(app(), W, H);
    host.pump();
    let _ = host.take_damage_for_age(1);
    host.pump();
    assert_eq!(host.damage(), Damage::None, "an idle frame with a panel damaged something");
}
