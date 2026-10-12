//! **The IME area is the painted caret** — M80/T80.5
//! (`docs/plan-text-fit.md`, "against the painted caret").
//!
//! `Host::caret_rect` is what a window passes to `set_ime_cursor_area`, so
//! an input method opens its candidate window beside the caret. Its
//! reference here is **the caret's pixels**, found in the software
//! canvas a window paints with, never the same `caret_rect` asked twice:
//! for a `TextInput` empty, mid-text and filled past its width (the
//! scrolled case), a `TextArea` with its caret on line 3, and a
//! `NumberInput` (which shares its field's index and must forward), at
//! scale 1.0 and 2.0. The quantity is the distance between the reported
//! rect's centre column and the painted caret column's centre, printed
//! beside its bound of 1 physical px.
//!
//! **The caret column is the column with the longest unbroken dark run.**
//! The caret is a one-logical-pixel rect as tall as the shaped line; no
//! glyph in these strings reaches from the line's top to its bottom (no
//! brackets, bars or descender-and-ascender pairs), a `TextArea`'s
//! stacked lines leave a gap between their letters, and the field's
//! border lies outside the padded box (plus the caret's pixel into the
//! right padding) the search is held to.
//!
//! Setup, asserted per case: the press landed on the field (it holds the
//! focus) and a caret column was found, at least 70% of a line tall.
//!
//! Injections (`docs/results-text.md` M80/T80.5 records them): the
//! `TextInput` leaves its scroll offset out (the scrolled case prints the
//! offset); `Host` returns the logical rect (scale 2.0 prints half).
#![cfg(feature = "truetype")]

use fulgurite::event::Key;
use fulgurite::style::{Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Color, Event, StaticTree};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::{FontMetrics, FontStack};
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;

#[path = "fonts/pinned.rs"]
mod pinned;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Ui {
    #[widget_id("input")]
    input: TextInput<String>,
    #[widget_id("area")]
    area: TextArea<String>,
    #[widget_id("number")]
    number: NumberInput,
}

#[derive(DeriveApp, Default)]
struct Page {
    #[tree]
    ui: Ui,
}

/// Logical window; physical is this times the scale.
const LW: f32 = 240.0;
const LH: f32 = 220.0;

/// Where the press goes, as a fraction of the field's box, and what is
/// pressed after it.
struct Case {
    name: &'static str,
    wid: &'static str,
    text: &'static str,
    /// Press at this fraction of the box (x, y).
    at: (f32, f32),
    /// A key after the press (End, for the scrolled case).
    then: Option<Key>,
}

const CASES: [Case; 5] = [
    Case { name: "TextInput empty", wid: "input", text: "", at: (0.5, 0.5), then: None },
    Case { name: "TextInput mid-text", wid: "input", text: "Hello world now", at: (0.25, 0.5), then: None },
    Case {
        name: "TextInput filled past its width (scrolled)",
        wid: "input",
        text: "the man who sat on the wall knew most of the rumours about town",
        at: (0.3, 0.5),
        then: Some(Key::End),
    },
    // The third line's middle: rows are a line apart from the padding.
    Case { name: "TextArea line 3", wid: "area", text: "one\ntwo\nthree more", at: (0.2, 0.0), then: None },
    Case { name: "NumberInput", wid: "number", text: "", at: (0.5, 0.5), then: None },
];

struct Measured {
    reported: [f32; 4],
    painted: f32,
    rows: u32,
    line: f32,
}

fn stack() -> FontStack {
    FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses")
}

fn index(wid: &str) -> usize {
    fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, wid).expect("a field id")
}

fn measure(font: &FontStack, c: &Case, scale: f32) -> Measured {
    let mut app = Page::default();
    app.ui.input = TextInput::new(c.text);
    app.ui.area = TextArea::new(if c.wid == "area" { c.text } else { "x" });
    app.ui.number = NumberInput::new(42.0, 0.0, 100.0, 1.0);
    let (pw, ph) = ((LW * scale) as u32, (LH * scale) as u32);
    let mut h: Host<Page> = Host::new(app, pw, ph);
    h.set_scale(scale);
    // FILL across, so an empty field has room for its caret: its own
    // width is its padding, and the clip would be zero wide.
    let ink = Patch::new()
        .font_size(16.0)
        .color(Color(0, 0, 0, 255))
        .background(Color(255, 255, 255, 255))
        .width(fulgurite::style::Length::FILL);
    h.set_stylesheet(&[
        Rule::new(Selector::Id("input"), ink),
        Rule::new(Selector::Id("area"), ink.height(fulgurite::style::Length::Fixed(80.0))),
        Rule::new(Selector::Id("number"), ink),
    ]);
    let mut m = FontMetrics::with_scale(font.clone(), scale);
    h.pump_with(&mut m);

    let i = index(c.wid);
    let s = h.states[i];
    let (x, mut y) = (s.pos.x + s.size.w * c.at.0, s.pos.y + s.size.h * c.at.1);
    if c.wid == "area" {
        // The middle of line 3, from the box's top padding.
        y = s.pos.y + s.pad.1 + 2.5 * 19.0;
    }
    h.click(x, y);
    h.pump_with(&mut m);
    if let Some(k) = c.then {
        h.post(Event::Key { key: k, down: true });
        h.post(Event::Key { key: k, down: false });
        h.pump_with(&mut m);
    }
    assert_eq!(h.input.focus.map(|f| f as usize), Some(i), "setup: {}: the press did not focus the field", c.name);
    if c.wid == "area" {
        assert_eq!(h.app.ui.area.caret_line(), 2, "setup: {}: the caret is not on line 3", c.name);
    }
    let reported = h.caret_rect().unwrap_or_else(|| panic!("{}: Host::caret_rect is None with the field focused", c.name));

    let mut canvas = Canvas::with_font(pw, ph, font.clone());
    canvas.clear(Color(255, 255, 255, 255));
    h.render(&mut canvas);

    // Held to the padded box across, plus the caret's one pixel into the
    // right padding where a full field's end caret is drawn, and the box
    // down, in physical px.
    let s = h.states[i];
    let k = scale;
    let (x0, x1) = (((s.pos.x + s.pad.0) * k).floor() as u32, ((s.pos.x + s.size.w - s.pad.2 + 1.0) * k).ceil() as u32);
    let (y0, y1) = (((s.pos.y + 1.0) * k).floor() as u32, ((s.pos.y + s.size.h - 1.0) * k).ceil() as u32);
    let dark = |x: u32, y: u32| {
        let p = ((y * pw + x) * 4) as usize;
        let px = &canvas.pixels[p..p + 3];
        (px[0] as u32 + px[1] as u32 + px[2] as u32) < 3 * 128
    };
    // Per column, the longest unbroken run of dark rows: a caret is one
    // run a line tall, where a `TextArea`'s lines stack a letter per line
    // in one column with gaps between them.
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
    // The caret's columns: the run of columns around the peak at least
    // 80% as tall (anti-aliasing spreads a fractional x over two), which a
    // glyph's stem beside the caret (about 63% of a line) is not.
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
    Measured { reported, painted: sum / w.max(1.0), rows: best, line: reported[3] - reported[1] }
}

#[test]
fn the_ime_area_is_the_painted_caret() {
    let font = stack();
    let mut worst = 0.0f32;
    let mut off = Vec::new();
    for scale in [1.0f32, 2.0] {
        for c in &CASES {
            let m = measure(&font, c, scale);
            let centre = (m.reported[0] + m.reported[2]) / 2.0;
            let d = (centre - m.painted).abs();
            println!(
                "{:<44} scale {scale}: caret_rect x {:.2}..{:.2} (centre {centre:.2}), painted caret column {:.2}, \
                 |d| {d:.2} physical px (bound 1.00); column {} px tall of a {:.1} px line",
                c.name, m.reported[0], m.reported[2], m.painted, m.rows, m.line
            );
            // Setup: a caret was painted and found -- most of a line tall.
            assert!(
                m.rows as f32 >= 0.7 * m.line,
                "setup: {} at {scale}: no caret column found ({} px tall against a {:.1} px line)",
                c.name,
                m.rows,
                m.line
            );
            worst = worst.max(d);
            if d > 1.0 {
                off.push(format!("{} at {scale}: {d:.2} px", c.name));
            }
        }
    }
    println!("worst |d| {worst:.2} physical px over {} cases (bound 1.00)", 2 * CASES.len());
    assert!(off.is_empty(), "the IME area is not the painted caret: {off:?}");
}
