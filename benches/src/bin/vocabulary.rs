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
//! minimum and the median of `RUNS` renders. M82/T82.5 and T82.8 add the
//! path and vector-text arms.

use std::time::Instant;

use lumen_static::display::{Cmd, Color, DisplayList, GradKind, GradShape, Renderer, Stops};
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
    println!("vocabulary: one gradient over {SIDE}x{SIDE} (1.05 Mpx), software canvas, {RUNS} renders\n");
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
}
