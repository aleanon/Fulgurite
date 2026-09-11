//! What the backdrop filter costs on the GPU — M22/T22.6.
//!
//! The number the task exists for. `docs/results-glass.md` §2 measures
//! the software filter at ~21 ns/px: 0.6 ms for a toolbar and **43 ms at
//! 1080p**, which is why a panel is affordable and a screen is not.
//!
//! Both arms render the *same* display list, and the GPU arm waits for
//! the device — `draw` returns as soon as the work is queued, and timing
//! that would report the queue submission as a render.
use lumen_static::display::{Color, DisplayList, Material, Renderer};
use lumen_static_shell::software::Canvas;
use lumen_static_wgpu::Gpu;
use std::time::Instant;

fn scene(w: u32, h: u32, panel: Option<[f32; 4]>) -> DisplayList {
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, w as f32, h as f32], Color(0x20, 0x40, 0x90, 0xff));
    l.rect([0.0, h as f32 / 2.0, w as f32, h as f32], Color(0xf0, 0xc0, 0x30, 0xff));
    if let Some(p) = panel {
        l.glass(p, 28.0, Material::LIQUID);
    }
    l
}

fn best_of(n: u32, mut f: impl FnMut()) -> f32 {
    let mut best = f32::INFINITY;
    for _ in 0..n {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f32() * 1000.0);
    }
    best
}

fn main() {
    println!("{:>16} {:>12} {:>12} {:>10}", "panel", "software ms", "gpu ms", "speedup");
    for (w, h, panel) in [
        (1280u32, 720u32, [40.0f32, 40.0, 440.0, 240.0]),
        (1920, 1080, [0.0, 0.0, 1920.0, 1080.0]),
        (3840, 2160, [0.0, 0.0, 3840.0, 2160.0]),
    ] {
        let Some(mut g) = Gpu::headless(w, h) else {
            println!("no adapter — skipping");
            return;
        };
        // **The filter's own cost**, by subtracting the same frame
        // without the glass command — `benches --bin glass-cost`'s
        // method, so the two tables can be read together.
        let list = scene(w, h, Some(panel));
        let bare = scene(w, h, None);
        let mut c = Canvas::new(w, h);
        let mut soft = |l: &DisplayList| {
            best_of(5, || {
                c.clear(Color(250, 250, 250, 255));
                Renderer::render(&mut c, l, w, h);
            })
        };
        let cpu = soft(&list) - soft(&bare);
        // Warm: the first frame compiles the pipeline and allocates.
        g.clear(Color(250, 250, 250, 255));
        g.render(&list, w, h);
        // **`wait`, or this times the queue submission.** `draw`
        // returns as soon as the work is *queued* — which is the whole
        // point of a GPU — so without the poll every row below reads the
        // same 0.03 ms whatever the resolution, and the first draft
        // reported a 27687x speedup at 4K from exactly that.
        let mut hard = |l: &DisplayList| {
            best_of(20, || {
                g.clear(Color(250, 250, 250, 255));
                g.render(l, w, h);
                g.wait();
            })
        };
        let gpu = hard(&list) - hard(&bare);
        let area = (panel[2] - panel[0]) * (panel[3] - panel[1]);
        println!(
            "{:>16} {cpu:>12.2} {gpu:>12.2} {:>9.0}x",
            format!("{:.0}x{:.0} @{w}x{h}", panel[2] - panel[0], panel[3] - panel[1]),
            cpu / gpu
        );
        let _ = area;
    }
}
