//! T6.4: what a binding costs per frame, and what it saves.
//!
//! The design runs the formatter on every frame and skips only the
//! *write* when the output is unchanged (`crates/lumen-static/src/bind.rs`).
//! That is only sound if formatting-and-comparing is far cheaper than the
//! measurement it protects. This measures both sides of that claim.
//!
//! Run: `cargo run --release -p benches --bin bindings`

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
    println!("bindings: per-frame cost of a binding, and the measurement it guards\n");

    let style = [Style::default()];
    let mut st = [NodeState::default()];

    // --- one binding, value unchanged: the common case ---
    let mut l: Label<String> = Label::new("count: 41".into());
    measure(&mut l, &style, &mut st);
    bench("1 binding, unchanged", 20_000, || {
        format_into(&mut l, format_args!("count: {}", 41)) as u64
    });

    // --- one binding, value changed every frame: the worst case ---
    let mut l2: Label<String> = Label::new(String::new());
    let mut n = 0u64;
    bench("1 binding, changed", 20_000, || {
        n += 1;
        format_into(&mut l2, format_args!("count: {n}")) as u64
    });

    // --- the same update with no compare at all: always write ---
    // This is the arm that says whether comparing pays for itself before
    // any shaping saving is counted.
    let mut l5: Label<String> = Label::new("count: 41".into());
    let mut buf = String::with_capacity(64); // reused, as format_into's is
    bench("1 update, no compare (write)", 20_000, || {
        use core::fmt::Write;
        buf.clear();
        let _ = buf.write_fmt(format_args!("count: {}", 41));
        l5.set_str(&buf);
        l5.as_str().len() as u64
    });

    // --- what the unchanged case avoids: re-measuring the label ---
    let mut l3: Label<String> = Label::new("count: 41".into());
    measure(&mut l3, &style, &mut st);
    bench("layout, memo warm", 20_000, || measure(&mut l3, &style, &mut st));

    let mut l4: Label<String> = Label::new("count: 41".into());
    bench("layout, memo cold (re-measure)", 20_000, || {
        l4.set_str("count: 41 "); // force dirty; same length class
        measure(&mut l4, &style, &mut st)
    });

    // --- how it scales: a screenful of bindings, none of them changing ---
    for count in [10usize, 100, 1000] {
        let mut labels: Vec<Label<String>> =
            (0..count).map(|i| Label::new(format!("row {i}: 41"))).collect();
        bench(&format!("{count} bindings, unchanged"), 2_000, || {
            let mut c = 0u64;
            for (i, l) in labels.iter_mut().enumerate() {
                c += format_into(l, format_args!("row {i}: {}", 41)) as u64;
            }
            c
        });
    }

    println!(
        "\nBlockMetrics is a stand-in measurer (char count x advance); a real\n\
         shaper is orders of magnitude slower, so the warm/cold gap below is a\n\
         *lower* bound on what the unchanged case saves."
    );
}
