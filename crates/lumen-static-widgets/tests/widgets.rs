//! Every widget in the crate, against the two contracts that are easy to
//! break and hard to see.

use lumen_static::display::{Cmd, DisplayList};
use lumen_static::{Constraints, LayoutCx, NodeState, Point, Size, Widget};
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::MAX_POINTS;

/// Lay a single leaf out at `box_` **with no stylesheet at all** and
/// collect what it paints.
fn painted<W: lumen_static::Node + Widget>(w: &mut W, box_: Size) -> DisplayList {
    let mut st = [NodeState::default(); 8];
    let style = [w.defaults(); 8];
    let limits = Constraints { min_w: 0.0, max_w: box_.w, min_h: 0.0, max_h: box_.h };
    let mut metrics = lumen_static::cx::BlockMetrics;
    let mut cx = LayoutCx::new(&mut metrics);
    w.layout(&mut cx, limits, &style, &mut st, 0);
    w.place(Point { x: 0.0, y: 0.0 }, &style, &mut st, 0);
    let mut list = DisplayList::new();
    w.paint(&st, 0, &mut list);
    list
}

fn ink(l: &DisplayList) -> usize {
    l.cmds
        .iter()
        .filter(|c| !matches!(c, Cmd::PushClip { .. } | Cmd::PopClip))
        .count()
}

/// **A control whose default styling draws nothing is not a default.**
///
/// `Toggle`'s module note records this shipping once in the core: an
/// "on" switch whose knob was `Style::background`'s transparent default,
/// so the mark was invisible. Every widget here is checked with **no
/// stylesheet**, which is the state an application sees before it writes
/// one and the state every screenshot in a README is taken in.
#[test]
fn every_widget_is_visible_with_no_stylesheet() {
    let box_ = Size { w: 200.0, h: 120.0 };
    let mut counts = [("", 0usize); 11];
    let mut n = 0;
    let put = |name: &'static str, l: DisplayList, slot: &mut (&'static str, usize)| {
        *slot = (name, ink(&l));
    };
    put("divider", painted(&mut Divider, box_), &mut counts[0]);
    n += 1;
    put("badge", painted(&mut Badge { text: "9" }, box_), &mut counts[1]);
    n += 1;
    put("chip", painted(&mut Chip { text: "tag" }, box_), &mut counts[2]);
    n += 1;
    put("avatar", painted(&mut Avatar { initials: "AB" }, box_), &mut counts[3]);
    n += 1;
    put("icon", painted(&mut Icon::new(Symbol::Check), box_), &mut counts[4]);
    n += 1;
    put("progressbar", painted(&mut ProgressBar::new(0.5), box_), &mut counts[5]);
    n += 1;
    put("spinner", painted(&mut Spinner::new(0.25), box_), &mut counts[6]);
    n += 1;
    put("skeleton", painted(&mut Skeleton::new(0.3), box_), &mut counts[7]);
    n += 1;
    put("barchart", painted(&mut BarChart::new(&[1.0, 4.0, 2.0]), box_), &mut counts[8]);
    n += 1;
    put("linechart", painted(&mut LineChart::new(&[1.0, 4.0, 2.0]), box_), &mut counts[9]);
    n += 1;
    put("piechart", painted(&mut PieChart::new(&[1.0, 2.0, 3.0]), box_), &mut counts[10]);
    n += 1;

    assert_eq!(n, counts.len(), "a widget was added without being listed here");
    for (name, drawn) in counts {
        println!("  {name}: {drawn} commands");
        assert!(drawn > 0, "`{name}` draws nothing with no stylesheet");
    }
}

/// **An empty bar is empty, and a full one is full.**
///
/// The zero end is the one with a trap in it: a rounded rect of zero
/// width still covers `2 × radius` px once the renderer clamps the
/// radius, so a 0% bar drawn without a guard shows a dot.
#[test]
fn a_progress_bar_is_empty_at_zero_and_full_at_one() {
    let box_ = Size { w: 200.0, h: 8.0 };
    let fills = |v: f32| {
        let l = painted(&mut ProgressBar::new(v), box_);
        // The track is the first rounded rect; the fill, if any, the
        // second.
        l.cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::RoundedRect { rect, .. } => Some(rect[2] - rect[0]),
                _ => None,
            })
            .nth(1)
    };
    println!("0.0 -> {:?}, 0.5 -> {:?}, 1.0 -> {:?}", fills(0.0), fills(0.5), fills(1.0));
    assert_eq!(fills(0.0), None, "an empty bar painted a fill");
    assert_eq!(fills(1.0), Some(200.0), "a full bar did not reach the end");
    let half = fills(0.5).expect("a half bar painted no fill");
    assert!((half - 100.0).abs() < 0.5, "a half bar is {half} wide, not 100");
    // NaN is not an error, it is empty. Every comparison against it is
    // false, so a bar written the other way round would draw it *full*.
    assert_eq!(fills(f32::NAN), None, "a NaN value painted a fill");
}

/// **A series is truncated, not refused, and bad samples cost one point.**
#[test]
fn a_series_survives_hostile_input() {
    let mut long = [1.0f32; MAX_POINTS + 9];
    long[0] = 5.0;
    let s = Series::new(&long);
    assert_eq!(s.len(), MAX_POINTS, "a long series was not truncated to the cap");

    let s = Series::new(&[2.0, f32::NAN, 8.0, f32::INFINITY, -3.0]);
    println!("max {} sum {}", s.max(), s.sum());
    assert_eq!(s.max(), 8.0, "a NaN or an infinity reached `max`");
    assert_eq!(s.sum(), 10.0, "a NaN, an infinity or a negative reached `sum`");
}

/// **The tallest bar reaches the top and the shortest is proportional.**
#[test]
fn a_bar_chart_scales_to_its_largest_value() {
    let box_ = Size { w: 300.0, h: 100.0 };
    let l = painted(&mut BarChart::new(&[1.0, 2.0, 4.0]), box_);
    let heights: Vec<f32> = l
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::RoundedRect { rect, .. } => Some(rect[3] - rect[1]),
            _ => None,
        })
        .collect();
    println!("bar heights {heights:?} in a {} px box", box_.h);
    assert_eq!(heights.len(), 3, "three values did not make three bars");
    assert!((heights[2] - 100.0).abs() < 0.5, "the tallest bar is {}, not the box", heights[2]);
    assert!((heights[0] - 25.0).abs() < 0.5, "the shortest bar is {}, not a quarter", heights[0]);
}

/// **An avatar trims on a `char` boundary.**
///
/// Two *bytes* of "Ål" is one byte into a two-byte character, and
/// slicing there panics. An avatar is decoration; taking a frame down
/// over somebody's name is not a trade worth making.
#[test]
fn an_avatar_trims_multibyte_initials() {
    let box_ = Size { w: 40.0, h: 40.0 };
    for name in ["ÅB", "ÆØÅ", "AB", "A", "", "日本語"] {
        let l = painted(&mut Avatar { initials: name }, box_);
        let shown = l.cmds.iter().find_map(|c| match c {
            Cmd::Text { text, .. } => Some(l.str_of(*text).to_string()),
            _ => None,
        });
        println!("  {name:?} -> {shown:?}");
        let shown = shown.expect("an avatar painted no text");
        assert!(shown.chars().count() <= 2, "{name:?} kept {shown:?}");
        assert!(name.starts_with(&shown), "{name:?} did not keep its own prefix");
    }
}

/// **Each symbol draws a different mark.**
///
/// A `match` that fell through to one arm would still paint, still pass
/// the visibility gate above, and give every icon in an application the
/// same picture.
#[test]
fn every_symbol_draws_a_distinct_mark() {
    let box_ = Size { w: 24.0, h: 24.0 };
    let syms = [
        Symbol::Play,
        Symbol::Pause,
        Symbol::Stop,
        Symbol::Check,
        Symbol::Close,
        Symbol::Down,
        Symbol::Plus,
        Symbol::Minus,
    ];
    let shapes: Vec<String> = syms
        .iter()
        .map(|s| {
            let l = painted(&mut Icon::new(*s), box_);
            l.cmds
                .iter()
                .map(|c| match c {
                    Cmd::Line { a, b, .. } => format!("L{:.1},{:.1}-{:.1},{:.1};", a.x, a.y, b.x, b.y),
                    Cmd::Rect { rect, .. } => {
                        format!("R{:.1},{:.1},{:.1},{:.1};", rect[0], rect[1], rect[2], rect[3])
                    }
                    _ => "?;".into(),
                })
                .collect()
        })
        .collect();
    for (s, d) in syms.iter().zip(shapes.iter()) {
        println!("  {s:?}: {} chars", d.len());
        assert!(!d.is_empty(), "{s:?} drew nothing");
    }
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            assert_ne!(
                shapes[i], shapes[j],
                "{:?} and {:?} draw the same mark",
                syms[i], syms[j]
            );
        }
    }
}
