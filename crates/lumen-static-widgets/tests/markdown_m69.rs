//! **M87/T87.0 — the Markdown gates that compile today**
//! (`docs/plan-markdown.md`). Headless, with `BlockMetrics`, as
//! `tests/rich.rs` is.
//!
//! Every gate here is committed **green** (`docs/plan-iced-roadmap.md`
//! §3.1): gates 1–4 pin today's gap — they assert the current value and
//! print it — and the task that closes each one flips its assertion to
//! the bound written beside it. Gate 5 pins the default display list
//! before `MarkdownTheme` (T87.2) moves anything.
//!
//! | gate | pinned today | flipped by | bound |
//! |---|---|---|---|
//! | 1 `every_word_is_kept` | 192 spans of 1,000 words | T87.1 | spans = words |
//! | 2 `set_str_is_linear` | ratio > 25 | T87.1 | ratio ≤ 25 |
//! | 3 `link_markup_is_not_drawn` | `](` drawn | T87.3 | no `[`, `](` or target |
//! | 4 `image_markup_is_not_drawn` | `](` drawn | T87.4 | no `![`, `](` or name |
//! | 5 `default_theme_is_todays` | the two folds below | — | unchanged |

use std::time::{Duration, Instant};

use lumen_static::display::{Cmd, DisplayList};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::Markdown;

fn lay<W: Widget + Node>(w: &mut W, max_w: f32) -> Vec<NodeState> {
    let styles = lumen_static::pass::default_styles(w);
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let limits = Constraints { min_w: 0.0, max_w, min_h: 0.0, max_h: 100_000.0 };
    let mut m = lumen_static::cx::BlockMetrics;
    {
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, limits, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

/// What the spans cover, joined with `|` so a dropped word shows.
fn drawn(m: &Markdown) -> String {
    let r = m.rich();
    (0..r.spans().len()).map(|k| r.span_str(k)).collect::<Vec<_>>().join("|")
}

/// Gate 1. **Pinned at 192**: `RichText::push` refuses silently past
/// `MAX_SPANS`, so a long document lays out its first 192 words and any
/// probe of it times a truncated parse. T87.1 flips this to `== words`.
#[test]
fn every_word_is_kept() {
    let words = 1_000;
    let src: String = (0..words).map(|k| format!("w{k}")).collect::<Vec<_>>().join(" ");
    let m = Markdown::new(&src);
    let spans = m.rich().spans().len();
    println!("every_word_is_kept: {spans} spans of {words} words (pinned today: 192; T87.1's bound: equal)");
    assert_eq!(spans, 192, "pinned: the span store keeps 192 of {words} words today");
}

/// `bytes` of 40-byte lines: 39 bytes of words and a newline.
fn lines40(bytes: usize) -> String {
    let line = "alpha bravo **charlie** delta `echo` fo\n";
    assert_eq!(line.len(), 40, "the fixture's line is 40 bytes with its newline");
    let mut s = String::with_capacity(bytes + line.len());
    while s.len() < bytes {
        s.push_str(line);
    }
    s
}

/// The minimum of five `set_str` timings.
fn parse_min(src: &str) -> Duration {
    let mut m = Markdown::new("");
    let mut best = Duration::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        m.set_str(src);
        best = best.min(t.elapsed());
    }
    assert_eq!(m.rich().text().len(), src.len(), "setup: set_str did not take the text");
    assert!(!m.rich().spans().is_empty(), "setup: set_str parsed nothing");
    best
}

/// Gate 2. Ten times the text costs about 10 when linear and about 100
/// when it is lines × bytes; the bound is 25, `textarea_scale.rs`'s. The
/// self-ratio (the small size against itself) is printed beside it, so
/// noise on a loaded machine is visible next to the number it could
/// spoil. **Pinned above 25**: `line_start` re-scans from byte 0 for
/// every line (T45.10's quadratic again). T87.1 flips it to `≤ 25`.
#[test]
fn set_str_is_linear() {
    let (small, large) = (lines40(6_400), lines40(64_000));
    let self_a = parse_min(&small).as_secs_f64();
    let ta = parse_min(&small).as_secs_f64();
    let tb = parse_min(&large).as_secs_f64();
    let r = tb / ta;
    println!(
        "set_str_is_linear: 6.4KB={:.3} ms 64KB={:.3} ms ratio={r:.1} (bound 25; pinned today: > 25), self-ratio {:.2}",
        ta * 1e3,
        tb * 1e3,
        ta / self_a
    );
    assert!(r > 25.0, "pinned: set_str is lines x bytes today, ratio {r:.1} is not above 25");
}

/// Gate 3. **Pinned**: the parser has no link markup, so `[the` and
/// `docs](https://a.example)` are drawn as words. T87.3 flips it: no
/// `[`, `](` or target bytes in what is drawn.
#[test]
fn link_markup_is_not_drawn() {
    let m = Markdown::new("see [the docs](https://a.example) now");
    let d = drawn(&m);
    println!("link_markup_is_not_drawn: drawn {d:?} (pinned today: contains \"](\")");
    assert!(d.contains("]("), "pinned: link markup is drawn today, and {d:?} has none");
}

/// Gate 4. The same for an image. T87.4 flips it.
#[test]
fn image_markup_is_not_drawn() {
    let m = Markdown::new("![a chart](chart)");
    let d = drawn(&m);
    println!("image_markup_is_not_drawn: drawn {d:?} (pinned today: contains \"](\")");
    assert!(d.contains("]("), "pinned: image markup is drawn today, and {d:?} has none");
}

/// The showcase's Markdown demo, `DOC` in `examples/showcase/src/catalog.rs`
/// — copied, because an example's constant cannot be imported. A drift
/// between the two changes only which document this gate pins.
const DOC: &str = "# Markdown\n\nHeadings, **bold**, `code` and bullets \
are the subset a user interface actually renders.\n\n- a label\n- an \
identifier\n- a list";

/// **Every element the theme will name**, which `DOC` is not: it has one
/// `#` heading and no `##` or `###`, so the plan's own injection (`## `
/// scale 1.25 → 1.3) left a `DOC`-only fold green. Found by that
/// injection at T87.0 and recorded in the plan's "What building found".
const EVERY: &str = "# One\n\n## Two words\n\n### Three\n\nbody **bold** and `code`\n\n- a bullet";

/// The folds gate 5 pins: today's default display lists.
const PINNED_DOC: u64 = 0x544b_680d_bb85_0c4e;
const PINNED_EVERY: u64 = 0xf343_b0e9_1944_032c;

/// `src` laid out at 320 px and painted: (commands, text runs, fold).
fn fold_of(src: &str) -> (usize, usize, u64) {
    let mut m = Markdown::new(src);
    let st = lay(&mut m, 320.0);
    let mut list = DisplayList::new();
    Node::paint(&m, &st, 0, &mut list);
    let texts = list.cmds.iter().filter(|c| matches!(c, Cmd::Text { .. })).count();
    (list.cmds.len(), texts, list.hash_from(0))
}

/// Gate 5. **The default theme is today's**: `DOC` (and [`EVERY`]) laid
/// out at 320 px and painted into a `DisplayList`, every command folded
/// with `DisplayList::hash_from(0)`, which reads a text run's bytes —
/// `Cmd::hash_into` folds only a run's length, so a same-length change
/// would pass it. T87.2's `MarkdownTheme::DEFAULT` must leave both folds
/// where they are.
#[test]
fn default_theme_is_todays() {
    let (cmds, texts, doc) = fold_of(DOC);
    let (ecmds, etexts, every) = fold_of(EVERY);
    println!(
        "default_theme_is_todays: DOC {cmds} cmds, {texts} Cmd::Text (setup bound >= 20), fold {doc:#018x} (pinned {PINNED_DOC:#018x}); \
         EVERY {ecmds} cmds, {etexts} Cmd::Text, fold {every:#018x} (pinned {PINNED_EVERY:#018x})"
    );
    assert!(texts >= 20, "setup: only {texts} text runs were painted, so the fold pins too little");
    assert!(etexts >= 9, "setup: EVERY painted only {etexts} text runs");
    assert_eq!(doc, PINNED_DOC, "DOC's default display list moved: fold {doc:#018x}, pinned {PINNED_DOC:#018x}");
    assert_eq!(every, PINNED_EVERY, "EVERY's default display list moved: fold {every:#018x}, pinned {PINNED_EVERY:#018x}");
}
