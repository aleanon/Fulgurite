//! Render one frame on both backends and write both, plus their
//! difference, as PPMs — the thing a person looks at when a tolerance
//! number stops being believable.
//!
//! `cargo run -p fulgurite-wgpu --example compare -- /tmp`

use fulgurite::display::{Color, DisplayList, Renderer, Stops};
use fulgurite::Point;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::FontStack;
use fulgurite_wgpu::Gpu;

const W: u32 = 480;
const H: u32 = 320;
const PAGE: Color = Color(250, 250, 250, 255);

fn ppm(w: u32, h: u32, px: &[u8]) -> Vec<u8> {
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in px.chunks_exact(4) {
        out.extend_from_slice(&p[..3]);
    }
    out
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "target".into());
    let font = FontStack::system();
    if font.is_none() {
        eprintln!("no system font — text will be block glyphs on both arms");
    }

    let mut l = DisplayList::new();
    l.rect([20.0, 20.0, 140.0, 80.0], Color(200, 60, 60, 255));
    l.rounded_rect([160.0, 20.0, 300.0, 90.0], Color(40, 90, 200, 255), 16.0);
    l.shadow([180.0, 120.0, 320.0, 200.0], 12.0, 14.0);
    l.rounded_rect([180.0, 120.0, 320.0, 200.0], Color(30, 38, 54, 255), 12.0);
    l.line(Point { x: 24.0, y: 120.0 }, Point { x: 140.0, y: 200.0 }, 4.0, Color(20, 160, 90, 255));
    l.gradient(
        [20.0, 230.0, 220.0, 300.0],
        Stops::two(Color(255, 200, 0, 255), Color(200, 0, 120, 255)),
        fulgurite::display::GradShape::Linear { angle: 0.0 },
        0.0,
    );
    l.text([200.0, 140.0, 320.0, 170.0], "Hamburgefonstiv", 18.0, Color(240, 240, 240, 255));
    l.text([20.0, 95.0, 300.0, 115.0], "the quick brown fox", 15.0, Color(20, 20, 20, 255));
    l.text_weighted([250.0, 240.0, 460.0, 275.0], "BOLD 123", 26.0, 700.0, Color(0, 0, 0, 255));

    let mut c = match font.clone() {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    };
    c.clear(PAGE);
    c.render(&l, W, H);

    let Some(mut g) = Gpu::headless(W, H) else {
        eprintln!("no adapter; wrote the software arm only");
        std::fs::write(format!("{dir}/compare-cpu.ppm"), c.to_ppm()).unwrap();
        return;
    };
    println!("gpu: {}", g.adapter());
    if let Some(f) = font {
        g.set_font(f);
    }
    g.clear(PAGE);
    g.render(&l, W, H);
    let gp = g.read_rgba();

    // The difference, amplified 4x so a one-level disagreement is
    // visible at all.
    let mut diff = Vec::with_capacity(gp.len());
    let (mut worst, mut over) = (0u8, 0usize);
    for (a, b) in c.pixels.chunks_exact(4).zip(gp.chunks_exact(4)) {
        let mut d = 0u8;
        for k in 0..3 {
            d = d.max(a[k].abs_diff(b[k]));
        }
        worst = worst.max(d);
        if d > 8 {
            over += 1;
        }
        let v = 255 - d.saturating_mul(4);
        diff.extend_from_slice(&[v, v, v, 255]);
    }

    std::fs::write(format!("{dir}/compare-cpu.ppm"), c.to_ppm()).unwrap();
    std::fs::write(format!("{dir}/compare-gpu.ppm"), ppm(W, H, &gp)).unwrap();
    std::fs::write(format!("{dir}/compare-diff.ppm"), ppm(W, H, &diff)).unwrap();
    println!(
        "worst channel delta {worst}; {over} px ({:.3}%) differ by more than 8 -> {dir}/compare-*.ppm",
        over as f32 / (W * H) as f32 * 100.0
    );
}
