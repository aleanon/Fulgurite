//! **The panic screen** — M36/T36.2 (`docs/plan-panic-recovery.md` §2.3).
//!
//! A display list, checked for what a person reading it needs: every line
//! inside the window (a long message wraps, it does not run off the
//! edge), readable contrast, and the location a person can open. Plus a
//! golden image at two sizes, with `truetype` off (see `golden`).

use fulgurite::cx::{BlockMetrics, TextMeasure};
use fulgurite::display::{Cmd, Color};
use fulgurite_shell::recover::{failure_frame, PanicReport, BODY, DIM, PAGE, TITLE};

fn report() -> PanicReport {
    PanicReport {
        message: "index out of bounds: the len is 4 but the index is 10000, which a handler \
                  passed to a function that trusts its caller about the table it walks"
            .into(),
        location: Some(("crates/fulgurite/src/style.rs".into(), 1147, 18)),
        app_frame: Some(("fixtures/panic-app/src/lib.rs".into(), 136)),
        note: None,
    }
}

fn texts(list: &fulgurite::display::DisplayList) -> Vec<(f32, f32, String, f32)> {
    list.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { rect, text, size, .. } => Some((rect[0], rect[1], list.str_of(*text).to_string(), *size)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_line_fits_the_window_at_two_sizes() {
    for (w, h) in [(480u32, 320u32), (260, 420)] {
        let mut m = BlockMetrics;
        let list = failure_frame(&report(), w, h, &mut m);
        let mut rightmost: f32 = 0.0;
        for (x, _, s, px) in texts(&list) {
            rightmost = rightmost.max(x + m.measure(&s, px, f32::INFINITY).w);
        }
        println!("{w}x{h}: {} text lines, rightmost x {rightmost:.1} of width {w}", texts(&list).len());
        assert!(rightmost <= w as f32, "{w}x{h}: a line runs {rightmost:.1} px, past the window's {w}");
    }
}

#[test]
fn the_location_and_the_application_frame_are_on_the_screen() {
    let mut m = BlockMetrics;
    let list = failure_frame(&report(), 480, 320, &mut m);
    let all: Vec<String> = texts(&list).into_iter().map(|t| t.2).collect();
    println!("{all:#?}");
    assert!(all.iter().any(|t| t.starts_with("The application")), "no title");
    assert!(all.iter().any(|t| t.contains("style.rs:1147:18")), "no location line");
    assert!(all.iter().any(|t| t.contains("fixtures/panic-app/src/lib.rs:136")), "no application frame");
    assert!(all.iter().any(|t| t.contains("Press R to retry")), "no hint");
}

/// WCAG contrast — the one implementation, `fulgurite_shell::contrast`
/// (M54/T54.5 moved this file's own copy there).
fn contrast(a: Color, b: Color) -> f32 {
    fulgurite_shell::contrast::ratio(a, b)
}

#[test]
fn every_text_colour_reaches_wcag_aa() {
    for (name, c) in [("title", TITLE), ("body", BODY), ("hint", DIM)] {
        let r = contrast(c, PAGE);
        println!("{name}: {r:.2}:1 (AA needs 4.5)");
        assert!(r >= 4.5, "{name} is {r:.2}:1 against the page");
    }
}

#[cfg(not(feature = "truetype"))]
#[test]
fn the_screen_matches_its_golden_image() {
    use fulgurite::Renderer;
    use fulgurite_shell::golden::{compare, Verdict};
    use fulgurite_shell::software::Canvas;
    for (w, h) in [(480u32, 320u32), (260, 420)] {
        let list = failure_frame(&report(), w, h, &mut BlockMetrics);
        let mut canvas = Canvas::new(w, h);
        canvas.clear(PAGE);
        canvas.render(&list, w, h);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
        // `Match` passes. `Recorded` does NOT: until M43/T43.9 every
        // verdict but `Differs` printed and passed, so a deleted golden was
        // re-recorded and the test went green (I-NEW-4), against
        // `Verdict::is_match`'s own rule. `Updated` happens only under
        // FULGURITE_UPDATE_GOLDENS=1, which is a deliberate re-record.
        match compare(&dir, &format!("panic_screen_{w}x{h}"), &canvas).unwrap() {
            Verdict::Match => println!("{w}x{h}: Match"),
            Verdict::Differs { .. } => panic!("the panic screen at {w}x{h} differs from its golden"),
            Verdict::Recorded(p) => panic!(
                "a first run is not a pass: the panic screen at {w}x{h} had no golden; recorded {} \
                 -- review it, commit it and re-run",
                p.display()
            ),
            v @ Verdict::Updated { .. } => {
                assert_eq!(
                    std::env::var("FULGURITE_UPDATE_GOLDENS").as_deref(),
                    Ok("1"),
                    "a golden was re-recorded without FULGURITE_UPDATE_GOLDENS=1"
                );
                println!("{w}x{h}: {v:?}");
            }
        }
    }
}
