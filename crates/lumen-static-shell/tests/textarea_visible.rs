//! **A caret move in a long `TextArea` draws what is visible** — M45/T45.0
//! item 14, for R8-F3 (`docs/plan-text-correctness.md` §1.6).
//!
//! `TextArea` painted its whole value as one text command. The software
//! backend culls per run and shapes and rasterises every glyph of a run
//! before clipping, so a caret move in a long document rasterised the
//! document — about twice per frame, since a caret move damages two small
//! regions.
//!
//! Counted, not timed: `text::glyphs_rasterised` over one damage-driven
//! render, bounded by visible lines × longest line × damage regions and
//! printed beside the bound. Alone in its binary, because the counter is
//! process-wide.
#![cfg(feature = "truetype")]

use lumen_static::prelude::*;
use lumen_static::{Color, Event, Key, TextMeasure};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{glyphs_rasterised, FontMetrics, FontStack};
use lumen_static_shell::Host;
use lumen_static_widgets::TextArea;

#[path = "fonts/pinned.rs"]
mod pinned;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("area")]
    area: TextArea,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const LINE: &str = "the quick brown fox jumps over the lazy dog 0123456789\n";

/// The counter is process-wide and tests run on threads: one frame at a
/// time.
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One Left in a `bytes`-long area: (glyphs rasterised, bound).
fn frame(bytes: usize) -> (u64, u64) {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let font = FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses");
    let body = LINE.repeat(bytes / LINE.len());
    let mut a = App::default();
    a.ui.area.set_str(&body);
    let mut m = FontMetrics::new(font.clone());
    let (w, h) = (640u32, 200u32);
    let mut host: Host<App> = Host::new(a, w, h);
    host.pump_with(&mut m);
    host.post(Event::Key { key: Key::Tab, down: true });
    host.pump_with(&mut m);
    assert_eq!(host.input.focus, Some(1), "setup: Tab did not focus the area");
    let mut canvas = Canvas::with_font(w, h, font);
    canvas.clear(Color(255, 255, 255, 255));
    host.render(&mut canvas);

    let caret = host.app.ui.area.caret();
    host.post(Event::Key { key: Key::Left, down: true });
    host.pump_with(&mut m);
    assert_ne!(host.app.ui.area.caret(), caret, "setup: Left did not move the caret");
    let regions: Vec<[f32; 4]> = host.damage().regions([0.0, 0.0, w as f32, h as f32]).collect();
    assert!(!regions.is_empty(), "setup: a caret move damaged nothing");
    let before = glyphs_rasterised();
    for r in &regions {
        canvas.render_within(&host.list, *r);
    }
    let drawn = glyphs_rasterised() - before;

    let s = host.states[1];
    let inner_h = s.size.h - s.pad.1 - s.pad.3;
    let line_h = m.line_height(16.0);
    let visible = (inner_h / line_h).ceil() as u64 + 1;
    let longest = LINE.chars().count() as u64;
    let bound = visible * longest * regions.len() as u64;
    println!(
        "{} B area, one Left: {drawn} glyphs rasterised over {} damage regions (bound {bound} = {visible} lines x {longest} x {})",
        bytes,
        regions.len(),
        regions.len()
    );
    (drawn, bound)
}

#[test]
#[ignore = "red until T45.11: R8-F3, a TextArea paints its whole value as one run"]
fn a_caret_move_in_a_long_area_rasterises_what_is_visible() {
    let (drawn, bound) = frame(64 * 1024);
    assert!(drawn <= bound, "{drawn} glyphs rasterised for a caret move; the visible bound is {bound}");
}

/// **The accept direction**: an area whose whole text is visible is
/// under the same bound. Five lines: every glyph of it may be drawn.
///
/// Not the plan's 4 KB: a 4 KB area is 74 lines, and it rasterised 3,330
/// glyphs against a bound of 330 at the commit that wrote this — it is a
/// long document too, and red for the same reason.
#[test]
fn a_caret_move_in_an_area_that_fits_is_under_the_bound() {
    let (drawn, bound) = frame(5 * LINE.len());
    assert!(drawn <= bound, "{drawn} > {bound}");
}
