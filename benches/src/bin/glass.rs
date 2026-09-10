//! **What a glass panel costs, and what blur it actually delivers** —
//! M19/T19.3, for `docs/results-glass.md`.
//!
//! Two questions, and the second is the one a formula would get wrong.
//!
//! 1. Cost against panel area and sigma. A box blur with a running sum
//!    is O(1) per pixel in the radius, so the σ column should be flat
//!    past the point the downsample kicks in — if it is not, the
//!    downsample is not working.
//! 2. **Delivered σ against requested σ.** Three box passes approximate
//!    a Gaussian, the box width is an integer, and the downsample
//!    quantises again — so what a caller asks for and what it gets are
//!    not the same number. Printed side by side, per `AGENT.md`: a
//!    reader should not have to re-derive it.

use std::time::Instant;

use lumen_static::display::{Color, DisplayList};
use lumen_static_shell::software::Canvas;

/// Requested sigma against what the renderer will actually deliver.
///
/// **The renderer's own function**, not a copy of it: a results table
/// computed from a duplicate of the code it describes goes wrong
/// silently, and this one exists precisely to publish a number a caller
/// cannot otherwise see.
use lumen_static_shell::software::blur_plan as delivered;

fn best_of(n: u32, mut f: impl FnMut()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..n {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    best
}

/// A backdrop with structure in it, so the blur has something to do and
/// a uniform result would be visibly wrong.
fn backdrop(w: u32, h: u32, list: &mut DisplayList) {
    for k in 0..24 {
        let y = k as f32 * (h as f32 / 24.0);
        let c = if k % 2 == 0 { Color(40, 60, 120, 255) } else { Color(200, 210, 240, 255) };
        list.rect([0.0, y, w as f32, y + h as f32 / 48.0], c);
    }
}

fn main() {
    let (w, h) = (1920u32, 1080u32);
    let mut canvas = Canvas::new(w, h);

    println!("glass: one panel over a striped backdrop, 1920x1080, min of 20\n");
    println!("{:>7} {:>4} {:>4} {:>11} {:>9} {:>10}", "req σ", "k", "d", "delivered σ", "error", "panel ms");
    for sigma in [2.0f32, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0] {
        let (k, d, got) = delivered(sigma);
        let mut list = DisplayList::new();
        backdrop(w, h, &mut list);
        list.glass([200.0, 100.0, 1000.0, 400.0], 12.0, sigma, Color(255, 255, 255, 40));

        let mut base = DisplayList::new();
        backdrop(w, h, &mut base);

        let area = [0.0, 0.0, w as f32, h as f32];
        best_of(3, || {
            canvas.clear(Color(20, 24, 34, 255));
            canvas.render_within(&list, area);
        });
        let with = best_of(20, || {
            canvas.clear(Color(20, 24, 34, 255));
            canvas.render_within(&list, area);
        });
        let without = best_of(20, || {
            canvas.clear(Color(20, 24, 34, 255));
            canvas.render_within(&base, area);
        });
        println!(
            "{sigma:>7.1} {k:>4} {d:>4} {got:>11.2} {:>8.1}% {:>10.2}",
            (got - sigma) / sigma * 100.0,
            with - without
        );
    }

    println!("\n{:>14} {:>9} {:>10} {:>12}", "panel", "px", "ms", "ns/px");
    for (pw, ph) in [(200.0f32, 80.0f32), (400.0, 200.0), (800.0, 300.0), (1600.0, 800.0)] {
        let mut list = DisplayList::new();
        backdrop(w, h, &mut list);
        list.glass([40.0, 40.0, 40.0 + pw, 40.0 + ph], 12.0, 12.0, Color(255, 255, 255, 40));
        let mut base = DisplayList::new();
        backdrop(w, h, &mut base);
        let area = [0.0, 0.0, w as f32, h as f32];
        best_of(3, || {
            canvas.clear(Color(20, 24, 34, 255));
            canvas.render_within(&list, area);
        });
        let with = best_of(20, || {
            canvas.clear(Color(20, 24, 34, 255));
            canvas.render_within(&list, area);
        });
        let without = best_of(20, || {
            canvas.clear(Color(20, 24, 34, 255));
            canvas.render_within(&base, area);
        });
        let px = (pw * ph) as f64;
        println!(
            "{:>14} {px:>9.0} {:>10.2} {:>12.1}",
            format!("{pw:.0}x{ph:.0}"),
            with - without,
            (with - without) * 1.0e6 / px
        );
    }
}
