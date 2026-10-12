//! `--bin textarea`: one keystroke plus layout in a `TextArea`, at a
//! document size given on the command line — M45/T45.10
//! (`docs/plan-text-correctness.md`, `docs/results-text.md` T45.10).
//!
//! `cargo run --release -p benches --bin textarea -- 1 100 1024` prints,
//! per size in KB, the median and minimum of the keystrokes it times.
//! The whole document is reshaped per edit — linear, and what §5's kill
//! rule weighs against a 16.7 ms frame; the two quadratics T45.10 removed
//! are what made it superlinear.
//!
//! **`--shaping basic|complex`** (M81/T81.3, kill rule K81a,
//! `docs/plan-shaped-paint.md`): new rows beside the one above, which is
//! K3's arm and is computed first and unchanged — "keystroke + layout +
//! paint \[mode\]", the same keystroke laid out and then rendered into a
//! truetype `Canvas`, on a stack in that mode. `complex` exists only with
//! the benches `shaping` feature. K81a weighs `[complex]` against
//! `[basic]` at 1.5×; the setup line prints the runs the shaper laid out
//! (> 0 under `complex`).

use std::time::Instant;

use fulgurite::{App as DeriveApp, Event, Key, StaticTree};
use fulgurite::{Color, Renderer};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::text::{text_counters, FontMetrics, FontStack, Shaping};
use fulgurite_shell::Host;
use fulgurite_widgets::TextArea;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("area")]
    area: TextArea,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn text(bytes: usize) -> String {
    let words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor ";
    let mut s = String::with_capacity(bytes + words.len());
    while s.len() < bytes {
        s.push_str(words);
    }
    s
}

fn main() {
    let Some(font) = FontStack::system() else {
        println!("no system face; skipped");
        return;
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.iter().position(|a| a == "--shaping").map(|i| match args.get(i + 1).map(String::as_str) {
        Some("basic") => (Shaping::Basic, "basic"),
        #[cfg(feature = "shaping")]
        Some("complex") => (Shaping::Complex, "complex"),
        other => panic!("--shaping takes basic or complex (complex needs the `shaping` feature), not {other:?}"),
    });
    let sizes: Vec<usize> = args.iter().filter_map(|a| a.parse().ok()).collect();
    let sizes = if sizes.is_empty() { vec![1, 100, 1024] } else { sizes };
    for &kb in &sizes {
        let mut a = App::default();
        a.ui.area.set_str(&text(kb * 1024));
        let mut m = FontMetrics::new(font.clone());
        let mut h: Host<App> = Host::new(a, 480, 400);
        h.pump_with(&mut m);
        h.post(Event::Key { key: Key::Tab, down: true });
        h.pump_with(&mut m);
        let n = if kb >= 512 { 5 } else { 21 };
        let mut t: Vec<f64> = (0..n)
            .map(|_| {
                h.post(Event::Text { ch: 'x' });
                let s = Instant::now();
                h.pump_with(&mut m);
                s.elapsed().as_secs_f64() * 1e3
            })
            .collect();
        t.sort_by(f64::total_cmp);
        println!("keystroke + layout, {kb} KB: median {:.3} ms  min {:.3} ms  ({n} keystrokes)", t[n / 2], t[0]);
    }
    if let Some((shaping, name)) = mode {
        let font = font.with_shaping(shaping);
        for &kb in &sizes {
            paint_row(&font, name, kb);
        }
    }
}

/// One "keystroke + layout + paint \[mode\]" row (M81/T81.3): the keystroke
/// arm above, then the frame rendered into a truetype canvas over the
/// same stack, timed together.
fn paint_row(font: &FontStack, name: &str, kb: usize) {
    let mut a = App::default();
    a.ui.area.set_str(&text(kb * 1024));
    let mut m = FontMetrics::new(font.clone());
    let mut h: Host<App> = Host::new(a, 480, 400);
    let mut canvas = Canvas::with_font(480, 400, font.clone());
    h.pump_with(&mut m);
    h.post(Event::Key { key: Key::Tab, down: true });
    h.pump_with(&mut m);
    canvas.render(&h.list, 480, 400);
    let n = if kb >= 512 { 5 } else { 21 };
    let before = text_counters();
    let mut t: Vec<f64> = (0..n)
        .map(|_| {
            h.post(Event::Text { ch: 'x' });
            let s = Instant::now();
            h.pump_with(&mut m);
            canvas.clear(Color(255, 255, 255, 255));
            canvas.render(&h.list, 480, 400);
            s.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    let after = text_counters();
    t.sort_by(f64::total_cmp);
    println!(
        "keystroke + layout + paint [{name}], {kb} KB: median {:.3} ms  min {:.3} ms  ({n} keystrokes; \
         setup: {} runs through the shaper, {} painted)",
        t[n / 2],
        t[0],
        after.complex - before.complex,
        after.shaped - before.shaped
    );
}
