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

/// Any leaf's layout against a **real** face — `measure_real` below, for
/// the two text widgets that carry no memo at all (M26/T26.0).
///
/// Real rather than the stand-in for the reason `docs/bindings-decision.md`
/// gives: under `BlockMetrics` a measurement is `char_count × advance`, so
/// a widget that reshapes every layout and one that never does cost the
/// same, and the first version of this arm duly printed 4.2 ns for the
/// widget whose missing memo it exists to price.
fn measure_node<N: Node>(
    w: &mut N,
    m: &mut dyn lumen_static::TextMeasure,
    style: &[Style],
    st: &mut [NodeState],
    limits: Constraints,
) -> u64 {
    let mut cx = LayoutCx::new(m);
    w.layout(&mut cx, limits, style, st, 0).w as u64
}

/// The same, against a real face (T7.3). `measure` above answers what
/// the stand-in costs, which is not the question the memo exists for.
fn measure_real(
    l: &mut Label<String>,
    m: &mut dyn lumen_static::TextMeasure,
    style: &[Style],
    st: &mut [NodeState],
    limits: Constraints,
) -> u64 {
    let mut cx = LayoutCx::new(m);
    l.layout(&mut cx, limits, style, st, 0).w as u64
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

    // ---- T7.3: what the memo is worth against a real face ----
    //
    // Four numbers, as the task specifies: hit and miss, for a short
    // unwrapped label and a 60-char label wrapped at 200 px. The bench
    // installs `FontMetrics` — running the arms above against
    // `BlockMetrics` measures char_count x advance, which is not what a
    // memo protects.
    //
    // fontdue has no content-keyed shaping cache of its own, so a miss
    // here is a real re-layout: the number is the honest upper bound,
    // and the T7.3 trap about warm-shaper caches does not apply
    // (docs/text-render-adoption-decision.md).
    println!("\n-- T7.3: the memo against a real face (fontdue) --");
    match lumen_static_shell::text::FontStack::system() {
        None => println!("   no system font found; skipped"),
        Some(stack) => {
            let mut real = lumen_static_shell::text::FontMetrics::new(stack);
            let short = "count: 41";
            // Exactly 60 characters, as T7.3 specifies.
            let long: String = "the quick brown fox jumps over the lazy dog and then some mo".into();
            assert_eq!(long.chars().count(), 60);
            let wrapped =
                Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 1080.0 };

            // Warm the face: fontdue rasterizes and caches per (glyph,
            // px) inside `Font`, so the first call for a size pays for
            // the whole alphabet. >=100 prior calls, as specified.
            for _ in 0..100 {
                let mut w: Label<String> = Label::new(short.into());
                measure_real(&mut w, &mut real, &style, &mut st, LIMITS);
                let mut w2: Label<String> = Label::new(long.clone());
                measure_real(&mut w2, &mut real, &style, &mut st, wrapped);
            }

            let mut a: Label<String> = Label::new(short.into());
            measure_real(&mut a, &mut real, &style, &mut st, LIMITS);
            bench("short, memo warm (hit)", 20_000, || {
                measure_real(&mut a, &mut real, &style, &mut st, LIMITS)
            });

            let mut b: Label<String> = Label::new(short.into());
            let mut f = false;
            bench("short, memo cold (miss)", 20_000, || {
                f = !f;
                b.set_str(if f { "count: 41" } else { "count: 42" });
                measure_real(&mut b, &mut real, &style, &mut st, LIMITS)
            });

            let mut c: Label<String> = Label::new(long.clone());
            measure_real(&mut c, &mut real, &style, &mut st, wrapped);
            bench("60-char wrapped, memo warm (hit)", 20_000, || {
                measure_real(&mut c, &mut real, &style, &mut st, wrapped)
            });

            // ---- Badge and Chip: no memo at all — M26/T26.0 -----------
            //
            // Here, inside the real-face arm, and not beside the
            // stand-in rows above: these two carry no `Measured`, so
            // their unchanged row *is* a miss, and only a real face can
            // show that. M26/T26.8's baseline — after it, these must fall
            // to the hit cost printed above.
            let mut badge: lumen_static_widgets::Badge<String> =
                lumen_static_widgets::Badge::default();
            badge.set(short.into());
            let bstyle = [badge.defaults()];
            let mut chip: lumen_static_widgets::Chip<String> =
                lumen_static_widgets::Chip::default();
            chip.set(short.into());
            let cstyle = [chip.defaults()];
            // Warm the face at these widgets' own font sizes, as the
            // loop above warms it at the labels'.
            for _ in 0..100 {
                measure_node(&mut badge, &mut real, &bstyle, &mut st, LIMITS);
                measure_node(&mut chip, &mut real, &cstyle, &mut st, LIMITS);
            }
            bench("Badge layout, unchanged (no memo)", 20_000, || {
                measure_node(&mut badge, &mut real, &bstyle, &mut st, LIMITS)
            });
            bench("Chip layout, unchanged (no memo)", 20_000, || {
                measure_node(&mut chip, &mut real, &cstyle, &mut st, LIMITS)
            });
            println!(
                "   against `short, memo warm (hit)` above: the gap is what M26/T26.8 removes"
            );

            // Same length, so the miss arm re-measures rather than
            // measuring a different amount of text.
            let long2 = long.replace("dog", "cat");
            assert_eq!(long2.chars().count(), 60);
            let mut d: Label<String> = Label::new(long.clone());
            let mut g = false;
            bench("60-char wrapped, memo cold (miss)", 20_000, || {
                g = !g;
                d.set_str(if g { &long } else { &long2 });
                measure_real(&mut d, &mut real, &style, &mut st, wrapped)
            });
            println!("   miss - hit = what one memoized label saves per frame");
        }
    }

    println!(
        "\nBlockMetrics is char_count x advance; the real-face arms above are\n\
         what the memo exists for. See docs/bindings-decision.md."
    );
}
