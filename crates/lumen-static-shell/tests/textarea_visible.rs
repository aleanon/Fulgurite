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
//!
//! **And its load-independent twin, the allocator** (M45/T45.11): the same
//! render's heap allocations, counted by this binary's own global
//! allocator and bounded by twice the glyph bound — a rasterised glyph is
//! one coverage buffer, and a run a few more. Here and not in
//! `alloc_gate.rs`: that file asserts zero on every path, and its truetype
//! arms are M50/T50.2's.
#![cfg(feature = "truetype")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

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

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocs() -> usize {
    ALLOCS.with(Cell::get)
}

/// The counter is process-wide and tests run on threads: one frame at a
/// time.
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// One Left in a `bytes`-long area: (glyphs rasterised, bound,
/// allocations during the render).
fn frame(bytes: usize) -> (u64, u64, usize) {
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
    let a0 = allocs();
    for r in &regions {
        canvas.render_within(&host.list, *r);
    }
    let heap = allocs() - a0;
    let drawn = glyphs_rasterised() - before;

    let s = host.states[1];
    let inner_h = s.size.h - s.pad.1 - s.pad.3;
    let line_h = m.line_height(16.0);
    let visible = (inner_h / line_h).ceil() as u64 + 1;
    let longest = LINE.chars().count() as u64;
    let bound = visible * longest * regions.len() as u64;
    println!(
        "{} B area, one Left: {drawn} glyphs rasterised over {} damage regions (bound {bound} = {visible} lines x {longest} x {}); {heap} allocations (bound {})",
        bytes,
        regions.len(),
        regions.len(),
        2 * bound
    );
    (drawn, bound, heap)
}

#[test]
fn a_caret_move_in_a_long_area_rasterises_what_is_visible() {
    let (drawn, bound, heap) = frame(64 * 1024);
    assert!(drawn <= bound, "{drawn} glyphs rasterised for a caret move; the visible bound is {bound}");
    assert!(heap as u64 <= 2 * bound, "{heap} allocations for a caret move; the visible bound is {}", 2 * bound);
}

/// **The accept direction**: an area whose whole text is visible is
/// under the same bound. Five lines: every glyph of it may be drawn.
///
/// Not the plan's 4 KB: a 4 KB area is 74 lines, and it rasterised 3,330
/// glyphs against a bound of 330 at the commit that wrote this — it is a
/// long document too, and red for the same reason.
#[test]
fn a_caret_move_in_an_area_that_fits_is_under_the_bound() {
    let (drawn, bound, heap) = frame(5 * LINE.len());
    assert!(drawn <= bound, "{drawn} > {bound}");
    assert!(heap as u64 <= 2 * bound, "{heap} allocations > {}", 2 * bound);
}
