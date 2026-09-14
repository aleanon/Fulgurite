//! M25/T25.10: a pie chart's wedges take the colours of its `series`.
//!
//! The wedges were a hard-coded six-colour palette no stylesheet could
//! reach. `Chart::series` defaults to it; this sets another and reads the
//! wedge colours back off the display list.

use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static::{Constraints, LayoutCx, Node, NodeState, Point};
use lumen_static_widgets::prelude::*;

const MINE: [Color; 3] = [Color(10, 20, 30, 255), Color(40, 50, 60, 255), Color(70, 80, 90, 255)];

/// The distinct colours the pie's wedges were drawn in, in drawing order.
fn wedge_colours(chart: &mut PieChart) -> Vec<Color> {
    let style = [chart.defaults(); 1];
    let mut st = [NodeState::default(); 1];
    let limits = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 200.0 };
    let mut metrics = lumen_static::cx::BlockMetrics;
    let mut cx = LayoutCx::new(&mut metrics);
    chart.layout(&mut cx, limits, &style, &mut st, 0);
    chart.place(Point { x: 0.0, y: 0.0 }, &style, &mut st, 0);
    let mut l = DisplayList::new();
    chart.paint(&st, 0, &mut l);
    let mut out: Vec<Color> = Vec::new();
    for c in &l.cmds {
        if let Cmd::Line { color, .. } = c {
            if out.last() != Some(color) {
                out.push(*color);
            }
        }
    }
    out
}

#[test]
fn a_pie_takes_its_wedge_colours_from_series() {
    let values = [3.0, 2.0, 1.0];
    let default = wedge_colours(&mut PieChart::new(&values));
    println!("default wedges {default:?}");
    assert_eq!(default.len(), 3, "setup: three wedges");
    assert!(
        MINE.iter().all(|c| !default.contains(c)),
        "setup: the series differs from the default"
    );

    let mut mine = PieChart::new(&values);
    mine.series = &MINE;
    let painted = wedge_colours(&mut mine);
    println!("set wedges {painted:?}");
    assert_eq!(painted, MINE.to_vec(), "the wedges ignored `series`");
}

#[test]
fn an_empty_series_falls_back_rather_than_painting_nothing() {
    let values = [3.0, 2.0, 1.0];
    let default = wedge_colours(&mut PieChart::new(&values));
    let mut empty = PieChart::new(&values);
    empty.series = &[];
    assert_eq!(wedge_colours(&mut empty), default);
}
