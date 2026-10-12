//! **M87/T87.0 — the Markdown gates that compile today**
//! (`docs/plan-markdown.md`). Headless, with `BlockMetrics`, as
//! `tests/rich.rs` is.
//!
//! Every gate here was committed **green** (`docs/plan-iced-roadmap.md`
//! §3.1): gates 1–4 pinned T87.0's gap — they asserted the value then and
//! printed it — and the task that closes each one flips its assertion to
//! the bound written beside it. Gates 1 and 2 are flipped (T87.1), gate 3
//! by T87.3, gate 4 by T87.4. Gate 5 pins the default display list
//! before `MarkdownTheme` (T87.2) moves anything.
//!
//! | gate | pinned today | flipped by | bound |
//! |---|---|---|---|
//! | 1 `every_word_is_kept` | 192 spans of 1,000 words | T87.1 (flipped) | spans = words |
//! | 2 `set_str_is_linear` | ratio > 25 | T87.1 (flipped) | ratio ≤ 25 |
//! | 3 `link_markup_is_not_drawn` | `](` drawn | T87.3 (flipped) | no `[`, `](` or target |
//! | 4 `image_markup_is_not_drawn` | `](` drawn | T87.4 (flipped) | no `![`, `](` or name |
//! | 5 `default_theme_is_todays` | the two folds below | — | unchanged |
//!
//! And T87.2's own: `a_theme_scales_its_heading` (a theme's `h1` reaches
//! the layout) and `a_quote_is_indented_and_barred_on_every_line`; and
//! T87.4's: `an_image_is_a_block_at_its_size` and
//! `an_image_scales_down_with_its_aspect`. Loading an image (its damage)
//! is the shell's `tests/markdown_images.rs`, which needs a `Host`.
//! And T87.6's: `append_matches_full_parse`, the gate for K87's build.

use std::time::{Duration, Instant};

use fulgurite::display::{Cmd, DisplayList, ImageId};
use fulgurite::widget::{Node, Widget};
use fulgurite::{Constraints, NodeState, Point, Size};
use fulgurite_widgets::{ImageRef, Markdown, MarkdownTheme, Span};

fn lay<W: Widget + Node>(w: &mut W, max_w: f32) -> Vec<NodeState> {
    let styles = fulgurite::pass::default_styles(w);
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let limits = Constraints { min_w: 0.0, max_w, min_h: 0.0, max_h: 100_000.0 };
    let mut m = fulgurite::cx::BlockMetrics;
    {
        let mut cx = fulgurite::LayoutCx::new(&mut m);
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
        std::mem::size_of::<fulgurite_widgets::RichText>()
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

/// Gate 3. **Link markup is not drawn** — flipped by T87.3. T87.0 pinned
/// it drawn: the parser had no links, so `[the` and
/// `docs](https://a.example)` were words. Now the words are `the` and
/// `docs`, both in link 0, and the target is the link's, not the text's.
#[test]
fn link_markup_is_not_drawn() {
    let m = Markdown::new("see [the docs](https://a.example) now");
    let d = drawn(&m);
    let links: Vec<u16> = m.rich().spans().iter().map(|s| s.link).collect();
    println!(
        "link_markup_is_not_drawn: drawn {d:?}, link per span {links:?}, link 0 -> {:?} (bound: no `[`, `](` or target)",
        m.link(0)
    );
    assert!(!d.contains('[') && !d.contains("](") && !d.contains("a.example"), "link markup is drawn: {d:?}");
    assert_eq!(d, "see|the|docs|now", "the words are not the link's text and its neighbours");
    assert_eq!(m.link(0), Some("https://a.example"), "the link's target was not kept");
}

/// Gate 4. **Image markup is not drawn** — flipped by T87.4. T87.0
/// pinned it drawn (`![a` and `chart](chart)` were words). Now an image
/// whose name no `ImageRef` holds draws its alt words, `a` and `chart`, as
/// [`Span::ALT`]; and one whose name resolves is a single [`Span::IMAGE`]
/// whose text is the alt. Neither draws `![`, `](` or `(chart)`.
#[test]
fn image_markup_is_not_drawn() {
    let mut m = Markdown::new("![a chart](chart)");
    let d = drawn(&m);
    let kinds: Vec<u8> = m.rich().spans().iter().map(|s| s.kind).collect();
    m.set_images(&[ImageRef { name: "chart", id: ImageId(3), size: Size { w: 120.0, h: 80.0 } }]);
    let r = drawn(&m);
    let rkinds: Vec<u8> = m.rich().spans().iter().map(|s| s.kind).collect();
    println!(
        "image_markup_is_not_drawn: unresolved drawn {d:?} kinds {kinds:?}; resolved drawn {r:?} kinds {rkinds:?}, \
         alt {:?} (bound: no `!`, `[`, `](` or `(chart)`)",
        m.image_alt(0)
    );
    for t in [&d, &r] {
        assert!(!t.contains('!') && !t.contains('[') && !t.contains("](") && !t.contains("(chart"), "image markup is drawn: {t:?}");
    }
    assert_eq!((d.as_str(), kinds.as_slice()), ("a|chart", &[Span::ALT, Span::ALT][..]), "the unresolved image is not its alt words");
    assert_eq!((r.as_str(), rkinds.as_slice()), ("a chart", &[Span::IMAGE][..]), "the resolved image is not one image span");
    assert_eq!(m.image_alt(0), Some("a chart"), "the alt text was not kept");
}

/// One 120 × 80 image, `chart`, as an application would bind it.
const CHART: [ImageRef; 1] = [ImageRef { name: "chart", id: ImageId(3), size: Size { w: 120.0, h: 80.0 } }];

/// **An image is a block at its size** — M87/T87.4. In a 320 px box a
/// 120 × 80 image keeps its size, starts a line of its own below the word
/// before it, and the word after it starts below its bottom. Injection
/// (recorded in the commit): lay the image out as an inline word — no
/// break around it, its line the font's — and the next word sits beside
/// it, above its bottom.
#[test]
fn an_image_is_a_block_at_its_size() {
    let mut m = Markdown::new("before ![a chart](chart) after");
    m.set_images(&CHART);
    let st = lay(&mut m, 320.0);
    let r = m.rich();
    let words: Vec<&str> = (0..r.spans().len()).map(|k| r.span_str(k)).collect();
    assert_eq!(words, ["before", "a chart", "after"], "setup: the document parsed as {words:?}");
    assert!(m.image_resolved(0), "setup: `chart` did not resolve");
    let img = m.image_rect(0).expect("a laid-out image has a rect");
    let (before, after) = (r.placed()[0], r.placed()[2]);
    let pad = st[0].pad;
    let line = 14.0 * 1.45;
    println!(
        "an_image_is_a_block_at_its_size: image rect {img:?} ({:.2} x {:.2}; bound 120 x 80 ± 0.5), \
         'before' at y {:.2} (line {line:.2}), 'after' at y {:.2} beside the image's bottom {:.2} (bound >=)",
        img[2] - img[0], img[3] - img[1], pad.1 + before.1, pad.1 + after.1, img[3]
    );
    assert!(((img[2] - img[0]) - 120.0).abs() <= 0.5 && ((img[3] - img[1]) - 80.0).abs() <= 0.5, "the image is not at its size: {img:?}");
    assert!(img[1] >= pad.1 + before.1 + line - 0.5, "the image does not start below the line before it");
    assert!(pad.1 + after.1 >= img[3] - 0.01, "the word after the image is above its bottom: y {} < {}", pad.1 + after.1, img[3]);
    // And it is painted there, once, with the id and frame 0.
    let mut list = DisplayList::new();
    Node::paint(&m, &st, 0, &mut list);
    let imgs: Vec<_> = list.cmds.iter().filter_map(|c| match c { Cmd::Image { rect, id, frame, .. } => Some((*rect, *id, *frame)), _ => None }).collect();
    println!("painted images: {imgs:?}");
    assert_eq!(imgs, [(img, ImageId(3), 0)], "the image is not painted at its rect with its id and frame 0");
}

/// **And scaled down to the width with its aspect kept.** In a 60 px box
/// (56 px inside the 2 px padding) the 120 × 80 image is 56 wide and
/// 56 × 80 / 120 tall. Injection (recorded in the commit): clamp the
/// width only — the height stays 80.
#[test]
fn an_image_scales_down_with_its_aspect() {
    let mut m = Markdown::new("![a chart](chart)");
    m.set_images(&CHART);
    let st = lay(&mut m, 60.0);
    let img = m.image_rect(0).expect("a laid-out image has a rect");
    let inner = st[0].size.w - 4.0;
    let want = (inner, 80.0 * inner / 120.0);
    let got = (img[2] - img[0], img[3] - img[1]);
    println!(
        "an_image_scales_down_with_its_aspect: box {:.1} px, inner {inner:.2}; image {:.2} x {:.2} (bound {:.2} x {:.2} ± 0.5)",
        st[0].size.w, got.0, got.1, want.0, want.1
    );
    assert!((st[0].size.w - 60.0).abs() < 0.01, "setup: the box is {} px, not 60", st[0].size.w);
    assert!((got.0 - want.0).abs() <= 0.5 && (got.1 - want.1).abs() <= 0.5, "the image lost its aspect: {got:?}, want {want:?}");
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
        std::mem::size_of::<fulgurite_widgets::RichText>(),
        std::mem::size_of::<fulgurite_widgets::Span>(),
        std::mem::size_of::<fulgurite::Style>(),
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
    assert!(r.spans().iter().all(|s| s.kind == fulgurite_widgets::Span::QUOTE), "a word is not a quote span");
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

/// `bytes` of the `--bin markdown` fixture's lines (`benches/src/bin/
/// markdown.rs`): a heading, prose with a bold and a code word, a bullet
/// and more prose, repeated, no blank lines.
fn doc_like_the_bench(bytes: usize) -> String {
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

fn lay_measured(w: &mut Markdown, m: &mut fulgurite_shell::text::FontMetrics) {
    let styles = fulgurite::pass::default_styles(w);
    let mut st = vec![NodeState::default(); <Markdown as Widget>::NODE_COUNT];
    let limits = Constraints { min_w: 0.0, max_w: 640.0, min_h: 0.0, max_h: f32::INFINITY };
    {
        let mut cx = fulgurite::LayoutCx::new(m);
        Node::layout(w, &mut cx, limits, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
}

/// The first byte of `pat` at or after the middle of `src`.
fn mid(src: &str, pat: &str) -> usize {
    src[src.len() / 2..].find(pat).map(|k| src.len() / 2 + k).expect("the fixture holds the pattern past its middle")
}

/// **An incremental measure equals a full one** — M87/T87.6, the gate for
/// K87's build. K87 fired (`docs/results-markdown.md` T87.6), and its
/// dominant split quantity was *measure*, so what T87.6 built is width
/// reuse keyed by word content (`RichText::remeasure`), not a block-level
/// re-parse: the parse is unchanged and still walks the whole text. So
/// what must not drift is what a reused width feeds — the spans, every
/// placement (`placed`, compared as `f32` bits: a reused width is the
/// word's own measurement or this is red) and the line count — between
/// one `Markdown` carried through ten edits of the 64 KB fixture and a
/// fresh `Markdown` laid out from each edit's text, both under the system
/// face stack (`--bin markdown`'s; skipped without one, as the shell's
/// face tests are), whose bold face is asserted to measure differently.
///
/// **The setup is asserted too**: with the measurer's process-wide
/// `measured` counter (no other test in this binary measures through a
/// face), the one-character append must measure fewer than 1% of the
/// runs a full measure does, or the comparison is of two full measures
/// and proves nothing about reuse.
///
/// Injections (recorded in the commit): reuse by index, not content —
/// the common prefix taken as every index both stores have, unchecked —
/// is red on the second edit, where the last word gains a trailing space
/// and keeps its old, zero gap; and a key that leaves out the weight is
/// red on "bold a word".
#[test]
fn append_matches_full_parse() {
    use fulgurite::TextMeasure;
    use fulgurite_shell::text::{text_counters, FontMetrics, FontStack};
    // The system stack, as `--bin markdown` measures under: it pairs a
    // bold face with the regular one. The pinned face alone has none, so
    // under it a bold word measures as a regular one and the "bold a
    // word" edit could not see a key that forgot the weight (found by that
    // injection, which stayed green under the pinned face).
    let Some(font) = FontStack::system() else {
        println!("append_matches_full_parse: no system face; skipped");
        return;
    };
    {
        let mut m = FontMetrics::new(font.clone());
        let (regular, bold) = (m.measure_weighted("sit", 16.0, 400.0, f32::INFINITY).w, m.measure_weighted("sit", 16.0, 700.0, f32::INFINITY).w);
        println!("append_matches_full_parse: 'sit' at 16 px measures {regular:.3} regular, {bold:.3} bold");
        assert!(bold != regular, "setup: this stack measures bold as regular, so the bold edit cannot exhibit a weight-blind key");
    }
    let mut src = doc_like_the_bench(64 * 1024);
    let mut inc = Markdown::new(&src);
    let mut metrics = FontMetrics::new(font.clone());
    lay_measured(&mut inc, &mut metrics);

    type Edit = fn(&mut String);
    let edits: [(&str, Edit); 10] = [
        ("append one character", |s| s.push('x')),
        ("append words and a line", |s| s.push_str(" and more words\nthen a new line\n")),
        ("insert a word inside a bullet", |s| {
            let at = mid(s, "- do eiusmod") + 2;
            s.insert_str(at, "extra ");
        }),
        ("delete a word", |s| {
            let at = mid(s, "tempor ");
            s.replace_range(at..at + 7, "");
        }),
        ("bold a word", |s| {
            let at = mid(s, " sit ") + 1;
            s.replace_range(at..at + 3, "**sit**");
        }),
        ("heading level", |s| {
            let at = mid(s, "# A heading");
            s.insert(at, '#');
        }),
        ("split a line", |s| {
            let at = mid(s, " minim ");
            s.replace_range(at..at + 1, "\n");
        }),
        ("delete a line", |s| {
            let at = mid(s, "aliqua ut");
            let end = at + s[at..].find('\n').unwrap() + 1;
            s.replace_range(at..end, "");
        }),
        ("edit the first word", |s| s.insert(4, 'x')),
        ("truncate to half", |s| {
            let at = s[..s.len() / 2].rfind('\n').unwrap() + 1;
            s.truncate(at);
        }),
    ];

    let mut compared = 0usize;
    let mut append_runs = (0u64, 0u64);
    for (k, (name, edit)) in edits.iter().enumerate() {
        edit(&mut src);
        let before = text_counters().measured;
        inc.set_str(&src);
        lay_measured(&mut inc, &mut metrics);
        let inc_runs = text_counters().measured - before;

        let mut full = Markdown::new(&src);
        let mut fresh = FontMetrics::new(font.clone());
        let before = text_counters().measured;
        lay_measured(&mut full, &mut fresh);
        let full_runs = text_counters().measured - before;

        let (a, b) = (inc.rich(), full.rich());
        let n = b.spans().len();
        println!("append_matches_full_parse: {name}: {n} spans compared, measured {inc_runs} runs incrementally against {full_runs} in full");
        assert_eq!(a.text(), b.text(), "setup: {name}: the two did not take the same text");
        assert_eq!(a.spans().len(), n, "{name}: span counts differ");
        if let Some(i) = (0..n).find(|&i| a.spans()[i] != b.spans()[i]) {
            panic!("{name}: span {i} differs: {:?} against {:?}", a.spans()[i], b.spans()[i]);
        }
        let bits = |p: &(f32, f32, f32)| (p.0.to_bits(), p.1.to_bits(), p.2.to_bits());
        if let Some(i) = (0..n).find(|&i| bits(&a.placed()[i]) != bits(&b.placed()[i])) {
            panic!(
                "{name}: span {i} ({:?}) is placed at {:?} incrementally and {:?} by a full measure",
                b.span_str(i),
                a.placed()[i],
                b.placed()[i]
            );
        }
        assert_eq!(a.lines(), b.lines(), "{name}: line counts differ");
        compared += n;
        if k == 0 {
            append_runs = (inc_runs, full_runs);
        }
    }
    let (inc_runs, full_runs) = append_runs;
    println!(
        "append_matches_full_parse: {compared} spans compared over ten edits; the one-character append measured \
         {inc_runs} runs against a full measure's {full_runs} (bound: under 1%, {})",
        full_runs / 100
    );
    assert!(full_runs > 10_000, "setup: a full measure of the 64 KB fixture measured only {full_runs} runs");
    assert!(inc_runs * 100 < full_runs, "setup: the append re-measured {inc_runs} of {full_runs} runs, so nothing was reused");
}
