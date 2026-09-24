//! **The panic screen** — M36/T36.2 (`docs/plan-panic-recovery.md` §2.3).
//!
//! A display list, checked for what a person reading it needs: every line
//! inside the window (a long message wraps, it does not run off the
//! edge), readable contrast, and the location a person can open. Plus a
//! golden image at two sizes, with `truetype` off (see `golden`).

use lumen_static::cx::{BlockMetrics, TextMeasure};
use lumen_static::display::{Cmd, Color};
use lumen_static_shell::recover::{failure_frame, PanicReport, BODY, DIM, PAGE, TITLE};

fn report() -> PanicReport {
    PanicReport {
        message: "index out of bounds: the len is 4 but the index is 10000, which a handler \
                  passed to a function that trusts its caller about the table it walks"
            .into(),
        location: Some(("crates/lumen-static/src/style.rs".into(), 1147, 18)),
        app_frame: Some(("fixtures/panic-app/src/lib.rs".into(), 136)),
        note: None,
    }
}

fn texts(list: &lumen_static::display::DisplayList) -> Vec<(f32, f32, String, f32)> {
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

/// WCAG relative luminance and contrast ratio.
fn contrast(a: Color, b: Color) -> f32 {
    fn lum(c: Color) -> f32 {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
    }
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
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
    use lumen_static::Renderer;
    use lumen_static_shell::golden::{compare, Verdict};
    use lumen_static_shell::software::Canvas;
    for (w, h) in [(480u32, 320u32), (260, 420)] {
        let list = failure_frame(&report(), w, h, &mut BlockMetrics);
        let mut canvas = Canvas::new(w, h);
        canvas.clear(PAGE);
        canvas.render(&list, w, h);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
        match compare(&dir, &format!("panic_screen_{w}x{h}"), &canvas).unwrap() {
            Verdict::Differs { .. } => panic!("the panic screen at {w}x{h} differs from its golden"),
            v => println!("{w}x{h}: {v:?}"),
        }
    }
}
