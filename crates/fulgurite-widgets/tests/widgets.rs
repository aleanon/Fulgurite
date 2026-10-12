//! Every widget in the crate, against the two contracts that are easy to
//! break and hard to see.

use fulgurite::display::{Cmd, DisplayList};
use fulgurite::{Constraints, LayoutCx, NodeState, Point, Size, Widget};
use fulgurite_widgets::prelude::*;
use fulgurite_widgets::MAX_POINTS;

/// Lay a single leaf out at `box_` **with no stylesheet at all** and
/// collect what it paints.
fn painted<W: fulgurite::Node + Widget>(w: &mut W, box_: Size) -> DisplayList {
    let mut st = [NodeState::default(); 8];
    let style = [w.defaults(); 8];
    let limits = Constraints { min_w: 0.0, max_w: box_.w, min_h: 0.0, max_h: box_.h };
    let mut metrics = fulgurite::cx::BlockMetrics;
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
    put("badge", painted(&mut Badge::new("9"), box_), &mut counts[1]);
    n += 1;
    put("chip", painted(&mut Chip::<&str>::new("tag"), box_), &mut counts[2]);
    n += 1;
    put("avatar", painted(&mut Avatar::new("AB"), box_), &mut counts[3]);
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
        let l = painted(&mut Avatar::new(name), box_);
        let shown = l.cmds.iter().find_map(|c| match c {
            Cmd::Text { text, .. } => Some(l.str_of(*text).to_string()),
            _ => None,
        });
        println!("  {name:?} -> {shown:?}");
        // No initials, no text run: since M57/B3 the run is the measured
        // initials' own box, which is empty for an empty name.
        if name.is_empty() {
            assert!(shown.as_deref().is_none_or(str::is_empty), "an empty name painted {shown:?}");
            continue;
        }
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

/// **A spinner is a ring, not a sunburst.**
///
/// The first version drew twelve *radial* spokes fading around the
/// circle — the Windows-95 hourglass look — where every toolkit's
/// spinner is a stroked circle with an arc missing. Both are "some lines
/// near a circle", so a test that counted commands could not tell them
/// apart, and neither could the visibility gate above.
///
/// The property that separates them is the **direction of a segment**: a
/// ring's endpoints are the same distance from the centre, a spoke's are
/// not. And the gap has to be real, or a full circle passes.
#[test]
fn a_spinner_draws_a_ring_with_a_gap() {
    use fulgurite::Point;
    let box_ = Size { w: 40.0, h: 40.0 };
    let l = painted(&mut Spinner::new(0.0), box_);
    let (cx, cy) = (20.0f32, 20.0f32);
    let segs: Vec<(Point, Point)> = l
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Line { a, b, .. } => Some((*a, *b)),
            _ => None,
        })
        .collect();
    assert!(segs.len() > 12, "only {} segments — not a ring", segs.len());

    let radius = |p: Point| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt();
    let mut worst = 0.0f32;
    for (a, b) in &segs {
        worst = worst.max((radius(*a) - radius(*b)).abs());
    }
    println!("{} segments, worst endpoint radius difference {worst:.2} px", segs.len());
    // A spoke runs from an inner radius to an outer one: the version
    // this replaced went from `0.30 × d` to `0.46 × d`, which on a 40 px
    // box is **6.4 px**. A chord's endpoints sit on the same circle, and
    // measure **0.56 px** apart — the six-decimal rounding in the unit
    // table, not a direction. The bound sits between the two.
    assert!(worst < 1.5, "segments are radial, not tangential ({worst:.2} px apart in radius)");

    // The gap: the segments must not close the circle. Measured as the
    // largest angular step between consecutive segment starts.
    let mut angles: Vec<f32> = segs
        .iter()
        .map(|(a, _)| (a.y - cy).atan2(a.x - cx))
        .collect();
    angles.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut biggest = angles[0] + core::f32::consts::TAU - angles[angles.len() - 1];
    for w in angles.windows(2) {
        biggest = biggest.max(w[1] - w[0]);
    }
    let frac = biggest / core::f32::consts::TAU;
    println!("largest angular gap: {:.0}% of the circle", frac * 100.0);
    assert!(frac > 0.10, "no gap — this is a circle, not a spinner ({:.0}%)", frac * 100.0);
    assert!(frac < 0.60, "the gap is most of the circle ({:.0}%)", frac * 100.0);
}

/// **A badge reads as an alert, and is round at a short count.**
///
/// Lumen's badge is a red notification count; a blue pill reads as a
/// tag, which is what `Chip` is for. Both halves are asserted because
/// the colour alone would pass on a red rectangle.
#[test]
fn a_badge_is_round_and_alarming() {
    let box_ = Size { w: 200.0, h: 60.0 };
    let l = painted(&mut Badge::new("3"), box_);
    let (rect, colour, radius) = l
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::RoundedRect { rect, color, radius } => Some((*rect, *color, *radius)),
            _ => None,
        })
        .expect("a badge painted no body");
    let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
    println!("badge {w:.0}x{h:.0} r{radius:.0} {colour:?}");
    assert!(colour.0 > 150 && colour.1 < 110 && colour.2 < 110, "a badge is not an alert colour");
    assert!(
        (radius - h / 2.0).abs() < 0.5,
        "the radius is {radius:.1}, not half the height ({:.1}) — it will not read as round",
        h / 2.0
    );
    assert!(w < h * 1.6, "a one-digit badge is {w:.0}x{h:.0}, too wide to read as round");
}

/// M24/T24.0 — every seed in this crate must say what its live node says.
///
/// The same gate `fulgurite/tests/seeds.rs` runs over the core, over
/// the widgets that live outside it.
///
/// **What the agreement half does and does not prove.** Every widget
/// here goes through `leaf_common!`, whose `kind_name` forwards to
/// `Widget::KIND` — so for a leaf the two sides of that assertion are
/// the same expression and it *cannot* fail. Changing a `KIND` changed
/// both and the test stayed green, which is how this was found. It is
/// kept because the forwarding is a macro anyone can stop using, and the
/// length check is real; the assertion that carries weight is
/// [`no_two_widgets_share_a_kind`] below.
#[test]
fn every_seed_agrees_with_its_live_node() {
    macro_rules! agrees {
        ($t:ty, $v:expr) => {{
            let v: $t = $v;
            let seeds = <$t as Widget>::SEEDS;
            assert_eq!(
                seeds.len(),
                <$t as Widget>::NODE_COUNT,
                "{}: SEEDS has {} entries for {} nodes",
                stringify!($t),
                seeds.len(),
                <$t as Widget>::NODE_COUNT
            );
            assert_eq!(
                seeds[0].kind,
                fulgurite::Node::kind_name(&v),
                "{}: the seed's kind and Node::kind_name disagree",
                stringify!($t)
            );
            assert_eq!(
                (seeds[0].defaults)(),
                fulgurite::Node::defaults(&v),
                "{}: the seed's defaults and Node::defaults disagree",
                stringify!($t)
            );
            seeds.len()
        }};
    }
    let mut n = 0;
    n += agrees!(Divider, Divider);
    n += agrees!(Badge<String>, Badge::default());
    n += agrees!(Chip<String>, Chip::default());
    n += agrees!(Avatar<String>, Avatar::default());
    n += agrees!(Icon, Icon::default());
    n += agrees!(ProgressBar, ProgressBar::default());
    n += agrees!(Spinner, Spinner::default());
    n += agrees!(Skeleton, Skeleton::default());
    n += agrees!(BarChart, BarChart::default());
    n += agrees!(LineChart, LineChart::default());
    n += agrees!(PieChart, PieChart::default());
    assert_eq!(n, 11, "every widget in the crate is one node; {n} checked");
    // The three charts are one type with three `KIND`s — the case a
    // per-type gate would miss if it only ever saw one of them.
    assert_eq!(<BarChart as Widget>::KIND, "barchart");
    assert_eq!(<LineChart as Widget>::KIND, "linechart");
    assert_eq!(<PieChart as Widget>::KIND, "piechart");
}

/// A kind name is a stylesheet's only handle on a widget type, and
/// **two widgets sharing one is the realistic failure**: a new widget is
/// written by copying the nearest one, and `const KIND` is one line of
/// the block that gets copied. Nothing else notices — both widgets
/// compile, both paint, and a `Kind` rule meant for one silently styles
/// the other.
///
/// `"widget"` is the trait default and means the `KIND` was never
/// written; the empty string would match nothing.
#[test]
fn no_two_widgets_share_a_kind() {
    let kinds = [
        <Divider as Widget>::KIND,
        <Badge<String> as Widget>::KIND,
        <Chip<String> as Widget>::KIND,
        <Avatar<String> as Widget>::KIND,
        <Icon as Widget>::KIND,
        <ProgressBar as Widget>::KIND,
        <Spinner as Widget>::KIND,
        <Skeleton as Widget>::KIND,
        <BarChart as Widget>::KIND,
        <LineChart as Widget>::KIND,
        <PieChart as Widget>::KIND,
    ];
    for (i, a) in kinds.iter().enumerate() {
        assert!(!a.is_empty() && *a != "widget", "kind {i} is {a:?}: never written");
        for b in &kinds[i + 1..] {
            assert_ne!(a, b, "two widgets both call themselves {a:?}");
        }
    }
    println!("{} distinct kinds: {kinds:?}", kinds.len());
}

/// **A badge is never narrower than it is tall** — M57/B1.
///
/// The rustdoc promised a one- or two-character count comes out circular,
/// and nothing made it so: "3" was round only because the digit is narrow
/// (`docs/widget-audit.md`, Badge). Tall padding makes every count here
/// narrower than its height by nature, so only the rule can round it —
/// without the padding, the block stand-in's wide digits pass either way.
#[test]
fn a_badge_is_at_least_as_wide_as_it_is_tall() {
    for text in ["3", "12"] {
        let mut b: Badge = Badge::new(text);
        let mut st = [NodeState::default(); 1];
        let mut style = [fulgurite::Node::defaults(&b); 1];
        style[0].padding.top = 14.0;
        style[0].padding.bottom = 14.0;
        let limits = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };
        let mut metrics = fulgurite::cx::BlockMetrics;
        let mut cx = LayoutCx::new(&mut metrics);
        let s = fulgurite::Node::layout(&mut b, &mut cx, limits, &style, &mut st, 0);
        println!("  {text:?}: {} x {}", s.w, s.h);
        assert!(s.w >= s.h - 0.01, "{text:?} is {} wide and {} tall", s.w, s.h);
    }
}

/// **A pie closes its ring** — M57/B32.
///
/// Each wedge's boundary was truncated, and the fractions summed in f32
/// end just below 1.0, so the last of the 96 segments was never drawn: a
/// 1.35% gap at 12 o'clock (`docs/widget-audit.md`, PieChart). A full
/// ring is 96 segment lines, whatever the data.
#[test]
fn a_pie_draws_every_segment_of_its_ring() {
    for data in [&[10.0f32, 6.0, 4.0, 3.0, 2.0][..], &[1.0, 1.0, 1.0][..], &[7.0][..], &[0.3, 0.3, 0.4][..]] {
        let mut pie: PieChart = PieChart::default();
        pie.data = Series::new(data);
        let l = painted(&mut pie, Size { w: 160.0, h: 160.0 });
        let lines = l.cmds.iter().filter(|c| matches!(c, Cmd::Line { .. })).count();
        println!("  {data:?}: {lines} segments");
        assert_eq!(lines, 96, "{data:?}: the ring has {lines} of its 96 segments");
    }
}
