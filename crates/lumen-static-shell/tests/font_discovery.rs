//! `FontStack::system` finds a face on this machine (M43/T43.11).
//!
//! The one test that is ABOUT system discovery, and so the one that still
//! needs a system face: every test that merely needs *a* face shapes the
//! committed one (`tests/fonts/`). Without a system face it skips -- a
//! plain `cargo test` is a developer's run, not a gate -- unless
//! LUMEN_REQUIRE_FONTS=1, which `just fonts` (and so `just dev-shape`)
//! sets: there, a missing face fails by name instead of passing silently.
#![cfg(feature = "truetype")]

use lumen_static::TextMeasure;
use lumen_static_shell::text::{FontMetrics, FontStack};

#[test]
fn a_system_face_is_found_and_measures() {
    let Some(stack) = FontStack::system() else {
        assert!(
            std::env::var("LUMEN_REQUIRE_FONTS").as_deref() != Ok("1"),
            "LUMEN_REQUIRE_FONTS=1 but no system face: FontStack::system() found none of its \
             candidates (set LUMEN_ALLOW_SKIP=1 on the recipe to skip deliberately)"
        );
        println!("fonts: no system face -- discovery skipped (not a gate run)");
        return;
    };
    let w = FontMetrics::new(stack).measure("Hamburgefonstiv", 16.0, f32::INFINITY).w;
    println!("fonts: system face found, 'Hamburgefonstiv' at 16 px is {w:.1} px wide");
    assert!(w > 0.0, "the system face measured nothing");
}
