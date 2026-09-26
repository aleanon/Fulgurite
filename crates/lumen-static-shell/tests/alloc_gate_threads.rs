//! **Gate: a banded frame allocates nothing, on any thread** — M50/T50.0,
//! for R18-F4.
//!
//! `alloc_gate.rs` counts per thread, which is what makes it correct
//! under `cargo test`'s parallel threads — and what makes it blind to the
//! threaded rasteriser: a band worker's allocations land on the worker's
//! counter, which no assertion reads. Its fixtures are 320×200, which
//! never bands anyway. So this is its own binary, with **one** test and a
//! process-wide counter that is armed only around the frames it bounds:
//! nothing else runs in the process while it is armed, so every
//! allocation it sees belongs to the frame, on whichever thread made it.
//!
//! The setup is asserted before the quantity (`docs/plan-raster-
//! allocation.md` §0 rule 3): the canvas must band, or a zero here would
//! be a zero for a single-band frame, which `alloc_gate.rs` already
//! proves.
//!
//! `required-features = ["threads"]`; `just threads` runs it with
//! `truetype` as well, so the bands draw real glyphs.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

use lumen_static::display::{Color, DisplayList};
use lumen_static::Renderer;
use lumen_static_shell::software::Canvas;

/// Counting only while armed, so the harness's own allocations before
/// and after the counted frames are not the frame's.
static ARMED: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static FREES: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// The frame thread's share, printed beside the process-wide count so
    /// the difference — the workers' — is visible.
    static MINE: Cell<usize> = const { Cell::new(0) };
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.load(Relaxed) {
            ALLOCS.fetch_add(1, Relaxed);
            let _ = MINE.try_with(|c| c.set(c.get() + 1));
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if ARMED.load(Relaxed) {
            FREES.fetch_add(1, Relaxed);
        }
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;

/// A 1080p list: rows of cards with a label each, so every band draws
/// rects and text.
fn scene(w: u32, h: u32) -> DisplayList {
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, w as f32, h as f32], Color(245, 245, 250, 255));
    let mut y = 4.0;
    let mut k = 0;
    while y + 24.0 < h as f32 {
        l.rounded_rect([8.0, y, w as f32 - 8.0, y + 22.0], Color(220, 225, 240, 255), 4.0);
        l.text([16.0, y + 2.0, w as f32 - 16.0, y + 20.0], &format!("row {k}: the quick brown fox"), 14.0, Color(20, 20, 30, 255));
        y += 26.0;
        k += 1;
    }
    l
}

fn canvas(w: u32, h: u32) -> Canvas {
    #[cfg(feature = "truetype")]
    {
        #[path = "fonts/pinned.rs"]
        mod pinned;
        let f = lumen_static_shell::text::FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses");
        Canvas::with_font(w, h, f)
    }
    #[cfg(not(feature = "truetype"))]
    Canvas::new(w, h)
}

#[test]
#[ignore = "red until T50.4: R18-F4, every banded frame spawns its workers"]
fn a_banded_frame_allocates_nothing_on_any_thread() {
    let (w, h) = (1920u32, 1080u32);
    let list = scene(w, h);
    let mut c = canvas(w, h);
    let bands = c.bands_for_list(&list, None);
    assert!(bands >= 2, "setup: a {w}x{h} full redraw bands into {bands}, so this would measure one thread");

    for _ in 0..5 {
        c.render(&list, w, h);
    }
    const N: usize = 20;
    let mine0 = MINE.with(Cell::get);
    ARMED.store(true, Relaxed);
    for _ in 0..N {
        c.render(&list, w, h);
    }
    ARMED.store(false, Relaxed);
    let (a, f) = (ALLOCS.load(Relaxed), FREES.load(Relaxed));
    let mine = MINE.with(Cell::get) - mine0;
    let bound = 0usize;
    println!(
        "bands={bands}, allocs/frame process-wide {:.2} (bound {bound}), frees/frame {:.2}, frame-thread {:.2}; {N} frames of {w}x{h}",
        a as f64 / N as f64,
        f as f64 / N as f64,
        mine as f64 / N as f64
    );
    assert_eq!((a, f), (bound * N, bound * N), "a banded frame touched the allocator: {a} allocs, {f} frees over {N} frames");
}
