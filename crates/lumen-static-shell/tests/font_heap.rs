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
//! the counter is process-wide; its tests take one lock, so nothing else
//! allocates beside the one measuring.
//!
//! **K81c — the face bytes a `Complex` stack keeps** (M81/T81.3,
//! `docs/plan-shaped-paint.md`), the second test: a stack in
//! `Shaping::Complex` keeps each face's bytes resident for the shaper,
//! read once per process; its resident heap after a frame, above the same
//! stack's under `Basic`, is bounded at the face's file size + 64 KiB. And
//! a `Basic` stack built from the file keeps none: within 64 KiB of one
//! built from bytes, which cannot.
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

/// One arm at a time: the counter is process-wide.
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn the_bold_face_is_parsed_on_its_first_bold_run() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
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

/// The live heap a stack and a canvas that drew one regular frame hold,
/// both kept: (resident bytes, the stack, the canvas).
#[cfg(feature = "shaping")]
fn resident(build: impl FnOnce() -> FontStack) -> (usize, FontStack, Canvas) {
    // Hebrew in it, so the run needs the shaper under Complex
    // (`needs_shaping`, K81a's response): plain Latin takes fontdue's path
    // and would load no bytes to measure.
    let base = LIVE.load(Relaxed);
    let stack = build();
    let mut c = Canvas::with_font(200, 40, stack.clone());
    let mut l = DisplayList::new();
    l.text_weighted([4.0, 4.0, 196.0, 36.0], "Regular \u{5e9}\u{5dc}\u{5d5}\u{5dd} 0123", 16.0, 400.0, Color(0, 0, 0, 255));
    c.render(&l, 200, 40);
    (LIVE.load(Relaxed) - base, stack, c)
}

/// **K81c** — a `Complex` stack's resident heap above a `Basic` one's is
/// at most the face's file size + 64 KiB; a `Basic` stack keeps no bytes.
/// Injection (T81.3's commit): keep the bytes under `Basic` too; the
/// `Basic` bound fires.
#[cfg(feature = "shaping")]
#[test]
fn a_complex_stack_keeps_one_copy_of_its_face_bytes() {
    use lumen_static_shell::text::{text_counters, Shaping};
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fonts/DejaVuSans.ttf");
    let file = std::fs::metadata(PATH).expect("the pinned face").len() as usize;
    let bytes = pinned::bytes();

    let (from_bytes, a, b) = resident(|| FontStack::from_bytes(bytes, None).expect("parses"));
    drop((a, b));
    let (basic, a, b) = resident(|| FontStack::from_files(PATH, None).expect("parses"));
    drop((a, b));
    let before = text_counters().complex;
    let (complex, stack, canvas) =
        resident(|| FontStack::from_files(PATH, None).expect("parses").with_shaping(Shaping::Complex));
    let shaped = text_counters().complex - before;
    // A second Complex stack over the same file reads nothing again.
    let (again, s2, c2) =
        resident(|| FontStack::from_files(PATH, None).expect("parses").with_shaping(Shaping::Complex));
    let k = 64 << 10;
    println!(
        "resident after one frame: Basic from bytes {from_bytes} B, Basic from the file {basic} B \
         (bound from bytes + 64 KiB = {}); Complex from the file {complex} B, {} B above Basic \
         (bound file {file} + 64 KiB = {}); a second Complex stack {again} B, {} B above Basic; \
         {shaped} runs through the shaper (setup > 0)",
        from_bytes + k,
        complex as isize - basic as isize,
        file + k,
        again as isize - basic as isize
    );
    assert!(shaped > 0, "setup: the Complex frame laid nothing out through the shaper");
    assert!(complex > basic + file / 2, "setup: the Complex stack kept no face bytes ({complex} B against {basic})");
    assert!(basic <= from_bytes + k, "a Basic stack built from the file kept {} B more than one from bytes: it keeps face bytes", basic - from_bytes);
    assert!(complex <= basic + file + k, "a Complex stack kept {} B above Basic, over the file's {file} + 64 KiB", complex - basic);
    assert!(again <= basic + k, "a second Complex stack over the same file kept {} B above Basic: the bytes were not interned", again.saturating_sub(basic));
    drop((stack, canvas, s2, c2));
}

/// Resident set size from `/proc/self/status`, in bytes; 0 elsewhere.
#[cfg(feature = "vector-text")]
fn rss() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<usize>().ok()))
        .map_or(0, |kb| kb * 1024)
}

/// **The face bytes `vector-text` keeps** — M82/T82.7
/// (`docs/plan-paint-vocabulary.md`, "Under the feature, `FontStack` keeps
/// each parsed face's bytes"). A stack with a bold face, after a bold run,
/// holds above its two parsed faces exactly one copy of each face's bytes:
/// at least the two files' size (the setup: they are kept), at most that +
/// 64 KiB. Printed with the RSS the same build moves by.
///
/// Injection (T82.7's commit): `Bold::new` keeps the bytes in its parse
/// buffer as well as in `kept`; the upper bound fires.
#[cfg(feature = "vector-text")]
#[test]
fn vector_text_keeps_one_copy_of_each_face() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let bytes = pinned::bytes();
    let base = LIVE.load(Relaxed);
    let parsed = [
        fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).unwrap(),
        fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).unwrap(),
    ];
    let faces = LIVE.load(Relaxed) - base;
    drop(parsed);

    let (base, rss0) = (LIVE.load(Relaxed), rss());
    let stack = FontStack::from_bytes(bytes, Some(bytes)).expect("parses");
    frame(&stack, 700.0);
    let held = LIVE.load(Relaxed) - base;
    let rss1 = rss();
    let kept = held as isize - faces as isize;
    let files = 2 * bytes.len();
    let k = 64 << 10;
    println!(
        "vector-text: a regular + bold stack holds {held} B after a bold run, its two parsed faces {faces} B; \
         kept {kept} B (bound files {files} B .. + 64 KiB = {}); RSS moved {} B",
        files + k,
        rss1 as isize - rss0 as isize
    );
    assert!(kept >= files as isize - k as isize, "setup: the stack kept {kept} B, not the faces' {files} B");
    assert!(kept <= (files + k) as isize, "the stack kept {kept} B above its faces, over one copy of each ({files} B) + 64 KiB");
    drop(stack);
}
