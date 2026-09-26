//! **An idle pump does nothing** — M53/T53.0, the gate for T53.3 (R6-F1,
//! `docs/plan-frame-cost.md` §1.3).
//!
//! `pump_with` has no idle check. Under `Boundaries` — the default for
//! every `Host` and every window since M34/T34.2 — layout skips a clean
//! tree, but `place`, paint through `Recording`, and `finish` run on
//! every pump. `docs/results-competitive.md` §5b calls an idle frame "the
//! 0.01 µs row", and that row times layout alone.
//!
//! Counts, not clocks (`Host::pass_counts`): 40 pumps with nothing
//! posted, nothing marked and nothing bound changing must run 0 place
//! and 0 paint passes and report damage `None`.

use lumen_static::invalidate::Boundaries;
use lumen_static::prelude::*;
use lumen_static::retain::{Damage, Incremental};
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("value")]
    value: Label<String>,
    go: Button<Still>,
}

#[derive(DeriveApp, Default)]
struct Still {
    #[tree]
    ui: Ui,
    #[bind(ui.value, "count: {}")]
    count: i64,
}

fn host() -> Host<Still, Incremental, Boundaries<256>> {
    let mut h: Host<Still, Incremental, Boundaries<256>> = Host::new(Still::default(), 320, 200);
    for _ in 0..3 {
        h.pump();
    }
    h
}

/// Red until T53.3 at 40 place and 40 paint passes per 40 idle pumps.
#[test]
fn an_idle_pump_places_and_paints_nothing() {
    let mut h = host();
    let before = h.pass_counts();
    let mut quiet = 0;
    for _ in 0..40 {
        h.pump();
        if h.damage() == Damage::None {
            quiet += 1;
        }
    }
    let d = h.pass_counts().since(before);
    println!(
        "40 idle pumps: {} place passes, {} paint passes (bound 0 each); damage None on {quiet}/40",
        d.places, d.paints
    );
    assert_eq!(quiet, 40, "an idle pump reported damage");
    assert_eq!((d.places, d.paints), (0, 0), "an idle pump placed or painted");
}

/// **The other direction, green today and after T53.3:** a pump after
/// `invalidate` still lays out, places and paints, and a bound value
/// that changed still reaches the screen.
#[test]
fn a_pump_after_invalidate_or_a_change_still_paints() {
    let mut h = host();
    let before = h.pass_counts();
    assert!(h.invalidate("title"), "setup: #title is on the table");
    h.pump();
    let d = h.pass_counts().since(before);
    println!("after invalidate: {} place, {} paint (bound >= 1 each)", d.places, d.paints);
    assert!(d.places >= 1 && d.paints >= 1, "a pump after invalidate did not paint");

    let before = h.pass_counts();
    h.app.count = 7;
    h.pump();
    let d = h.pass_counts().since(before);
    println!("after a bound change: {} paint, value {:?}, damage {:?}", d.paints, h.app.ui.value.as_str(), h.damage());
    assert_eq!(h.app.ui.value.as_str(), "count: 7", "setup: the binding did not write");
    assert!(d.paints >= 1 && h.damage() != Damage::None, "a changed frame did not paint");
}
