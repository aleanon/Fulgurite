//! **What is painted is what the shaper produced** — M81/T81.3
//! (`docs/plan-shaped-paint.md`), the pins M45/T45.4 and M81/T81.0 wrote
//! in `fulgurite-text/tests/paint_shaped.rs`, moved here without their
//! `should_panic` now that a stack in `Shaping::Complex` paints shaped.
//!
//! Each pin reads `FontStack::place_run`, which *is* the painter's
//! placement — both backends place glyphs through `place_run_into`, which
//! `place_run` wraps — so it reads what is painted (the review's G2, T7),
//! and the Arabic pin renders through a `Canvas` as well to hold the ink
//! to the measured box.
//!
//! - The Arabic pin: on a `Complex` stack the painted glyph ids, left to
//!   right, are the shaper's in visual order (`ComplexText::shaped_glyphs`),
//!   and the ink stays inside the box the same stack measured.
//! - The Hebrew pin: `שלום`'s painted x falls strictly in logical order.
//! - The `Basic` pin: on a `Basic` stack the word is still painted as its
//!   isolated cmap letters in logical order — what every application that
//!   does not ask for shaping keeps.
//!
//! Injection (T81.3's commit): `place_run_into` ignores the stack's mode;
//! both `Complex` pins print the unshaped ids.
#![cfg(all(feature = "shaping", feature = "truetype"))]

use fulgurite::display::{Color, DisplayList, Renderer};
use fulgurite::TextMeasure;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::{FontMetrics, FontStack, Shaping};
use fulgurite_text::ComplexText;

#[path = "fonts/pinned.rs"]
mod pinned;

const WORD: &str = "\u{645}\u{631}\u{62d}\u{628}\u{627}";
const HEBREW: &str = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}";
const PX: f32 = 32.0;

fn complex() -> FontStack {
    FontStack::from_static(pinned::bytes(), None).expect("the pinned face parses").with_shaping(Shaping::Complex)
}

/// Each letter's cmap glyph in the pinned face, in logical order, read
/// from fontdue directly rather than from any stack under test.
fn cmap(text: &str) -> Vec<u16> {
    let face = fontdue::Font::from_bytes(pinned::bytes(), fontdue::FontSettings::default()).expect("parses");
    text.chars().map(|c| face.lookup_glyph_index(c)).collect()
}

/// **The Arabic pin** — painted ids equal the shaper's, ink inside the
/// box. Was `should_panic` (M45/T45.4): the painter drew the string's
/// cmap glyphs, unjoined and left to right.
#[test]
fn arabic_paints_the_shapers_glyphs_right_to_left_inside_its_box() {
    let bytes = pinned::bytes();
    let shaped: Vec<u16> =
        ComplexText::new(bytes, PX).expect("parses").shaped_glyphs(WORD, PX).into_iter().map(|(g, _)| g).collect();
    let isolated = cmap(WORD);
    assert!(isolated.iter().all(|&g| g != 0), "setup: the pinned face lacks an Arabic letter: {isolated:?}");

    let font = complex();
    let boxed = FontMetrics::new(font.clone()).measure(WORD, PX, f32::INFINITY);
    let (mut placed, _) = font.place_run(WORD, PX, 400.0, boxed.w);
    placed.sort_by(|a, b| a.x.total_cmp(&b.x));
    let painted: Vec<u16> = placed.iter().map(|p| p.id.index()).collect();

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
    println!("isolated forms (cmap, logical order): {isolated:?}");
    println!("box {r:?}, ink {x0}..{x1} x {y0}..{y1}, inside (1 px): {inside}");
    assert!(x1 > 0, "setup: the canvas painted no ink");
    assert_eq!(painted, shaped, "the painter drew other glyphs than the shaper produced");
    assert!(inside, "the painted ink left the measured box");
}

/// **The Hebrew pin** — M81/T81.0's, flipped: `שלום`'s glyphs, taken in
/// logical order, are painted at strictly falling x. Each painted glyph is
/// matched to its letter by id (the four ids are distinct).
#[test]
fn hebrew_paints_right_to_left() {
    let ids_logical = cmap(HEBREW);
    println!("setup: cmap ids per letter (logical order) {ids_logical:?}");
    assert!(ids_logical.iter().all(|&g| g != 0), "setup: the pinned face lacks a Hebrew letter: {ids_logical:?}");

    let (placed, _) = complex().place_run(HEBREW, PX, 400.0, f32::INFINITY);
    for p in &placed {
        println!("painted: glyph {} at x {}", p.id.index(), p.x);
    }
    assert_eq!(placed.len(), ids_logical.len(), "setup: the painter placed {} glyphs for {} letters", placed.len(), ids_logical.len());
    let xs: Vec<f32> = ids_logical
        .iter()
        .map(|id| placed.iter().find(|p| p.id.index() == *id).map(|p| p.x).unwrap_or(f32::NAN))
        .collect();
    println!("x per letter in logical order: {xs:?} (bound: strictly falling)");
    let falling = xs.windows(2).all(|w| w[1] < w[0]);
    assert!(falling, "the painter drew Hebrew in logical order: x {xs:?} for glyphs {ids_logical:?}, not strictly decreasing");
}

/// **The `Basic` pin** — M45/T45.4's limitation, kept as what a `Basic`
/// stack promises: with no width limit the word is painted as its letters'
/// isolated cmap glyphs, in logical order.
#[test]
fn painted_arabic_is_its_isolated_letters_in_logical_order() {
    let isolated = cmap(WORD);
    let font = FontStack::from_static(pinned::bytes(), None).expect("parses");
    assert_eq!(font.shaping(), Shaping::Basic, "setup: a stack is Basic unless asked");
    let (placed, _) = font.place_run(WORD, PX, 400.0, f32::INFINITY);
    let painted: Vec<u16> = placed.iter().map(|p| p.id.index()).collect();
    println!("Basic: painted {painted:?}, isolated (logical order) {isolated:?}");
    assert_eq!(painted, isolated, "a Basic stack no longer draws the isolated letters in logical order");
}
