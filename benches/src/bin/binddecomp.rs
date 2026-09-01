//! Decomposition: is the cost in `format_into` the *comparison*, or the
//! thread-local scratch buffer it uses? Batched, because the difference
//! is single-digit nanoseconds and per-iteration timing cannot see it.
use core::fmt::Write;
use std::hint::black_box;

use lumen_static::bind::format_into;
use lumen_static::ops::Label;

/// N inner iterations inside one timed window; median of `blocks`.
fn batched(label: &str, blocks: usize, n: usize, mut f: impl FnMut() -> u64) {
    for _ in 0..3 {
        for _ in 0..n { black_box(f()); }
    }
    let mut s: Vec<f64> = Vec::with_capacity(blocks);
    for _ in 0..blocks {
        let t = std::time::Instant::now();
        for _ in 0..n { black_box(f()); }
        s.push(t.elapsed().as_nanos() as f64 / n as f64);
    }
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{label:<52} {:>7.2} ns/op   (min {:>6.2}  max {:>6.2})",
        s[s.len()/2], s[0], s[s.len()-1]);
}

fn main() {
    black_box(());
    println!("binding decomposition: 20k iters/block, 21 blocks, median\n");

    // A: as shipped — thread-local scratch + compare + skip write.
    let mut a: Label<String> = Label::new("count: 41".into());
    batched("A  format_into        (TLS + compare + skip)", 21, 20_000,
        || format_into(&mut a, format_args!("count: {}", 41)) as u64);

    // B: no TLS, no compare — always write.
    let mut b: Label<String> = Label::new("count: 41".into());
    let mut bb = String::with_capacity(64);
    batched("B  local buf, always write   (no TLS, no compare)", 21, 20_000, || {
        bb.clear();
        let _ = bb.write_fmt(format_args!("count: {}", 41));
        b.set_str(&bb);
        b.as_str().len() as u64
    });

    // C: no TLS, but compare and skip. The compare, isolated.
    let mut c: Label<String> = Label::new("count: 41".into());
    let mut cb = String::with_capacity(64);
    batched("C  local buf, compare then skip  (no TLS)", 21, 20_000, || {
        cb.clear();
        let _ = cb.write_fmt(format_args!("count: {}", 41));
        if c.as_str() != cb.as_str() { c.set_str(&cb); }
        c.as_str().len() as u64
    });

    println!("\n  C - B  = the comparison alone      (negative => comparing is cheaper than writing)");
    println!("  A - C  = the thread-local scratch alone");
}
