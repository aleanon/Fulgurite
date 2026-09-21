//! M25/T25.10: a toggle's mark is a colour the toggle carries.
//!
//! Before M25 the tick, the dot and the knob were a hard-coded white,
//! because a third colour on every node was too dear for one part of one
//! widget. A field on the widget costs bytes only where a toggle is — the
//! shape `Image::tint` already had — and this is the test that the field
//! is what paints.

use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static::{Constraints, LayoutCx, Node, NodeState, Point};
use lumen_static_widgets::{CheckBox, Switch};

const RED: Color = Color(200, 30, 30, 255);
const WHITE: Color = Color(255, 255, 255, 255);

/// The colours of every rounded rect a toggle paints, in order: the body,
/// then the mark.
fn painted<N: Node>(w: &mut N) -> Vec<Color> {
    let style = [w.defaults(); 1];
    let mut st = [NodeState::default(); 1];
    let limits = Constraints { min_w: 0.0, max_w: 100.0, min_h: 0.0, max_h: 100.0 };
    let mut metrics = lumen_static::cx::BlockMetrics;
    let mut cx = LayoutCx::new(&mut metrics);
    w.layout(&mut cx, limits, &style, &mut st, 0);
    w.place(Point { x: 0.0, y: 0.0 }, &style, &mut st, 0);
    let mut l = DisplayList::new();
    w.paint(&st, 0, &mut l);
    l.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::RoundedRect { color, .. } => Some(*color),
            _ => None,
        })
        .collect()
}

#[test]
fn the_default_mark_is_white() {
    let mut w = CheckBox::<()> { on: true, ..Default::default() };
    let colours = painted(&mut w);
    assert!(colours.len() > 1, "setup: an on checkbox paints a body and a tick");
    assert!(colours[1..].iter().all(|c| *c == WHITE), "the default tick is not white: {colours:?}");
}

#[test]
fn a_mark_colour_paints_the_tick_and_the_knob() {
    // Setup: not the default, or a paint that ignored the field would pass.
    assert_ne!(RED, CheckBox::<()>::default().mark);

    let mut check = CheckBox::<()> { on: true, mark: RED, ..Default::default() };
    let colours = painted(&mut check);
    println!("checkbox {colours:?}");
    assert!(colours.len() > 1, "setup: an on checkbox paints a body and a tick");
    assert!(colours[1..].iter().all(|c| *c == RED), "the tick ignored `mark`: {:?}", &colours[1..]);

    // The switch's knob, off as well as on: it is drawn in both states.
    for on in [false, true] {
        let mut switch = Switch::<()> { on, mark: RED, ..Default::default() };
        let parts = painted(&mut switch);
        println!("switch on={on} {parts:?}");
        assert_eq!(parts.len(), 2, "setup: a track and a knob");
        assert_eq!(parts[1], RED, "the knob ignored `mark` (on = {on})");
    }
}
