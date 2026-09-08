//! **Does the rasteriser scale to a real display?**
//!
//! Every render number this project had before 2026-09-07 was at
//! 400x800 or 440x520. That is the one variable never swept, and it is
//! the variable a CPU rasteriser is most sensitive to: its cost is per
//! *pixel*, not per node. A 4K frame is 36x the pixels of the tally's
//! window.
//!
//! One scene, several canvas sizes, min of 30. The scene scales with
//! the canvas so the *content* is comparable — a fixed scene on a
//! bigger canvas would measure empty space.
//!
//! Since T15.12 it sweeps **both** backends over the identical scene
//! and the identical display list, which is the comparison that
//! un-deferred the GPU work. `docs/results-gpu.md`.

use std::time::Instant;

use lumen_static::display::{Color, DisplayList, Stops};
use lumen_static::Point;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;
use lumen_static::display::Painter as _;

/// A frame with the mix a real application paints: cards with shadows,
/// text, rules, and a gradient header.
fn scene(w: u32, h: u32, list: &mut DisplayList) {
    let (fw, fh) = (w as f32, h as f32);
    list.gradient([0.0, 0.0, fw, 64.0], Stops::two(Color(30, 40, 80, 255), Color(20, 24, 40, 255)), 1.57);
    let cols = (fw / 220.0).max(1.0) as u32;
    let rows = (fh / 90.0).max(1.0) as u32;
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = (c as f32 * 220.0 + 8.0, r as f32 * 90.0 + 72.0);
            let card = [x, y, x + 204.0, y + 74.0];
            list.shadow(card, 8.0, 6.0);
            list.rounded_rect(card, Color(36, 42, 58, 255), 8.0);
            list.text([x + 12.0, y + 10.0, x + 190.0, y + 30.0], "Card title", 16.0, Color(240, 244, 250, 255));
            list.text([x + 12.0, y + 34.0, x + 190.0, y + 50.0], "a secondary line", 13.0, Color(150, 160, 180, 255));
            list.line(Point { x: x + 12.0, y: y + 60.0 }, Point { x: x + 190.0, y: y + 60.0 }, 1.0, Color(70, 80, 100, 255));
        }
    }
}

/// Min of `n` timings of `f`, in milliseconds. Min rather than mean:
/// the question is what the renderer costs, and every sample above the
/// minimum is something else the machine was doing.
fn best_of(n: u32, mut f: impl FnMut()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..n {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    best
}

fn main() {
    let font = FontStack::system().expect("a system face: text is most of a raster's cost");
    let mut gpu_name = String::from("none");
    println!("raster: one scene, several canvas sizes, min of 30\n");
    println!(
        "{:<14} {:>8} {:>10} {:>10} {:>10} {:>9} {:>8}",
        "canvas", "Mpx", "cpu (ms)", "gpu (ms)", "cpu/Mpx", "speedup", "cmds"
    );

    for (w, h, label) in [
        (440, 520, "tally window"),
        (1280, 720, "720p"),
        (1920, 1080, "1080p"),
        (2560, 1440, "1440p"),
        (3840, 2160, "4K"),
    ] {
        let mut list = DisplayList::new();
        scene(w, h, &mut list);
        let mut canvas = Canvas::with_font(w, h, font.clone());
        let area = [0.0, 0.0, w as f32, h as f32];

        best_of(5, || {
            canvas.clear(lumen_static_shell::CLEAR);
            canvas.render_within(&list, area);
        });
        let cpu = best_of(30, || {
            canvas.clear(lumen_static_shell::CLEAR);
            canvas.render_within(&list, area);
        });

        // The GPU arm, headless, over the identical list. `wait` after
        // every frame: `draw` only *queues* the work, and timing a
        // queue submission and calling it a render would flatter this
        // arm by an order of magnitude.
        let gpu = match lumen_static_wgpu::Gpu::headless(w, h) {
            Some(mut g) => {
                g.set_font(font.clone());
                gpu_name = g.adapter().to_owned();
                let page = lumen_static_shell::CLEAR;
                best_of(5, || {
                    g.draw(&list, None, Some((page, area)));
                    g.wait();
                });
                Some(best_of(30, || {
                    g.draw(&list, None, Some((page, area)));
                    g.wait();
                }))
            }
            None => None,
        };

        let mpx = (w as f64 * h as f64) / 1e6;
        match gpu {
            Some(g) => println!(
                "{label:<14} {mpx:>8.2} {cpu:>10.2} {g:>10.2} {:>10.2} {:>8.1}x {:>8}",
                cpu / mpx,
                cpu / g,
                list.cmds.len()
            ),
            None => println!(
                "{label:<14} {mpx:>8.2} {cpu:>10.2} {:>10} {:>10.2} {:>9} {:>8}",
                "-",
                cpu / mpx,
                "-",
                list.cmds.len()
            ),
        }
    }
    println!("\n   gpu: {gpu_name}");
    println!("   the live gate's budget is 16 ms a frame; 60 fps is 16.7");

    crossover(&font);
}

/// **Where does the GPU start winning?** — `docs/plan-invalidation.md`
/// T16.13.
///
/// `docs/results-gpu.md` §1 measured a full-surface redraw and found
/// 20x at 4K; §5b measured the incremental path and found the GPU
/// *losing* every damage-clipped frame. Both are true, so a crossover
/// exists, and four points do not locate it.
///
/// Two variables, because §5b showed both moving the answer: the
/// **damaged fraction** of the surface, and the **command count** of
/// the frame. One surface size throughout (1080p), so the only things
/// changing are the two being swept.
fn crossover(font: &FontStack) {
    const W: u32 = 1920;
    const H: u32 = 1080;
    println!("\n\ncrossover: 1080p, damaged fraction x command count, min of 20\n");

    let Some(mut gpu) = lumen_static_wgpu::Gpu::headless(W, H) else {
        println!("   no adapter — skipped");
        return;
    };
    gpu.set_font(font.clone());
    let mut canvas = Canvas::with_font(W, H, font.clone());
    let page = lumen_static_shell::CLEAR;

    print!("{:<10}", "cmds");
    for f in [0.01f32, 0.02, 0.03, 0.05, 0.10] {
        print!("{:>18}", format!("{:.0}% damage", f * 100.0));
    }
    println!();

    for cards in [8usize, 80, 800] {
        let mut list = DisplayList::new();
        // A band of cards, tiled to fill the surface, so every damage
        // fraction below intersects roughly the same density of them.
        let cols = 8usize;
        for k in 0..cards {
            let (cx, cy) = ((k % cols) as f32 * 236.0 + 8.0, (k / cols) as f32 * 90.0 + 8.0);
            let card = [cx, cy, cx + 220.0, cy + 74.0];
            list.rounded_rect(card, Color(36, 42, 58, 255), 8.0);
            list.text(
                [cx + 12.0, cy + 10.0, cx + 200.0, cy + 30.0],
                "Card title",
                16.0,
                Color(240, 244, 250, 255),
            );
        }
        print!("{:<10}", list.cmds.len());

        for frac in [0.01f32, 0.02, 0.03, 0.05, 0.10] {
            // A full-width band, which is the shape a scrolling list or
            // an animating row damages.
            let h = (H as f32 * frac).max(1.0);
            let area = [0.0, 0.0, W as f32, h];

            for _ in 0..3 {
                canvas.clear_within(area, page);
                canvas.render_within(&list, area);
                gpu.draw(&list, Some(area), Some((page, area)));
                gpu.wait();
            }
            let sw = best_of(20, || {
                canvas.clear_within(area, page);
                canvas.render_within(&list, area);
            });
            let hw = best_of(20, || {
                gpu.draw(&list, Some(area), Some((page, area)));
                gpu.wait();
            });
            let mark = if hw < sw { "gpu" } else { "cpu" };
            print!("{:>18}", format!("{sw:.2}/{hw:.2} {mark}"));
        }
        println!();
    }
    println!("\n   each cell is cpu/gpu in ms, and which won");
}
