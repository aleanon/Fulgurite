//! **A bar or line chart has a scale, labels and a hover readout** —
//! M57/B30, B31 (`docs/plan-widget-fixes.md`, `docs/widget-audit.md`
//! BarChart, LineChart). It was a shape with no scale, and nothing
//! answered "what is this one?".

use fulgurite::display::Cmd;
use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::{fmt_num, nice_scale, BarChart, LineChart};

#[test]
fn a_nice_scale_and_its_labels() {
    let cases = [(37.0, (10.0, 4)), (8.0, (2.0, 4)), (0.9, (0.5, 2)), (1234.0, (500.0, 3))];
    for (max, want) in cases {
        let got = nice_scale(max);
        println!("max {max}: step {} x {}", got.0, got.1);
        assert_eq!(got, want, "the scale for {max}");
        assert!(got.0 * got.1 as f32 >= max, "the top tick is under the data");
    }
    let f = |v: f32, s: f32| fmt_num(v, s, &mut [0u8; 16]).to_string();
    assert_eq!((f(30.0, 10.0), f(2.5, 0.5), f(0.05, 0.01), f(0.0, 1.0), f(-3.0, 1.0)),
        ("30".into(), "2.5".into(), "0.05".into(), "0".into(), "-3".into()));
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 20.0)]
struct Ui {
    #[widget_id("bar")]
    bar: BarChart,
    #[widget_id("line")]
    line: LineChart,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const MONTHS: &[&str] = &["Jan", "Feb", "Mar", "Apr"];

fn host() -> Host<App> {
    let mut app = App::default();
    let mut b = BarChart::new(&[12.0, 30.0, 21.0, 37.0]);
    b.axes = true;
    b.labels = MONTHS;
    // No axes on the line: its inset is then the half-stroke alone, which
    // is what keeps the peak's dot inside the box.
    let l = LineChart::new(&[12.0, 30.0, 21.0, 37.0]);
    app.ui.bar = b;
    app.ui.line = l;
    let mut h: Host<App> = Host::new(app, 400, 400);
    h.pump();
    h
}

#[test]
fn axes_label_the_scale_and_the_bars() {
    let h = host();
    let texts: Vec<String> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::Text { text, .. } => Some(h.list.str_of(*text).to_string()),
        _ => None,
    }).collect();
    println!("text runs: {texts:?}");
    for want in ["0", "10", "20", "30", "40", "Jan", "Apr"] {
        assert!(texts.iter().any(|t| t == want), "no {want:?} on the axes");
    }
}

#[test]
fn a_hover_marks_a_bar_and_shows_its_value() {
    let mut h = host();
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "bar").unwrap();
    let s = h.states[i];
    // The last of four bars: right of the y gutter, three quarters along.
    h.post(Event::PointerMove { x: s.pos.x + s.size.w - 20.0, y: s.pos.y + s.size.h / 2.0 });
    h.pump();
    let hot = h.app.ui.bar.hot();
    let shown = h.list.cmds.iter().any(|c| matches!(c, Cmd::Text { text, .. } if h.list.str_of(*text) == "37"));
    h.post(Event::PointerMove { x: 390.0, y: 395.0 });
    h.pump();
    println!("hot {hot:?}, value shown {shown}; after leaving hot {:?}", h.app.ui.bar.hot());
    assert_eq!(hot, Some(3), "the hover did not find the last bar");
    assert!(shown, "the hovered value is not shown");
    assert_eq!(h.app.ui.bar.hot(), None, "the mark outlived the pointer");
}

/// Round joins, one dot per point, and the peak inside the box.
#[test]
fn a_line_joins_round_and_stays_inside() {
    let h = host();
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "line").unwrap();
    let s = h.states[i];
    let dots: Vec<[f32; 4]> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::RoundedRect { rect, .. } if rect[1] >= s.pos.y - 10.0 && rect[3] <= s.pos.y + s.size.h + 10.0
            && (rect[2] - rect[0] - (rect[3] - rect[1])).abs() < 0.01 && rect[2] - rect[0] < 10.0 => Some(*rect),
        _ => None,
    }).collect();
    let top = dots.iter().map(|d| d[1]).fold(f32::INFINITY, f32::min);
    let left = dots.iter().map(|d| d[0]).fold(f32::INFINITY, f32::min);
    println!("{} dots; topmost at {top}, leftmost at {left}; box {:?}+{:?}", dots.len(), s.pos, s.size);
    // Position first: without the inset the peak's dot sits 1 px above
    // the box and the first one's left half off it -- and that one is
    // then dropped from the list entirely, so a count checked first would
    // fire for the same defect under the wrong name.
    assert!(top >= s.pos.y && left >= s.pos.x, "a dot is cut by the box");
    assert_eq!(dots.len(), 4, "not one round join per point");
}
