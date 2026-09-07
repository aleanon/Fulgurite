//! **Does the software rasteriser scale to a real display?**
//!
//! Every render number this project has is at 400x800 or 440x520. That
//! is the one variable never swept, and it is the variable a CPU
//! rasteriser is most sensitive to: its cost is per *pixel*, not per
//! node. A 4K frame is 36x the pixels of the tally's window.
//!
//! One scene, several canvas sizes, min of 30. The scene scales with
//! the canvas so the *content* is comparable — a fixed scene on a
//! bigger canvas would measure empty space.

use std::time::Instant;

use lumen_static::display::{Color, DisplayList, Painter, Stops};
use lumen_static::Point;
use lumen_static_shell::software::Canvas;
use lumen_static_shell::text::FontStack;

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

fn main() {
    let font = FontStack::system().expect("a system face: text is most of a raster's cost");
    println!("raster: one scene, several canvas sizes, min of 30\n");
    println!("{:<14} {:>10} {:>12} {:>12} {:>10}", "canvas", "Mpx", "full (ms)", "per Mpx (ms)", "cmds");

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

        for _ in 0..5 {
            canvas.clear(lumen_static_shell::CLEAR);
            canvas.render_within(&list, area);
        }
        let mut best = f64::INFINITY;
        for _ in 0..30 {
            let t = Instant::now();
            canvas.clear(lumen_static_shell::CLEAR);
            canvas.render_within(&list, area);
            best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        }
        let mpx = (w as f64 * h as f64) / 1e6;
        println!(
            "{label:<14} {mpx:>10.2} {best:>12.2} {:>12.2} {:>10}",
            best / mpx,
            list.cmds.len()
        );
    }
    println!("\n   the live gate's budget is 16 ms a frame; 60 fps is 16.7");
}
