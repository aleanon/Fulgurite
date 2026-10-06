//! `--bin markdown`: a one-character append to a Markdown document of a
//! size given in KB, then `Markdown::set_str` and a layout under
//! `FontStack` — M87/T87.1, judged by T87.6 (`docs/plan-markdown.md`, kill
//! rule K87; `docs/results-markdown.md` owns the numbers).
//!
//! `cargo run --release -p benches --bin markdown -- 1 16 64` prints, per
//! size, the median and minimum of the appends it times (whole: parse +
//! measure + place), and the three split quantities' medians:
//!
//! - **parse** — `set_str` over the whole document, which is what the
//!   editor does on every keystroke;
//! - **measure** — the first layout after the parse minus the place-only
//!   layout below. Until T87.6 the memo keyed the whole text (`RichText`'s
//!   `fold`), so an append re-measured every word through `FontStack` —
//!   88% of the append, and K87 fired on it. Since T87.6 a miss of that
//!   fold re-measures only the words whose content key changed
//!   (`RichText::remeasure`), so what is left here is the key comparison
//!   and the one word the append touched;
//! - **place** — a second layout of the same text, memo warm: the line
//!   breaker, plus the fold of the whole text that decides the memo is
//!   warm.
//!
//! **The setup-did-not-happen check comes first**: the span count must
//! equal the document's word count (printed), or no row is printed. Before
//! T87.1 the span store kept 192 words, and a timing of it timed a
//! truncated parse.

use std::time::Instant;

use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, LayoutCx, NodeState, Point};
use lumen_static_shell::text::{FontMetrics, FontStack};
use lumen_static_widgets::Markdown;

/// Lines of Markdown: headings, bullets, bold and code words, prose. No
/// blank lines, so every span is a word (a blank line is an empty span).
fn doc(bytes: usize) -> String {
    let lines = [
        "# A heading over the section\n",
        "lorem ipsum **dolor** sit amet `consectetur` adipiscing elit sed\n",
        "- do eiusmod tempor incididunt ut labore et dolore magna\n",
        "aliqua ut enim ad minim veniam quis nostrud exercitation\n",
    ];
    let mut s = String::with_capacity(bytes + 80);
    'fill: loop {
        for l in lines {
            if s.len() >= bytes {
                break 'fill;
            }
            s.push_str(l);
        }
    }
    s
}

/// Words a reader sees: whitespace-separated tokens less the line markers.
fn words(src: &str) -> usize {
    src.split_whitespace().filter(|w| !matches!(*w, "#" | "##" | "###" | "-")).count()
}

fn lay(m: &mut Markdown, metrics: &mut FontMetrics, styles: &[lumen_static::Style], st: &mut [NodeState]) {
    let limits = Constraints { min_w: 0.0, max_w: 640.0, min_h: 0.0, max_h: f32::INFINITY };
    {
        let mut cx = LayoutCx::new(metrics);
        Node::layout(m, &mut cx, limits, styles, st, 0);
    }
    Node::place(m, Point { x: 0.0, y: 0.0 }, styles, st, 0);
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let Some(font) = FontStack::system() else {
        println!("no system face; skipped");
        return;
    };
    let sizes: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let sizes = if sizes.is_empty() { vec![1, 16, 64] } else { sizes };
    for &kb in &sizes {
        let mut src = doc(kb * 1024);
        let mut m = Markdown::new(&src);
        let mut metrics = FontMetrics::new(font.clone());
        let styles = lumen_static::pass::default_styles(&m);
        let mut st = vec![NodeState::default(); <Markdown as Widget>::NODE_COUNT];
        lay(&mut m, &mut metrics, &styles, &mut st);

        // Setup: every word is a span, or the timing is of a truncated parse.
        let (spans, want) = (m.rich().spans().len(), words(&src));
        println!("markdown, {kb} KB: setup {spans} spans of {want} words (bound: equal)");
        assert_eq!(spans, want, "setup: the span store did not keep every word, so no row is printed");

        let n = 21;
        let (mut whole, mut parse, mut measure, mut place) = (vec![], vec![], vec![], vec![]);
        for _ in 0..n {
            src.push('x');
            let t0 = Instant::now();
            m.set_str(&src);
            let t1 = Instant::now();
            lay(&mut m, &mut metrics, &styles, &mut st);
            let t2 = Instant::now();
            lay(&mut m, &mut metrics, &styles, &mut st);
            let t3 = Instant::now();
            let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1e3;
            let (p, l, w) = (ms(t0, t1), ms(t1, t2), ms(t2, t3));
            whole.push(p + l);
            parse.push(p);
            measure.push((l - w).max(0.0));
            place.push(w);
        }
        assert_eq!(m.rich().text().len(), src.len(), "setup: the appends did not land");
        let min = whole.iter().copied().fold(f64::INFINITY, f64::min);
        println!(
            "append + set_str + layout, {kb} KB: median {:.3} ms  min {min:.3} ms  ({n} appends; \
             split medians: parse {:.3} ms, measure {:.3} ms, place {:.3} ms)",
            median(&mut whole),
            median(&mut parse),
            median(&mut measure),
            median(&mut place)
        );
    }
}
