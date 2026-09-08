//! What the wide-store rewrite is worth — M17/T17.0.
//!
//! The four per-pixel write paths, against the cheapest thing that
//! touches the same bytes. `fill` and `copy_from_slice` are the floors:
//! a clear cannot beat a memset and a blit cannot beat a copy, so the
//! distance to them is the headroom that remains.
//!
//! Every buffer and pattern is `black_box`ed. The first version of this
//! measurement filled with a *constant* colour and never read the
//! result, so LLVM deleted the loop and reported 0.000 ms — an 89 000x
//! speed-up that did not exist.

use std::hint::black_box;
use std::time::Instant;

use lumen_static::display::Color;
use lumen_static_shell::software::Canvas;

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
    println!("stores: per-pixel write paths, min of 300\n");
    println!("{:<28}{:>12}{:>12}{:>10}", "path", "1080p (ms)", "floor (ms)", "vs floor");

    for (w, h, label) in [(1920u32, 1080u32, "1080p"), (3840, 2160, "4K")] {
        let n = (w * h) as usize;
        let col = Color(20, 24, 34, 255);
        let cb = [col.0, col.1, col.2, col.3];

        let mut canvas = Canvas::new(w, h);
        let clear = best(300, || {
            canvas.clear(black_box(col));
            black_box(canvas.pixels[0]);
        });
        let mut u32buf = vec![0u32; n];
        let pat = u32::from_le_bytes(cb);
        let memset = best(300, || {
            black_box(&mut u32buf).fill(black_box(pat));
            black_box(u32buf[0]);
        });

        for (i, px) in canvas.pixels.chunks_exact_mut(4).enumerate() {
            px.copy_from_slice(&[(i % 251) as u8, (i % 241) as u8, (i % 239) as u8, 255]);
        }
        let mut dst = vec![0u32; n];
        let blit = best(300, || {
            canvas.blit_0rgb(black_box(&mut dst));
            black_box(dst[0]);
        });
        let src = vec![0u32; n];
        let mut d2 = vec![0u32; n];
        let memcpy = best(300, || {
            black_box(&mut d2).copy_from_slice(black_box(&src));
            black_box(d2[0]);
        });

        println!("{:<28}{clear:>12.3}{memset:>12.3}{:>9.2}x", format!("clear ({label})"), clear / memset);
        println!("{:<28}{blit:>12.3}{memcpy:>12.3}{:>9.2}x", format!("blit_0rgb ({label})"), blit / memcpy);
    }
    println!("\n   the frame-level effect is in docs/results-wide-stores.md;");
    println!("   `just profile` reports it per stage on a real tally frame");
}
