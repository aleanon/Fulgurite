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

use lumen_static::display::{Color, DisplayList, Material};
use lumen_static_shell::software::Canvas;

/// Requested sigma against what the renderer will actually deliver.
///
/// **The renderer's own function**, not a copy of it: a results table
/// computed from a duplicate of the code it describes goes wrong
/// silently, and this one exists precisely to publish a number a caller
/// cannot otherwise see.
use lumen_static_shell::software::blur_plan as delivered;

/// Frosted at `sigma`: the material M19 shipped, so these numbers stay
/// comparable with the table already in `docs/results-glass.md`.
fn frosted(sigma: f32) -> Material {
    Material { blur: sigma, tint: Color(255, 255, 255, 40), ..Material::FROSTED }
}

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
        list.glass([200.0, 100.0, 1000.0, 400.0], 12.0, frosted(sigma));

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

    // What a cache could ever save. A cache must still CAPTURE the
    // source to know whether it is still valid, so its best case is the
    // cost with the box passes removed -- which `blur = 0` measures
    // exactly, because `glass_within` skips them at zero.
    // What the bevel costs: the same panel, frosted against liquid.
    // Liquid is nearly all composite -- its blur is 3 px -- so this is
    // the price of refraction and specular, not of a different blur.
    // M22/T22.0: what Fresnel, the inner shadow and saturation add on
    // top of the bevel. All three are rim-band only except saturation,
    // which is the whole panel -- so the two sizes below separate them.
    //
    // M22/T22.2 shares the harness: `dispersion` is the only field that
    // changes the *number of taps*, so it gets its own column rather
    // than being folded into "full".
    println!("\n  T22.0/T22.2/T22.3: the bevel alone, then each milestone field group");
    println!(
        "{:>14} {:>12} {:>12} {:>10} {:>12} {:>10} {:>12} {:>10}",
        "panel", "bevel ms", "T22.0 ms", "added", "disp ms", "added", "adapt ms", "added"
    );
    for (pw, ph) in [(400.0f32, 200.0f32), (800.0, 300.0)] {
        let area = [0.0, 0.0, w as f32, h as f32];
        let mut timed = |m: Material| {
            let mut list = DisplayList::new();
            backdrop(w, h, &mut list);
            list.glass([40.0, 40.0, 40.0 + pw, 40.0 + ph], 26.0, m);
            let mut base = DisplayList::new();
            backdrop(w, h, &mut base);
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
            with - without
        };
        // M21's material: no shadow, neutral saturation, and Fresnel
        // rides `specular` so it cannot be switched off separately --
        // which is what "zero bytes" buys and costs.
        let m21 = Material {
            shadow: 0,
            saturation: Material::NEUTRAL_SATURATION,
            dispersion: 0,
            ..Material::LIQUID
        };
        let bevel = timed(m21);
        // `LIQUID` carries dispersion since T22.2, so the T22.0 column
        // has to switch it off or it stops measuring T22.0.
        let full = timed(Material { dispersion: 0, ..Material::LIQUID });
        let disp = timed(Material { adapt: 0, ..Material::LIQUID });
        // M22/T22.3: one reduction over the panel before the composite.
        let adapt = timed(Material::LIQUID);
        println!(
            "{:>14} {bevel:>12.2} {full:>12.2} {:>9.0}% {disp:>12.2} {:>9.0}% {adapt:>12.2} {:>9.0}%",
            format!("{pw:.0}x{ph:.0}"),
            (full - bevel) / bevel * 100.0,
            (disp - full) / full * 100.0,
            (adapt - disp) / disp * 100.0
        );
    }

    println!("\n  the bevel: frosted against liquid, same panel");
    println!("{:>14} {:>12} {:>12} {:>10}", "panel", "frosted ms", "liquid ms", "bevel");
    for (pw, ph) in [(400.0f32, 200.0f32), (800.0, 300.0)] {
        let area = [0.0, 0.0, w as f32, h as f32];
        let mut timed = |m: Material| {
            let mut list = DisplayList::new();
            backdrop(w, h, &mut list);
            list.glass([40.0, 40.0, 40.0 + pw, 40.0 + ph], 26.0, m);
            let mut base = DisplayList::new();
            backdrop(w, h, &mut base);
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
            with - without
        };
        // Frosted at the liquid blur, so only the bevel differs.
        let flat = timed(Material { blur: Material::LIQUID.blur, ..Material::FROSTED });
        let bevel = timed(Material::LIQUID);
        println!(
            "{:>14} {flat:>12.2} {bevel:>12.2} {:>9.0}%",
            format!("{pw:.0}x{ph:.0}"),
            (bevel - flat) / flat * 100.0
        );
    }

    println!("\n  what the box passes cost, against capture + composite alone");
    println!("{:>14} {:>12} {:>12} {:>10}", "panel", "blur 12 ms", "blur 0 ms", "passes");
    for (pw, ph) in [(400.0f32, 200.0f32), (800.0, 300.0)] {
        let area = [0.0, 0.0, w as f32, h as f32];
        let mut timed = |sigma: f32| {
            let mut list = DisplayList::new();
            backdrop(w, h, &mut list);
            list.glass([40.0, 40.0, 40.0 + pw, 40.0 + ph], 12.0, frosted(sigma));
            let mut base = DisplayList::new();
            backdrop(w, h, &mut base);
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
            with - without
        };
        let full = timed(12.0);
        let none = timed(0.0);
        println!(
            "{:>14} {full:>12.2} {none:>12.2} {:>9.0}%",
            format!("{pw:.0}x{ph:.0}"),
            (full - none) / full * 100.0
        );
    }

    println!("\n{:>14} {:>9} {:>10} {:>12}", "panel", "px", "ms", "ns/px");
    for (pw, ph) in [(200.0f32, 80.0f32), (400.0, 200.0), (800.0, 300.0), (1600.0, 800.0)] {
        let mut list = DisplayList::new();
        backdrop(w, h, &mut list);
        list.glass([40.0, 40.0, 40.0 + pw, 40.0 + ph], 12.0, frosted(12.0));
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
