//! **M12/T12.2's gate:** deliberately break a paint and check that the
//! diff image shows *where*, not merely that.
//!
//! "1,482 pixels differ" is a number. "They are all in the button's
//! top-left corner" is a diagnosis, and the difference between them is
//! the whole reason this task exists — the comparison already existed in
//! `damage_oracle.rs`, which on failure did `assert_eq!` on two 264 KB
//! `Vec<u8>`s.
//!
//! Runs with `truetype` **off**, which is the default here and which
//! `golden::compare` refuses to run without — see its module docs for
//! why a golden recorded under that feature records the recording
//! machine's font install.

#![cfg(not(feature = "truetype"))]

use lumen_static::display::{Color, DisplayList};
use lumen_static::Renderer;
use lumen_static_shell::golden::{compare, Verdict};
use lumen_static_shell::software::Canvas;

const W: u32 = 120;
const H: u32 = 80;

/// A frame with one rounded rect, whose radius the injection moves.
fn frame(radius: f32) -> Canvas {
    let mut c = Canvas::new(W, H);
    let mut list = DisplayList::new();
    list.rect([0.0, 0.0, W as f32, H as f32], Color(20, 24, 34, 255));
    list.rounded_rect([20.0, 16.0, 100.0, 64.0], Color(226, 232, 240, 255), radius);
    c.render(&list, W, H);
    c
}

fn dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

#[test]
fn a_matching_frame_matches_and_a_changed_one_shows_where() {
    let d = dir();

    // **Accepting direction first.** A golden harness that reported a
    // difference for every frame would pass the injection below and be
    // useless, which is the "gate that refuses everything" shape.
    let v = compare(&d, "rounded", &frame(12.0)).expect("io");
    match v {
        Verdict::Match => println!("  ok  unchanged frame: {v}"),
        // A first run records and does **not** pass -- an unreviewed
        // reference must not be indistinguishable from a checked one
        // (`Verdict::is_match`). Until M43/T43.9 this branch re-ran the
        // comparison and passed, so a deleted golden was re-recorded
        // green (I-NEW-4, the same shape as failure_screen.rs).
        Verdict::Recorded(ref p) => panic!(
            "a first run is not a pass: no reference existed, recorded {} -- review it, \
             commit it and re-run",
            p.display()
        ),
        other => panic!("an unchanged frame should match, got: {other}"),
    }

    // **The injection the plan names: change a radius by 1 px.**
    let v = compare(&d, "rounded", &frame(11.0)).expect("io");
    let Verdict::Differs { differing, total, region, ref diff, .. } = v else {
        panic!("a 1 px radius change must be seen, got: {v}");
    };
    println!("  ok  1 px radius change: {v}");

    // Not merely "it differs": the *count* is small and the *region* is
    // the corners, which is what a diff image is for.
    assert!(differing > 0, "the change produced no differing pixels");
    assert!(
        differing < total / 10,
        "a 1 px radius change touched {differing} of {total} pixels — that is not a \
         corner, so either the comparison or the render is wrong"
    );

    // The region must be inside the shape that moved, and must not be
    // the whole canvas. This is the assertion that says the diff
    // localises rather than merely reports.
    assert!(
        region[0] >= 19 && region[1] >= 15 && region[2] <= 101 && region[3] <= 65,
        "the differing region {region:?} is outside the rounded rect at [20, 16, 100, 64]"
    );
    assert!(
        (region[2] - region[0]) < W && (region[3] - region[1]) < H,
        "the differing region {region:?} is the whole canvas; it does not localise"
    );

    // And the artifact exists and is a picture, not a promise.
    let bytes = std::fs::read(diff).expect("the diff image was written");
    assert!(bytes.starts_with(b"P6\n"), "the diff is not a PPM");
    // **Find the body, do not assume its offset.** The first version of
    // this assertion hardcoded 15 and the header is 14 bytes, so every
    // RGB triple was read one byte out of phase and the count came back
    // zero — a bound computed from the wrong quantity, in a test written
    // to catch exactly that shape.
    let body = {
        let mut seen = 0;
        let mut at = 0;
        for (i, b) in bytes.iter().enumerate() {
            if *b == b'\n' {
                seen += 1;
                if seen == 3 {
                    at = i + 1;
                    break;
                }
            }
        }
        &bytes[at..]
    };
    let red = body.chunks_exact(3).filter(|p| p == &[255u8, 0, 0]).count();
    println!("  ok  diff image: {red} red pixels marking the change (of {differing} differing)");
    assert_eq!(
        red, differing,
        "the diff image must mark every differing pixel and no others: {red} red for \
         {differing} differences"
    );

    // Clean up the failure artifacts, but keep the reference.
    let _ = std::fs::remove_file(d.join("rounded.actual.ppm"));
    let _ = std::fs::remove_file(d.join("rounded.diff.ppm"));
}

/// A golden that is missing is **recorded, and does not pass**.
#[test]
fn a_first_run_records_rather_than_passing() {
    let d = dir();
    let name = "first-run-probe";
    let _ = std::fs::remove_file(d.join(format!("{name}.ppm")));

    let v = compare(&d, name, &frame(8.0)).expect("io");
    assert!(
        !v.is_match(),
        "a first run must not pass: an unreviewed reference would be indistinguishable \
         from a checked one, which is how a golden suite stops meaning anything"
    );
    assert!(matches!(v, Verdict::Recorded(_)), "got {v}");
    println!("  ok  {v}");

    let _ = std::fs::remove_file(d.join(format!("{name}.ppm")));
}

/// M82/T82.1: **a three-stop gradient keeps its pixels when `Stops`'
/// offsets become 1/255 steps.** The interior stop sits at 0.2, which is
/// 51/255 and exact in both encodings, so the golden -- recorded at the
/// parent commit with `f32` offsets -- must stay byte-identical. No other
/// golden holds a gradient, so without this one nothing could change.
#[test]
fn a_three_stop_gradient_keeps_its_golden() {
    let d = dir();
    let mut c = Canvas::new(W, H);
    let mut list = DisplayList::new();
    list.rect([0.0, 0.0, W as f32, H as f32], Color(255, 255, 255, 255));
    let stops = lumen_static::display::Stops {
        at: [0, 51, 255, 0],
        color: [Color(10, 20, 200, 255), Color(250, 240, 30, 255), Color(200, 30, 40, 255), Color(0, 0, 0, 0)],
        len: 3,
    };
    list.gradient([8.0, 8.0, 112.0, 72.0], stops, 0.0);
    let grads = list.cmds.iter().filter(|c| matches!(c, lumen_static::Cmd::Gradient { .. })).count();
    assert_eq!(grads, 1, "setup: the frame must hold the gradient it pins");
    c.render(&list, W, H);

    let want = std::fs::read(d.join("gradient3.ppm")).ok();
    let got = c.to_ppm();
    if let Some(want) = &want {
        let max = want.iter().zip(&got).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
        println!("gradient3: max channel delta {max} against the golden (bound 0), {} B", got.len());
    }
    let v = compare(&d, "gradient3", &c).expect("io");
    assert!(v.is_match(), "the three-stop gradient moved: {v}");
    let _ = std::fs::remove_file(d.join("gradient3.actual.ppm"));
    let _ = std::fs::remove_file(d.join("gradient3.diff.ppm"));
}
