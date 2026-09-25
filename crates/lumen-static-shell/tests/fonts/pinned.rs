//! The pinned test face, for tests that shape text (M43/T43.11).
//!
//! Included with `#[path]` by the shell's, the text crate's and the
//! embedded fixture's tests, so one file says where the face is and how
//! it is checked. A missing face panics with `pinned face not found` --
//! it never returns early, which is the skip-as-a-pass this replaces.

/// The committed face's bytes. Prints `face: <file>, glyphs: N` and
/// asserts the glyph count is non-zero, so a truncated or wrong file is
/// named rather than shaped.
#[allow(dead_code)]
pub fn bytes() -> &'static [u8] {
    let path = format!("{}/{}/DejaVuSans.ttf", env!("CARGO_MANIFEST_DIR"), dir());
    let b = std::fs::read(&path).unwrap_or_else(|e| panic!("pinned face not found: {path}: {e}"));
    let n = glyph_count(&b);
    println!("face: DejaVuSans.ttf, glyphs: {n}");
    assert!(n > 0, "pinned face at {path} has no glyphs");
    Box::leak(b.into_boxed_slice())
}

/// Where the face lives relative to the including crate's manifest.
fn dir() -> &'static str {
    match env!("CARGO_PKG_NAME") {
        "lumen-static-shell" => "tests/fonts",
        "lumen-static-text" => "../lumen-static-shell/tests/fonts",
        _ => "../../crates/lumen-static-shell/tests/fonts",
    }
}

/// `maxp.numGlyphs`, read from the table directory by hand: the tests
/// that include this do not all have a font parser of their own.
fn glyph_count(b: &[u8]) -> u16 {
    let u16_at = |o: usize| b.get(o..o + 2).map(|s| u16::from_be_bytes([s[0], s[1]])).unwrap_or(0);
    let u32_at = |o: usize| b.get(o..o + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]])).unwrap_or(0);
    let tables = u16_at(4) as usize;
    for i in 0..tables {
        let rec = 12 + 16 * i;
        if b.get(rec..rec + 4) == Some(b"maxp") {
            return u16_at(u32_at(rec + 8) as usize + 4);
        }
    }
    0
}
