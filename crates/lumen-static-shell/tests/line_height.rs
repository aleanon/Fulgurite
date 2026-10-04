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

// ---- M80/T80.2: `Style.line_height`, measured and painted -----------------

use lumen_static::display::Cmd;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Event, Key, StaticTree};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;
use lumen_static_widgets::TextArea;

/// Three lines at 16 px in `LABEL_W`, every line with a capital, so each
/// line's first inked row is its cap height and line tops compare.
const PROSE: &str = "Hold Lid Bold Kid Held Did";
const LABEL_W: f32 = 90.0;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct LabelUi {
    #[widget_id("prose")]
    prose: Label<String>,
}

#[derive(DeriveApp, Default)]
struct LabelApp {
    #[tree]
    ui: LabelUi,
}

fn prose_index() -> usize {
    lumen_static::style::index_of_wid(<LabelUi as lumen_static::Widget>::SUB_TABLE, "prose").expect("#prose")
}

/// A host laying `PROSE` out at `lh` sixteenths and `scale`, with the
/// pinned face; the label's logical box and the canvas it painted.
///
/// **The window is the label's width**, not a `Fixed` width on the label.
/// When this was written `Label::layout` measured its text against the
/// limits it was offered, not its own resolved width, so a fixed-width
/// label in a wider parent measured fewer lines than it painted. M80/T80.3
/// fixed that (its gate is `text_containment.rs`'s
/// `a_fixed_width_label_wraps_at_its_own_width_less_padding`); the setup is
/// kept so this gate measures what it always measured.
fn label_frame(font: &FontStack, lh: u8, scale: f32) -> (lumen_static::NodeState, Canvas) {
    let (host, canvas) = label_host(font, PROSE, lh, scale);
    (host.states[prose_index()], canvas)
}

/// [`label_frame`], keeping the host.
fn label_host(font: &FontStack, text: &str, lh: u8, scale: f32) -> (Host<LabelApp>, Canvas) {
    let (lw, lhgt) = (LABEL_W, 160.0f32);
    let (w, h) = ((lw * scale).round() as u32, (lhgt * scale).round() as u32);
    let mut a = LabelApp::default();
    a.ui.prose.set(text.to_owned());
    let mut host: Host<LabelApp> = Host::new(a, w, h);
    host.set_scale(scale);
    let mut patch = Patch::new().color(Color(0, 0, 0, 255));
    if lh != 0 {
        patch = patch.line_height(lh as f32 / 16.0);
    }
    host.set_stylesheet(&[Rule::new(Selector::Id("prose"), patch)]);
    let mut m = FontMetrics::with_scale(font.clone(), scale);
    host.pump_with(&mut m);
    let mut canvas = Canvas::with_font(w, h, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    host.render(&mut canvas);
    (host, canvas)
}

/// The first row of each run of inked rows inside `x0..x1`, in physical px.
fn ink_line_tops(canvas: &Canvas, x0: u32, x1: u32) -> Vec<u32> {
    let w = canvas.width;
    let inked = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        canvas.pixels[i..i + 3].iter().any(|&c| c != 255)
    };
    let mut tops = Vec::new();
    let mut prev = false;
    for y in 0..canvas.height {
        let r = (x0..x1.min(w)).any(|x| inked(x, y));
        if r && !prev {
            tops.push(y);
        }
        prev = r;
    }
    tops
}

/// **Measured and painted agree** — M80/T80.2's first gate. A `Label`
/// that wraps to three lines at `line_height` 24 measures exactly `3 ×
/// pitch` and paints its third line two pitches below its first, at
/// scale 1.0 and 2.0; at 0 it measures what `measure_weighted` says.
#[test]
fn a_label_at_a_line_height_measures_and_paints_three_pitches() {
    let font = stack();
    let lines = {
        struct Lines(u16);
        impl GlyphSink for Lines {
            fn cluster(&mut self, c: Cluster) {
                self.0 = self.0.max(c.line + 1);
            }
        }
        let mut sink = Lines(0);
        FontMetrics::new(font.clone()).shape(PROSE, PX, 400.0, LABEL_W, &mut sink);
        sink.0
    };
    println!("setup: {PROSE:?} in {LABEL_W} px breaks into {lines} lines (must be 3)");
    assert_eq!(lines, 3, "setup: the prose did not wrap to three lines");

    let lh = 24u8;
    let pitch = lh as f32 / 16.0 * PX;
    for scale in [1.0f32, 2.0] {
        let (st, canvas) = label_frame(&font, lh, scale);
        let content = st.size.h - st.pad.1 - st.pad.3;
        let want = 3.0 * pitch;
        let (x0, x1) = ((st.pos.x * scale) as u32, ((st.pos.x + st.size.w) * scale).ceil() as u32);
        let tops = ink_line_tops(&canvas, x0, x1);
        println!(
            "scale {scale}: content height {content} (want exactly 3 x 24/16 x 16 = {want}); \
             ink line tops {tops:?} (physical)"
        );
        assert!(tops.len() >= 3, "setup @ {scale}: {} line(s) of ink painted", tops.len());
        let d = tops[2] as f32 - tops[0] as f32;
        let want_d = 2.0 * pitch * scale;
        println!("scale {scale}: line 3's first ink row - line 1's = {d} px (want 2 x pitch x scale = {want_d} +- 1)");
        assert_eq!(content, want, "@ {scale}: the label measured {content}, not 3 pitches");
        assert!((d - want_d).abs() <= 1.0, "@ {scale}: line 3 painted {d} px below line 1, want {want_d}");
    }

    // Accept: at 0 the box is the measurer's own, as it was before T80.2.
    let (st, _) = label_frame(&font, 0, 1.0);
    let today = FontMetrics::new(font.clone()).measure_weighted(PROSE, PX, 400.0, LABEL_W);
    let content = st.size.h - st.pad.1 - st.pad.3;
    println!("line_height 0: content height {content} against measure_weighted's {}", today.h);
    assert_eq!(content, today.h, "line_height 0 changed the label's box");
}

/// **M80's line height under `Complex`** — M81/T81.3
/// (`docs/plan-shaped-paint.md`). The shaped branch re-places lines at the
/// pitch by M80's rule, as the fontdue branch does, and does not hand the
/// ratio to fontdue: a `Label` at `line_height` 12 (0.75 em, under the
/// face's spacing) on a `Complex` stack measures `3 × pitch + (nls −
/// pitch)` and paints line 2 one pitch below line 1, at scales 1 and 2.
/// Injection (T81.3's commit): the `Complex` branch ignores `line_height`;
/// the painted gap prints the face's spacing instead of the pitch.
/// `PROSE` with a zero-width joiner after its last word, so the run needs
/// the shaper (`needs_shaping`): plain Latin takes fontdue's path under
/// `Complex` since K81a's response. The joiner inks nothing and takes no
/// width, so the lines break where `PROSE`'s do.
#[cfg(feature = "shaping")]
const PROSE_SHAPED: &str = "Hold Lid Bold Kid Held Did\u{200d}";

#[cfg(feature = "shaping")]
#[test]
fn a_shaped_label_at_line_height_12_paints_its_second_line_one_pitch_down() {
    use lumen_static_shell::text::{text_counters, Shaping};
    let font = FontStack::from_static(pinned::bytes(), None).expect("parses").with_shaping(Shaping::Complex);
    let nls = nls(&font);
    let lh = 12u8;
    let pitch = lh as f32 / 16.0 * PX;
    let before = text_counters().complex;
    for scale in [1.0f32, 2.0] {
        let (host, _) = label_host(&font, PROSE_SHAPED, lh, scale);
        let st = host.states[prose_index()];
        let content = st.size.h - st.pad.1 - st.pad.3;
        let want = 3.0 * pitch + (nls - pitch).max(0.0);
        // Painted: the label's run placed as both canvases place it. At a
        // pitch under the face's spacing the lines' ink overlaps, so the
        // lines are told apart by their first glyphs (each line of `PROSE`
        // starts with a capital at the run's left edge), not by ink rows.
        let (rect, size, weight, line_height) = host
            .list
            .cmds
            .iter()
            .find_map(|c| match c {
                Cmd::Text { rect, size, weight, line_height, .. } => Some((*rect, *size, *weight, *line_height)),
                _ => None,
            })
            .expect("setup: the label painted no run");
        let mut scratch = ShapeScratch::new();
        font.place_run_into(&mut scratch, PROSE_SHAPED, size, weight, line_height, rect[2] - rect[0]);
        let left = scratch.placed().iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let mut starts: Vec<f32> = scratch.placed().iter().filter(|p| p.x <= left + 1.0).map(|p| p.y).collect();
        starts.sort_by(f32::total_cmp);
        println!(
            "Complex, scale {scale}: measured content height {content} (want 3 x pitch + (nls - pitch) = {want}, \
             pitch {pitch}, nls {nls}); painted line starts at y {starts:?} (physical, run line_height {line_height})"
        );
        assert_eq!(line_height, lh, "setup @ {scale}: the run carries line_height {line_height}");
        assert_eq!(starts.len(), 3, "setup @ {scale}: {} lines start at the left edge", starts.len());
        let d = starts[1] - starts[0];
        println!("Complex, scale {scale}: line 2's baseline - line 1's = {d} px (want pitch x scale = {} +- 1)", pitch * scale);
        assert!((content - want).abs() <= 1.0, "@ {scale}: the label measured {content}, not {want}");
        assert!((d - pitch * scale).abs() <= 1.0, "@ {scale}: line 2 painted {d} px below line 1, want {}", pitch * scale);
    }
    let shaped = text_counters().complex - before;
    println!("Complex: {shaped} runs through the shaper (setup > 0)");
    assert!(shaped > 0, "setup: nothing went through the shaper");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct AreaUi {
    #[widget_id("area")]
    area: TextArea,
}

#[derive(DeriveApp, Default)]
struct AreaApp {
    #[tree]
    ui: AreaUi,
}

/// **`TextArea`'s rows are the pitch** — the caret on line 3 sits two
/// pitches below the content top, over line 3's ink.
#[test]
fn a_text_area_puts_its_caret_on_the_pitch() {
    let font = stack();
    let lh = 32u8;
    let pitch = lh as f32 / 16.0 * PX;
    let (w, h) = (300u32, 240u32);
    let mut host: Host<AreaApp> = Host::new(AreaApp::default(), w, h);
    host.set_stylesheet(&[Rule::new(
        Selector::Id("area"),
        Patch::new()
            .width(Length::Fixed(240.0))
            .height(Length::Fixed(200.0))
            .color(Color(0, 0, 0, 255))
            .background(Color(255, 255, 255, 255))
            .line_height(lh as f32 / 16.0),
    )]);
    let mut m = FontMetrics::new(font.clone());
    host.pump_with(&mut m);
    host.post(Event::Key { key: Key::Tab, down: true });
    host.pump_with(&mut m);
    let i = lumen_static::style::index_of_wid(<AreaUi as lumen_static::Widget>::SUB_TABLE, "area").expect("#area");
    assert_eq!(host.input.focus, Some(i as u32), "setup: Tab did not focus the area");
    for (k, line) in ["Hd", "Hd", "Hd"].iter().enumerate() {
        if k > 0 {
            host.post(Event::Key { key: Key::Enter, down: true });
        }
        for ch in line.chars() {
            host.post(Event::Text { ch });
        }
    }
    host.pump_with(&mut m);
    assert_eq!(host.app.ui.area.as_str(), "Hd\nHd\nHd", "setup: the typing did not land");

    let st = host.states[i];
    let content_top = st.pos.y + st.pad.1;
    let (x0, x1) = (st.pos.x + st.pad.0, st.pos.x + st.size.w - st.pad.2);
    // The caret: the one 1 px wide rect inside the text box.
    let caret = host
        .list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Rect { rect, .. } if (rect[2] - rect[0] - 1.0).abs() < 0.01 && rect[0] >= x0 && rect[0] <= x1 => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("setup: no caret was painted");
    let mut canvas = Canvas::with_font(w, h, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    host.render(&mut canvas);
    // The text's ink: dark pixels inside the text box, left of the caret
    // (which is dark too), so neither the border nor the caret counts.
    let (y0, y1) = (content_top as u32, (st.pos.y + st.size.h - st.pad.3) as u32);
    let ink = |y: u32| {
        (x0 as u32..caret[0] as u32 - 1).any(|x| {
            let p = ((y * w + x) * 4) as usize;
            canvas.pixels[p..p + 3].iter().any(|&c| c < 128)
        })
    };
    let inked_rows: Vec<u32> = (y0..y1).filter(|&y| ink(y)).collect();
    let mut tops = Vec::new();
    for (k, &y) in inked_rows.iter().enumerate() {
        if k == 0 || inked_rows[k - 1] + 1 != y {
            tops.push(y);
        }
    }
    let d = caret[1] - content_top;
    println!(
        "caret top - content top = {d} px (want 2 x pitch = {} +- 1); caret rows {}..{}; ink line tops {tops:?}",
        2.0 * pitch,
        caret[1],
        caret[3]
    );
    assert!(tops.len() >= 3, "setup: {} line(s) of ink", tops.len());
    let line3: Vec<u32> = inked_rows.iter().copied().filter(|&y| y >= tops[2]).collect();
    let (lo, hi) = (*line3.first().unwrap() as f32, *line3.last().unwrap() as f32 + 1.0);
    println!("line 3's ink rows {lo}..{hi} against the caret's {}..{} (+- 1)", caret[1], caret[3]);
    assert!((d - 2.0 * pitch).abs() <= 1.0, "the caret sits {d} px below the content top, want {}", 2.0 * pitch);
    assert!(
        lo >= caret[1] - 1.0 && hi <= caret[3] + 1.0,
        "line 3's ink {lo}..{hi} is not under the caret {}..{}",
        caret[1],
        caret[3]
    );
}
