//! **Can a persistent worker pool beat the scoped spawn?** — M50/T50.4,
//! the probe `docs/plan-raster-allocation.md` §2.2 orders before (A) is
//! built, judged by §5's kill rule.
//!
//! `render_banded` opens `std::thread::scope` and spawns its workers on
//! every banded frame: 3w + 1 allocations a frame on the frame thread
//! (`tests/alloc_gate_threads.rs`). A pool created once cannot borrow the
//! frame's `&DisplayList` or its pixel rows — they are not `'static` —
//! and the shell crate forbids `unsafe`, so the usual lifetime-erasing
//! pool is not available. What is left is (A): owned inputs. Each banded
//! frame the pool must **copy the damaged rows out** to buffers it owns
//! (a band blends over what is there, so it needs the current pixels),
//! let the workers render into them, and **copy them back**, plus copy the
//! command list. Its saving is the spawns.
//!
//! So this measures, at 1080p, eight interleaved runs:
//!
//! - the scoped banded frame as shipped, and its run-to-run spread — the
//!   kill rule's yardstick;
//! - what the spawns cost on their own: a scope that spawns the same
//!   number of workers and does nothing;
//! - what (A) cannot avoid: two copies of the banded rows.
//!
//! (A)'s overhead is at least `copies - spawns`. If that exceeds the
//! scoped frame's spread, (A) is slower by more than the spread whatever
//! else it does well, and §5 says ship (B), the exact budget.
//!
//! Run: `just bench-lock cargo run --release -p benches --features threads --bin banded_pool`

use std::hint::black_box;
use std::time::Instant;

use fulgurite::display::{Color, DisplayList};
use fulgurite::Renderer;
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::FontStack;

const W: u32 = 1920;
const H: u32 = 1080;

/// The scene `tests/alloc_gate_threads.rs` renders: rows of cards with a
/// label each.
fn scene() -> DisplayList {
    let mut l = DisplayList::new();
    l.rect([0.0, 0.0, W as f32, H as f32], Color(245, 245, 250, 255));
    let mut y = 4.0;
    let mut k = 0;
    while y + 24.0 < H as f32 {
        l.rounded_rect([8.0, y, W as f32 - 8.0, y + 22.0], Color(220, 225, 240, 255), 4.0);
        l.text([16.0, y + 2.0, W as f32 - 16.0, y + 20.0], &format!("row {k}: the quick brown fox"), 14.0, Color(20, 20, 30, 255));
        y += 26.0;
        k += 1;
    }
    l
}

fn median_of(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    v[v.len() / 2]
}

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/fulgurite-shell/tests/fonts/DejaVuSans.ttf");
    let face = FontStack::from_bytes(&std::fs::read(path).expect("the pinned face"), None).expect("parses");
    let list = scene();
    let mut canvas = Canvas::with_font(W, H, face);
    let workers = canvas.bands_for_list(&list, None);
    assert!(workers >= 2, "setup: a 1080p full redraw bands into {workers}");
    for _ in 0..20 {
        canvas.render(&list, W, H);
    }
    let bytes = (W * H * 4) as usize;
    let mut own = vec![0u8; bytes];

    const ITERS: usize = 50;
    let (mut frame, mut spawn, mut copy) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..8 {
        let t = Instant::now();
        for _ in 0..ITERS {
            canvas.render(&list, W, H);
        }
        frame.push(t.elapsed().as_secs_f64() * 1e6 / ITERS as f64);

        let t = Instant::now();
        for _ in 0..ITERS {
            std::thread::scope(|sc| {
                for _ in 0..workers {
                    sc.spawn(|| black_box(0u8));
                }
            });
        }
        spawn.push(t.elapsed().as_secs_f64() * 1e6 / ITERS as f64);

        let t = Instant::now();
        for _ in 0..ITERS {
            own.copy_from_slice(&canvas.pixels);
            black_box(&mut own);
            canvas.pixels.copy_from_slice(&own);
            black_box(&mut canvas.pixels);
        }
        copy.push(t.elapsed().as_secs_f64() * 1e6 / ITERS as f64);
    }
    let lo = frame.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = frame.iter().cloned().fold(0.0, f64::max);
    let (f, s, c) = (median_of(frame.clone()), median_of(spawn.clone()), median_of(copy.clone()));
    println!("banded_pool: {W}x{H} full redraw, {workers} workers, 8 runs x {ITERS} frames, us/frame\n");
    println!("scoped banded frame   median {f:8.1}   runs {:?}", frame.iter().map(|v| v.round()).collect::<Vec<_>>());
    println!("  spread (max - min)         {:8.1}", hi - lo);
    println!("spawns alone          median {s:8.1}   runs {:?}", spawn.iter().map(|v| v.round()).collect::<Vec<_>>());
    println!("two row copies        median {c:8.1}   runs {:?}", copy.iter().map(|v| v.round()).collect::<Vec<_>>());
    let floor = c - s;
    println!("\n(A)'s overhead is at least copies - spawns = {floor:.1} us; kill rule: (A) dies if that exceeds the spread {:.1} us", hi - lo);
    println!("verdict: {}", if floor > hi - lo { "KILL (A), ship (B)" } else { "(A) survives the floor; build and measure it" });
}
