//! M40/T40.0, R1-F7: the display scale reaches the text memo through a
//! `Host`, not only through a hand-built `LayoutCx`.
//!
//! `crates/lumen-static/tests/measure_scale.rs` proves the memo keys on
//! the scale *if it is given one*. What gives it one in an application is
//! a single line in `Host::pump_with` — `.with_scale(self.scale)` — and
//! deleting that line left every test binary green (69 binaries, 345
//! tests, in the re-triage), because `BlockMetrics` is linear in the
//! scale and no host test could tell whether the scale arrived.
//!
//! So this drives the ordering that reaches it — winit's
//! `ScaleFactorChanged`, which delivers a new scale *and* a new physical
//! size, holding the logical size constant: the ordinary drag from a 1×
//! monitor to a 2× one — through a real `Host`, with the non-linear
//! measurer, and compares the label's box with a fresh host's at 2×.

use lumen_static::{App as DeriveApp, StaticTree};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;

#[path = "../../lumen-static/tests/support/hinted.rs"]
mod hinted;
use hinted::Hinted;

/// Logical size; physical is this times the scale.
const LW: u32 = 300;
const LH: u32 = 100;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Ui {
    #[widget_id("label")]
    label: Label<String>,
}

#[derive(DeriveApp, Default)]
struct Page {
    #[tree]
    ui: Ui,
}

fn index() -> usize {
    let table = <Ui as lumen_static::Widget>::SUB_TABLE;
    lumen_static::style::index_of_wid(table, "label").expect("id exists")
}

/// A host at `scale`, pumped once with a measurer at the same scale —
/// the two move together in the shell, from one `ScaleFactorChanged`.
fn fresh(scale: f32) -> Host<Page> {
    let mut app = Page::default();
    app.ui.label.set("POSITIVE".to_owned());
    let k = scale as u32;
    let mut h = Host::new(app, LW * k, LH * k);
    h.set_scale(scale);
    h.set_stylesheet(&[]);
    h.pump_with(&mut Hinted::at(scale));
    h
}

fn width(h: &Host<Page>) -> f32 {
    h.states[index()].size.w
}

#[test]
fn a_host_dragged_to_a_2x_display_re_measures_its_labels() {
    let at_1x = fresh(1.0);
    let at_2x = fresh(2.0);
    let (w1, w2) = (width(&at_1x), width(&at_2x));
    println!("setup: fresh 1x width {w1}, fresh 2x width {w2} (must differ under Hinted)");
    assert_ne!(w1, w2, "setup: Hinted must answer differently at 1x and 2x, or nothing here can fail");

    // The drag: scale doubles, physical size doubles, logical size holds.
    let mut dragged = fresh(1.0);
    dragged.set_scale(2.0);
    dragged.resize(LW * 2, LH * 2);
    let mut m = Hinted::at(2.0);
    dragged.pump_with(&mut m);
    let got = width(&dragged);
    println!(
        "dragged 1x -> 2x: width {got}; fresh 2x: {w2} (bound: equal); measurer calls on the drag frame {}",
        m.calls.get()
    );
    assert_eq!(
        got, w2,
        "the label kept its 1x box after the drag: the host did not hand its scale to the memo"
    );
}

#[test]
fn a_host_that_stays_at_1x_keeps_its_box() {
    // The control: with no scale change the dragged host and a fresh one
    // agree whether or not the scale reaches the memo, so a red here is a
    // fault in the fixture, not in the line under test.
    let mut stay = fresh(1.0);
    stay.resize(LW, LH);
    stay.pump_with(&mut Hinted::at(1.0));
    let (got, want) = (width(&stay), width(&fresh(1.0)));
    println!("no drag: width {got}; fresh 1x: {want} (bound: equal)");
    assert_eq!(got, want);
}
