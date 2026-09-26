//! **A character the primary face lacks is drawn by a face that has it**
//! — M45/T45.0 item 6, for R10-F12 (`docs/plan-text-correctness.md` §1.2,
//! §2.4).
//!
//! `FontStack::system` took the first candidate pair that loaded, and
//! `prepare` substituted `?` for every character that face lacked — so
//! CJK, Hebrew and Arabic rendered as `?` even where installed faces
//! cover them, and an IME user committing `你好` saw `??`.
//!
//! Needs **system** faces (§0 rule 3): the fallback faces are the
//! machine's, not the repository's. It prints its skip reason when none
//! of the faces below is installed, and fails instead under
//! `LUMEN_REQUIRE_FONTS=1`, which `just fonts` sets.
#![cfg(feature = "truetype")]

use lumen_static::display::{Color, DisplayList, Renderer};
use lumen_static::TextMeasure;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::{FontMetrics, FontStack};

/// Faces that cover each script, any one of which makes the case
/// meaningful on this machine.
const CJK: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
];
const HEBREW: &[&str] = &[
    "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

fn installed(list: &[&str]) -> bool {
    list.iter().any(|p| std::path::Path::new(p).exists())
}

/// The system stack, or the skip (a failure under the requirement).
fn system() -> Option<FontStack> {
    let why = match FontStack::system() {
        None => "no system face at all",
        Some(_) if !installed(CJK) || !installed(HEBREW) => "no CJK or no Hebrew face installed",
        Some(s) => return Some(s),
    };
    assert!(std::env::var("LUMEN_REQUIRE_FONTS").as_deref() != Ok("1"), "LUMEN_REQUIRE_FONTS=1 but {why}");
    println!("fonts: fallback skipped -- {why}");
    None
}

/// Canvas pixels of `text` at 24 px, and the glyph ids the painter used.
fn paint(font: &FontStack, text: &str) -> (u64, Vec<u16>, f32) {
    let w = FontMetrics::new(font.clone()).measure(text, 24.0, f32::INFINITY).w;
    let mut list = DisplayList::new();
    list.text_weighted([4.0, 4.0, 4.0 + w, 40.0], text, 24.0, 400.0, Color(0, 0, 0, 255));
    let mut c = Canvas::with_font(200, 48, font.clone());
    c.clear(Color(255, 255, 255, 255));
    c.render(&list, 200, 48);
    let hash = c.pixels.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    let (placed, _) = font.place_run(text, 24.0, 400.0, f32::INFINITY);
    (hash, placed.iter().map(|p| p.id.index()).collect(), w)
}

#[test]
#[ignore = "red until T45.3: R10-F12, the stack is one face and substitutes '?'"]
fn cjk_and_hebrew_are_drawn_from_a_face_that_has_them() {
    let Some(font) = system() else { return };
    let (_, q, _) = paint(&font, "?");
    let q = q[0];
    let mut wrong = 0;
    for (text, tofu) in [("\u{4f60}\u{597d}", "??"), ("\u{5e9}\u{5dc}\u{5d5}\u{5dd}", "????")] {
        let (h, ids, w) = paint(&font, text);
        let (ht, _, wt) = paint(&font, tofu);
        let substituted = ids.iter().filter(|&&g| g == q).count();
        let ok = h != ht && w != wt && substituted == 0;
        println!(
            "{text:?}: glyphs {ids:?} ('?' is {q}, {substituted} substituted), width {w} vs {tofu:?} {wt}, pixels differ {} {}",
            h != ht,
            if ok { "ok" } else { "WRONG" }
        );
        wrong += !ok as usize;
    }
    assert_eq!(wrong, 0, "{wrong} of 2 scripts drawn as '?' although an installed face covers them");
}
