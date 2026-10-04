//! **Text over a gradient is measured at its worst point** — M82/T82.2
//! (`docs/plan-paint-vocabulary.md`), inverting M82/T82.0's pin.
//!
//! Until T82.2, `contrast::text_contrast` named a run's ground only when
//! the topmost fill under its centre was one opaque colour, so a run over
//! a `Cmd::Gradient` landed in `Report::unmeasured` as `"gradient"`. Now
//! the gradient is sampled at the run's centre and both horizontal ends,
//! and the minimum ratio is the run's: the centre alone would pass a run
//! whose end sits on a failing stop.

use lumen_static::display::{Cmd, Color, DisplayList, GradShape, Stops};
use lumen_static_shell::contrast::{text_contrast, Report};

const PAGE: Color = Color(255, 255, 255, 255);
const INK: Color = Color(0, 0, 0, 255);
const AA: f32 = 4.5;

/// One run, 180 px wide, over a left-to-right gradient from white to
/// `end`: the run's centre sits on the middle of the ramp and its right
/// end close to `end`.
fn report(end: Color) -> Report {
    let mut list = DisplayList::new();
    list.gradient([0.0, 0.0, 200.0, 40.0], Stops::two(PAGE, end), GradShape::Linear { angle: 0.0 }, 0.0);
    list.text_weighted([10.0, 10.0, 190.0, 30.0], "over a gradient", 14.0, 400.0, INK);

    // The setup landed: exactly one text run, over exactly one gradient.
    let texts = list.cmds.iter().filter(|c| matches!(c, Cmd::Text { .. })).count();
    let grads = list.cmds.iter().filter(|c| matches!(c, Cmd::Gradient { .. })).count();
    println!("setup: {texts} text run(s), {grads} gradient(s) (want 1 and 1)");
    assert_eq!((texts, grads), (1, 1), "the fixture must hold one run over one gradient");

    let r = text_contrast(&list, PAGE);
    assert_eq!(r.clipped, 0, "the run's centre is inside no clip; it must be seen");
    assert!(r.unmeasured.is_empty(), "a run over an opaque gradient went unmeasured: {:?}", r.unmeasured);
    assert_eq!(r.measured.len(), 1, "the one run must be measured");
    r
}

#[test]
fn a_run_whose_end_sits_on_a_failing_stop_fails() {
    // Black on Color(90, 90, 90) is about 3.0:1, and on the ramp's middle
    // about 9:1: only a sample at the run's end can see the failure.
    let r = report(Color(90, 90, 90, 255));
    let run = &r.measured[0];
    println!("failing second stop: ratio {:.2} against ground {:?} (want < {AA})", run.ratio, run.ground);
    assert!(run.ratio < AA, "a run whose end sits on a failing stop measured {:.2}", run.ratio);
}

#[test]
fn a_run_over_two_passing_stops_passes() {
    // The accept direction: black on Color(150, 150, 150) is about 7:1.
    let r = report(Color(150, 150, 150, 255));
    let run = &r.measured[0];
    println!("passing stops: ratio {:.2} against ground {:?} (want >= {AA})", run.ratio, run.ground);
    assert!(run.ratio >= AA, "a run over two passing stops measured {:.2}", run.ratio);
}

#[test]
fn a_translucent_gradient_is_still_unmeasured() {
    let mut list = DisplayList::new();
    list.gradient(
        [0.0, 0.0, 200.0, 40.0],
        Stops::two(PAGE, Color(0, 0, 0, 128)),
        GradShape::Linear { angle: 0.0 },
        0.0,
    );
    list.text_weighted([10.0, 10.0, 190.0, 30.0], "over a gradient", 14.0, 400.0, INK);
    let r = text_contrast(&list, PAGE);
    println!("translucent gradient: measured {}, unmeasured {:?}", r.measured.len(), r.unmeasured);
    assert_eq!((r.measured.len(), r.unmeasured.len()), (0, 1));
    assert_eq!(r.unmeasured[0].2, "translucent gradient");
}

/// **A path is non-text ink, measured at 3:1** — M82/T82.5. A filled
/// wedge on the page, at about 2:1 and at about 4.6:1, through
/// `Report::shapes`; and a text run over the path's box is unmeasured as
/// `"path"`, never judged against the page behind it.
#[test]
fn a_path_is_measured_as_non_text_ink() {
    use lumen_static::display::{PathEl, PathPaint, AFFINE_IDENTITY};
    use lumen_static::Point;
    use lumen_static_shell::contrast::NON_TEXT_RATIO;
    let p = |x: f32, y: f32| Point { x, y };
    let wedge = [PathEl::Move(p(10.0, 10.0)), PathEl::Line(p(90.0, 30.0)), PathEl::Line(p(10.0, 50.0)), PathEl::Close];
    let shapes = |fill: Color| {
        let mut list = DisplayList::new();
        list.path(&wedge, AFFINE_IDENTITY, PathPaint { fill, stroke: Color(0, 0, 0, 0), width: 0.0 });
        list.text_weighted([20.0, 20.0, 80.0, 40.0], "on a path", 12.0, 400.0, INK);
        assert!(matches!(list.cmds[..], [Cmd::Path { .. }, Cmd::Text { .. }]), "setup: one path, then one run over it");
        text_contrast(&list, PAGE)
    };
    let faint = shapes(Color(183, 183, 183, 255));
    let firm = shapes(Color(118, 118, 118, 255));
    let ratio = |r: &Report| r.shapes.first().map(|s| s.ratio);
    println!(
        "shapes: faint {:?}, {} below {NON_TEXT_RATIO}; firm {:?}, {} below; text over the path: unmeasured {:?}",
        ratio(&faint),
        faint.shapes_below(NON_TEXT_RATIO).count(),
        ratio(&firm),
        firm.shapes_below(NON_TEXT_RATIO).count(),
        faint.unmeasured
    );
    assert_eq!(faint.shapes.len(), 1, "the path was not measured as a shape");
    assert_eq!(faint.shapes_below(NON_TEXT_RATIO).count(), 1, "a path at {:?}:1 passed 3:1", ratio(&faint));
    assert_eq!(firm.shapes_below(NON_TEXT_RATIO).count(), 0, "a path at {:?}:1 failed 3:1", ratio(&firm));
    assert_eq!(faint.unmeasured.len(), 1, "text over a path must be unmeasured, not judged against the page");
    assert_eq!(faint.unmeasured[0].2, "path");
}
