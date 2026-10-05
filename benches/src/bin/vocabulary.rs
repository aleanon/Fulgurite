//! **What the paint vocabulary costs on the software canvas** — M82
//! (`docs/plan-paint-vocabulary.md`), for
//! `docs/results-paint-vocabulary.md`.
//!
//! Created by M82/T82.2 with three gradient arms over one 1024 × 1024
//! canvas (one Mpx), each a single `Cmd::Gradient` covering it:
//!
//! - `linear`: the gradient as it was before M82, square;
//! - `radial`: a circular gradient from an off-centre point, square;
//! - `rounded-radial`: the same with a 24 px corner, so every pixel also
//!   pays the rounded box's coverage.
//!
//! Each arm asserts its setup before timing -- exactly one `Gradient` of
//! its kind in the list -- because an arm whose list was empty would time
//! a clear and report it as a gradient. Printed as ns per Mpx, the
//! minimum and the median of `RUNS` renders.
//!
//! M82/T82.5 adds two path arms over the same canvas, each one
//! `Cmd::Path` through `rasterize_path`:
//!
//! - `star-fill`: a five-pointed star of lines, 960 px across, filled;
//! - `circle-stroke`: a circle of four cubics, 960 px across, stroked
//!   8 px wide (a quad per flattened segment and a disc per vertex).
//!
//! Each asserts one path command with elements in the store before
//! timing.
//!
//! M82/T82.8 adds K82e's arm, built with `--features vector-text`: a
//! 24-glyph `VectorText` at 32 px, turned 30°, a whole frame each time —
//! layout (its outline memoised, as in a window), paint, and the software
//! canvas rasterising the path from its elements, which nothing caches
//! (`uncached`) — and the same with the memo missing every frame (`cold`:
//! the measurer's outline as well). Each asserts one path with elements
//! before timing; without the feature the arm says it was not built.

use std::time::Instant;

use lumen_static::display::{Cmd, Color, DisplayList, GradKind, GradShape, PathEl, PathPaint, Renderer, Stops, AFFINE_IDENTITY};
use lumen_static::Point;
use lumen_static_shell::software::Canvas;

const SIDE: u32 = 1024;
const RUNS: usize = 31;

fn stops() -> Stops {
    Stops {
        at: [0, 96, 255, 0],
        color: [Color(250, 200, 40, 255), Color(220, 60, 120, 255), Color(30, 40, 160, 255), Color(0, 0, 0, 0)],
        len: 3,
    }
}

/// Render `list` `RUNS` times; the minimum and the median, in ns per Mpx.
fn time(canvas: &mut Canvas, list: &DisplayList) -> (f64, f64) {
    let mpx = (SIDE as f64 * SIDE as f64) / 1e6;
    let mut ns: Vec<f64> = (0..RUNS)
        .map(|_| {
            canvas.clear(Color(0, 0, 0, 255));
            let t = Instant::now();
            canvas.render(list, SIDE, SIDE);
            t.elapsed().as_nanos() as f64 / mpx
        })
        .collect();
    ns.sort_by(|a, b| a.total_cmp(b));
    (ns[0], ns[RUNS / 2])
}

fn main() {
    let mut canvas = Canvas::new(SIDE, SIDE);
    let full = [0.0, 0.0, SIDE as f32, SIDE as f32];
    let radial = GradShape::Radial { center: [0.3, 0.4], radius: 0.8 };
    let arms: [(&str, GradShape, f32, GradKind); 3] = [
        ("linear", GradShape::Linear { angle: 0.6 }, 0.0, GradKind::Linear),
        ("radial", radial, 0.0, GradKind::Radial),
        ("rounded-radial", radial, 24.0, GradKind::Radial),
    ];
    println!("vocabulary: one command per arm over {SIDE}x{SIDE} (1.05 Mpx), software canvas, {RUNS} renders\n");
    println!("{:>16} {:>14} {:>14}", "arm", "min ns/Mpx", "median ns/Mpx");
    for (name, shape, corner, kind) in arms {
        let mut list = DisplayList::new();
        list.gradient(full, stops(), shape, corner);
        let n = list
            .cmds
            .iter()
            .filter(|c| matches!(c, Cmd::Gradient { kind: k, corner: r, .. } if *k == kind && *r == corner))
            .count();
        assert_eq!((n, list.cmds.len()), (1, 1), "setup: arm {name} must hold exactly one {kind:?} gradient");
        let (min, median) = time(&mut canvas, &list);
        println!("{name:>16} {min:>14.0} {median:>14.0}");
    }

    // M82/T82.5: the path arms.
    let p = |x: f32, y: f32| Point { x, y };
    let (c, r) = (SIDE as f32 / 2.0, 480.0);
    let mut star = Vec::new();
    for i in 0..10 {
        let rr = if i % 2 == 0 { r } else { r * 0.4 };
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        let q = p(c + rr * a.cos(), c + rr * a.sin());
        star.push(if i == 0 { PathEl::Move(q) } else { PathEl::Line(q) });
    }
    star.push(PathEl::Close);
    let k = 0.552_284_8 * r;
    let circle = vec![
        PathEl::Move(p(c + r, c)),
        PathEl::Cubic(p(c + r, c + k), p(c + k, c + r), p(c, c + r)),
        PathEl::Cubic(p(c - k, c + r), p(c - r, c + k), p(c - r, c)),
        PathEl::Cubic(p(c - r, c - k), p(c - k, c - r), p(c, c - r)),
        PathEl::Cubic(p(c + k, c - r), p(c + r, c - k), p(c + r, c)),
        PathEl::Close,
    ];
    let none = Color(0, 0, 0, 0);
    let paths: [(&str, Vec<PathEl>, PathPaint); 2] = [
        ("star-fill", star, PathPaint { fill: Color(220, 60, 120, 255), stroke: none, width: 0.0 }),
        ("circle-stroke", circle, PathPaint { fill: none, stroke: Color(30, 40, 160, 255), width: 8.0 }),
    ];
    for (name, els, paint) in paths {
        let mut list = DisplayList::new();
        list.path(&els, AFFINE_IDENTITY, paint);
        let n = list.cmds.iter().filter(|c| matches!(c, Cmd::Path { .. })).count();
        assert_eq!((n, list.cmds.len()), (1, 1), "setup: arm {name} must hold exactly one path");
        assert!(!list.paths.is_empty(), "setup: arm {name}'s path has no elements in the store");
        let (min, median) = time(&mut canvas, &list);
        println!("{name:>16} {min:>14.0} {median:>14.0}");
    }

    vector_text();
}

/// K82e's arm (M82/T82.8): a whole `VectorText` frame, in µs.
#[cfg(feature = "vector-text")]
fn vector_text() {
    use lumen_static::widget::{Node, Widget};
    use lumen_static::{Constraints, LayoutCx, NodeState, Style};
    use lumen_static_shell::text::{FontMetrics, FontStack};
    use lumen_static_widgets::VectorText;

    const RUN: &str = "Vector text, turned 30°!";
    assert_eq!(RUN.chars().count(), 24, "setup: K82e's run is 24 glyphs");
    let stack = FontStack::system().expect("K82e needs a system face");
    let styles = [Style { font_size: 32.0, ..<VectorText as Widget>::type_defaults() }];
    let view = Constraints { min_w: 0.0, max_w: 512.0, min_h: 0.0, max_h: 512.0 };
    let mut canvas = Canvas::new(512, 512);
    let frame = |w: &mut VectorText, m: &mut FontMetrics, canvas: &mut Canvas| -> (f64, usize) {
        let mut st = [NodeState::default()];
        let t = Instant::now();
        {
            let mut cx = LayoutCx::new(m);
            Node::layout(w, &mut cx, view, &styles, &mut st, 0);
        }
        Node::place(w, Point { x: 8.0, y: 8.0 }, &styles, &mut st, 0);
        let mut list = DisplayList::new();
        Node::paint(w, &st, 0, &mut list);
        canvas.render(&list, 512, 512);
        let us = t.elapsed().as_nanos() as f64 / 1e3;
        let n = list.cmds.iter().filter(|c| matches!(c, Cmd::Path { .. })).count();
        assert_eq!(n, 1, "setup: a VectorText frame holds one path (the measurer had no outlines?)");
        (us, list.paths.len())
    };
    println!("\nvector text: a 24-glyph VectorText at 32 px, turned 30 deg, one whole frame on a 512x512 canvas, {RUNS} frames\n");
    println!("{:>16} {:>14} {:>14} {:>10}", "arm", "min us", "median us", "elements");
    for cold in [false, true] {
        let mut w = VectorText::new(RUN, 30.0);
        let mut m = FontMetrics::new(stack.clone());
        let _ = frame(&mut w, &mut m, &mut canvas);
        let mut els = 0;
        let mut us: Vec<f64> = (0..RUNS)
            .map(|_| {
                if cold {
                    // A new generation: the memo misses and asks again.
                    m.set_stack(stack.clone());
                }
                canvas.clear(Color(255, 255, 255, 255));
                let (t, n) = frame(&mut w, &mut m, &mut canvas);
                els = n;
                t
            })
            .collect();
        assert!(els > 0, "setup: the outline has no elements");
        us.sort_by(|a, b| a.total_cmp(b));
        let name = if cold { "cold" } else { "uncached" };
        println!("{name:>16} {:>14.1} {:>14.1} {els:>10}", us[0], us[RUNS / 2]);
    }
}

#[cfg(not(feature = "vector-text"))]
fn vector_text() {
    println!("\nvector text: not built -- K82e's arm needs `--features vector-text`");
}
