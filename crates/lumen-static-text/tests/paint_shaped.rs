//! **What T15.8 claimed: an Arabic string measures *and paints*
//! right-to-left** — M45/T45.0 item 2, for R10-F2
//! (`docs/plan-text-correctness.md` §1.1, §2.1).
//!
//! `ComplexText` shapes, orders and reports clusters. The painter then
//! draws the original string with fontdue: cmap glyphs in logical order.
//! So Arabic is painted unjoined and left to right. This asserts the
//! claim as T15.8 made it — the painted glyph ids are the shaper's, in
//! visual order, and the ink stays in the shaper's box — and prints
//! whether what was painted is simply the isolated forms.
//!
//! **It is expected to fail** (M45/T45.4, `docs/plan-text-correctness.md`
//! §2.1 B): the claim is T15.8's, T15.8 is now "measurement only", and
//! the second test states the limitation as it stands.
//!
//! `hebrew_paints_right_to_left` (M81/T81.0) pins the same gap for a
//! script with no joining, where only the order is wrong.

use lumen_static::display::{Color, DisplayList, Renderer};
use lumen_static::TextMeasure;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;
use lumen_static_text::ComplexText;

#[path = "../../lumen-static-shell/tests/fonts/pinned.rs"]
mod pinned;

const WORD: &str = "\u{645}\u{631}\u{62d}\u{628}\u{627}";
const PX: f32 = 32.0;

/// **The pin** — M45/T45.4. Expected to fail: T15.8 is "measurement
/// only" (`docs/plan-capabilities.md`), and this is the claim it used to
/// make. The day a painter draws the shaper's glyphs, this stops
/// panicking, the harness reports it red, and T15.8's status, this
/// crate's rustdoc and the book's table have to move with it.
#[test]
#[should_panic(expected = "the painter drew other glyphs than the shaper produced")]
fn arabic_paints_the_shapers_glyphs_right_to_left_inside_its_box() {
    let bytes = pinned::bytes();
    let mut shaper = ComplexText::new(bytes, PX).expect("the pinned face parses");
    let face = rustybuzz::Face::from_slice(bytes, 0).expect("the pinned face parses");
    let boxed = shaper.measure(WORD, PX, f32::INFINITY);
    let shaped: Vec<u16> = shaper.shaped_glyphs(WORD, PX).into_iter().map(|(g, _)| g).collect();
    let isolated: Vec<u16> = WORD.chars().map(|c| face.glyph_index(c).map_or(0, |g| g.0)).collect();
    assert!(isolated.iter().all(|&g| g != 0), "setup: the pinned face lacks an Arabic letter: {isolated:?}");

    // The window's painter, at the shaper's box.
    let font = FontStack::from_bytes(bytes, None).expect("the pinned face parses");
    let (mut placed, _) = font.place_run(WORD, PX, 400.0, boxed.w);
    placed.sort_by(|a, b| a.x.total_cmp(&b.x));
    let painted: Vec<u16> = placed.iter().map(|p| p.id.index()).collect();
    // The same run with no width limit, in the order it is painted: what
    // the painter draws when nothing wraps it.
    let (free, _) = font.place_run(WORD, PX, 400.0, f32::INFINITY);
    let free: Vec<u16> = free.iter().map(|p| p.id.index()).collect();

    let mut list = DisplayList::new();
    let r = [10.0, 10.0, 10.0 + boxed.w, 10.0 + boxed.h];
    list.text_weighted(r, WORD, PX, 400.0, Color(0, 0, 0, 255));
    let mut canvas = Canvas::with_font(300, 100, font);
    canvas.clear(Color(255, 255, 255, 255));
    canvas.render(&list, 300, 100);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..100u32 {
        for x in 0..300u32 {
            let i = ((y * 300 + x) * 4) as usize;
            if canvas.pixels[i..i + 3].iter().any(|&c| c != 255) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    let inside = x0 as f32 >= r[0] - 1.0 && x1 as f32 <= r[2] + 1.0 && y0 as f32 >= r[1] - 1.0 && y1 as f32 <= r[3] + 1.0;
    println!("shaper, visual order:   {shaped:?}");
    println!("painted, left to right: {painted:?}");
    println!("painted, unwrapped:     {free:?}");
    println!("isolated forms (cmap, logical order): {isolated:?} -> unwrapped painted == isolated: {}", free == isolated);
    println!("box {r:?}, ink {x0}..{x1} x {y0}..{y1}, inside (1 px): {inside}");
    assert_eq!(painted, shaped, "the painter drew other glyphs than the shaper produced");
    assert!(inside, "the painted ink left the shaper's box");
}

/// **The limitation, as it stands** — M45/T45.4. With no width limit, the
/// painter draws the word's letters as their isolated cmap glyphs, in
/// logical order. When shaped paint lands this goes red beside the pin.
#[test]
fn painted_arabic_is_its_isolated_letters_in_logical_order() {
    let bytes = pinned::bytes();
    let face = rustybuzz::Face::from_slice(bytes, 0).expect("the pinned face parses");
    let isolated: Vec<u16> = WORD.chars().map(|c| face.glyph_index(c).map_or(0, |g| g.0)).collect();
    let font = FontStack::from_bytes(bytes, None).expect("the pinned face parses");
    let (placed, _) = font.place_run(WORD, PX, 400.0, f32::INFINITY);
    let painted: Vec<u16> = placed.iter().map(|p| p.id.index()).collect();
    println!("painted {painted:?}, isolated (logical order) {isolated:?}");
    assert_eq!(painted, isolated, "the painter no longer draws the isolated letters: shaped paint has landed, so T15.8 and this crate's docs must say so");
}

/// **The Hebrew pin** — M81/T81.0 (`docs/plan-shaped-paint.md`). Expected
/// to fail: the painter, [`FontStack::place_run`], lays a run out in
/// logical order whatever its direction, so `שלום`'s first letter is
/// painted leftmost. When the painter draws right to left (T81.3, under
/// a stack in its Complex mode) this stops panicking, the harness reports it red,
/// and the pin moves into the shell's tests without its `should_panic`.
///
/// Its accept direction was shown in T81.0's commit: fed
/// `ComplexText::shaped_glyphs`' x for each painted glyph in place of
/// `place_run`'s, the assertion holds and the harness reports "did not
/// panic".
#[test]
#[should_panic(expected = "the painter drew Hebrew in logical order")]
fn hebrew_paints_right_to_left() {
    const HEBREW: &str = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}";
    let bytes = pinned::bytes();
    let face = rustybuzz::Face::from_slice(bytes, 0).expect("the pinned face parses");
    let cmap: Vec<u16> = HEBREW.chars().map(|c| face.glyph_index(c).map_or(0, |g| g.0)).collect();
    println!("setup: cmap ids per letter (logical order) {cmap:?}");
    assert!(cmap.iter().all(|&g| g != 0), "setup: the pinned face lacks a Hebrew letter: {cmap:?}");

    let font = FontStack::from_bytes(bytes, None).expect("the pinned face parses");
    let (placed, _) = font.place_run(HEBREW, PX, 400.0, f32::INFINITY);
    let ids: Vec<u16> = placed.iter().map(|p| p.id.index()).collect();
    let xs: Vec<f32> = placed.iter().map(|p| p.x).collect();
    for (id, x) in ids.iter().zip(&xs) {
        println!("painted (logical order): glyph {id} at x {x}");
    }
    assert_eq!(ids.len(), cmap.len(), "setup: the painter placed {} glyphs for {} letters", ids.len(), cmap.len());
    let falling = xs.windows(2).all(|w| w[1] < w[0]);
    assert!(falling, "the painter drew Hebrew in logical order: x {xs:?} for glyphs {ids:?}, not strictly decreasing");
}
