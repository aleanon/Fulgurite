//! What the harness itself costs. Any result within a few multiples of
//! the floor below is the timer, not the workload.
use std::hint::black_box;
use benches::bench;

fn main() {
    black_box(());
    println!("calibration: the harness measuring (nearly) nothing\n");
    bench("empty closure", 20_000, || 0);
    bench("empty closure", 20_000, || 0);
    let mut n = 0u64;
    bench("one wrapping_add", 20_000, || { n = n.wrapping_add(1); n });

    // Cost of the timer pair itself, measured in bulk.
    let iters = 200_000;
    let t = std::time::Instant::now();
    for _ in 0..iters {
        let t0 = std::time::Instant::now();
        black_box(t0.elapsed());
    }
    println!("\nInstant::now()+elapsed() pair, batched: {:.1} ns",
        t.elapsed().as_nanos() as f64 / iters as f64);
}
