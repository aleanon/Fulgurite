//! **Text over a gradient is counted, not measured** — a pin, M82/T82.0
//! (`docs/plan-paint-vocabulary.md`).
//!
//! `contrast::text_contrast` names a run's ground only when the topmost
//! fill under its centre is one opaque colour. A `Cmd::Gradient` is not,
//! so a run over one lands in `Report::unmeasured` with the reason
//! `"gradient"`. That is today's gap, asserted green so that the task
//! closing it (M82/T82.2, which samples the gradient at the run's centre
//! and both ends) turns this test red by name and inverts it.

use lumen_static::display::{Cmd, Color, DisplayList, Stops};
use lumen_static_shell::contrast::text_contrast;

const PAGE: Color = Color(255, 255, 255, 255);

#[test]
fn text_over_a_gradient_is_unmeasured_with_the_reason() {
    let mut list = DisplayList::new();
    list.gradient(
        [0.0, 0.0, 200.0, 40.0],
        Stops::two(Color(20, 20, 20, 255), Color(240, 240, 240, 255)),
        0.0,
    );
    list.text_weighted([10.0, 10.0, 190.0, 30.0], "over a gradient", 14.0, 400.0, Color(0, 0, 0, 255));

    // The setup landed: exactly one text run, over exactly one gradient.
    let texts = list.cmds.iter().filter(|c| matches!(c, Cmd::Text { .. })).count();
    let grads = list.cmds.iter().filter(|c| matches!(c, Cmd::Gradient { .. })).count();
    println!("setup: {texts} text run(s), {grads} gradient(s) (want 1 and 1)");
    assert_eq!((texts, grads), (1, 1), "the fixture must hold one run over one gradient");

    let r = text_contrast(&list, PAGE);
    println!(
        "measured {}, unmeasured {:?}, clipped {} (want 0, one \"gradient\", 0)",
        r.measured.len(),
        r.unmeasured,
        r.clipped
    );
    assert_eq!(r.clipped, 0, "the run's centre is inside no clip; it must be seen");
    assert!(
        r.measured.is_empty(),
        "a run over a gradient was measured (ratio {:?}): the pin's gap closed -- \
         invert this test in the task that closed it (M82/T82.2)",
        r.measured.first().map(|m| m.ratio)
    );
    assert_eq!(r.unmeasured.len(), 1, "the one run must be counted, never skipped");
    assert_eq!(r.unmeasured[0].2, "gradient", "the run is unmeasured for another reason");
}
