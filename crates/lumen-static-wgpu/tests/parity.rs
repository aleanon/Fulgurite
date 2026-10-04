//! T15.12's gate: the same frame on both backends — **by region** since
//! M51/T51.1.
//!
//! Three claims, in increasing strength:
//!
//! 1. **The GPU path draws.** A frame is not blank and its ink is where
//!    the commands put it.
//! 2. **It agrees with the software canvas, shape by shape.** Not
//!    bit-exact, and the reason is written down rather than waved at:
//!    the GPU antialiases edges the software rasteriser rounds to whole
//!    pixels, so a fractional edge differs by up to one pixel of ramp.
//!    Each named shape in [`frame`] is counted in its own box against its
//!    own bound ([`REGIONS`]); everything outside every box is the page
//!    and must agree exactly.
//! 3. **A partial redraw equals a full one, exactly**, on the GPU path —
//!    the property `lumen-static-shell/tests/damage_oracle.rs` holds the
//!    software path to. This one *is* bit-exact, because the clip test
//!    in the shader is hard and the damage rect is snapped to whole
//!    pixels.
//!
//! **Why regions (M51/T51.1, R9-F7).** Until M51 one bound covered the
//! whole frame: fewer than 0.2% of 240×180 pixels (86 px) over 8 levels.
//! On llvmpipe the frame's baseline already spent 56 of those, and on
//! the RTX 4070 making the software border's ring coverage binary moved
//! the frame from 4 px to 60 — green. The border, which this frame exists
//! to hold to the shared `sd_round_box`, could lose its antialiasing and
//! nothing noticed. A bound belongs to a region.
//!
//! Every test here **skips** when no adapter can be found rather than
//! failing, under a plain `cargo test`; `just gpu` sets
//! LUMEN_REQUIRE_GPU=1, under which a missing adapter fails
//! (M43/T43.10), and `the_gpu_half_ran` prints the one that ran.

use lumen_static::display::{Cmd, Color, DisplayList, GradKind, GradShape, ImageId, Renderer, Stops};
use lumen_static::Point;
use lumen_static_shell::software::Canvas;
use lumen_static_wgpu::Gpu;

const W: u32 = 240;
const H: u32 = 180;
const PAGE: Color = Color(250, 250, 250, 255);

/// How many `Cmd` variants there are. Bumped by hand, which is the point:
/// a new variant does not compile in [`kind`] until it has an index, and
/// once it has one [`the_frame_uses_every_command`] refuses to pass until
/// [`frame`] draws it or [`kind`] names the file that does.
const KINDS: usize = 12;

/// A stable index per command kind, from an **exhaustive** match with no
/// wildcard arm — M51/T51.1, as `lumen-static-shell/tests/svg.rs` does
/// since T51.6. "Every command in the vocabulary" is a claim this file
/// made in prose and was false about (no `Image`, R9-F2); now the
/// compiler holds it. The third field is where a kind is compared when it
/// is *not* in this frame, which only glass is: a backdrop filter needs a
/// backdrop built for it and bounds of its own (`docs/plan-liquid.md` §8).
fn kind(c: &Cmd) -> (usize, &'static str, Option<&'static str>) {
    match c {
        Cmd::Rect { .. } => (0, "Rect", None),
        Cmd::RoundedRect { .. } => (1, "RoundedRect", None),
        Cmd::StrokedRect { .. } => (2, "StrokedRect", None),
        Cmd::Shadow { .. } => (3, "Shadow", None),
        Cmd::Text { .. } => (4, "Text", None),
        Cmd::Line { .. } => (5, "Line", None),
        Cmd::Gradient { .. } => (6, "Gradient", None),
        Cmd::Image { .. } => (7, "Image", None),
        Cmd::Glass { .. } => (8, "Glass", Some("tests/glass_parity.rs")),
        Cmd::PushClip { .. } => (9, "PushClip", None),
        Cmd::PopClip => (10, "PopClip", None),
        // M82/T82.5: the GPU arm is empty until T82.6, which draws paths in
        // this frame and sets this field to `None`.
        Cmd::Path { .. } => (11, "Path", Some("tests/parity.rs: path pin, until T82.6")),
    }
}

/// The image the frame's `Image` region draws.
const CARD_ID: ImageId = ImageId(1);

/// One named shape in [`frame`]: the box its pixels are counted in (whole
/// pixels, `[x0, y0, x1, y1)`, with two pixels of reach so an edge that
/// moved is inside it), and its bound.
///
/// **The bounds, and where they come from.** Each is `2 × b + 2`, where
/// `b` is the larger of the region's measured baselines on the two
/// adapter classes this machine has (`BASE_DISCRETE`, `BASE_CPU`,
/// `docs/results-gpu.md` §2). Twice, so a baseline that doubles is still
/// green and a shape that moved by a pixel — a whole edge over the line —
/// is not; plus two, so a region whose baseline is 0 is not a bound of 0
/// that one antialiased corner on a third driver would trip. On an
/// adapter class with no measured baseline the line says so, and the
/// bound still applies (§2.2: a larger baseline raises the bound, and a
/// region whose injection then stops firing is split, not loosened).
struct Region {
    name: &'static str,
    bbox: [u32; 4],
}

const REGIONS: [Region; 10] = [
    Region { name: "rect", bbox: [8, 8, 62, 42] },
    Region { name: "rounded rect", bbox: [68, 8, 132, 47] },
    Region { name: "clip scope", bbox: [138, 2, 232, 58] },
    Region { name: "text", bbox: [8, 58, 142, 80] },
    Region { name: "shadow", bbox: [148, 60, 240, 121] },
    Region { name: "line", bbox: [8, 84, 74, 136] },
    Region { name: "gradient", bbox: [78, 86, 142, 130] },
    Region { name: "translucent rect", bbox: [8, 138, 72, 172] },
    Region { name: "image", bbox: [86, 138, 126, 176] },
    Region { name: "border", bbox: [150, 123, 234, 172] },
];

/// Per-region baselines, pixels over 8, in [`REGIONS`]' order — measured
/// 2026-09-27 on this machine: the RTX 4070 (Vulkan, `DiscreteGpu`) and
/// llvmpipe through lavapipe (Vulkan, `Cpu`), `docs/results-gpu.md` §2.
///
/// **The shadow's 54 is one row, and it is a tie, not a gradient.** The
/// shadow's box is dropped 3.5 px (`DisplayList::shadow`), so the row
/// one pixel inside its bottom edge sits at a signed distance of exactly
/// −1, which is the threshold both backends skip "interior" pixels at
/// (`d >= -1` in software, `d < -1` discards in `shader.wgsl`). Which side
/// of it a GPU lands on is float noise that depends on where the quad is:
/// at this frame's position the RTX drops the row and llvmpipe keeps it,
/// and at the pre-M51 frame's position it was the other way round (which
/// is llvmpipe's 56-px whole-frame baseline in the review). The row it
/// decides is below the card, so it shows; `docs/results-gpu.md` §2 has
/// the rest.
const BASE_DISCRETE: [usize; 10] = [0, 0, 0, 0, 54, 4, 0, 0, 0, 0];
const BASE_CPU: [usize; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn bound(i: usize) -> usize {
    2 * BASE_DISCRETE[i].max(BASE_CPU[i]) + 2
}

/// A frame using every command in the vocabulary but glass, one named
/// shape per [`REGIONS`] entry, their boxes disjoint.
///
/// Coordinates are whole pixels wherever the shape's *edge* is, because
/// a fractional edge is exactly where the two backends are allowed to
/// differ — putting one in this list would measure the tolerance rather
/// than the agreement. `tolerance_is_bounded_at_fractional_edges` uses
/// fractional ones on purpose.
fn frame() -> DisplayList {
    let mut l = DisplayList::new();
    l.rect([10.0, 10.0, 60.0, 40.0], Color(200, 60, 60, 255));
    l.rounded_rect([70.0, 10.0, 130.0, 45.0], Color(40, 90, 200, 255), 10.0);
    // A clip scope with something escaping it on every side, so the clip
    // stack is exercised on both backends.
    l.push_clip([150.0, 10.0, 190.0, 50.0]);
    l.rect([140.0, 4.0, 230.0, 56.0], Color(120, 120, 120, 255));
    l.pop_clip();
    l.text([10.0, 60.0, 140.0, 78.0], "parity", 14.0, Color(0, 0, 0, 255));
    l.shadow([160.0, 70.0, 230.0, 110.0], 8.0, 10.0);
    l.rounded_rect([160.0, 70.0, 230.0, 110.0], Color(30, 38, 54, 255), 8.0);
    l.line(Point { x: 12.0, y: 90.0 }, Point { x: 70.0, y: 130.0 }, 3.0, Color(20, 160, 90, 255));
    l.gradient([80.0, 88.0, 140.0, 128.0], Stops::two(Color(255, 200, 0, 255), Color(200, 0, 120, 255)), GradShape::Linear { angle: 0.0 }, 0.0);
    // Half-transparent, so the blend arithmetic is compared too and not
    // just the coverage.
    l.rect([10.0, 140.0, 70.0, 170.0], Color(0, 120, 200, 128));
    // The 16×16 card at 2× (M51/T51.1, R9-F2): no pixel corpus held an
    // image until M51, and deleting the GPU's image arm left the
    // workspace green. `images_agree_across_the_backends` has the other
    // placements.
    l.image([90.0, 140.0, 122.0, 172.0], CARD_ID, Color(255, 255, 255, 255));
    // A border (M27/T27.1). The two backends compute the ring from the
    // *same* expression -- `sd_round_box` in `shader.wgsl`, transcribed
    // into `software::rr_cov` -- and the border region's own bound is what
    // holds them to it.
    l.stroked_rect([160.0, 125.0, 230.0, 170.0], Color(220, 40, 40, 255), 8.0, 3.0);
    l
}

/// Whether the recipe that runs this demands an adapter (M43/T43.10):
/// `just gpu` sets LUMEN_REQUIRE_GPU=1 unless LUMEN_ALLOW_SKIP=1. A plain
/// `cargo test` keeps the skip, because a developer's partial run is not
/// a gate.
fn gpu_required() -> bool {
    std::env::var("LUMEN_REQUIRE_GPU").is_ok_and(|v| v == "1")
}

/// `None` when there is no GPU here, and the test returns early. Its
/// "skipping" line is captured by libtest on a pass, so on its own a skip
/// looked exactly like a pass (R17-F6); under LUMEN_REQUIRE_GPU=1, which
/// `just gpu` sets, a missing adapter panics instead, and
/// `the_gpu_half_ran` below says which adapter did run.
fn gpu() -> Option<Gpu> {
    gpu_sized(W, H)
}

fn gpu_sized(w: u32, h: u32) -> Option<Gpu> {
    match Gpu::headless(w, h) {
        Some(mut g) => {
            eprintln!("gpu: {}", g.adapter());
            if let Some(f) = lumen_static_shell::text::FontStack::system() {
                g.set_font(f);
            }
            g.set_images(Box::new(Card::new()));
            Some(g)
        }
        None => {
            assert!(
                !gpu_required(),
                "LUMEN_REQUIRE_GPU=1 but no adapter: this run was asked to compare the GPU \
                 with the CPU and there is no GPU to compare (set LUMEN_ALLOW_SKIP=1 on the \
                 recipe to skip deliberately)"
            );
            eprintln!("gpu: no adapter — skipping (this is a supported configuration)");
            None
        }
    }
}

fn canvas() -> Canvas {
    let mut c = match lumen_static_shell::text::FontStack::system() {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    };
    c.images = Some(Box::new(Card::new()));
    c
}

/// Which baseline row applies to the adapter that ran, from the device
/// type the adapter line carries (M51 §0 rule 1).
fn baseline_for(adapter: &str) -> (&'static str, Option<&'static [usize; 10]>) {
    if adapter.contains("DiscreteGpu") {
        ("discrete", Some(&BASE_DISCRETE))
    } else if adapter.contains("Cpu") {
        ("cpu", Some(&BASE_CPU))
    } else {
        ("unmeasured class", None)
    }
}

/// Pixels over `soft` inside `r` (`[x0, y0, x1, y1)`, whole pixels,
/// clamped to the frame), and the worst channel delta there.
fn region_over(a: &[u8], b: &[u8], w: u32, r: [u32; 4], soft: u8) -> (usize, u8) {
    let h = (a.len() / 4) as u32 / w;
    let mut over = 0;
    let mut worst = 0u8;
    for y in r[1]..r[3].min(h) {
        for x in r[0]..r[2].min(w) {
            let i = ((y * w + x) * 4) as usize;
            let d = (0..3).map(|k| a[i + k].abs_diff(b[i + k])).max().unwrap_or(0);
            worst = worst.max(d);
            if d > soft {
                over += 1;
            }
        }
    }
    (over, worst)
}

/// Per-pixel comparison over the whole frame: the worst channel
/// difference, and how many pixels differ by more than `soft`.
fn compare(a: &[u8], b: &[u8], soft: u8) -> (u8, usize) {
    let mut worst = 0u8;
    let mut over = 0usize;
    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let mut d = 0u8;
        for k in 0..3 {
            d = d.max(pa[k].abs_diff(pb[k]));
        }
        worst = worst.max(d);
        if d > soft {
            over += 1;
        }
    }
    (worst, over)
}

/// M51/T51.1: [`frame`] draws every kind [`kind`] knows, except those
/// `kind` names another file for. Runs without an adapter.
#[test]
fn the_frame_uses_every_command() {
    let mut seen = [false; KINDS];
    for c in &frame().cmds {
        seen[kind(c).0] = true;
    }
    // One sample of every kind, to ask `kind` for its name and owner:
    // the exhaustive match cannot be iterated, so the samples are the
    // list, and a missing sample is an uncovered index below.
    let r = [0.0, 0.0, 1.0, 1.0];
    let samples = [
        Cmd::Rect { rect: r, color: PAGE },
        Cmd::RoundedRect { rect: r, color: PAGE, radius: 0.0 },
        Cmd::StrokedRect { rect: r, color: PAGE, radius: 0.0, width: 1.0 },
        Cmd::Shadow { rect: r, color: PAGE, radius: 0.0, spread: 1.0 },
        Cmd::Text { rect: r, text: (0, 0), size: 1.0, weight: 400.0, line_height: 0, color: PAGE },
        Cmd::Line { a: Point { x: 0.0, y: 0.0 }, b: Point { x: 1.0, y: 1.0 }, width: 1.0, color: PAGE },
        Cmd::Gradient { rect: r, stops: Stops::two(PAGE, PAGE), kind: GradKind::Linear, geom: [0.0; 3], corner: 0.0 },
        Cmd::Image { rect: r, id: CARD_ID, tint: PAGE },
        Cmd::Glass { rect: r, radius: 0.0, material: lumen_static::display::Material::LIQUID },
        Cmd::PushClip { rect: r },
        Cmd::PopClip,
        Cmd::Path { rect: r, els: (0, 0), fill: PAGE, stroke: PAGE, width: 1.0 },
    ];
    let mut named = [false; KINDS];
    let mut missing = Vec::new();
    for c in &samples {
        let (i, name, elsewhere) = kind(c);
        named[i] = true;
        match elsewhere {
            Some(file) => println!("{name}: compared in {file}"),
            None if seen[i] => println!("{name}: in the frame"),
            None => missing.push(name),
        }
    }
    let unsampled: Vec<usize> = (0..KINDS).filter(|i| !named[*i]).collect();
    assert!(unsampled.is_empty(), "kinds with no sample here: {unsampled:?}");
    assert!(missing.is_empty(), "commands the parity frame does not draw: {missing:?}");
}

#[test]
fn the_gpu_path_draws_a_frame() {
    let Some(mut g) = gpu() else { return };
    g.clear(PAGE);
    g.render(&frame(), W, H);
    let px = g.read_rgba();
    assert_eq!(px.len(), (W * H * 4) as usize);

    let inked = px
        .chunks_exact(4)
        .filter(|p| p[0] != PAGE.0 || p[1] != PAGE.1 || p[2] != PAGE.2)
        .count();
    let frac = inked as f32 / (W * H) as f32;
    assert!(frac > 0.10, "the frame is nearly blank ({frac:.3} inked)");

    // The red rect's interior is exactly the colour asked for: coverage
    // is 1 well inside a shape, so this is not a tolerance question.
    let at = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        Color(px[i], px[i + 1], px[i + 2], px[i + 3])
    };
    assert_eq!(at(40, 25), Color(200, 60, 60, 255), "inside the rect");
    assert_eq!(at(5, 5), Color(250, 250, 250, 255), "outside everything");
    // Clipped away: the grey rect spans x 140..230 but its scope starts
    // at 150, so 145 must still be the page.
    assert_eq!(at(145, 30), Color(250, 250, 250, 255), "escaped its clip scope");
    assert_eq!(at(170, 30), Color(120, 120, 120, 255), "inside its clip scope");
}

/// M51/T51.1 (R9-F7, R9-F2): every region within its own bound, and the
/// page outside every region identical.
#[test]
fn every_region_is_within_its_own_bound() {
    let Some(mut g) = gpu() else { return };
    let mut c = canvas();
    c.clear(PAGE);
    c.render(&frame(), W, H);
    g.clear(PAGE);
    g.render(&frame(), W, H);
    let gp = g.read_rgba();
    let (class, base) = baseline_for(g.adapter());
    println!("adapter: {} -- baselines: {class}", g.adapter());

    let mut red = Vec::new();
    for (i, r) in REGIONS.iter().enumerate() {
        let (over, worst) = region_over(&c.pixels, &gp, W, r.bbox, 8);
        let b = match base {
            Some(row) => format!("baseline {} on {class}", row[i]),
            None => format!("no baseline for {class}"),
        };
        println!("{}: {over} px over 8 (bound {}, {b}), worst {worst}", r.name, bound(i));
        if over > bound(i) {
            red.push((r.name, over, bound(i)));
        }
    }
    // The page: every pixel in no region's box.
    let mut page_over = 0;
    for y in 0..H {
        for x in 0..W {
            let inside = REGIONS.iter().any(|r| x >= r.bbox[0] && x < r.bbox[2] && y >= r.bbox[1] && y < r.bbox[3]);
            if !inside {
                let i = ((y * W + x) * 4) as usize;
                if (0..3).any(|k| c.pixels[i + k].abs_diff(gp[i + k]) > 8) {
                    page_over += 1;
                }
            }
        }
    }
    println!("page (outside every region): {page_over} px over 8 (bound 0)");
    let (worst, over) = compare(&c.pixels, &gp, 8);
    println!(
        "whole frame, for the record: {over} px ({:.2}%) over 8, worst {worst} -- no longer a bound",
        over as f32 / (W * H) as f32 * 100.0
    );
    assert!(red.is_empty(), "regions over their bounds (name, px, bound): {red:?}");
    assert_eq!(page_over, 0, "{page_over} px differ outside every named region: a shape escaped its box");
}

#[test]
fn interiors_are_identical_not_merely_close() {
    // The stronger half of the comparison: away from every edge, the
    // two backends must agree almost exactly. If they do not, the
    // difference is a blend or a colour-space bug rather than
    // antialiasing, and the region bounds would have hidden it.
    let Some(mut g) = gpu() else { return };
    let mut c = canvas();
    c.clear(PAGE);
    c.render(&frame(), W, H);
    g.clear(PAGE);
    g.render(&frame(), W, H);
    let gp = g.read_rgba();

    for (x, y, what) in [
        (40u32, 25u32, "opaque rect"),
        (100, 28, "rounded rect"),
        (195, 90, "card over its shadow"),
        (40, 155, "translucent rect over the page"),
        (110, 108, "gradient"),
        (98, 148, "image"),
    ] {
        let i = ((y * W + x) * 4) as usize;
        for k in 0..3 {
            let d = c.pixels[i + k].abs_diff(gp[i + k]);
            assert!(d <= 2, "{what} at ({x},{y}) channel {k}: {} vs {} ", c.pixels[i + k], gp[i + k]);
        }
    }
}

#[test]
fn a_partial_redraw_is_bit_identical_to_a_full_one() {
    let Some(mut g) = gpu() else { return };

    // Frame 1, the ground both later frames are compared from.
    g.clear(PAGE);
    g.render(&frame(), W, H);
    let full_first = g.read_rgba();

    // The change, and the region it damaged.
    //
    // **Both are placed inside the translucent rect on purpose, and the
    // region's edges are fractional on purpose.** Those two choices are
    // what give the test teeth, and it took a fault injection to learn
    // it: with whole-pixel damage over opaque content, this test passes
    // whether or not `snap_out` exists, because a boundary pixel that
    // is then covered by an opaque command has its half-cleared value
    // overwritten, and a boundary pixel over unchanged background is
    // being blended with a copy of itself.
    //
    // Under *translucent* content neither rescue applies: an
    // antialiased clear leaves `0.7·page + 0.3·(page under glass)` at
    // the edge, the glass is then blended over it a second time, and
    // the pixel differs from a full render. That is the artefact
    // `snap_out` exists to prevent, and this is where it shows.
    let mut second = frame();
    second.rect([20.0, 148.0, 60.0, 162.0], Color(0, 0, 0, 255));
    let damage = [15.3, 144.7, 65.2, 166.4];

    g.draw(&second, Some(damage), Some((PAGE, damage)));
    let partial = g.read_rgba();

    g.clear(PAGE);
    g.render(&second, W, H);
    let full = g.read_rgba();

    assert_ne!(partial, full_first, "the second frame changed nothing — the test proves nothing");
    let (worst, over) = compare(&partial, &full, 0);
    assert_eq!(
        (worst, over),
        (0, 0),
        "a partial redraw must be *identical* to a full one on this backend"
    );
}

#[test]
fn tolerance_is_bounded_at_fractional_edges() {
    // The one place the two backends are known to disagree, isolated:
    // a rect whose edges land between pixels. The software canvas fills
    // from floor(x0) to ceil(x1) — whole pixels, no ramp — and the GPU
    // ramps. The disagreement must stay *at the edge*: one column and
    // one row, not a shifted shape.
    let Some(mut g) = gpu() else { return };
    let mut l = DisplayList::new();
    l.rect([10.4, 10.4, 60.6, 50.6], Color(0, 0, 0, 255));

    let mut c = canvas();
    c.clear(PAGE);
    c.render(&l, W, H);
    g.clear(PAGE);
    g.render(&l, W, H);
    let gp = g.read_rgba();

    let at = |px: &[u8], x: u32, y: u32| px[((y * W + x) * 4) as usize];
    // Interiors agree exactly.
    assert_eq!(at(&c.pixels, 30, 30), at(&gp, 30, 30), "interior");
    assert_eq!(at(&c.pixels, 5, 5), at(&gp, 5, 5), "exterior");
    // The boundary column is where they differ, and the difference is
    // the software arm being *more* covered, not less.
    assert_eq!(at(&c.pixels, 10, 30), 0, "software fills the boundary pixel whole");
    assert!(at(&gp, 10, 30) > 0, "the GPU ramps it");
}


/// M40/T40.0, R9-F5's GPU twin: hairlines painted **through `Scaled`**
/// at a fractional display scale agree within the stated tolerance.
///
/// `tolerance_is_bounded_at_fractional_edges` isolates the ramp
/// difference on a hand-built list, which is the documented one. What no
/// test did was paint through `Scaled` at a scale other than 1, which is
/// how every edge of a HiDPI frame is produced: at 1.25 each
/// 1-logical-px divider's edges land between pixels, the software canvas
/// fills both boundary rows whole and the GPU ramps them, and the review
/// measured 7.92% of a frame's pixels over 8 levels against this file's
/// 0.2% (`docs/results-deep-review.md` R9-F5). Snapping a rect's scaled
/// edges to device pixels (`docs/plan-physical-damage.md` §2.2) makes
/// the two agree, because a whole-pixel edge is where they already do.
const HAIRLINE_BOUND: usize = 2;

#[test]
fn hairlines_through_scaled_agree_at_fractional_scales() {
    let Some(mut g) = gpu() else { return };
    let mut worst_over = 0usize;
    for scale in [1.25f32, 1.5] {
        let mut l = DisplayList::new();
        {
            let mut s = lumen_static::Scaled::new(&mut l, scale);
            let p: &mut dyn lumen_static::Painter = &mut s;
            for y in [10.0f32, 13.0, 17.0, 22.0, 28.0, 35.0, 41.0, 50.0, 63.0, 77.0, 90.0, 111.0] {
                p.rect([3.0, y, 180.0, y + 1.0], Color(40, 40, 40, 255));
            }
            // Two fills that share a fractional edge, and a column rule.
            p.rect([3.0, 120.0, 90.0, 131.0], Color(200, 60, 60, 255));
            p.rect([3.0, 131.0, 90.0, 139.0], Color(40, 90, 200, 255));
            p.rect([150.0, 115.0, 151.0, 140.0], Color(0, 0, 0, 255));
        }
        let mut c = canvas();
        c.clear(PAGE);
        c.render(&l, W, H);
        g.clear(PAGE);
        g.render(&l, W, H);
        let (worst, over) = compare(&c.pixels, &g.read_rgba(), 8);
        // M51/T51.1: a region of its own, like every shape in `frame`.
        // The whole list is the region; its baseline is 0 on both adapter
        // classes (M40/T40.3 measured 0 before the move, and it reads 0
        // after it), so the bound is `2 × 0 + 2` where it was 0.2% of the
        // frame, 86 px.
        let bound = HAIRLINE_BOUND;
        println!(
            "hairlines at {scale}: {over} px over 8 (bound {bound}, baseline 0 on discrete and cpu), \
             worst {worst}"
        );
        worst_over = worst_over.max(over);
    }
    assert!(worst_over <= HAIRLINE_BOUND, "hairlines differ on {worst_over} px at a fractional scale (bound {HAIRLINE_BOUND})");
}

/// The GPU half of this file ran at all (M43/T43.10). Every other test
/// here returns early without an adapter, and a run of early returns is
/// green; this one prints the adapter and backend a comparison used, and
/// under LUMEN_REQUIRE_GPU=1 fails when there is none.
#[test]
fn the_gpu_half_ran() {
    match gpu() {
        Some(g) => println!("adapter: {}", g.adapter()),
        None => println!("adapter: none -- the GPU half of this file compared nothing"),
    }
}

/// **Radial and rounded gradients on both backends** — M82/T82.2
/// (`docs/plan-paint-vocabulary.md`).
///
/// Two shapes, each rendered by both backends and held to the gradient
/// region's own bound ([`REGIONS`]):
///
/// - a radial whose resolved centre is pixel (47, 47)'s centre and whose
///   radius is 48 px along that row: the centre pixel is stop 0 and the
///   pixel at the radius, (95, 47), the last stop, within 2/255 on each
///   backend. Setup: the list holds exactly one radial `Gradient`;
/// - a gradient of one colour with `corner = 12` at (150, 20): the pixel
///   at (1, 1) inside it has coverage ≤ 1/255, and its centre is the stop
///   colour, on each backend. Not (3, 3), which is half-covered on
///   correct code.
///
/// **And it is the first frame here with two gradients in it.** The
/// rounded one is second on purpose: until T82.2 the shader's `Grad` was
/// 112 B against the Rust struct's 96 (a `vec3<u32>` pad aligns to 16),
/// so every gradient after the first read the wrong entry and drew
/// nothing (`shader.wgsl`, `Grad`).
///
/// Injections (T82.2's commit): the shader measures a radial from its
/// far end -- centre and edge swapped -- and the radial's region fails
/// parity; the shader ignores `params.x` for `KIND_GRADIENT`, and the
/// GPU's corner pixel is the stop colour; the `vec3<u32>` pad restored,
/// and the GPU's rounded gradient is the page.
#[test]
fn radial_and_rounded_gradients_agree_across_backends() {
    const FROM: Color = Color(250, 30, 30, 255);
    const TO: Color = Color(30, 30, 250, 255);
    // 128 x 64 at (16, 16): (47.5 - 16) / 128 and / 64 are exact in binary.
    let rect = [16.0, 16.0, 144.0, 80.0];
    let center = [0.24609375, 0.4921875];
    // The farthest corner from (47.5, 47.5) is (144, 80): 96.5 x 32.5.
    let radius = 48.0 / (96.5f32 * 96.5 + 32.5 * 32.5).sqrt();
    let rounded = [150.0, 20.0, 230.0, 80.0];
    let mut l = DisplayList::new();
    l.gradient(rect, Stops::two(FROM, TO), GradShape::Radial { center, radius }, 0.0);
    l.gradient(rounded, Stops::two(FROM, FROM), GradShape::Linear { angle: 0.0 }, 12.0);
    let radials = l.cmds.iter().filter(|c| matches!(c, Cmd::Gradient { kind: GradKind::Radial, .. })).count();
    println!("setup: {radials} radial gradient(s), {} command(s) (want 1 and 2)", l.cmds.len());
    assert_eq!((radials, l.cmds.len()), (1, 2), "the fixture must hold one radial and one rounded gradient");

    let Some(mut g) = gpu() else { return };
    let mut c = canvas();
    c.clear(PAGE);
    c.render(&l, W, H);
    g.clear(PAGE);
    g.render(&l, W, H);
    let gp = g.read_rgba();

    let at = |px: &[u8], x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        Color(px[i], px[i + 1], px[i + 2], 255)
    };
    let d = |a: Color, b: Color| a.0.abs_diff(b.0).max(a.1.abs_diff(b.1)).max(a.2.abs_diff(b.2));
    let cov = |got: Color| {
        let ch = |g: u8, i: u8, p: u8| if i == p { 0.0 } else { (g as f32 - p as f32) / (i as f32 - p as f32) };
        ch(got.0, FROM.0, PAGE.0).max(ch(got.1, FROM.1, PAGE.1)).max(ch(got.2, FROM.2, PAGE.2))
    };
    let mut wrong = Vec::new();
    for (name, px) in [("cpu", &c.pixels[..]), ("gpu", &gp[..])] {
        let (dc, de) = (d(at(px, 47, 47), FROM), d(at(px, 95, 47), TO));
        let (k, mid) = (cov(at(px, 151, 21)), at(px, 190, 50));
        println!(
            "{name}: radial centre {dc}/255 from stop 0, radius {de}/255 from the last stop (bound 2); \
             corner (1, 1) coverage {:.1}/255 (bound 1), centre {mid:?}",
            k * 255.0
        );
        if dc > 2 || de > 2 {
            wrong.push(format!("{name}: radial {dc}/{de}"));
        }
        if k > 1.0 / 255.0 || mid != FROM {
            wrong.push(format!("{name}: corner {:.1}/255, centre {mid:?}", k * 255.0));
        }
    }
    let gradient = REGIONS.iter().position(|r| r.name == "gradient").expect("a gradient region");
    let (ro, rw) = region_over(&c.pixels, &gp, W, [14, 14, 146, 82], 8);
    let (co, cw) = region_over(&c.pixels, &gp, W, [148, 18, 232, 82], 8);
    println!(
        "parity: radial {ro} px over 8 (worst {rw}), rounded {co} px over 8 (worst {cw}); bound {} (the gradient region's)",
        bound(gradient)
    );
    assert!(wrong.is_empty(), "probes off on a backend: {wrong:?}");
    assert!(ro <= bound(gradient), "the radial disagrees across backends: {ro} px over 8");
    assert!(co <= bound(gradient), "the rounded gradient disagrees across backends: {co} px over 8");
}

/// **Each backend passes the line pitch** — M80/T80.1.
///
/// Both backends place glyphs with the same `FontStack::place_run_into`,
/// so this cannot see a placer bug (`tests/line_height.rs` in the shell
/// is that gate); it sees a backend that drops `Cmd::Text.line_height`
/// on its way there, which would paint the second line at the face's
/// spacing on one side and at the pitch on the other. The text region's
/// own bound applies.
#[test]
fn a_line_pitch_reaches_both_backends() {
    let Some(mut g) = gpu() else { return };
    let mut l = DisplayList::new();
    l.text_spaced([10.0, 20.0, 140.0, 100.0], "pitch\npitch", 14.0, 400.0, 24, Color(0, 0, 0, 255));
    let mut c = canvas();
    c.clear(PAGE);
    c.render(&l, W, H);
    g.clear(PAGE);
    g.render(&l, W, H);
    let gp = g.read_rgba();
    let bbox = [8, 18, 142, 102];
    let ink = |px: &[u8]| {
        (bbox[1]..bbox[3])
            .filter(|&y| (bbox[0]..bbox[2]).any(|x| px[((y * W + x) * 4) as usize] < 128))
            .count()
    };
    let (rows_cpu, rows_gpu) = (ink(&c.pixels), ink(&gp));
    let (over, worst) = region_over(&c.pixels, &gp, W, bbox, 8);
    let text = REGIONS.iter().position(|r| r.name == "text").expect("a text region");
    println!(
        "line_height 24: {over} px over 8 (bound {}, the text region's), worst {worst}; \
         inked rows cpu {rows_cpu}, gpu {rows_gpu}",
        bound(text)
    );
    assert!(rows_cpu > 14, "setup: the canvas painted {rows_cpu} rows, not two lines");
    assert!(over <= bound(text), "the backends disagree on a pitched run: {over} px over 8");
}


/// **Both backends paint shaped** — M81/T81.3 (`docs/plan-shaped-paint.md`).
///
/// A region holding `שלום world 123` on a stack in `Shaping::Complex`,
/// given to both backends: each places its glyphs through the same
/// `FontStack::place_run_into`, whose `Complex` branch shapes, so the
/// region agrees within the text region's own bound. Setup: the region
/// has ink on both, and the run went through `unicode_bidi` (2 runs: one
/// per backend, printed).
///
/// Injection (T81.3's commit): the GPU shapes with a `Basic` clone of the
/// stack it was given (a GPU-only break; G9's "logical order" injection
/// could not fire, since glyph positions are absolute); the region's
/// differing pixels exceed its bound.
///
/// **Phased since M81/T81.8**: both backends draw a shaped glyph at the
/// column and third `text::pen_of` decides, and the region's glyphs land on
/// more than one third (counted from the same placements, printed; setup
/// ≥ 2). Injection (T81.8's commit): the wgpu site rounds instead of
/// calling `pen_of`; the region's differing pixels print over the bound.
#[cfg(feature = "shaping")]
#[test]
fn a_shaped_run_agrees_across_the_backends() {
    use lumen_static_shell::text::{text_counters, FontStack, Shaping};
    let Some(mut g) = gpu() else { return };
    let font = FontStack::system().expect("a system face").with_shaping(Shaping::Complex);
    g.set_font(font.clone());
    let mut c = Canvas::with_font(W, H, font.clone());
    let mut l = DisplayList::new();
    l.text([10.0, 20.0, 230.0, 44.0], "\u{5e9}\u{5dc}\u{5d5}\u{5dd} world 123", 16.0, Color(0, 0, 0, 255));
    let before = text_counters();
    c.clear(PAGE);
    c.render(&l, W, H);
    g.clear(PAGE);
    g.render(&l, W, H);
    let after = text_counters();
    let gp = g.read_rgba();
    let bbox = [8, 18, 232, 46];
    let ink = |px: &[u8]| {
        (bbox[1]..bbox[3])
            .flat_map(|y| (bbox[0]..bbox[2]).map(move |x| (x, y)))
            .filter(|&(x, y)| px[((y * W + x) * 4) as usize] < 128)
            .count()
    };
    let (ink_cpu, ink_gpu) = (ink(&c.pixels), ink(&gp));
    let (over, worst) = region_over(&c.pixels, &gp, W, bbox, 8);
    let text = REGIONS.iter().position(|r| r.name == "text").expect("a text region");
    let (shaped, bidi) = (after.complex - before.complex, after.bidi - before.bidi);
    let (placed, _) = font.place_run(l.str_of(text_range(&l)), 16.0, 400.0, 220.0);
    let mut phases = [0usize; 4];
    for p in &placed {
        phases[lumen_static_shell::text::pen_of(10.0 + p.x, p).id.key().phase as usize] += 1;
    }
    let thirds = phases[1..].iter().filter(|&&n| n > 0).count();
    println!(
        "shaped region: {over} px over 8 (bound {}, the text region's), worst {worst}; inked px cpu {ink_cpu}, \
         gpu {ink_gpu}; {shaped} runs through the shaper, {bidi} ordered by unicode_bidi (setup > 0); \
         glyphs per phase 0/1/2/3 {phases:?}, {thirds} thirds (setup ≥ 2)",
        bound(text)
    );
    assert!(thirds >= 2, "setup: the shaped region's glyphs land on {thirds} third(s) {phases:?}: nothing phased to compare");
    assert!(ink_cpu > 0 && ink_gpu > 0, "setup: the region has no ink (cpu {ink_cpu}, gpu {ink_gpu})");
    // At least the software canvas shaped it; a GPU that did not is the
    // defect, and the bound below names it.
    assert!(bidi >= 1, "setup: the run was never shaped ({bidi} bidi runs)");
    assert!(over <= bound(text), "the backends disagree on a shaped run: {over} px over 8");
}

/// The string range of the list's first `Cmd::Text`.
#[cfg(feature = "shaping")]
fn text_range(l: &DisplayList) -> (u32, u32) {
    l.cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Text { text, .. } => Some(*text),
            _ => None,
        })
        .expect("the list holds a text run")
}

/// M51/T51.0 (R9-F1): a glyph set larger than one frame's atlas.
///
/// The review's corpus: printable ASCII in eight sizes and two weights,
/// painted **through `Scaled` at 2** into a 3840×2160 frame, which is a
/// 4K HiDPI screen of type specimens. At scale 1 it fits the 1024² glyph
/// atlas; at 2 it does not fit an *empty* one either, and until M51 the
/// give-up branch submitted what fitted, counted nothing and logged
/// nothing — so a tenth of the text's ink was missing every frame while
/// three documents said a glyph is never dropped.
///
/// Two frames: the first fills the atlas from empty, the second is the
/// steady state. Printed: resets and give-ups per frame, the atlas edge,
/// and the dark-pixel count on both backends beside each other.
fn specimen(scale: f32) -> DisplayList {
    let ascii: String = (0x20u8..0x7f).map(char::from).collect();
    let mut l = DisplayList::new();
    {
        let mut s = lumen_static::Scaled::new(&mut l, scale);
        let p: &mut dyn lumen_static::Painter = &mut s;
        let mut y = 4.0f32;
        for size in [10.0f32, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0] {
            for weight in [400.0f32, 700.0] {
                p.text_weighted([4.0, y, 1916.0, y + size * 1.4], &ascii, size, weight, Color(0, 0, 0, 255));
                y += size * 1.4;
            }
        }
    }
    l
}

/// Dark pixels, which on a light page is text ink.
fn ink(px: &[u8]) -> usize {
    px.chunks_exact(4).filter(|p| p[0] < 128).count()
}

fn atlas_case(scale: f32) -> Option<(lumen_static_wgpu::AtlasStats, usize, usize)> {
    const AW: u32 = 3840;
    const AH: u32 = 2160;
    let mut g = gpu_sized(AW, AH)?;
    let face = lumen_static_shell::text::FontStack::system()
        .expect("the atlas case needs a real face: block glyphs never touch the atlas");
    g.set_font(face.clone());
    let list = specimen(scale);
    let mut c = Canvas::with_font(AW, AH, face);
    c.clear(PAGE);
    c.render(&list, AW, AH);
    let mut per_frame = Vec::new();
    for _ in 0..2 {
        let before = g.atlas_stats();
        g.clear(PAGE);
        g.render(&list, AW, AH);
        let after = g.atlas_stats();
        per_frame.push((
            after.resets - before.resets,
            after.growths - before.growths,
            after.give_ups - before.give_ups,
        ));
    }
    let stats = g.atlas_stats();
    let (sw, gw) = (ink(&c.pixels), ink(&g.read_rgba()));
    eprintln!(
        "atlas at scale {scale}: resets/growths/give-ups per frame {per_frame:?}, glyph atlas {}², \
         ink software {sw} px, gpu {gw} px ({:.1}% missing)",
        stats.glyph_size,
        (sw as f64 - gw as f64) / sw as f64 * 100.0
    );
    Some((stats, sw, gw))
}

#[test]
fn a_glyph_set_that_fits_is_drawn_whole_at_scale_1() {
    let Some((stats, sw, gw)) = atlas_case(1.0) else { return };
    assert_eq!(
        (stats.resets, stats.growths, stats.give_ups),
        (0, 0, 0),
        "scale 1 fits the starting atlas: nothing to reset, grow or give up"
    );
    assert!(sw > 10_000, "the corpus drew almost nothing ({sw} px): the setup did not happen");
    let d = sw.abs_diff(gw) as f64 / sw as f64;
    assert!(d < 0.001, "text ink differs by {:.3}% (bound 0.1%)", d * 100.0);
}

#[test]
fn a_glyph_set_larger_than_the_atlas_is_drawn_whole_at_scale_2() {
    let Some((stats, sw, gw)) = atlas_case(2.0) else { return };
    assert!(sw > 10_000, "the corpus drew almost nothing ({sw} px): the setup did not happen");
    assert_eq!(stats.give_ups, 0, "the atlas gave up {} times: glyphs were dropped", stats.give_ups);
    // The setup: this corpus must overflow the starting atlas, or the
    // growth path was never taken and the case proves nothing about it.
    assert!(stats.growths >= 1, "the atlas never grew: the corpus did not overflow it");
    let d = sw.abs_diff(gw) as f64 / sw as f64;
    assert!(d < 0.001, "text ink differs by {:.3}% (bound 0.1%)", d * 100.0);
}



/// A 16×16 RGBA test card: four quadrants of distinct colour, one of them
/// half transparent, and a one-texel dark frame so a wrong scale or a
/// half-texel shift moves ink across an edge the comparison can see.
struct Card(Vec<u8>);

const CARD: u32 = 16;

impl Card {
    fn new() -> Card {
        let mut px = Vec::with_capacity((CARD * CARD * 4) as usize);
        for y in 0..CARD {
            for x in 0..CARD {
                let c = if x == 0 || y == 0 || x == CARD - 1 || y == CARD - 1 {
                    [20, 20, 30, 255]
                } else {
                    match (x < CARD / 2, y < CARD / 2) {
                        (true, true) => [220, 40, 40, 255],
                        (false, true) => [40, 180, 60, 255],
                        (true, false) => [40, 80, 220, 255],
                        (false, false) => [240, 200, 0, 128],
                    }
                };
                px.extend_from_slice(&c);
            }
        }
        Card(px)
    }
}

impl lumen_static::display::ImageSource for Card {
    fn image(&self, _: lumen_static::display::ImageId) -> Option<(u32, u32, &[u8])> {
        Some((CARD, CARD, &self.0))
    }
}

/// The image placements: 1:1, 2×, tinted at 3×, with a translucent tint,
/// and one fractional placement. Each is `(name, rect, tint)`.
fn image_cases() -> [(&'static str, [f32; 4], Color); 5] {
    [
        ("1:1", [10.0, 10.0, 26.0, 26.0], Color(255, 255, 255, 255)),
        ("2x", [40.0, 10.0, 72.0, 42.0], Color(255, 255, 255, 255)),
        ("tinted 3x", [80.0, 10.0, 128.0, 58.0], Color(255, 160, 60, 255)),
        ("alpha", [140.0, 10.0, 172.0, 42.0], Color(255, 255, 255, 128)),
        ("fractional", [10.3, 70.6, 42.3, 102.6], Color(255, 255, 255, 255)),
    ]
}

fn image_frame() -> DisplayList {
    let mut l = DisplayList::new();
    // A dark bar under the translucent placement, so alpha is blended
    // over something that is not the page.
    l.rect([130.0, 20.0, 190.0, 32.0], Color(30, 30, 30, 255));
    for (_, r, tint) in image_cases() {
        l.image(r, lumen_static::display::ImageId(1), tint);
    }
    l
}

/// M51/T51.0 (R9-F2): images agree across the backends.
///
/// No pixel corpus held a `Cmd::Image` until now, so deleting the GPU's
/// image arm left the workspace green. Each placement's interior (one
/// pixel in from its box) must agree to 8 levels, and its ink — pixels
/// that are not the page — must cover more than half its area, so an
/// image that is not drawn at all cannot pass by agreeing with a page
/// that is not drawn either.
#[test]
fn images_agree_across_the_backends() {
    let Some(mut g) = gpu() else { return };
    g.set_images(Box::new(Card::new()));
    let mut c = canvas();
    c.images = Some(Box::new(Card::new()));
    c.clear(PAGE);
    c.render(&image_frame(), W, H);
    g.clear(PAGE);
    g.render(&image_frame(), W, H);
    let gp = g.read_rgba();
    for (name, r, _) in image_cases() {
        let inner = [r[0].ceil() as u32 + 1, r[1].ceil() as u32 + 1, r[2].floor() as u32 - 1, r[3].floor() as u32 - 1];
        let area = ((inner[2] - inner[0]) * (inner[3] - inner[1])) as usize;
        let (over, worst) = region_over(&c.pixels, &gp, W, inner, 8);
        let inked = |px: &[u8]| {
            let mut n = 0;
            for y in inner[1]..inner[3] {
                for x in inner[0]..inner[2] {
                    let i = ((y * W + x) * 4) as usize;
                    if px[i..i + 3] != [PAGE.0, PAGE.1, PAGE.2] {
                        n += 1;
                    }
                }
            }
            n
        };
        let (ci, gi) = (inked(&c.pixels), inked(&gp));
        eprintln!(
            "image {name}: {over} px over 8 inside (bound 0), worst {worst}; inked software {ci}, \
             gpu {gi} of {area} (bound > {})",
            area / 2
        );
        assert!(gi > area / 2 && ci > area / 2, "image {name} is not drawn: inked {ci}/{gi} of {area}");
        assert_eq!(over, 0, "image {name}: {over} px differ by more than 8 inside the placement");
    }
}

/// **M82/T82.5's pin: the GPU backend draws no path yet**, and T82.6
/// inverts this (`docs/plan-paint-vocabulary.md`). A filled square is
/// rendered on both backends; the software canvas inks it (the setup: the
/// scene puts a path where the GPU must not draw) and the GPU frame stays
/// the page, pixel for pixel. Printed beside each other so the inversion
/// reads as a number.
#[test]
fn path_pin_the_gpu_draws_no_path_until_t82_6() {
    use lumen_static::display::{PathEl, PathPaint, AFFINE_IDENTITY};
    let p = |x: f32, y: f32| Point { x, y };
    let els = [
        PathEl::Move(p(40.0, 40.0)),
        PathEl::Line(p(120.0, 40.0)),
        PathEl::Line(p(120.0, 120.0)),
        PathEl::Line(p(40.0, 120.0)),
        PathEl::Close,
    ];
    let mut l = DisplayList::new();
    l.path(&els, AFFINE_IDENTITY, PathPaint { fill: Color(200, 40, 40, 255), stroke: Color(0, 0, 0, 0), width: 0.0 });
    assert!(matches!(l.cmds[..], [Cmd::Path { .. }]), "setup: the list must hold exactly one path");
    let inked = |px: &[u8]| px.chunks_exact(4).filter(|q| q[0] != PAGE.0 || q[1] != PAGE.1 || q[2] != PAGE.2).count();
    let mut c = canvas();
    c.clear(PAGE);
    c.render(&l, W, H);
    let soft = inked(&c.pixels);
    assert!(soft > 0, "setup: the software canvas must draw the path (inked {soft})");
    let Some(mut g) = gpu() else { return };
    g.clear(PAGE);
    g.render(&l, W, H);
    let gpu = inked(&g.read_rgba());
    println!("path pin: software inks {soft} px, the GPU {gpu} px (pinned at 0 until T82.6)");
    assert_eq!(gpu, 0, "the GPU drew a path: T82.6 has landed, so invert this pin");
}
