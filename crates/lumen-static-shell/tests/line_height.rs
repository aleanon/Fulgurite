//! **Lines at a pitch** — M80/T80.1 (`docs/plan-text-fit.md`).
//!
//! `FontStack::place_run_into` takes a `line_height` in sixteenths of the
//! size and re-places fontdue's lines itself: line *k*'s box at `k ×
//! pitch`, its glyphs at half-leading inside it, and, when the pitch is
//! below the face's own spacing, every line shifted down by half the
//! deficit so the ink stays inside a box `n × pitch + max(0, nls −
//! pitch)` tall. Nothing sets the byte from a `Style` until T80.2, so
//! these drive the placer and the software canvas directly.
//!
//! Three gates, each printing its quantity beside its bound:
//! - **placement**: line 2's first glyph sits one pitch below line 1's,
//!   and the run's box is `2 × pitch + max(0, nls − pitch)`, at 16, 24
//!   and 32 sixteenths; at 0 the placement is fontdue's own, checked
//!   against a fontdue layout this file builds itself.
//! - **ink**: at 1.0× (under the face's spacing) no painted row lies
//!   above or below the run's rect.
//! - **damage**: a text change over that run, recorded through
//!   `Recording`, damages every pixel it changes.
#![cfg(feature = "truetype")]

use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use lumen_static::display::{Color, DisplayList, Painter, Renderer};
use lumen_static::retain::{Incremental, Recording, Retention};
use lumen_static::{Cluster, GlyphSink, TextMeasure};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack, ShapeScratch};

#[path = "fonts/pinned.rs"]
mod pinned;

/// Two lines, each with an ascender (Å) and descenders (g, j, y), so the
/// ink reaches both ends of a line box.
const RUN: &str = "Ågjy\nÅgjy";
/// The same geometry with different glyphs, for the damage gate.
const CHANGED: &str = "Ébjq\nÉbjq";
const PX: f32 = 16.0;
/// Inked glyphs per line of `RUN`.
const PER_LINE: usize = 4;

fn stack() -> FontStack {
    FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses")
}

/// The face's new-line size at `PX`, ceiled as fontdue ceils it.
fn nls(font: &FontStack) -> f32 {
    FontMetrics::new(font.clone()).line_height(PX).ceil()
}

/// Lines the run breaks into, from the clusters the measurer reports.
fn lines_of(font: &FontStack, text: &str) -> u16 {
    struct Lines(u16);
    impl GlyphSink for Lines {
        fn cluster(&mut self, c: Cluster) {
            self.0 = self.0.max(c.line + 1);
        }
        fn is_full(&self) -> bool {
            false
        }
    }
    let mut sink = Lines(0);
    FontMetrics::new(font.clone()).shape(text, PX, 400.0, f32::INFINITY, &mut sink);
    sink.0
}

#[test]
fn lines_land_one_pitch_apart_and_the_box_is_n_pitches() {
    let font = stack();
    let nls = nls(&font);
    let n = lines_of(&font, RUN);
    println!("setup: {RUN:?} breaks into {n} lines (must be 2); nls {nls} px at {PX} px");
    assert_eq!(n, 2, "setup: the run did not break into two lines");

    let mut scratch = ShapeScratch::new();
    for lh in [16u8, 24, 32] {
        let size = font.place_run_into(&mut scratch, RUN, PX, 400.0, lh, f32::INFINITY);
        let placed = scratch.placed();
        assert_eq!(placed.len(), 2 * PER_LINE, "setup: {lh}: {} glyphs inked", placed.len());
        let pitch = lh as f32 / 16.0 * PX;
        let delta = placed[PER_LINE].y - placed[0].y;
        let deficit = (nls - pitch).max(0.0);
        let ink = placed.iter().map(|p| p.y + p.h as f32).fold(0.0, f32::max);
        let want = (2.0 * pitch + deficit).ceil();
        println!(
            "line_height {lh}: line 2 - line 1 = {delta} px (pitch {pitch} +- 0.5); \
             box h {} (want 2 x pitch + max(0, nls - pitch) = {want}; nls {nls}, deficit {deficit}, \
             placed ink bottom {ink})",
            size.h
        );
        assert!((delta - pitch).abs() <= 0.5, "line_height {lh}: lines {delta} px apart, pitch {pitch}");
        assert_eq!(size.h, want, "line_height {lh}: the box is {} px, want {want}", size.h);
    }
}

/// **At 0 the placement is fontdue's** — the accept direction, and the
/// observer for a placer that treated 0 as `pitch = px`.
///
/// Compared against a fontdue layout built here, not against
/// `place_run` (which now calls `place_run_into` with 0, so it would move
/// with it): every glyph keeps fontdue's `x`, and its `y` differs from
/// fontdue's by one constant, the run's cap shift. A re-placed second
/// line breaks the constant.
#[test]
fn at_zero_the_placement_is_fontdues_own() {
    let font = stack();
    let face = fontdue::Font::from_bytes(pinned::bytes(), fontdue::FontSettings::default()).expect("face");
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings::default());
    layout.append(&[&face], &TextStyle::new(RUN, PX, 0));
    let want: Vec<_> = layout.glyphs().iter().filter(|g| g.width > 0 && g.height > 0).collect();

    let mut scratch = ShapeScratch::new();
    let size = font.place_run_into(&mut scratch, RUN, PX, 400.0, 0, f32::INFINITY);
    let got = scratch.placed();
    assert_eq!(got.len(), want.len(), "setup: {} placed against fontdue's {}", got.len(), want.len());
    assert_eq!(got.len(), 2 * PER_LINE, "setup: both lines inked");
    let shift = got[0].y - want[0].y;
    let moved = got.iter().zip(&want).filter(|(p, g)| p.x != g.x || p.y - g.y != shift).count();
    let measured = FontMetrics::new(font.clone()).measure_weighted(RUN, PX, 400.0, f32::INFINITY);
    println!(
        "line_height 0: {moved} of {} glyphs off fontdue's placement (bound 0; cap shift {shift}); \
         box {}x{} against the measurer's {}x{}",
        got.len(),
        size.w,
        size.h,
        measured.w,
        measured.h
    );
    assert_eq!(moved, 0, "line_height 0 moved glyphs off fontdue's own placement");
    assert_eq!((size.w, size.h), (measured.w, measured.h), "line_height 0 changed the run's box");
}

/// Paint `text` at `lh` into a canvas, in a rect the placer's own box
/// sizes, and return (rect, pixels).
fn paint(font: &FontStack, text: &str, lh: u8, canvas: &mut Canvas) -> [f32; 4] {
    let mut scratch = ShapeScratch::new();
    let size = font.place_run_into(&mut scratch, text, PX, 400.0, lh, f32::INFINITY);
    let rect = [20.0, 30.0, 20.0 + size.w, 30.0 + size.h];
    let mut list = DisplayList::new();
    list.text_spaced(rect, text, PX, 400.0, lh, Color(0, 0, 0, 255));
    canvas.clear(Color(255, 255, 255, 255));
    canvas.render(&list, W, H);
    rect
}

const W: u32 = 160;
const H: u32 = 120;

fn inked(canvas: &Canvas, x: u32, y: u32) -> bool {
    let i = ((y * W + x) * 4) as usize;
    canvas.pixels[i..i + 3].iter().any(|&c| c != 255)
}

#[test]
fn below_the_faces_spacing_the_ink_stays_in_the_rect() {
    let font = stack();
    let nls = nls(&font);
    let lh = 16u8;
    let pitch = lh as f32 / 16.0 * PX;
    let half = (pitch - nls) / 2.0;
    println!("setup: line_height {lh} = pitch {pitch} px against nls {nls}: half-leading {half} (must be < 0)");
    assert!(half < 0.0, "setup: the pitch is not below the face's spacing");

    let mut canvas = Canvas::with_font(W, H, font.clone());
    let rect = paint(&font, RUN, lh, &mut canvas);
    let (mut above, mut below, mut total) = (0u32, 0u32, 0u32);
    for y in 0..H {
        let row = (0..W).any(|x| inked(&canvas, x, y));
        if !row {
            continue;
        }
        total += 1;
        if (y as f32) < rect[1] {
            above += 1;
        }
        if (y as f32) >= rect[3] {
            below += 1;
        }
    }
    println!("ink rows: {total} in all; {above} above rect y {} (bound 0), {below} below y {} (bound 0)", rect[1], rect[3]);
    assert!(total > pitch as u32, "setup: {total} inked rows cannot be two lines");

    // The damage `Recording` reports for a text change over the run must
    // cover every pixel the change moves.
    let mut ret = Incremental::default();
    let record = |ret: &mut Incremental, text: &str| {
        let mut sink = DisplayList::new();
        let mut scratch = ShapeScratch::new();
        let size = font.place_run_into(&mut scratch, text, PX, 400.0, lh, f32::INFINITY);
        {
            let mut rec = Recording::new(&mut sink, ret);
            rec.text_spaced([20.0, 30.0, 20.0 + size.w, 30.0 + size.h], text, PX, 400.0, lh, Color(0, 0, 0, 255));
        }
        ret.finish()
    };
    record(&mut ret, RUN);
    record(&mut ret, RUN);
    let damage = record(&mut ret, CHANGED);
    let regions: Vec<[f32; 4]> = damage.regions([0.0, 0.0, W as f32, H as f32]).collect();

    let mut before = Canvas::with_font(W, H, font.clone());
    paint(&font, RUN, lh, &mut before);
    let mut after = Canvas::with_font(W, H, font.clone());
    paint(&font, CHANGED, lh, &mut after);
    let (mut changed, mut outside) = (0u32, 0u32);
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if before.pixels[i..i + 3] == after.pixels[i..i + 3] {
                continue;
            }
            changed += 1;
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if !regions.iter().any(|r| r[0] <= fx && fx < r[2] && r[1] <= fy && fy < r[3]) {
                outside += 1;
            }
        }
    }
    println!("text change: {changed} pixels changed, {outside} outside the damage {regions:?} (bound 0)");
    // Asserted after both are printed, so a red run shows both numbers.
    assert!(changed > 0, "setup: the text change moved no pixel");
    assert_eq!(above, 0, "ascenders rose above the run's rect");
    assert_eq!(below, 0, "descenders fell below the run's rect");
    assert_eq!(outside, 0, "a text change left pixels outside its damage");
}

/// **The software canvas paints the byte** — the software half of
/// "each backend passes the byte" (the GPU half is `parity.rs`'s
/// `line_height` case). In one rect, line 2's first inked row sits one
/// pitch below line 1's at 24 and at 32 sixteenths.
#[test]
fn the_canvas_paints_the_second_line_at_the_pitch() {
    let font = stack();
    let rect = [20.0, 10.0, 120.0, 110.0];
    // The first inked row of each of the first two runs of inked rows.
    let line_tops = |lh: u8| -> (u32, u32) {
        let mut canvas = Canvas::with_font(W, H, font.clone());
        canvas.clear(Color(255, 255, 255, 255));
        let mut list = DisplayList::new();
        list.text_spaced(rect, RUN, PX, 400.0, lh, Color(0, 0, 0, 255));
        canvas.render(&list, W, H);
        let mut tops = Vec::new();
        let mut prev = false;
        for y in 0..H {
            let r = (0..W).any(|x| inked(&canvas, x, y));
            if r && !prev {
                tops.push(y);
            }
            prev = r;
        }
        assert!(tops.len() >= 2, "setup: line_height {lh} painted {} line(s) of ink", tops.len());
        (tops[0], tops[1])
    };
    for lh in [24u8, 32] {
        let pitch = lh as f32 / 16.0 * PX;
        let (a, b) = line_tops(lh);
        let d = b as f32 - a as f32;
        println!("canvas, line_height {lh}: line 2's top ink row - line 1's = {d} px (pitch {pitch} +- 1)");
        assert!((d - pitch).abs() <= 1.0, "the canvas painted line 2 {d} px below line 1, pitch {pitch}");
    }
}
