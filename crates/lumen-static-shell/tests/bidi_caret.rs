//! **Caret, selection and hit test over bidi runs** — M81/T81.5
//! (`docs/plan-shaped-paint.md`).
//!
//! Every gate is headless, on the pinned face in `Shaping::Complex`
//! (`FontStack::from_static`, so runs that need the shaper reach it),
//! through a `Host` — once with `Everything` and once with
//! `Boundaries<256>` — and asserts that its input was handled.
//!
//! - **typing**: `שלום 123` typed into an empty `TextInput`, and into an
//!   empty `TextArea`. The paragraph is right to left (its first strong
//!   letter is), so every keystroke's caret is the paragraph's end, at the
//!   run's *left*. Lines are laid out from x 0 whatever their direction
//!   (`lumen_static_text::ShapedGlyph`), so that end does not move on
//!   screen; what moves is the start. The quantity is the caret's x
//!   measured **from the run's right edge**, where a right-to-left
//!   paragraph starts: it must fall at every keystroke.
//! - **tint**: `abc שלום` with bytes 2..8 selected (`c`, the space, `ש`,
//!   `ל`) in a `TextInput`, and over a `Selectable<String>`: the tint is
//!   two rects, and their columns cover every selected letter's ink
//!   columns and no unselected letter's (bound 0 stray, ±1 px).
//! - **click**: a press at the visual left end of the first line of a
//!   right-to-left `TextArea` puts the caret at that line's logical end
//!   (before its `\n`); a press at its right end, at its start.
//! - **the end wins a tie** (M81/T81.6): `שלום world 123`, the showcase's
//!   `#d_textinput_rtl`, ends at x 0, where `w`'s leading edge also is; a
//!   press there, or in the padding beyond it, lands at the text's end in a
//!   `TextInput` and a `TextArea`.
//! - **caret_rect**: at scale 2, `Host::caret_rect` is within 1 physical
//!   px of the painted caret column, at a direction change, at a
//!   right-to-left paragraph's end, and in a `TextArea`.
//!
//! Injections (T81.5's commit records their output): `find(byte >= at)`
//! restored in `caret::at` (typing: both lists stop falling at the
//! digits); one rect from the selection's first offset's x to its last's
//! (tint: stray columns over `ם` and `ו`); `TextArea::offset_at` measuring
//! each cluster at its trailing edge (click: the offset moves off the
//! `\n`); `TextInput::caret_rect` with `find(byte >= at)` while paint keeps
//! the exact lookup (caret_rect: the direction-change case is a word's
//! width off).
#![cfg(all(feature = "shaping", feature = "truetype"))]

use lumen_static::cx::Cluster;
use lumen_static::event::{Event, Key, Mods, PointerButton};
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::retain::Incremental;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{App as DeriveApp, Color, StaticTree, TextMeasure};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack, Shaping};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::Selectable;

#[path = "fonts/pinned.rs"]
mod pinned;

const PX: f32 = 16.0;
/// Typed one character at a time; a right-to-left paragraph whose last
/// run (the digits) runs left to right, so its end is a direction change.
const TYPED: &str = "\u{5e9}\u{5dc}\u{5d5}\u{5dd} 123";
/// Left-to-right paragraph, then a right-to-left word.
const MIXED: &str = "abc \u{5e9}\u{5dc}\u{5d5}\u{5dd}";
/// `c`, the space, `ש`, `ל`.
const SEL: core::ops::Range<usize> = 2..8;
/// Two right-to-left lines.
const LINES: &str = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}\n\u{5e2}\u{5d5}\u{5dc}\u{5dd}";
const TINT: Color = Color(0, 200, 0, 255);
const W: u32 = 480;
const H: u32 = 260;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Ui {
    #[widget_id("input")]
    input: TextInput<String>,
    #[widget_id("area")]
    area: TextArea<String>,
    #[widget_id("sel")]
    sel: Selectable<String>,
}

#[derive(DeriveApp, Default)]
struct Page {
    #[tree]
    ui: Ui,
}

type H<I> = Host<Page, Incremental, I>;

fn stack() -> FontStack {
    FontStack::from_static(pinned::bytes(), None).expect("the pinned face parses").with_shaping(Shaping::Complex)
}

fn index(wid: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as lumen_static::Widget>::SUB_TABLE, wid).expect(wid)
}

fn host<I: Invalidation>(font: &FontStack, input: &str, area: &str, sel: &str, scale: f32) -> (H<I>, FontMetrics) {
    let mut a = Page::default();
    a.ui.input = TextInput::new(input);
    a.ui.input.selection_color = Some(TINT);
    a.ui.area = TextArea::new(area);
    a.ui.sel.label.set(sel.to_owned());
    a.ui.sel.selection_color = Some(TINT);
    let mut h: H<I> = Host::new(a, (W as f32 * scale) as u32, (H as f32 * scale) as u32);
    h.set_scale(scale);
    let ink = Patch::new().font_size(PX).color(Color(0, 0, 0, 255)).background(Color(255, 255, 255, 255)).width(Length::FILL);
    h.set_stylesheet(&[
        Rule::new(Selector::Id("input"), ink),
        Rule::new(Selector::Id("area"), ink.height(Length::Fixed(80.0))),
        Rule::new(Selector::Id("sel"), ink),
    ]);
    let mut m = FontMetrics::with_scale(font.clone(), scale);
    h.pump_with(&mut m);
    h.pump_with(&mut m);
    (h, m)
}

fn key<I: Invalidation>(h: &mut H<I>, m: &mut FontMetrics, k: Key, mods: Mods) -> bool {
    h.post(Event::Modifiers(mods));
    h.post(Event::Key { key: k, down: true });
    h.post(Event::Key { key: k, down: false });
    let took = h.pump_with(m);
    h.post(Event::Modifiers(Mods::NONE));
    h.pump_with(m);
    took
}

/// Click the middle of a node, and assert it took the focus.
fn focus<I: Invalidation>(h: &mut H<I>, m: &mut FontMetrics, wid: &str) {
    let s = h.states[index(wid)];
    h.click(s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    let took = h.pump_with(m);
    assert!(took, "setup: the press on #{wid} was not handled");
    assert_eq!(h.input.focus, Some(index(wid) as u32), "setup: the press did not focus #{wid}");
}

fn name<I: Invalidation>() -> &'static str {
    let n = core::any::type_name::<I>();
    if n.contains("Everything") {
        "Everything"
    } else {
        "Boundaries<256>"
    }
}

// --- typing ---------------------------------------------------------------

/// Each keystroke's (caret x − the run's width), and whether it was
/// handled: the caret measured from the right edge, where the run starts.
fn typing<I: Invalidation>(font: &FontStack, wid: &str) -> Vec<(f32, bool)> {
    let (mut h, mut m) = host::<I>(font, "", "", "x", 1.0);
    focus(&mut h, &mut m, wid);
    let mut out = Vec::new();
    for ch in TYPED.chars() {
        h.post(Event::Text { ch });
        let took = h.pump_with(&mut m);
        let value = if wid == "input" { h.app.ui.input.as_str().to_owned() } else { h.app.ui.area.as_str().to_owned() };
        let run = m.measure_weighted(&value, PX, 400.0, f32::INFINITY).w;
        let caret = h.caret_rect().expect("a focused field reports its caret")[0];
        let s = h.states[index(wid)];
        out.push((caret - (s.pos.x + s.pad.0) - run, took));
    }
    let value = if wid == "input" { h.app.ui.input.as_str() } else { h.app.ui.area.as_str() };
    assert_eq!(value, TYPED, "setup: #{wid} does not hold what was typed");
    out
}

fn typing_both<I: Invalidation>(font: &FontStack) -> Vec<String> {
    let mut bad = Vec::new();
    for wid in ["input", "area"] {
        let xs = typing::<I>(font, wid);
        let rel: Vec<String> = xs.iter().map(|x| format!("{:.2}", x.0)).collect();
        println!(
            "{} #{wid}: caret x from the run's right edge per keystroke [{}] (bound: strictly falling); handled {:?}",
            name::<I>(),
            rel.join(", "),
            xs.iter().map(|x| x.1).collect::<Vec<_>>()
        );
        assert!(xs.iter().all(|x| x.1), "setup: {} #{wid}: a keystroke was not handled", name::<I>());
        if !xs.windows(2).all(|w| w[1].0 < w[0].0 - 0.5) {
            bad.push(format!("{} #{wid}: [{}]", name::<I>(), rel.join(", ")));
        }
    }
    bad
}

#[test]
fn typing_right_to_left_moves_the_caret_away_from_the_start() {
    let font = stack();
    let mut bad = typing_both::<Everything>(&font);
    bad.extend(typing_both::<Boundaries<256>>(&font));
    assert!(bad.is_empty(), "the caret did not move left of the run's start at every keystroke: {bad:?}");
}

// --- tint -------------------------------------------------------------------

/// Columns of a node's box: which hold ink (dark), which hold the tint.
fn columns(canvas: &Canvas, s: &lumen_static::NodeState, pw: u32) -> (Vec<bool>, Vec<bool>, u32) {
    let (x0, x1) = (s.pos.x as u32, (s.pos.x + s.size.w) as u32);
    let (y0, y1) = ((s.pos.y + 1.0) as u32, (s.pos.y + s.size.h - 1.0) as u32);
    let px = |x: u32, y: u32| {
        let p = ((y * pw + x) * 4) as usize;
        (canvas.pixels[p], canvas.pixels[p + 1], canvas.pixels[p + 2])
    };
    let mut ink = Vec::new();
    let mut tint = Vec::new();
    for x in x0..x1 {
        ink.push((y0..y1).any(|y| {
            let (r, g, b) = px(x, y);
            (r as u32 + g as u32 + b as u32) < 3 * 128
        }));
        tint.push((y0..y1).any(|y| {
            let (r, g, b) = px(x, y);
            g > 150 && r < 120 && b < 120
        }));
    }
    (ink, tint, x0)
}

/// Runs of `true`, as `(start, end)` column indices, end exclusive.
fn groups(v: &[bool]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < v.len() {
        if v[k] {
            let s = k;
            while k < v.len() && v[k] {
                k += 1;
            }
            out.push((s, k));
        } else {
            k += 1;
        }
    }
    out
}

/// The tint against the ink, `wid` selected over `SEL`: (covered,
/// selected ink columns, stray, tint rects), printed by the caller.
fn tint<I: Invalidation>(font: &FontStack, wid: &str) -> (usize, usize, Vec<usize>, usize) {
    // The ink, unselected and unfocused.
    let (plain, _) = host::<I>(font, MIXED, "x", MIXED, 1.0);
    let mut canvas = Canvas::with_font(W, H, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    plain.render(&mut canvas);
    let (ink, tint0, x0) = columns(&canvas, &plain.states[index(wid)], W);
    assert!(!tint0.iter().any(|&t| t), "setup: tint before anything is selected");
    let letters = groups(&ink);
    // a b c, then the word right to left: ם ו ל ש.
    assert_eq!(letters.len(), 7, "setup: #{wid}: {} ink groups, not the seven letters: {letters:?}", letters.len());
    let selected = [2usize, 5, 6];

    let (mut h, mut m) = host::<I>(font, MIXED, "x", MIXED, 1.0);
    if wid == "input" {
        focus(&mut h, &mut m, wid);
        assert!(key(&mut h, &mut m, Key::Home, Mods::NONE) || h.app.ui.input.caret() == 0, "setup: Home");
        for _ in 0..SEL.start {
            assert!(key(&mut h, &mut m, Key::Right, Mods::NONE), "setup: Right not handled");
        }
        for _ in SEL.start..SEL.start + 4 {
            assert!(key(&mut h, &mut m, Key::Right, Mods::SHIFT), "setup: Shift+Right not handled");
        }
        assert_eq!(h.app.ui.input.selection(), Some(SEL), "setup: the keys did not select bytes 2..8");
    } else {
        // A drag from byte 2's leading edge to byte 8's.
        let mut cl: Vec<Cluster> = Vec::new();
        m.shape(MIXED, PX, 400.0, f32::INFINITY, &mut cl);
        let x_of = |b: usize| cl.iter().rev().find(|c| c.byte as usize == b).expect("a cluster").x;
        let s = h.states[index(wid)];
        let (ox, y) = (s.pos.x + s.pad.0, s.pos.y + s.size.h / 2.0);
        h.post(Event::PointerDown { x: ox + x_of(SEL.start), y, button: PointerButton::Left, clicks: 1 });
        let down = h.pump_with(&mut m);
        h.post(Event::PointerMove { x: ox + x_of(SEL.end), y });
        let moved = h.pump_with(&mut m);
        h.post(Event::up(ox + x_of(SEL.end), y));
        h.pump_with(&mut m);
        assert!(down && moved, "setup: the press or the drag over #{wid} was not handled");
        assert_eq!(h.copy(), Some(&MIXED[SEL]), "setup: the drag did not select bytes 2..8");
    }
    let mut canvas = Canvas::with_font(W, H, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    h.render(&mut canvas);
    let (_, tint, x1) = columns(&canvas, &h.states[index(wid)], W);
    assert_eq!(x0, x1, "setup: the node moved");
    let near = |x: usize| (x.saturating_sub(1)..=(x + 1).min(tint.len() - 1)).any(|k| tint[k]);
    let inside = |x: usize| (x.saturating_sub(1)..=(x + 1).min(tint.len() - 1)).all(|k| tint[k]);
    let (mut covered, mut total) = (0, 0);
    let mut stray = Vec::new();
    for (g, &(a, b)) in letters.iter().enumerate() {
        for x in a..b {
            if selected.contains(&g) {
                total += 1;
                covered += near(x) as usize;
            } else if inside(x) {
                stray.push(x0 as usize + x);
            }
        }
    }
    (covered, total, stray, groups(&tint).len())
}

fn tint_both<I: Invalidation>(font: &FontStack, wid: &str) -> Vec<String> {
    let (covered, total, stray, rects) = tint::<I>(font, wid);
    println!(
        "{} #{wid} over {MIXED:?}, bytes {SEL:?}: selected ink columns covered {covered}/{total}, \
         stray columns over unselected letters {} {stray:?} (bound 0, ±1 px), tint rects {rects} (want 2)",
        name::<I>(),
        stray.len()
    );
    let mut bad = Vec::new();
    if covered != total || !stray.is_empty() || rects != 2 {
        bad.push(format!("{} #{wid}: covered {covered}/{total}, stray {stray:?}, rects {rects}", name::<I>()));
    }
    bad
}

#[test]
fn a_selection_across_a_direction_change_tints_exactly_the_selected_letters() {
    let font = stack();
    let mut bad = tint_both::<Everything>(&font, "input");
    bad.extend(tint_both::<Boundaries<256>>(&font, "input"));
    assert!(bad.is_empty(), "the tint is not the selected letters: {bad:?}");
}

#[test]
fn a_selectable_over_a_mixed_run_tints_exactly_the_selected_letters() {
    let font = stack();
    let mut bad = tint_both::<Everything>(&font, "sel");
    bad.extend(tint_both::<Boundaries<256>>(&font, "sel"));
    assert!(bad.is_empty(), "the Selectable's tint is not the selected letters: {bad:?}");
}

// --- click ------------------------------------------------------------------

fn click<I: Invalidation>(font: &FontStack, left: bool) -> (usize, bool) {
    let (mut h, mut m) = host::<I>(font, "x", LINES, "x", 1.0);
    let s = h.states[index("area")];
    let (ox, oy) = (s.pos.x + s.pad.0, s.pos.y + s.pad.1);
    let run = m.measure_weighted(LINES.split('\n').next().unwrap(), PX, 400.0, f32::INFINITY).w;
    let x = if left { ox + 1.0 } else { ox + run - 1.0 };
    let y = oy + m.line_height(PX) * 0.5;
    h.click(x, y);
    let took = h.pump_with(&mut m);
    assert_eq!(h.input.focus, Some(index("area") as u32), "setup: the press did not focus the TextArea");
    (h.app.ui.area.caret(), took)
}

fn click_both<I: Invalidation>(font: &FontStack) -> Vec<String> {
    let end = LINES.find('\n').unwrap();
    let (l, lt) = click::<I>(font, true);
    let (r, rt) = click::<I>(font, false);
    println!(
        "{}: a press at line 1's visual left end -> offset {l} (want {end}, the line's logical end); \
         at its right end -> offset {r} (want 0, its start); handled {lt} {rt}",
        name::<I>()
    );
    assert!(lt && rt, "setup: {}: a press was not handled", name::<I>());
    let mut bad = Vec::new();
    if l != end || r != 0 {
        bad.push(format!("{}: left {l} (want {end}), right {r} (want 0)", name::<I>()));
    }
    bad
}

#[test]
fn a_click_at_the_left_end_of_a_right_to_left_line_lands_at_its_logical_end() {
    let font = stack();
    let mut bad = click_both::<Everything>(&font);
    bad.extend(click_both::<Boundaries<256>>(&font));
    assert!(bad.is_empty(), "the click did not land at the line's logical end: {bad:?}");
}

// --- the end wins a tie (M81/T81.6) -----------------------------------------

/// The showcase's `#d_textinput_rtl` value: a right-to-left paragraph whose
/// last run is left to right, so its end (x 0) and `w`'s leading edge (x 0)
/// are one place with two offsets.
const SEEDED: &str = "\u{5e9}\u{5dc}\u{5d5}\u{5dd} world 123";

/// Press `dx` px from the run's left edge of `wid` (seeded with
/// [`SEEDED`]); answer the caret and whether the press was handled.
fn press_left<I: Invalidation>(font: &FontStack, wid: &str, dx: f32) -> (usize, bool) {
    let (mut h, mut m) = host::<I>(font, SEEDED, SEEDED, "x", 1.0);
    let s = h.states[index(wid)];
    h.click(s.pos.x + s.pad.0 + dx, s.pos.y + s.pad.1 + m.line_height(PX) * 0.5);
    let took = h.pump_with(&mut m);
    assert_eq!(h.input.focus, Some(index(wid) as u32), "setup: the press did not focus #{wid}");
    let caret = if wid == "input" { h.app.ui.input.caret() } else { h.app.ui.area.caret() };
    (caret, took)
}

#[test]
fn a_click_left_of_a_right_to_left_paragraph_ending_left_to_right_lands_at_its_end() {
    let font = stack();
    let end = SEEDED.len();
    let mut bad = Vec::new();
    for (strategy, run) in [
        ("Everything", press_left::<Everything> as fn(&FontStack, &str, f32) -> (usize, bool)),
        ("Boundaries<256>", press_left::<Boundaries<256>>),
    ] {
        for wid in ["input", "area"] {
            // In the padding, on the run's left edge, and 1 px inside it.
            for dx in [-3.0, 0.0, 1.0] {
                let (at, took) = run(&font, wid, dx);
                println!("{strategy}: #{wid} press {dx:+} px from the run's left -> offset {at} (want {end}, the end; `w` is 9); handled {took}");
                assert!(took, "setup: {strategy}: the press on #{wid} was not handled");
                if at != end {
                    bad.push(format!("{strategy} #{wid} {dx:+}: {at}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "a press at the visual left end did not land at the logical end ({end}): {bad:?}");
}

// --- caret_rect -------------------------------------------------------------

/// The painted caret's centre column in the node's padded box, physical
/// px, and its height: the column with the longest unbroken dark run, and
/// its neighbours at least 80% as tall (`tests/ime_area.rs`' rule).
fn painted<I: Invalidation>(h: &H<I>, font: &FontStack, wid: &str, scale: f32) -> (f32, u32) {
    let (pw, ph) = ((W as f32 * scale) as u32, (H as f32 * scale) as u32);
    let mut canvas = Canvas::with_font(pw, ph, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    h.render(&mut canvas);
    let s = h.states[index(wid)];
    let k = scale;
    let (x0, x1) = (((s.pos.x + s.pad.0) * k).floor() as u32, ((s.pos.x + s.size.w - s.pad.2 + 1.0) * k).ceil() as u32);
    let (y0, y1) = (((s.pos.y + 1.0) * k).floor() as u32, ((s.pos.y + s.size.h - 1.0) * k).ceil() as u32);
    let dark = |x: u32, y: u32| {
        let p = ((y * pw + x) * 4) as usize;
        let px = &canvas.pixels[p..p + 3];
        (px[0] as u32 + px[1] as u32 + px[2] as u32) < 3 * 128
    };
    let run = |x: u32| {
        let (mut best, mut cur) = (0u32, 0u32);
        for y in y0..y1.min(ph) {
            cur = if dark(x, y) { cur + 1 } else { 0 };
            best = best.max(cur);
        }
        best
    };
    let counts: Vec<u32> = (x0..x1.min(pw)).map(run).collect();
    let best = counts.iter().copied().max().unwrap_or(0);
    let peak = counts.iter().position(|&n| n == best).unwrap_or(0);
    let (mut lo, mut hi) = (peak, peak);
    while lo > 0 && counts[lo - 1] * 5 >= best * 4 {
        lo -= 1;
    }
    while hi + 1 < counts.len() && counts[hi + 1] * 5 >= best * 4 {
        hi += 1;
    }
    let (mut sum, mut w) = (0.0f32, 0.0f32);
    for (j, &n) in counts.iter().enumerate().take(hi + 1).skip(lo) {
        sum += (x0 as f32 + j as f32 + 0.5) * n as f32;
        w += n as f32;
    }
    (sum / w.max(1.0), best)
}

fn caret_cases<I: Invalidation>(font: &FontStack) -> Vec<String> {
    let scale = 2.0;
    let mut bad = Vec::new();
    for case in ["input at the direction change (byte 4)", "input at a right-to-left end", "area at line 1's end"] {
        let (mut h, mut m) = if case.starts_with("input at a") {
            host::<I>(font, "\u{5e9}\u{5dc}\u{5d5}\u{5dd}", "x", "x", scale)
        } else if case.starts_with("input") {
            host::<I>(font, MIXED, "x", "x", scale)
        } else {
            host::<I>(font, "x", LINES, "x", scale)
        };
        let wid = if case.starts_with("area") { "area" } else { "input" };
        let want = match case {
            c if c.starts_with("input at the") => {
                focus(&mut h, &mut m, wid);
                key(&mut h, &mut m, Key::Home, Mods::NONE);
                for _ in 0..4 {
                    assert!(key(&mut h, &mut m, Key::Right, Mods::NONE), "setup: Right not handled");
                }
                4
            }
            c if c.starts_with("input") => {
                focus(&mut h, &mut m, wid);
                key(&mut h, &mut m, Key::End, Mods::NONE);
                8
            }
            _ => {
                let s = h.states[index(wid)];
                h.click(s.pos.x + s.pad.0 + 1.0, s.pos.y + s.pad.1 + m.line_height(PX) * 0.5);
                assert!(h.pump_with(&mut m), "setup: the press was not handled");
                8
            }
        };
        let caret = if wid == "area" { h.app.ui.area.caret() } else { h.app.ui.input.caret() };
        assert_eq!(caret, want, "setup: {case}: the caret is at {caret}, not {want}");
        let r = h.caret_rect().expect("a focused field reports its caret");
        let (col, rows) = painted(&h, font, wid, scale);
        let line = r[3] - r[1];
        let centre = (r[0] + r[2]) / 2.0;
        let d = (centre - col).abs();
        println!(
            "{} {case} at scale {scale}: caret_rect centre {centre:.2}, painted caret column {col:.2}, |d| {d:.2} \
             physical px (bound 1.00); column {rows} px of a {line:.1} px line",
            name::<I>()
        );
        assert!(rows as f32 >= 0.7 * line, "setup: {case}: no caret column found ({rows} px of {line:.1})");
        if d > 1.0 {
            bad.push(format!("{} {case}: {d:.2} px", name::<I>()));
        }
    }
    bad
}

#[test]
fn caret_rect_is_the_painted_caret_over_bidi_runs_at_scale_2() {
    let font = stack();
    let mut bad = caret_cases::<Everything>(&font);
    bad.extend(caret_cases::<Boundaries<256>>(&font));
    assert!(bad.is_empty(), "caret_rect is not the painted caret: {bad:?}");
}
