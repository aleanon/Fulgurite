//! **M87/T87.0 — the Markdown gates that compile today**
//! (`docs/plan-markdown.md`). Headless, with `BlockMetrics`, as
//! `tests/rich.rs` is.
//!
//! Every gate here was committed **green** (`docs/plan-iced-roadmap.md`
//! §3.1): gates 1–4 pinned T87.0's gap — they asserted the value then and
//! printed it — and the task that closes each one flips its assertion to
//! the bound written beside it. Gates 1 and 2 are flipped (T87.1). Gate 5 pins the default display list
//! before `MarkdownTheme` (T87.2) moves anything.
//!
//! | gate | pinned today | flipped by | bound |
//! |---|---|---|---|
//! | 1 `every_word_is_kept` | 192 spans of 1,000 words | T87.1 (flipped) | spans = words |
//! | 2 `set_str_is_linear` | ratio > 25 | T87.1 (flipped) | ratio ≤ 25 |
//! | 3 `link_markup_is_not_drawn` | `](` drawn | T87.3 | no `[`, `](` or target |
//! | 4 `image_markup_is_not_drawn` | `](` drawn | T87.4 | no `![`, `](` or name |
//! | 5 `default_theme_is_todays` | the two folds below | — | unchanged |
//!
//! And T87.2's own: `a_theme_scales_its_heading` (a theme's `h1` reaches
//! the layout) and `a_quote_is_indented_and_barred_on_every_line`.

use std::time::{Duration, Instant};

use lumen_static::display::{Cmd, DisplayList};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, NodeState, Point};
use lumen_static_widgets::{Markdown, MarkdownTheme};

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

/// Gate 1. **Every word is a span** — flipped by T87.1. Pinned at 192 by
/// T87.0: `RichText::push` refused silently past `MAX_SPANS`, so a long
/// document laid out its first 192 words and any probe of it timed a
/// truncated parse. The store grows now; `RichText`'s inline size is
/// printed beside it (7,736 B before T87.1, `docs/results-markdown.md`).
#[test]
fn every_word_is_kept() {
    let words = 1_000;
    let src: String = (0..words).map(|k| format!("w{k}")).collect::<Vec<_>>().join(" ");
    let m = Markdown::new(&src);
    let spans = m.rich().spans().len();
    println!(
        "every_word_is_kept: {spans} spans of {words} words (bound: equal); size_of::<RichText>() = {} B",
        std::mem::size_of::<lumen_static_widgets::RichText>()
    );
    assert_eq!(spans, words, "the span store kept {spans} of {words} words");
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
/// spoil. **Flipped by T87.1 to `≤ 25`**: T87.0 pinned it above 25,
/// because `line_start` re-scanned from byte 0 for every line (T45.10's
/// quadratic again); `Markdown::set_str` carries the offset now.
#[test]
fn set_str_is_linear() {
    let (small, large) = (lines40(6_400), lines40(64_000));
    let self_a = parse_min(&small).as_secs_f64();
    let ta = parse_min(&small).as_secs_f64();
    let tb = parse_min(&large).as_secs_f64();
    let r = tb / ta;
    println!(
        "set_str_is_linear: 6.4KB={:.3} ms 64KB={:.3} ms ratio={r:.1} (bound 25), self-ratio {:.2}",
        ta * 1e3,
        tb * 1e3,
        ta / self_a
    );
    assert!(r <= 25.0, "set_str grows faster than its text: ratio {r:.1}, bound 25");
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

/// **A theme's `h1` reaches the layout** — M87/T87.2. With `h1 = (2.0,
/// 700.0)` a heading's line is twice a body line's: the line heights are
/// read off the breaker's own placements (`RichText::placed`), heading to
/// first body line and first body line to the second. Injection (recorded
/// in the commit): `heading()` keeps its literals and ignores the theme —
/// the ratio is 1.5.
#[test]
fn a_theme_scales_its_heading() {
    let mut m = Markdown::new("# Big\nbody one\nbody two");
    m.set_theme(MarkdownTheme { h1: (2.0, 700.0), ..MarkdownTheme::DEFAULT });
    let _st = lay(&mut m, 320.0);
    let r = m.rich();
    let words: Vec<&str> = (0..r.spans().len()).map(|k| r.span_str(k)).collect();
    assert_eq!(words, ["Big", "body", "one", "body", "two"], "setup: the document parsed as {words:?}");
    let y: Vec<f32> = r.placed().iter().map(|p| p.1).collect();
    let (heading, body) = (y[1] - y[0], y[3] - y[1]);
    assert!(body > 0.0, "setup: the body lines did not break ({y:?})");
    let ratio = heading / body;
    let tol = 0.05 * 1.45;
    println!(
        "a_theme_scales_its_heading: heading line {heading:.2} px, body line {body:.2} px, ratio {ratio:.3} \
         (bound 2.0 ± {tol:.4}); spans {:?}",
        r.spans().iter().map(|s| (s.scale, s.weight)).collect::<Vec<_>>()
    );
    assert!((ratio - 2.0).abs() <= tol, "a theme with h1 = 2.0 laid the heading at {ratio:.3} of a body line");
    // What the theme costs, and what the withdrawn parts would have
    // (`docs/results-markdown.md`, T87.2): two `Style`s and two
    // `NodeState`s per `Markdown`.
    println!(
        "sizes: MarkdownTheme {} B, Markdown {} B, RichText {} B, Span {} B; a part node's Style + NodeState {} + {} B",
        std::mem::size_of::<MarkdownTheme>(),
        std::mem::size_of::<Markdown>(),
        std::mem::size_of::<lumen_static_widgets::RichText>(),
        std::mem::size_of::<lumen_static_widgets::Span>(),
        std::mem::size_of::<lumen_static::Style>(),
        std::mem::size_of::<NodeState>(),
    );
}

/// **A blockquote is drawn, not just parsed** — M87/T87.2. `> ` is markup
/// (no `>` among the words), every word is a quote span, the first is
/// indented like a bullet and a wrapped line hangs under it, and the
/// 2 px bar is painted once per visual line in the quote colour —
/// a role nothing draws is the ghost-button shape the plan names.
/// Injection (recorded in the commit): skip the bar in `RichText::draw` —
/// zero bars against the line count.
#[test]
fn a_quote_is_indented_and_barred_on_every_line() {
    let src = "> one two three four five six seven eight nine ten eleven twelve";
    let mut m = Markdown::new(src);
    let st = lay(&mut m, 120.0);
    let r = m.rich();
    let words: Vec<&str> = (0..r.spans().len()).map(|k| r.span_str(k)).collect();
    assert!(!words.iter().any(|w| w.contains('>')), "the marker is drawn: {words:?}");
    assert!(r.spans().iter().all(|s| s.kind == lumen_static_widgets::Span::QUOTE), "a word is not a quote span");
    let xs: Vec<f32> = r.placed().iter().map(|p| p.0).collect();
    let lines = r.lines();
    println!("quote at 120 px: {lines} lines, word x {xs:?}");
    assert!(lines >= 3, "setup: the quote did not wrap ({lines} lines)");
    let mut list = DisplayList::new();
    Node::paint(&m, &st, 0, &mut list);
    let quote = m.theme().quote_ink(st[0].fg);
    let x0 = st[0].pos.x + st[0].pad.0;
    let bars = list
        .cmds
        .iter()
        .filter(|c| matches!(c, Cmd::RoundedRect { rect, color, .. }
            if *color == quote && (rect[2] - rect[0] - 2.0).abs() < 0.01 && (rect[0] - x0 - 2.0).abs() < 0.01))
        .count();
    let starts = xs.iter().filter(|x| (**x - 14.0).abs() < 0.01).count();
    println!("bars {bars} (bound = lines {lines}); words at the 14 px indent {starts} (bound = lines)");
    assert_eq!(starts, lines, "a quote line does not start at the bullet indent");
    assert_eq!(bars, lines, "the quote bar is not drawn once per line");
}
