//! What a binding costs per frame, decomposed — and what it guards.
//!
//! The design runs the formatter every frame and skips only the *write*
//! when the output is unchanged (`crates/lumen-static/src/bind.rs`). The
//! question "does the comparison pay for itself?" is answered by arm C
//! against arm B below, with everything else held constant. The
//! thread-local scratch buffer is isolated separately (A against C),
//! because the first version of this bench conflated the two and drew
//! the wrong conclusion (`docs/bindings-decision.md`).
//!
//! Run: `cargo run --release -p benches --bin bindings`

use core::fmt::Write;
use std::hint::black_box;

use benches::bench;
use lumen_static::bind::format_into;
use lumen_static::ops::Label;
use lumen_static::{BlockMetrics, Constraints, LayoutCx, Node, NodeState, Style};

const LIMITS: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 1080.0 };

/// One label's layout — the work a warm memo skips.
fn measure(l: &mut Label<String>, style: &[Style], st: &mut [NodeState]) -> u64 {
    let mut m = BlockMetrics;
    let mut cx = LayoutCx::new(&mut m);
    l.layout(&mut cx, LIMITS, style, st, 0).w as u64
}

fn main() {
    black_box(());
    println!("bindings: per-frame cost of a binding, decomposed\n");

    // ---- the decomposition: same format, same label, three paths ----
    // The value goes through black_box so the formatter actually runs;
    // a literal 41 lets the compiler fold the whole format_args away and
    // the arms measure a memcmp, not a binding.
    println!("-- unchanged value, one 9-char label --");
    let v = black_box(41);

    // A: as shipped — thread-local scratch + compare + skip write.
    let mut a: Label<String> = Label::new("count: 41".into());
    bench("A  format_into (TLS + compare + skip)", 200_000, || {
        format_into(&mut a, format_args!("count: {}", v)) as u64
    });

    // B: local buffer, no compare — always write. What "no binding
    // machinery" would cost if the update step just wrote every frame.
    let mut b: Label<String> = Label::new("count: 41".into());
    let mut bb = String::with_capacity(64);
    bench("B  local buf, always write", 200_000, || {
        bb.clear();
        let _ = bb.write_fmt(format_args!("count: {}", v));
        b.set_str(&bb);
        b.as_str().len() as u64
    });

    // C: local buffer, compare then skip. The comparison, isolated.
    let mut c: Label<String> = Label::new("count: 41".into());
    let mut cb = String::with_capacity(64);
    bench("C  local buf, compare then skip", 200_000, || {
        cb.clear();
        let _ = cb.write_fmt(format_args!("count: {}", v));
        if c.as_str() != cb.as_str() {
            c.set_str(&cb);
        }
        c.as_str().len() as u64
    });
    println!("   C - B = the comparison alone (negative: cheaper than writing)");
    println!("   A - C = the thread-local scratch alone\n");

    // ---- the changed case, with a *fixed* workload ----
    // The first version let the value run 1..20000, so the string grew
    // across digit-count and reallocation boundaries mid-run; that arm
    // had the widest spread in the table and it was the workload, not
    // the machine. Alternate between two same-length values instead.
    println!("-- changed value every frame --");
    let mut d: Label<String> = Label::new(String::new());
    let mut flip = false;
    bench("D  format_into, value alternates 41/42", 200_000, || {
        flip = !flip;
        format_into(&mut d, format_args!("count: {}", if flip { 41 } else { 42 })) as u64
    });

    // ---- what the memo guards ----
    println!("\n-- the measurement the memo protects (BlockMetrics stand-in) --");
    let style = [Style::default()];
    let mut st = [NodeState::default()];
    let mut w: Label<String> = Label::new("count: 41".into());
    measure(&mut w, &style, &mut st);
    bench("layout, memo warm", 200_000, || measure(&mut w, &style, &mut st));

    let mut cold: Label<String> = Label::new("count: 41".into());
    let mut flip2 = false;
    bench("layout, memo cold (re-measure)", 200_000, || {
        flip2 = !flip2;
        cold.set_str(if flip2 { "count: 41" } else { "count: 42" });
        measure(&mut cold, &style, &mut st)
    });

    // ---- scale ----
    println!("\n-- a screenful of bindings, none changing --");
    let v = black_box(41);
    for count in [10usize, 100, 1000] {
        let mut labels: Vec<Label<String>> =
            (0..count).map(|i| Label::new(format!("row {i}: 41"))).collect();
        bench(&format!("{count} bindings, unchanged"), 21 * 200, || {
            let mut n = 0u64;
            for (i, l) in labels.iter_mut().enumerate() {
                n += format_into(l, format_args!("row {i}: {}", v)) as u64;
            }
            n
        });
    }

    println!(
        "\nBlockMetrics is char_count x advance; a real shaper is what the memo\n\
         exists for and is not measured here. See docs/bindings-decision.md."
    );
}
