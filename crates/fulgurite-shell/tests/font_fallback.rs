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
//! `FULGURITE_REQUIRE_FONTS=1`, which `just fonts` sets.
#![cfg(feature = "truetype")]

use fulgurite::display::{Color, DisplayList, Renderer};
use fulgurite::TextMeasure;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::{FontMetrics, FontStack};

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
    assert!(std::env::var("FULGURITE_REQUIRE_FONTS").as_deref() != Ok("1"), "FULGURITE_REQUIRE_FONTS=1 but {why}");
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
fn cjk_and_hebrew_are_drawn_from_a_face_that_has_them() {
    let Some(font) = system() else { return };
    let (_, q, _) = paint(&font, "?");
    let q = q[0];
    let mut wrong = 0;
    for (text, tofu) in [("\u{4f60}\u{597d}", "??"), ("\u{5e9}\u{5dc}\u{5d5}\u{5dd}", "????")] {
        let (h, ids, w) = paint(&font, text);
        let (ht, _, wt) = paint(&font, tofu);
        let substituted = ids.iter().filter(|&&g| g == q).count();
        let faces: Vec<Option<usize>> = text.chars().map(|c| font.face_of(c, 400.0)).collect();
        println!("{text:?}: face per character {faces:?} (0 primary, 1 + k fallback k, None substituted)");
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

/// **Latin is drawn from the primary face, pixel for pixel as before** —
/// T45.3's accept direction and its kill rule's first half. The hash of
/// T45.0 item 1's twelve strings, painted through the system stack, is
/// printed so a commit can compare it across the change.
#[test]
fn latin_is_unchanged_by_the_fallback_faces() {
    let Some(font) = FontStack::system() else {
        println!("fonts: no system face -- latin hash skipped");
        return;
    };
    let strings = ["Taj", "Tj", "fj", "Settings", "jejunj", "Hij", "POSITIVE", "Wolf", "gypsy", "quay j", "Taj Mahal", "W"];
    let mut all = 0xcbf29ce484222325u64;
    for s in strings {
        for px in [11.0f32, 16.0, 24.0] {
            let (h, _, _) = paint_at(&font, s, px);
            all = (all ^ h).wrapping_mul(0x100000001b3);
        }
    }
    println!("latin corpus hash through the system stack: {all:#018x}");
}

fn paint_at(font: &FontStack, text: &str, px: f32) -> (u64, Vec<u16>, f32) {
    let w = FontMetrics::new(font.clone()).measure(text, px, f32::INFINITY).w;
    let mut list = DisplayList::new();
    list.text_weighted([4.0, 4.0, 4.0 + w, 44.0], text, px, 400.0, Color(0, 0, 0, 255));
    let mut c = Canvas::with_font(240, 48, font.clone());
    c.clear(Color(255, 255, 255, 255));
    c.render(&list, 240, 48);
    let hash = c.pixels.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    (hash, Vec::new(), w)
}

#[derive(Default)]
struct Clusters(Vec<fulgurite::cx::Cluster>);

impl fulgurite::cx::GlyphSink for Clusters {
    fn cluster(&mut self, c: fulgurite::cx::Cluster) {
        self.0.push(c);
    }
}

/// **A run drawn from several faces reports its caret positions in the
/// caller's string.** fontdue counts a glyph's byte offset from the start
/// of the segment it was appended in, and a run split by face is several
/// segments: a caret after `你` must land at byte 5 of `ab你好cd`, not at
/// byte 3 of `你好`.
#[test]
fn a_run_split_across_faces_reports_the_callers_offsets() {
    let Some(font) = system() else { return };
    let text = "ab\u{4f60}\u{597d}cd";
    let mut c = Clusters::default();
    let mut m = FontMetrics::new(font);
    m.shape(text, 16.0, 400.0, f32::INFINITY, &mut c);
    let bytes: Vec<u32> = c.0.iter().map(|c| c.byte).collect();
    let xs: Vec<f32> = c.0.iter().map(|c| c.x).collect();
    let want: Vec<u32> = text.char_indices().map(|(i, _)| i as u32).chain([text.len() as u32]).collect();
    println!("clusters {bytes:?} (want {want:?}), x {xs:?}");
    assert_eq!(bytes, want, "a cluster's byte is not the caller's offset");
    assert!(xs.windows(2).all(|w| w[1] > w[0]), "x does not rise across the faces: {xs:?}");
}
