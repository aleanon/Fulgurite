//! **Contrast, measured on what is painted** — M54/T54.0
//! (`docs/plan-a11y-platform.md` §0 rule 5, R16-F8, R16-F9).
//!
//! Every widget in the catalogue (`tests/catalogue`) is laid out and
//! painted, and every text run in the display list is measured against
//! the ground it is painted on by `fulgurite_shell::contrast`. The
//! catalogue has each strip with a selected and an unselected item, and
//! each toggle in both states.
//!
//! Two bounds: **3:1**, the non-text minimum, which the catalogue must
//! meet everywhere (R16-F8: a selected rail label drawn in its pill's own
//! colour measured 1.0 and nothing noticed); and **4.5:1**, WCAG AA for
//! text, which the framework's default palette does not yet meet (R16-F9,
//! T54.6). Each prints the measured and unmeasured counts beside the
//! bound, so an empty measurement cannot pass.

mod catalogue;

use fulgurite::display::Color;
use fulgurite_shell::contrast::{text_contrast, Report};

const PAGE: Color = Color(255, 255, 255, 255);

fn measure() -> Report {
    let mut ui = catalogue::build();
    let (_, _, list) = catalogue::frame(&mut ui, 800.0);
    text_contrast(&list, PAGE)
}

fn check(r: &Report, bound: f32) -> Vec<String> {
    let min = r.min().expect("setup: no text was measured");
    println!(
        "min contrast: {:.2} at {:?} (bound {bound}), measured: {}, unmeasured: {}, clipped: {}",
        min.ratio,
        min.text,
        r.measured.len(),
        r.unmeasured.len(),
        r.clipped
    );
    for (k, t, why) in &r.unmeasured {
        println!("  unmeasured #{k} {t:?}: {why}");
    }
    let below: Vec<String> = r
        .below(bound)
        .map(|x| format!("{:?} {:.2}:1 (fg {:?} on {:?})", x.text, x.ratio, x.fg, x.ground))
        .collect();
    println!("runs below {bound}: {} / {}", below.len(), r.measured.len());
    below
}

#[test]
fn every_painted_text_run_reaches_the_non_text_minimum() {
    let r = measure();
    // The catalogue paints dozens of labels; far fewer means the harness
    // stopped finding them, and a harness that measured nothing passes.
    assert!(r.measured.len() >= 40, "setup: only {} text runs measured", r.measured.len());
    let below = check(&r, 3.0);
    assert!(below.is_empty(), "{} run(s) under 3:1:\n  {}", below.len(), below.join("\n  "));
}

#[test]
fn the_default_palette_reaches_aa_for_text() {
    let r = measure();
    assert!(r.measured.len() >= 40, "setup: only {} text runs measured", r.measured.len());
    let below = check(&r, 4.5);
    assert!(below.is_empty(), "{} run(s) under 4.5:1:\n  {}", below.len(), below.join("\n  "));
}
