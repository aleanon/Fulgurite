//! `--bin textarea`: one keystroke plus layout in a `TextArea`, at a
//! document size given on the command line — M45/T45.10
//! (`docs/plan-text-correctness.md`, `docs/results-text.md` T45.10).
//!
//! `cargo run --release -p benches --bin textarea -- 1 100 1024` prints,
//! per size in KB, the median and minimum of the keystrokes it times.
//! The whole document is reshaped per edit — linear, and what §5's kill
//! rule weighs against a 16.7 ms frame; the two quadratics T45.10 removed
//! are what made it superlinear.

use std::time::Instant;

use lumen_static::{App as DeriveApp, Event, Key, StaticTree};
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_shell::Host;
use lumen_static_widgets::TextArea;

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
    let sizes: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let sizes = if sizes.is_empty() { vec![1, 100, 1024] } else { sizes };
    for kb in sizes {
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
}
