//! **The bold face is parsed when it is first used** — M50/T50.7, R8-F10.
//!
//! fontdue's parsed faces are most of an idle window's heap: Noto Sans
//! regular is 8.6 MB of it and bold 8.4 MB more (`docs/results-competitive.md`
//! §3). `FontStack::system` parsed both at start-up, whether or not the
//! application ever drew a bold run. It reads the bold bytes at start-up
//! now, so a missing file is still found there, and parses them on the
//! first run that needs them.
//!
//! Measured with a live-bytes counting allocator, on the pinned face
//! standing in for both weights so the numbers do not depend on the
//! machine: one face's heap first, then a stack with a "bold" face, a
//! regular-only frame on it (peak below one face plus the bold bytes it
//! holds and the frame's glyph cache), then one bold run (the face appears). Its own binary, because
//! the counter is process-wide; one test, so nothing else allocates
//! beside it.
#![cfg(feature = "truetype")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use lumen_static::display::{Color, DisplayList};
use lumen_static::Renderer;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;

#[path = "fonts/pinned.rs"]
mod pinned;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let now = LIVE.fetch_add(l.size(), Relaxed) + l.size();
        PEAK.fetch_max(now, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn frame(stack: &FontStack, weight: f32) {
    let mut c = Canvas::with_font(200, 40, stack.clone());
    let mut l = DisplayList::new();
    l.text_weighted([4.0, 4.0, 196.0, 36.0], "Regular text 0123", 16.0, weight, Color(0, 0, 0, 255));
    c.render(&l, 200, 40);
}

#[test]
fn the_bold_face_is_parsed_on_its_first_bold_run() {
    let bytes = pinned::bytes();
    // One face's heap: a stack with no bold face at all.
    let base = LIVE.load(Relaxed);
    let one = FontStack::from_bytes(bytes, None).expect("parses");
    let face = LIVE.load(Relaxed) - base;
    drop(one);

    // A stack with a bold face, and a regular-only frame on it.
    let base = LIVE.load(Relaxed);
    PEAK.store(base, Relaxed);
    let stack = FontStack::from_bytes(bytes, Some(bytes)).expect("parses");
    let built = LIVE.load(Relaxed) - base;
    frame(&stack, 400.0);
    let peak = PEAK.load(Relaxed) - base;
    // The bold bytes the stack holds until they are parsed, and the frame:
    // the canvas's glyph cache reserves 2 MiB on first use (M50/T50.2).
    let margin = bytes.len() + (4 << 20);
    println!(
        "one face {face} B; stack with bold built {built} B; peak through a regular frame {peak} B \
         (bound one face + the bold bytes + 4 MiB of frame = {})",
        face + margin
    );
    assert!(peak < face + margin, "a regular-only frame peaked at {peak} B, over one face's {face} B + {margin}: bold was parsed");

    // One bold run: now the face is parsed.
    let before = LIVE.load(Relaxed);
    frame(&stack, 700.0);
    let grew = LIVE.load(Relaxed) as isize - before as isize;
    println!("after one bold run: live heap grew {grew} B (bound >= 0.9 x one face = {})", face * 9 / 10);
    assert!(grew as usize >= face * 9 / 10, "a bold run did not load the bold face (heap grew {grew} B)");

    // Accept: a stack with no bold face never parses one, bold run or not.
    let plain = FontStack::from_bytes(bytes, None).expect("parses");
    let before = LIVE.load(Relaxed);
    frame(&plain, 700.0);
    let grew = LIVE.load(Relaxed) as isize - before as isize;
    println!("no bold face, one bold run: live heap grew {grew} B (bound < one face {face})");
    assert!((grew as usize) < face / 2, "a stack with no bold face grew {grew} B on a bold run");
    drop(stack);
}
