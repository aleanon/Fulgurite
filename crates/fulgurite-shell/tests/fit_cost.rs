//! **What one cut shapes** — M80/T80.3, kill rule K80d
//! (`docs/plan-text-fit.md`).
//!
//! `fulgurite_widgets::fit::cut` is a binary search: it measures the
//! first line, the `…` and one prefix per halving of the line's byte
//! length, so at most `⌈log2(len)⌉ + 3` runs per cut. This counts them
//! with `text_counters().measured` — a count, so it answers the same
//! under load — across T80.0's corpus titles and longer ones, at every
//! room that cuts, and prints the worst per cut against the bound.
//!
//! **One test in its own binary**: the counters are process-wide and
//! `text.rs` asks for one test at a time, and cargo runs a binary's tests
//! in parallel.
#![cfg(feature = "truetype")]

use fulgurite_shell::text::{text_counters, FontMetrics, FontStack};
use fulgurite_widgets::fit;

#[path = "fonts/pinned.rs"]
mod pinned;

const CLUSTERS: [&str; 4] =
    ["e\u{301}", "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}", "\u{1f1f3}\u{1f1f4}", "\u{1f44d}\u{1f3fd}"];

#[test]
fn a_cut_measures_at_most_log2_len_plus_three_runs() {
    let font = FontStack::from_bytes(pinned::bytes(), None).expect("the pinned face parses");
    let mut m = FontMetrics::new(font);
    let mut titles: Vec<String> =
        CLUSTERS.iter().map(|c| format!("WWWWWW{c}WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW")).collect();
    titles.push("The quick brown fox jumps over the lazy dog ".repeat(8));
    titles.push("Supercalifragilisticexpialidocious".repeat(12));
    titles.push("A short title".to_owned());
    let (mut cuts, mut over) = (0usize, Vec::new());
    for t in &titles {
        let bound = (t.len() as f64).log2().ceil() as u64 + 3;
        let mut worst = 0u64;
        for room in (8..1200).step_by(7) {
            for mark in [true, false] {
                let before = text_counters().measured;
                let cut = fit::cut(&mut m, t, 16.0, 400.0, room as f32, mark);
                let n = text_counters().measured - before;
                if cut.is_some() {
                    cuts += 1;
                    worst = worst.max(n);
                    if n > bound {
                        over.push(format!("{} bytes at {room} px (mark {mark}): {n} runs > {bound}", t.len()));
                    }
                }
            }
        }
        println!("{} bytes: worst {worst} runs measured per cut (bound ceil(log2 len) + 3 = {bound})", t.len());
    }
    println!("{cuts} cuts counted (setup > 0)");
    assert!(cuts > 0, "setup: nothing was cut, so nothing was counted");
    assert!(over.is_empty(), "K80d fires:\n{}", over.join("\n"));
}
