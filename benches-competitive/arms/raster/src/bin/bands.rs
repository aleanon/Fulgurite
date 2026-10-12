//! **Would threading the software rasteriser scale?**
//!
//! A band-split rasteriser gives each thread a horizontal slice and lets
//! it draw only what intersects — which is `render_within` with a
//! different rect, so the primitive already exists. What it does *not*
//! share is the command walk: every band tests every command against its
//! own rect, so the walk is repeated once per band.
//!
//! This measures that redundancy sequentially, which is the honest way to
//! bound the parallel speed-up: whatever the bands cost in total, threads
//! can at best divide the *wall clock* by the core count, and can never
//! remove the duplicated walk. Sum-of-bands over one-full-render is the
//! efficiency ceiling.

use std::time::Instant;

use fulgurite::display::{Color, DisplayList, Stops};
use fulgurite::Point;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::FontStack;

fn scene(w: u32, h: u32, list: &mut DisplayList) {
    let (fw, fh) = (w as f32, h as f32);
    list.gradient([0.0, 0.0, fw, 64.0], Stops::two(Color(30, 40, 80, 255), Color(20, 24, 40, 255)), fulgurite::display::GradShape::Linear { angle: 1.57 }, 0.0);
    let cols = (fw / 220.0).max(1.0) as u32;
    let rows = (fh / 90.0).max(1.0) as u32;
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = (c as f32 * 220.0 + 8.0, r as f32 * 90.0 + 72.0);
            let card = [x, y, x + 204.0, y + 74.0];
            list.shadow(card, 8.0, 6.0);
            list.rounded_rect(card, Color(36, 42, 58, 255), 8.0);
            list.text([x + 12.0, y + 10.0, x + 190.0, y + 30.0], "Card title", 16.0, Color(240, 244, 250, 255));
            list.line(Point { x: x + 12.0, y: y + 60.0 }, Point { x: x + 190.0, y: y + 60.0 }, 1.0, Color(70, 80, 100, 255));
        }
    }
}

fn best(n: u32, mut f: impl FnMut()) -> f64 {
    let mut lo = f64::INFINITY;
    for _ in 0..n {
        let t = Instant::now();
        f();
        lo = lo.min(t.elapsed().as_secs_f64() * 1e3);
    }
    lo
}

fn main() {
    let font = FontStack::system().expect("a system face");
    println!("bands: sum of N band renders against one full render, min of 20\n");
    println!("{:<10} {:>6} {:>10} {:>26}", "canvas", "bands", "total (ms)", "vs one full render");

    for (w, h, label) in [(1920u32, 1080u32, "1080p"), (3840, 2160, "4K")] {
        let mut list = DisplayList::new();
        scene(w, h, &mut list);
        let mut canvas = Canvas::with_font(w, h, font.clone());
        let full_area = [0.0, 0.0, w as f32, h as f32];
        let full = best(20, || canvas.render_within(&list, full_area));
        println!("{label:<10} {:>6} {full:>10.2} {:>25}", 1, "1.00x (baseline)");

        for bands in [2usize, 4, 8, 16, 32, 64, 128] {
            let bh = h as f32 / bands as f32;
            let t = best(20, || {
                for b in 0..bands {
                    let area = [0.0, b as f32 * bh, w as f32, (b + 1) as f32 * bh];
                    canvas.render_within(&list, area);
                }
            });
            println!(
                "{:<10} {bands:>6} {t:>10.2} {:>24.2}x",
                "", t / full
            );
        }
        println!("   {} commands\n", list.cmds.len());
    }
    println!("   a ratio of 1.0 means band-splitting is free; 2.0 means threads");
    println!("   must beat a 2x handicap before they win anything");
}
