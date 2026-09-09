//! **Our software rasteriser against Vello's** — one scene, one binary.
//!
//! `vello_cpu` is the closest thing to a control this project has: a CPU
//! 2D rasteriser, in Rust, SIMD and multithreaded, written by people who
//! do this full time. Every other arm in `benches-competitive` stops at
//! a display list (`docs/results-competitive.md` §2), so none of them
//! measure rasterisation at all. This one does, and it is the only arm
//! where lumen-static is *not* structurally advantaged: both sides take
//! the same scene to the same pixels on the same core.
//!
//! **Both renderers in one binary, deliberately.** Two binaries put a
//! difference in what the optimiser could see into a column labelled as
//! a difference in what the code costs, which is the confound
//! `docs/results-wide-stores.md` records finding twice.
//!
//! **What is and is not comparable** — stated here because the table
//! cannot carry it:
//!
//! - Vello is a *vector* renderer: every shape is a filled Bézier path
//!   through a sparse-strip pipeline. Ours is a fixed command set with a
//!   closed-form rasteriser per command. The scene is one a UI paints,
//!   so it is the case ours is built for and a fraction of what Vello
//!   accepts. A scene of arbitrary paths would not run here at all.
//! - Placement is shared. Both sides get glyph **indices and advances
//!   from the same fontdue face**, so the columns differ in glyph
//!   rasterisation and compositing, not in shaping.
//! - Shadows differ in model: ours is a quadratic falloff from a signed
//!   distance, Vello's is a true Gaussian. Same silhouette, different
//!   pixels.
//! - Vello composites premultiplied; we composite straight. Neither is
//!   converted here, because the conversion is the `blit` stage and both
//!   arms stop before it.

use std::sync::Arc;
use std::time::Instant;

use lumen_static::display::{Color, DisplayList, Stops};
use lumen_static::Point;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;

use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{Point as KPoint, Rect as KRect, Shape as _, Stroke};
use vello_cpu::peniko::{Blob, FontData, Gradient};
use vello_cpu::{
    Level, Pixmap, RasterizerSettings, RenderContext, RenderMode, RenderSettings, Resources,
};

/// The same face both sides load. `FontStack::system` picks the first of
/// these that exists; this repeats the list so the vello arm and the
/// lumen arm cannot end up on different faces, which would make the text
/// column a comparison of two fonts.
const CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
];

fn font_bytes() -> (Vec<u8>, &'static str) {
    for p in CANDIDATES {
        if let Ok(b) = std::fs::read(p) {
            return (b, p);
        }
    }
    panic!("no system face: text is most of a raster's cost, so a run without one is not the workload");
}

const CARD_W: f32 = 220.0;
const CARD_H: f32 = 90.0;
const TITLE: &str = "Card title";
const SUB: &str = "a secondary line";

/// The scene from `benches-competitive --bin raster-scaling`, verbatim:
/// cards with shadows, text, rules, and a gradient header. Kept
/// identical so this arm's lumen column can be checked against the
/// numbers in `docs/results-raster-scaling.md` rather than standing
/// alone.
fn lumen_scene(w: u32, h: u32, list: &mut DisplayList) {
    let (fw, fh) = (w as f32, h as f32);
    list.gradient(
        [0.0, 0.0, fw, 64.0],
        Stops::two(Color(30, 40, 80, 255), Color(20, 24, 40, 255)),
        1.57,
    );
    for (x, y) in cards(fw, fh) {
        let card = [x, y, x + 204.0, y + 74.0];
        list.shadow(card, 8.0, 6.0);
        list.rounded_rect(card, Color(36, 42, 58, 255), 8.0);
        list.text([x + 12.0, y + 10.0, x + 190.0, y + 30.0], TITLE, 16.0, Color(240, 244, 250, 255));
        list.text([x + 12.0, y + 34.0, x + 190.0, y + 50.0], SUB, 13.0, Color(150, 160, 180, 255));
        list.line(
            Point { x: x + 12.0, y: y + 60.0 },
            Point { x: x + 190.0, y: y + 60.0 },
            1.0,
            Color(70, 80, 100, 255),
        );
    }
}

fn cards(fw: f32, fh: f32) -> Vec<(f32, f32)> {
    let cols = (fw / CARD_W).max(1.0) as u32;
    let rows = (fh / CARD_H).max(1.0) as u32;
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            out.push((c as f32 * CARD_W + 8.0, r as f32 * CARD_H + 72.0));
        }
    }
    out
}

fn rgba(c: Color) -> AlphaColor<Srgb> {
    AlphaColor::from_rgba8(c.0, c.1, c.2, c.3)
}

/// Glyph indices and pen positions for one run, from the **fontdue**
/// face — the same metrics `FontStack` lays out with.
///
/// Not a shaper: one advance per character, no kerning, which is what
/// fontdue's `Layout` does for these strings too. The point is that
/// neither renderer is credited or charged for the other's text
/// placement; each is handed the same glyphs at the same pen positions
/// and asked to draw them.
fn run_glyphs(font: &fontdue::Font, text: &str, px: f32, x: f32, y: f32) -> Vec<vello_cpu::Glyph> {
    let mut pen = x;
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let idx = font.lookup_glyph_index(ch);
        out.push(vello_cpu::Glyph { id: idx as u32, x: pen, y });
        pen += font.metrics_indexed(idx, px).advance_width;
    }
    out
}

/// The same scene, recorded into a `RenderContext`.
fn vello_scene(
    w: u32,
    h: u32,
    ctx: &mut RenderContext,
    res: &mut Resources,
    fd: &FontData,
    fdue: &fontdue::Font,
) {
    let (fw, fh) = (w as f32, h as f32);

    // The header. `angle: 1.57` in the lumen list is straight down.
    ctx.set_paint(
        Gradient::new_linear(KPoint::new(0.0, 0.0), KPoint::new(0.0, 64.0))
            .with_stops([rgba(Color(30, 40, 80, 255)), rgba(Color(20, 24, 40, 255))]),
    );
    ctx.fill_rect(&KRect::new(0.0, 0.0, fw as f64, 64.0));

    for (x, y) in cards(fw, fh) {
        let card = KRect::new(x as f64, y as f64, (x + 204.0) as f64, (y + 74.0) as f64);

        // The shadow. Ours is a quadratic falloff over `spread` px from
        // the signed distance; Vello's is a Gaussian, and it cuts the
        // kernel at 2.5 sigma -- so 6 px of visible spread is sigma 2.4.
        ctx.set_paint(rgba(Color(0, 0, 0, 90)));
        ctx.fill_blurred_rounded_rect(&card, 8.0, 6.0 / 2.5, false);

        ctx.set_paint(rgba(Color(36, 42, 58, 255)));
        ctx.fill_path(&KRect::to_rounded_rect(card, 8.0).to_path(0.1));

        ctx.set_paint(rgba(Color(240, 244, 250, 255)));
        ctx.glyph_run(res, fd)
            .font_size(16.0)
            .hint(false)
            .fill_glyphs(run_glyphs(fdue, TITLE, 16.0, x + 12.0, y + 10.0 + 16.0).into_iter());

        ctx.set_paint(rgba(Color(150, 160, 180, 255)));
        ctx.glyph_run(res, fd)
            .font_size(13.0)
            .hint(false)
            .fill_glyphs(run_glyphs(fdue, SUB, 13.0, x + 12.0, y + 34.0 + 13.0).into_iter());

        ctx.set_paint(rgba(Color(70, 80, 100, 255)));
        ctx.set_stroke(Stroke::new(1.0));
        ctx.stroke_path(
            &vello_cpu::kurbo::Line::new(
                KPoint::new((x + 12.0) as f64, (y + 60.0) as f64),
                KPoint::new((x + 190.0) as f64, (y + 60.0) as f64),
            )
            .to_path(0.1),
        );
    }
}

/// Min of `n` timings of `f`, in milliseconds. Min rather than mean, as
/// everywhere else in this harness: every sample above the minimum is
/// something else the machine was doing.
fn best_of(n: u32, mut f: impl FnMut()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..n {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    best
}

/// What a frame *looks like*, as two scalars: the fraction of pixels
/// that are not the page colour, and the mean luminance.
///
/// **Two, because one is blind.** Ink alone catches an arm that drew
/// nothing, and nothing else: the text in this scene sits on top of a
/// card that is already inked, so dropping every glyph moves the ink
/// fraction by exactly zero -- measured, on the first version of this
/// gate, while the same injection made the vello arm 13% faster.
/// That is the second vacuous shape `AGENT.md` names, arriving by the
/// front door. Mean luminance sees it: the glyphs are light on a dark
/// card, so losing them darkens the frame.
///
/// Both sides are reduced to the same representation first -- straight
/// RGBA over the page colour -- because vello composites premultiplied
/// onto transparency and we composite straight onto an opaque page, and
/// comparing those two buffers directly would compare the convention.
fn shape(px: &[u8]) -> (f32, f32) {
    let n = (px.len() / 4) as f32;
    let mut inked = 0u32;
    let mut luma = 0.0f64;
    for p in px.chunks_exact(4) {
        if p[0] != 255 || p[1] != 255 || p[2] != 255 {
            inked += 1;
        }
        luma += 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
    }
    (inked as f32 / n, (luma / n as f64) as f32)
}

/// Mean luminance per 8x8 block.
///
/// **The whole-frame mean cannot see a missing command class and this
/// can.** Measured: dropping every glyph from the vello arm moved the
/// frame's mean luminance by 1.3 -- less than the shadow models already
/// differ by -- because the glyphs are 3% of the pixels. The same
/// injection moves the worst 8x8 block by 60, because a block that held
/// a word now holds none. Local is what makes the difference visible;
/// 8x8 is small enough to isolate a word and large enough that
/// antialiasing and a half-pixel of shadow falloff average out.
fn blocks(px: &[u8], w: u32, h: u32) -> Vec<f32> {
    const B: u32 = 8;
    let (bw, bh) = (w.div_ceil(B), h.div_ceil(B));
    let mut out = vec![0.0f32; (bw * bh) as usize];
    let mut cnt = vec![0u32; (bw * bh) as usize];
    for y in 0..h {
        for x in 0..w {
            let p = &px[((y * w + x) * 4) as usize..][..4];
            let l = 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
            let k = ((y / B) * bw + x / B) as usize;
            out[k] += l;
            cnt[k] += 1;
        }
    }
    for (v, n) in out.iter_mut().zip(cnt) {
        *v /= n as f32;
    }
    out
}

/// Vello's premultiplied output over the same opaque page our canvas
/// clears to, so `shape` sees one convention.
fn over_page(pm: &Pixmap, page: Color) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.data().len() * 4);
    for p in pm.data() {
        let [r, g, b, a] = p.to_u8_array();
        let inv = 255 - a as u32;
        let mix = |c: u8, bg: u8| ((c as u32) + (bg as u32 * inv) / 255).min(255) as u8;
        out.extend_from_slice(&[mix(r, page.0), mix(g, page.1), mix(b, page.2), 255]);
    }
    out
}

fn main() {
    let threads: usize = std::env::var("VELLO_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let (bytes, path) = font_bytes();
    let font = FontStack::from_bytes(&bytes, None).expect("the face fontdue just read");
    let fdue = fontdue::Font::from_bytes(&bytes[..], fontdue::FontSettings::default())
        .expect("the face fontdue just read");
    let fd = FontData::new(Blob::new(Arc::new(bytes.clone()) as Arc<dyn AsRef<[u8]> + Send + Sync>), 0);

    println!("lumen-static software canvas vs vello_cpu {}", env!("CARGO_PKG_VERSION"));
    println!("   face: {path}");
    println!("   lumen: simd={} threads={}", cfg!(feature = "simd"), cfg!(feature = "threads"));
    println!("   vello: num_threads={threads} (0 = single-threaded), OptimizeSpeed, Level::new()\n");
    println!(
        "{:<14} {:>7} {:>10} {:>10} {:>9} {:>8} {:>8} {:>8} {:>8}",
        "canvas", "Mpx", "lumen ms", "vello ms", "ratio", "v.rec", "dink<4", "dlum<5", "dblk<65"
    );

    for (w, h, label) in [
        (440, 520, "tally window"),
        (1280, 720, "720p"),
        (1920, 1080, "1080p"),
        (2560, 1440, "1440p"),
        (3840, 2160, "4K"),
    ] {
        // ---- lumen ----
        let mut list = DisplayList::new();
        lumen_scene(w, h, &mut list);
        let mut canvas = Canvas::with_font(w, h, font.clone());
        let area = [0.0, 0.0, w as f32, h as f32];
        let draw_lumen = |canvas: &mut Canvas, list: &DisplayList| {
            canvas.clear(lumen_static_shell::CLEAR);
            canvas.render_within(list, area);
        };
        best_of(5, || draw_lumen(&mut canvas, &list));
        let lumen = best_of(30, || draw_lumen(&mut canvas, &list));
        let (l_ink, l_luma) = shape(&canvas.pixels);
        let l_blocks = blocks(&canvas.pixels, w, h);

        // ---- vello ----
        let settings = RenderSettings { level: Level::new(), num_threads: threads as u16 };
        let rs = RasterizerSettings { render_mode: RenderMode::OptimizeSpeed, ..Default::default() };
        let mut ctx = RenderContext::new_with(w as u16, h as u16, settings);
        let mut res = Resources::new();
        let mut pixmap = Pixmap::new(w as u16, h as u16);

        // Recording is Vello's own stage, not ours: a `RenderContext` is
        // filled per frame the way our `DisplayList` is, so the honest
        // total is record + rasterise and the split is worth printing.
        let record = |ctx: &mut RenderContext, res: &mut Resources| {
            ctx.reset();
            vello_scene(w, h, ctx, res, &fd, &fdue);
            ctx.flush();
        };
        let frame = |ctx: &mut RenderContext, res: &mut Resources, pm: &mut Pixmap| {
            record(ctx, res);
            ctx.render_with(pm, res, rs);
        };
        best_of(5, || frame(&mut ctx, &mut res, &mut pixmap));
        let vello = best_of(30, || frame(&mut ctx, &mut res, &mut pixmap));
        let v_record = best_of(30, || record(&mut ctx, &mut res));
        let v_px = over_page(&pixmap, lumen_static_shell::CLEAR);
        let (v_ink, v_luma) = shape(&v_px);
        let d_block = l_blocks
            .iter()
            .zip(blocks(&v_px, w, h))
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);

        // Eyes on it, not only the ink proxy: `VELLO_DUMP=1` writes both
        // frames so the scenes can be compared as pictures. Two
        // renderers agreeing about an ink fraction is exactly the shape
        // of a comparison of two wrong frames.
        if std::env::var("VELLO_DUMP").is_ok() && label == "tally window" {
            std::fs::write("lumen.ppm", canvas.to_ppm()).unwrap();
            let copy = Pixmap::from_parts(pixmap.data().to_vec(), w as u16, h as u16);
            std::fs::write("vello.png", copy.into_png().unwrap()).unwrap();
        }

        // **Both arms drew, and both drew the same scene.**
        //
        // Three bounds, because the first two are each blind to
        // something and both blind spots were found by injection, not
        // by reading:
        //
        // - ink catches an arm that drew nothing, and nothing else;
        // - the frame's mean luminance catches a dropped *large* class
        //   -- but not the glyphs, which are 3% of the pixels;
        // - the worst 8x8 block catches anything, anywhere.
        //
        // Bounds and the measured maxima they sit over are in the
        // table, per `AGENT.md`: 4.0 over 1.8, 5.0 over 2.3, 65 over
        // 46. The block bound sits between two measured numbers rather
        // than at a round multiple of one: the shadow models genuinely
        // disagree by 46 in the darkest corner of a card, and dropping
        // every glyph from the vello arm takes the worst block to 88.
        // Tighter than 46 gates on the shadow model; looser than 88
        // does not see the glyphs go. Both are deterministic.
        let d_ink = (l_ink - v_ink).abs() * 100.0;
        let d_luma = (l_luma - v_luma).abs();
        assert!(l_ink > 0.10, "{label}: lumen inked {l_ink:.3} of the canvas -- it drew nothing");
        assert!(v_ink > 0.10, "{label}: vello inked {v_ink:.3} of the canvas -- it drew nothing");
        assert!(
            d_ink <= 4.0,
            "{label}: ink differs by {d_ink:.1} points (lumen {:.1}%, vello {:.1}%) -- \
             the two arms are not drawing the same scene",
            l_ink * 100.0,
            v_ink * 100.0
        );
        assert!(
            d_luma <= 5.0,
            "{label}: mean luminance differs by {d_luma:.1} (lumen {l_luma:.1}, vello \
             {v_luma:.1}) -- one arm is missing a command class",
        );
        assert!(
            d_block <= 65.0,
            "{label}: worst 8x8 block differs by {d_block:.0} of 255 -- one arm is \
             missing something the other drew there",
        );

        let mpx = (w as f64 * h as f64) / 1e6;
        println!(
            "{label:<14} {mpx:>7.2} {lumen:>10.2} {vello:>10.2} {:>8.2}x {v_record:>8.2} \
             {d_ink:>8.1} {d_luma:>8.1} {d_block:>8.0}",
            vello / lumen
        );
    }
}
