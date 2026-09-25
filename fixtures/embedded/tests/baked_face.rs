//! **Gate: the baked face matches a live render** —
//! `docs/plan-embedded.md` T13.7.
//!
//! `fixtures/embedded/src/face.rs` is generated, and generated data
//! rots silently: a regeneration at the wrong size, a hand-patched
//! advance, or a fontdue version bump all produce a face that still
//! *compiles* and draws subtly wrong glyphs. So every byte is checked
//! against a fresh `fontdue` rasterisation of the same TTF at the same
//! size — which is the one comparison a bitmap face can fail invisibly.
//!
//! The source font is the committed test face (M43/T43.11) -- the same
//! file `just bake-font` reads by default from the system -- so this runs
//! on every machine. It used to read the system copy and skip, passing,
//! where it was absent.

use embedded::face::FACE;

/// The font and size `face.rs` was generated from. If you regenerate at
/// a different size, change this too — a mismatch here is the failure
/// this test exists to catch.
const PX: f32 = 16.0;

#[path = "../../../crates/lumen-static-shell/tests/fonts/pinned.rs"]
mod pinned;

fn font() -> fontdue::Font {
    fontdue::Font::from_bytes(pinned::bytes(), fontdue::FontSettings::default()).expect("the pinned face parses")
}

#[test]
fn every_baked_glyph_matches_a_live_render() {
    let font = font();

    assert_eq!(FACE.px, PX, "the face was baked at a different size than this test assumes");
    let lm = font.horizontal_line_metrics(PX).expect("line metrics");
    assert_eq!(FACE.line_height, lm.new_line_size, "line height");
    assert_eq!(FACE.ascent, lm.ascent, "ascent");

    let mut bytes = 0usize;
    for g in FACE.glyphs {
        let (m, bitmap) = font.rasterize(g.ch, PX);
        assert_eq!(g.advance, m.advance_width, "advance of {:?}", g.ch);
        assert_eq!((g.w as usize, g.h as usize), (m.width, m.height), "extent of {:?}", g.ch);
        assert_eq!(g.left, m.xmin, "left bearing of {:?}", g.ch);
        assert_eq!(g.top, m.ymin + m.height as i32, "top edge of {:?}", g.ch);
        assert_eq!(FACE.coverage(g), &bitmap[..], "coverage of {:?}", g.ch);
        bytes += bitmap.len();
    }
    assert_eq!(bytes, FACE.cov.len(), "the blob has no bytes no glyph refers to");
    println!("baked_face: {} glyphs, {bytes} bytes, all matching", FACE.glyphs.len());
}

#[test]
fn the_table_is_sorted_so_lookup_can_binary_search() {
    assert!(FACE.glyphs.windows(2).all(|w| w[0].ch < w[1].ch), "GLYPHS must be sorted by ch");
    for g in FACE.glyphs {
        assert_eq!(FACE.glyph(g.ch).map(|f| f.ch), Some(g.ch));
    }
    assert!(FACE.glyph('\u{4e2d}').is_none(), "a glyph the face lacks is None, not a box");
}
