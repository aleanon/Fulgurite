//! M27/T27.12 — the text tail: `RichText`, `Markdown`, `RichTextEditor`,
//! and `PaintBox`.
//!
//! # A span is a word
//!
//! Text that wraps has to break somewhere, and the framework's measure
//! hook answers *how wide is this string*, not *where would it break*.
//! So a span here is **atomic**: it never wraps inside itself, and the
//! line breaker only ever decides whether the next one fits. The parser
//! therefore splits on whitespace, and a "span" is a word plus the
//! formatting it carries.
//!
//! That is one measure call per word rather than a cluster walk, it is
//! exact (the breaker never guesses a width), and it makes the paint a
//! straight loop over placed boxes.
//!
//! # What it costs is heap, sized once per parse
//!
//! **There is no word ceiling** — M87/T87.1. The spans, where each was
//! placed and what each measured are three `Vec`s, grown when the text is
//! parsed ([`RichText::new`], [`Markdown::set_str`]) and never on a
//! layout, so a still frame touches the allocator not at all
//! (`crates/fulgurite-shell/tests/alloc_gate.rs`, the Markdown arm).
//! Until T87.1 they were arrays of 192 words held inline, and `push`
//! refused the 193rd silently: a 64 KB document laid out its first 192
//! words, and any probe of it timed a truncated parse. Since M87/T87.6 a
//! fourth, each measured word's content key, lets an edit re-measure only
//! the words it touched (`RichText::remeasure`). The widget's inline size
//! and the heap per word are `docs/results-markdown.md`'s (T87.1, T87.6).
//! This crate's `rich` module is behind `alloc`, and the leaf firmware
//! links none of it.
//!
//! # What the Markdown subset leaves out, and why
//!
//! **Italic.** [`fulgurite::Painter`] has one text method and its
//! only typographic axis is *weight*: there is no italic face and no
//! synthetic slant. A parser that accepted `*emphasis*` and rendered it
//! identically to plain text would be telling the reader it had done
//! something. So `*` is not markup here, and the gap is the text
//! stack's rather than this widget's — a slant belongs beside
//! `FontStack`, next to the weight axis it would join.
//!
//! M87/T87.5 probed it again and recorded (`docs/results-markdown.md`,
//! T87.5): the shell's pinned face lists name no italic face, and no
//! `Painter` method carries a face — the shaped run's `GlyphKey::face`
//! (M81) is chosen inside the shell from the weight. **It reopens when M81
//! (or a successor) exposes a per-run face to `Painter`.**
//!
//! Headings, `**bold**`, `` `code` ``, bullets, paragraphs, and since
//! M87 blockquotes, `[links](target)` and `![images](name)`, are the
//! subset, which is what a UI actually renders: labels, emphasis, an
//! identifier, a list, an aside, a reference, an illustration.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use fulgurite::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use fulgurite::display::{Color, ImageId, Painter};
use fulgurite::style::{Fields, Length, Seed, Style};
use fulgurite::widget::{Node, Widget};
use fulgurite::{Edges, LayoutCx};

/// A word, and how it is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Span {
    /// Byte range into the owner's text.
    pub range: (u32, u32),
    /// Multiplied into the node's own `font_size`, so a stylesheet that
    /// resizes the widget resizes its headings with it.
    pub scale: f32,
    pub weight: f32,
    /// Drawn on a tinted chip — the only way to say "identifier" with
    /// one face.
    pub code: bool,
    /// Starts a new line: a heading, a paragraph, a bullet.
    pub br: bool,
    /// Draws a bullet before it.
    pub bullet: bool,
    /// A gap after this word. False at the end of a line's last word,
    /// so a trailing space cannot push a line over its width.
    pub space: bool,
    /// What the word is part of: [`Span::TEXT`], [`Span::QUOTE`] —
    /// M87/T87.2 — or, since M87/T87.4, [`Span::IMAGE`] and [`Span::ALT`].
    /// A quote's words are drawn in the quote colour (`MarkdownTheme`'s,
    /// derived from the node's own), and a quote line's first word sets
    /// `bullet` for its indent and draws a bar instead of a dot.
    pub kind: u8,
    /// Which link this word is part of, or [`Span::NO_LINK`] — M87/T87.3.
    /// The index is the link's order of appearance in the source, the
    /// same count [`link_target`] makes, so a handler that receives it in
    /// `ctx.row` can find the target from the application's own source.
    pub link: u16,
}

impl Span {
    /// An ordinary word.
    pub const TEXT: u8 = 0;
    /// A word of a blockquote (`> ` at a line's start) — M87/T87.2.
    pub const QUOTE: u8 = 1;
    /// A resolved image — M87/T87.4: one span for the whole
    /// `![alt](name)`, laid out as a **block** (on a line of its own, at
    /// its size scaled down to the width with its aspect kept) and painted
    /// with `Painter::image`. Its `range` is the alt text. The `n`th image
    /// span draws the owner's `n`th image (`RichText::image_rect`).
    pub const IMAGE: u8 = 2;
    /// A word of an **unresolved** image's alt text — M87/T87.4: the name
    /// matched no `ImageRef`, so the alt is drawn in its place, inline, in
    /// the quote colour.
    pub const ALT: u8 = 3;
    /// `link` for a word that is in no link.
    pub const NO_LINK: u16 = u16::MAX;

    pub const fn plain(range: (u32, u32)) -> Span {
        Span { range, scale: 1.0, weight: 400.0, code: false, br: false, bullet: false, space: true, kind: Span::TEXT, link: Span::NO_LINK }
    }
}

/// Wrapped, styled text.
///
/// Its three per-word stores are `Vec`s sized when the text is parsed and
/// never on a layout — this module's header, "What it costs".
#[derive(Default)]
pub struct RichText {
    text: String,
    spans: Vec<Span>,
    /// Where each span was placed, and how wide it measured. As long as
    /// `spans` after every parse (`fit`).
    placed: Vec<(f32, f32, f32)>,
    /// Lines the last layout produced — published because "did it wrap"
    /// is the question a test about wrapping actually asks.
    lines: usize,
    height: f32,
    /// Each span's measured width and the space after it, and the fold
    /// they were measured under — M57/B6. Every layout re-measured every
    /// word, and a probe lays out twice more per frame under `observe`.
    /// The widths do not depend on the box (a span never wraps inside
    /// itself), so the key is the text, the spans, the font size and the
    /// display scale, folded; a still frame measures nothing.
    ///
    /// **In step with `word_keys`, not with `spans`**: between a parse and
    /// the next layout it still holds the widths of the spans it was
    /// measured for, which is what lets that layout reuse them
    /// (`remeasure`).
    widths: Vec<(f32, f32)>,
    measured_under: u64,
    /// Each measured span's content key (`RichText::word_key`: its
    /// word's bytes, scale, weight, trailing space and kind) — M87/T87.6.
    /// When the fold misses, a span whose key matches the one at its place
    /// in the common prefix or suffix keeps its width, so an edit
    /// re-measures the words it touched and not the document
    /// (`docs/results-markdown.md` T87.6).
    word_keys: Vec<u64>,
    /// The font size and display scale `widths` were measured at, as bits:
    /// a change to either re-measures every word.
    measured_at: (u32, u32),
    /// The last kept layout's padding origin, font size and line ratio —
    /// M87/T87.3: what turns a point in the node into a placed word
    /// ([`RichText::link_at`]) for a hit and for `cursor_at`, which is
    /// handed no `NodeState`.
    pad: (f32, f32),
    font_size: f32,
    ratio: f32,
    /// Each [`Span::IMAGE`]'s picture and its natural size, in span order —
    /// M87/T87.4. Pushed by `Markdown`'s parser beside the span, so it is
    /// sized at parse time like the other stores; `RichText::new` pushes
    /// none.
    images: Vec<(ImageId, Size)>,
}

impl RichText {
    /// Plain text, split into words.
    pub fn new(text: &str) -> Self {
        let mut r = RichText { text: text.to_string(), ..Default::default() };
        r.split_plain();
        r
    }

    /// The measure memo's key: FNV-1a over everything a width depends on.
    fn fold(&self, font_size: f32, scale: f32) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |b: &[u8]| {
            for &x in b {
                h = (h ^ x as u64).wrapping_mul(0x100_0000_01b3);
            }
        };
        eat(self.text.as_bytes());
        eat(&font_size.to_bits().to_le_bytes());
        eat(&scale.to_bits().to_le_bytes());
        for s in &self.spans {
            eat(&s.range.0.to_le_bytes());
            eat(&s.range.1.to_le_bytes());
            eat(&s.scale.to_bits().to_le_bytes());
            eat(&s.weight.to_bits().to_le_bytes());
            eat(&[s.space as u8, s.kind]);
        }
        // Never 0, which is "never measured".
        h | 1
    }

    /// The spans, as parsed.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text one span covers.
    pub fn span_str(&self, i: usize) -> &str {
        match self.spans.get(i.min(self.spans.len().saturating_sub(1))) {
            Some(s) => self.text.get(s.range.0 as usize..s.range.1 as usize).unwrap_or(""),
            None => "",
        }
    }

    /// Lines the last layout produced.
    pub fn lines(&self) -> usize {
        self.lines
    }

    /// Where the last layout put each span, relative to the content box
    /// (inside the padding): `(x, y, w)`, in step with [`RichText::spans`].
    /// A test that asks "how tall is this line" or "where is that word"
    /// reads the breaker's own answer here rather than re-deriving it.
    pub fn placed(&self) -> &[(f32, f32, f32)] {
        &self.placed
    }

    /// Span `k`'s box from the node's origin, as the last layout placed
    /// it — and, when the next word is in the same link, across the gap to
    /// it, so a multi-word link is one target and one underline.
    fn span_rect(&self, k: usize) -> [f32; 4] {
        let (x, y, w) = self.placed[k];
        let sp = self.spans[k];
        let joined = sp.link != Span::NO_LINK
            && sp.space
            && self.spans.get(k + 1).is_some_and(|n| n.link == sp.link && !n.br && self.placed[k + 1].1 == y);
        // `widths` follows the last measure, not the last parse (its
        // field's doc), so a hit between the two reads no gap rather than
        // past the store's end.
        let w = w + if joined { self.widths.get(k).map_or(0.0, |g| g.1) } else { 0.0 };
        let h = self.font_size * sp.scale * self.ratio;
        let (x0, y0) = (self.pad.0 + x, self.pad.1 + y);
        [x0, y0, x0 + w, y0 + h]
    }

    /// The link under a point given from the node's origin, if any —
    /// M87/T87.3. One function for `Markdown`'s hit and its `cursor_at`,
    /// so the pointer shape and what a click reaches cannot disagree.
    pub fn link_at(&self, x: f32, y: f32) -> Option<u16> {
        (0..self.spans.len().min(self.placed.len())).find_map(|k| {
            let n = self.spans[k].link;
            let r = self.span_rect(k);
            (n != Span::NO_LINK && x >= r[0] && x < r[2] && y >= r[1] && y < r[3]).then_some(n)
        })
    }

    /// Append a span. **Never refuses** since M87/T87.1: the store grows,
    /// which is a parse-time allocation, never a layout-time one
    /// (`fit` sizes the other two stores to match).
    pub fn push(&mut self, s: Span) {
        self.spans.push(s);
    }

    /// Forget every span (and every image), keeping the stores' capacity
    /// for the next parse.
    pub fn clear(&mut self) {
        self.spans.clear();
        self.images.clear();
    }

    /// Image `n`'s box from the node's origin, as the last layout placed
    /// it — M87/T87.4: the `n`th [`Span::IMAGE`], at its natural size
    /// scaled down to the content width with its aspect kept. `None` past
    /// the last image, or before a layout.
    pub fn image_rect(&self, n: usize) -> Option<[f32; 4]> {
        let k = self.spans.iter().enumerate().filter(|(_, s)| s.kind == Span::IMAGE).nth(n)?.0;
        let (x, y, w) = *self.placed.get(k)?;
        let h = image_h(self.images.get(n)?.1, w);
        let (x0, y0) = (self.pad.0 + x, self.pad.1 + y);
        Some([x0, y0, x0 + w, y0 + h])
    }

    /// Size `placed` to `spans`, and make room in `widths` and
    /// `word_keys` for them — called once a parse is done, and by `layout`
    /// only when a span was pushed since, so a still frame resizes
    /// nothing. Shrinking keeps the capacity; growing allocates, at parse
    /// time. `widths` keeps its length here (M87/T87.6): it is resized by
    /// the next measure, which reuses what it held, inside the capacity
    /// reserved here.
    fn fit(&mut self) {
        let n = self.spans.len();
        if self.placed.len() != n {
            self.placed.resize(n, (0.0, 0.0, 0.0));
        }
        self.widths.reserve(n.saturating_sub(self.widths.len()));
        self.word_keys.reserve(n.saturating_sub(self.word_keys.len()));
    }

    /// Span `k`'s content key — M87/T87.6: FNV-1a over what its width
    /// depends on besides the font size and scale (its word's bytes, its
    /// scale, weight, trailing space and kind), and **not** its position,
    /// so a word that moved keeps its key.
    fn word_key(&self, k: usize) -> u64 {
        let s = self.spans[k];
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |b: &[u8]| {
            for &x in b {
                h = (h ^ x as u64).wrapping_mul(0x100_0000_01b3);
            }
        };
        eat(self.span_str(k).as_bytes());
        eat(&s.scale.to_bits().to_le_bytes());
        eat(&s.weight.to_bits().to_le_bytes());
        eat(&[s.space as u8, s.kind]);
        h
    }

    /// Bring `widths` up to `spans`, re-measuring only what changed —
    /// M87/T87.6, K87's response (`docs/plan-markdown.md`). Until T87.6 a
    /// miss of the whole-text fold re-measured every word through the
    /// measurer, so one appended character to 64 KB cost 2.7 ms of
    /// measuring (`docs/results-markdown.md` T87.1). Now the spans are
    /// compared with the ones `widths` was measured for, by content key:
    /// the common prefix and the common suffix keep their widths (the
    /// suffix moved to its new index), and only the spans between are
    /// measured. At a new font size or scale nothing is reused.
    ///
    /// Reuse is by **content at an aligned place**, not by index: an
    /// inserted word shifts every word after it, and a width reused by
    /// index would be its neighbour's (the gate,
    /// `markdown_m69.rs`'s `append_matches_full_parse`).
    fn remeasure(&mut self, cx: &mut LayoutCx, font_size: f32) {
        let at = (font_size.to_bits(), cx.scale().to_bits());
        let (old, new) = (self.word_keys.len(), self.spans.len());
        let (mut p, mut q) = (0usize, 0usize);
        if at == self.measured_at && self.widths.len() == old {
            let both = old.min(new);
            while p < both && self.word_keys[p] == self.word_key(p) {
                p += 1;
            }
            while q < both - p && self.word_keys[old - 1 - q] == self.word_key(new - 1 - q) {
                q += 1;
            }
        }
        // Move the kept suffix to where it now starts; `fit` reserved the
        // room, so this does not allocate.
        self.widths.resize(old, (0.0, 0.0));
        if new >= old {
            self.widths.resize(new, (0.0, 0.0));
            self.word_keys.resize(new, 0);
            self.widths.copy_within(old - q..old, new - q);
            self.word_keys.copy_within(old - q..old, new - q);
        } else {
            self.widths.copy_within(old - q..old, new - q);
            self.word_keys.copy_within(old - q..old, new - q);
            self.widths.truncate(new);
            self.word_keys.truncate(new);
        }
        for k in p..new - q {
            let s = self.spans[k];
            self.word_keys[k] = self.word_key(k);
            if s.kind == Span::IMAGE {
                // A block's width is the box's, decided in `lay`.
                self.widths[k] = (0.0, 0.0);
                continue;
            }
            let size = font_size * s.scale;
            let w = cx.text.measure_weighted(self.span_str(k), size, s.weight, f32::INFINITY).w;
            let gap = if s.space { cx.text.measure_weighted(" ", size, s.weight, f32::INFINITY).w } else { 0.0 };
            self.widths[k] = (w, gap);
        }
        self.measured_at = at;
    }

    fn split_plain(&mut self) {
        self.spans.clear();
        let text = core::mem::take(&mut self.text);
        let mut at = 0usize;
        for w in text.split_whitespace() {
            // `split_whitespace` drops the separators, so the offset is
            // found rather than accumulated: a run of two spaces would
            // otherwise put every following span one byte short.
            let start = text[at..].find(w).map(|k| at + k).unwrap_or(at);
            at = start + w.len();
            self.push(Span::plain((start as u32, at as u32)));
        }
        self.text = text;
        self.fit();
    }
}

/// Extra leading between wrapped lines, as a multiple of the font size —
/// the default when `Style::line_height` is 0 ([`line_ratio`]).
const LINE: f32 = 1.45;

/// The line spacing as a multiple of the font size: `Style::line_height`'s
/// sixteenths when a rule sets one, [`LINE`] when it is 0 — M80/T80.2.
/// One function for `layout` (from the style) and `paint` (from the
/// `NodeState` copy), so the two cannot space the lines differently.
fn line_ratio(line_height: u8) -> f32 {
    if line_height == 0 {
        LINE
    } else {
        line_height as f32 / 16.0
    }
}
/// A bullet's indent and its dot's radius.
const BULLET_X: f32 = 14.0;
/// How far below the line box's middle a bullet's centre sits, in ems —
/// M57/B7. The renderer centres a run's *capitals* in its line box
/// (`cap_shift` in `fulgurite-shell`'s text module), so the middle of a
/// lower-case letter is below it by half of cap height less x-height:
/// (0.714 − 0.536) / 2 em for Noto Sans, plus the 0.08 em the run's box
/// sits below the dot's box here. The dot was centred on the box and read
/// 2.5 px high at 15 px; `tests/markdown_bullet.rs` in the shell checks
/// the dot against a rasterised `x`, since this is a fact about ink.
/// **A constant for one face**: on DejaVu Sans the dot sits 1.5 px off
/// the x-height's middle at 15 px (bound 1.0), found by running the test
/// job on a machine with no Noto (`docs/results-ci-time.md`, "Tier 1").
/// Deriving it from the face's own cap and x heights is not built.
const BULLET_DROP: f32 = 0.17;
/// Padding around a `code` chip.
const CHIP: f32 = 3.0;

/// An image's height at width `w`, its aspect kept — M87/T87.4. One
/// function for the layout, the paint and [`RichText::image_rect`], so
/// the three cannot disagree about a block's box.
fn image_h(natural: Size, w: f32) -> f32 {
    if natural.w > 0.0 {
        natural.h * w / natural.w
    } else {
        0.0
    }
}

/// What an image is multiplied by: opaque white is untinted, as
/// `Image`'s default.
const UNTINTED: Color = Color(255, 255, 255, 255);

impl Widget for RichText {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "richtext";
    const SEEDS: &'static [Seed] = &[Seed::new(
        <RichText as Widget>::KIND,
        <RichText as Widget>::type_defaults, Fields::of(&[
        // Measured since M80/T80.2, when `style_reads.rs` first probed this
        // kind: a span carries its own weight, so `font_weight` was never
        // read, and the hit and `record_painted`'s shade make `cursor` and
        // both dims take effect.
        Fields::WIDTH,
        Fields::HEIGHT,
        Fields::PADDING,
        Fields::CURSOR,
        Fields::BACKGROUND,
        Fields::BORDER,
        Fields::BACKGROUND_FILL,
        Fields::COLOR,
        Fields::FONT_SIZE,
        Fields::RADIUS,
        Fields::VISIBLE,
        Fields::PRESSED_DIM,
        Fields::HOVER_DIM,
        Fields::TRANSITION,
        Fields::LINE_HEIGHT,
    ])).with_roles(fulgurite::style::SeedRoles::NONE.color(fulgurite::style::Role::Text))];

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Shrink,
            padding: Edges::all(2.0),
            color: Color(0x1f, 0x29, 0x37, 0xff),
            font_size: 14.0,
            font_weight: 400.0,
            ..Style::default()
        }
    }
}

impl RichText {
    /// The layout, with `keep` false for [`Node::probe`]: the words are
    /// placed and measured as ever, but `placed` is not written, so the
    /// probe keeps the last real layout without copying a store (M87/T87.1
    /// — it copied three inline arrays, which a `Vec` would allocate for).
    fn lay(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
        keep: bool,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return fulgurite::ops::hide_subtree(st, i, limits, 1);
        }
        // **The wrap width is the box, decided before the words are
        // placed.** A `Shrink` width would make the answer depend on
        // the wrap and the wrap depend on the answer; text fills the
        // width it is given and shrinks in height, which is what
        // `type_defaults` says and what this relies on.
        let inner = (limits.max_w - me.padding.horizontal()).max(0.0);
        self.fit();
        let count = self.spans.len();
        let key = self.fold(me.font_size, cx.scale());
        if key != self.measured_under {
            self.remeasure(cx, me.font_size);
            self.measured_under = key;
        }
        let (mut x, mut y) = (0.0f32, 0.0f32);
        // **A wrapped bullet hangs** (M57/B7): its continuation lines
        // start under its text, not under the dot. Set by a bullet span,
        // cleared by the next line break.
        let mut hang = 0.0f32;
        let ratio = line_ratio(me.line_height);
        if keep {
            self.pad = (me.padding.left, me.padding.top);
            self.font_size = me.font_size;
            self.ratio = ratio;
        }
        let mut line_h = me.font_size * ratio;
        self.lines = if count == 0 { 0 } else { 1 };
        // **An image is a block** (M87/T87.4): it starts a line, its line
        // is exactly its height, and the span after it starts the next.
        let mut img = 0usize;
        let mut after_block = false;
        for k in 0..count {
            let s = self.spans[k];
            let block = s.kind == Span::IMAGE;
            let size = me.font_size * s.scale;
            let (w, gap) = self.widths[k];
            let (w, h) = if block {
                let natural = self.images.get(img).map_or(Size::default(), |m| m.1);
                img += 1;
                let w = natural.w.min(inner).max(0.0);
                (w, image_h(natural, w))
            } else {
                (w + if s.code { CHIP * 2.0 } else { 0.0 }, size * ratio)
            };
            let lead = if s.bullet { BULLET_X } else { 0.0 };
            if (s.br || block || after_block) && k > 0 {
                x = 0.0;
                hang = 0.0;
                y += line_h;
                line_h = h;
                self.lines += 1;
            } else if x > hang && x + w > inner {
                // Wraps *between* spans: a span is a word and never
                // breaks inside itself.
                x = hang;
                y += line_h;
                line_h = h;
                self.lines += 1;
            }
            line_h = if block { h } else { line_h.max(h) };
            after_block = block;
            if keep {
                self.placed[k] = (x + lead, y, w);
            }
            if s.bullet {
                hang = x + lead;
            }
            x += lead + w + gap;
        }
        let content =
            Size { w: limits.max_w, h: if count == 0 { 0.0 } else { y + line_h } };
        let s = fulgurite::ops::resolve_self(
            limits,
            &me,
            Size { w: content.w, h: content.h + me.padding.vertical() },
        );
        self.height = s.h;
        fulgurite::ops::record_painted(st, i, limits, s, &me)
    }
}

impl RichText {
    /// Paint, with a quote's words in `quote` — M87/T87.2. `RichText`
    /// passes its own `fg` (it parses no quotes); a `Markdown` passes its
    /// `markdown-quote` part's, which the cascade resolved from the
    /// palette's `Muted` role. **The colour comes in as an argument**
    /// because a `Span` carries none and `paint` has no style array: the
    /// part's `NodeState::fg` is the only place the cascade's answer for
    /// it lands.
    ///
    /// A link's words are drawn in `link` over a 1 px underline, 2 px for
    /// the `active` one — the link the keyboard is on (M87/T87.3).
    fn draw(
        &self,
        st: &[NodeState],
        i: usize,
        out: &mut dyn Painter,
        quote: Color,
        link: Color,
        active: Option<u16>,
    ) {
        let s = &st[i];
        let r = fulgurite::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        fulgurite::ops::fill(st, i, out);
        if s.bg.3 > 0 {
            out.rounded_rect(r, s.bg, s.radius);
        }
        fulgurite::ops::border(st, i, out);
        out.push_clip(r);
        let (x0, y0) = (r[0] + s.pad.0, r[1] + s.pad.1);
        let mut img = 0usize;
        for (k, (&sp, &(x, y, w))) in self.spans.iter().zip(&self.placed).enumerate() {
            if sp.kind == Span::IMAGE {
                // **A still: frame 0** (M84/T84.2's argument). A Markdown
                // image changes by id, never in place, so the command
                // changes when the binding names new pixels, and the
                // damage is this rect (`docs/plan-markdown.md`).
                if let Some(&(id, natural)) = self.images.get(img) {
                    let rect = [x0 + x, y0 + y, x0 + x + w, y0 + y + image_h(natural, w)];
                    out.image(rect, id, UNTINTED, 0);
                }
                img += 1;
                continue;
            }
            let size = s.font_size * sp.scale;
            let h = size * line_ratio(s.line_height);
            let ink = if sp.link != Span::NO_LINK {
                link
            } else if sp.kind == Span::QUOTE || sp.kind == Span::ALT {
                quote
            } else {
                s.fg
            };
            if sp.kind == Span::QUOTE {
                // **A 2 px bar down the quote's left edge**, one segment
                // per visual line: drawn by the first quote word on each
                // line, so a quote that wraps is barred on every line it
                // takes and consecutive quote lines join up.
                let first_on_line = k == 0
                    || self.spans[k - 1].kind != Span::QUOTE
                    || self.placed[k - 1].1 != y;
                if first_on_line {
                    let bx = x0 + x - BULLET_X;
                    out.rounded_rect([bx + 2.0, y0 + y, bx + 4.0, y0 + y + h], ink, 1.0);
                }
            } else if sp.bullet {
                let mid = y0 + y + h / 2.0 + size * BULLET_DROP;
                out.rounded_rect(
                    [x0 + x - BULLET_X + 2.0, mid - 2.0, x0 + x - BULLET_X + 6.0, mid + 2.0],
                    s.fg,
                    2.0,
                );
            }
            let mut tx = x0 + x;
            if sp.code {
                out.rounded_rect(
                    [tx, y0 + y + 1.0, tx + w, y0 + y + h - 1.0],
                    Color(ink.0, ink.1, ink.2, 0x1a),
                    4.0,
                );
                tx += CHIP;
            }
            out.text_weighted(
                [tx, y0 + y + h / 2.0 - size * 0.6, x0 + x + w, y0 + y + h],
                self.span_str(k),
                size,
                sp.weight,
                ink,
            );
            if sp.link != Span::NO_LINK {
                // Just under the baseline: the renderer centres capitals in
                // the line box, so the baseline is about 0.36 em below
                // its middle.
                let t = if active == Some(sp.link) { 2.0 } else { 1.0 };
                let b = self.span_rect(k);
                let uy = y0 + y + h / 2.0 + size * 0.45;
                out.rect([x0 + x, uy, r[0] + b[2], uy + t], ink);
            }
        }
        out.pop_clip();
    }
}

impl Node for RichText {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.lay(cx, limits, style, st, i, true)
    }

    /// `layout` with nothing kept — M41/T41.7. `layout` places every
    /// word against the width it is offered, and `paint` draws from those
    /// places; the audit's probe offers 1e6 px, which unwraps the text,
    /// and measures with the audit's measurer. Kept, an unmarked
    /// `RichText` under `Boundaries` painted one unwrapped line.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let kept = (self.lines, self.height);
        let size = self.lay(cx, limits, style, st, i, false);
        (self.lines, self.height) = kept;
        size
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.draw(st, i, out, st[i].fg, st[i].fg, None);
    }

    fulgurite::leaf_common!(RichText, 0);

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Paragraph);
        // The *rendered* text, not the markup: what a screen reader
        // should say is what a reader sees, and for `Markdown` those
        // differ by every asterisk.
        //
        // **A space only where the previous span has one** — M57/B6. The
        // join put a space between every pair, so "`code`." was read as
        // "code ." — the separator the layout honours (`Span::space`) and
        // the one it published disagreed.
        //
        // An image's alt is not in the paragraph: it is the label of the
        // image's own `Image` child (`Markdown`'s `observe`, M87/T87.4).
        let mut v = String::new();
        for k in 0..self.spans.len() {
            if self.spans[k].kind == Span::IMAGE {
                continue;
            }
            if k > 0 && !v.is_empty() {
                if self.spans[k].br {
                    v.push('\n');
                } else if self.spans[k - 1].space {
                    v.push(' ');
                }
            }
            v.push_str(self.span_str(k));
        }
        node.set_value(v);
    }
}

impl<S> fulgurite::event::Handlers<S> for RichText {
    fn handler(
        &self,
        _sel: fulgurite::event::NodeSel,
    ) -> Option<fn(&mut S, &fulgurite::event::EventCtx)> {
        None
    }
}

// ---- Markdown --------------------------------------------------------

/// A `Markdown`'s per-element scale, weight and colour — M87/T87.2.
///
/// Each `(scale, weight)` entry belongs to an element. A line's kind picks
/// its entry — `h1` to `h3` for `#` to `###`, `quote` for `> `, `body`
/// for everything else (a bullet included) — and the scale multiplies the
/// node's own `font_size`, so a stylesheet that resizes the widget resizes
/// its headings with it. `code` is an **inline** element and so relative
/// to its line: its scale multiplies the line's, and its weight is the
/// larger of its own and the line's, so `` `x` `` in a heading stays a
/// heading's size and weight under [`MarkdownTheme::DEFAULT`]. `**bold**`
/// is weight 700 at least, as ever, and is not an entry. `link` is the
/// same shape as `code`, and is read once links parse (M87/T87.3).
///
/// # The two colours, and why they are not the cascade's
///
/// `quote_color` and `link_color` are `None` by default, and then
/// **derived from the node's own colour** — the cascade's `color`, the
/// palette's `Text` role — so they follow a palette switch with no node
/// of their own: a quote is that colour at [`QUOTE_ALPHA`], a link that
/// colour mixed halfway toward [`LINK_TINT`]. `Some(c)` is a **fixed**
/// colour, and **does not follow the palette**: `Host::set_palette`
/// leaves it where the theme put it.
///
/// That is kill rule K87c's fallback (`docs/plan-markdown.md`), with one
/// deviation recorded there. The plan was two zero-size part nodes,
/// `markdown-link` and `markdown-quote`, seeded with the `Accent` and
/// `Muted` roles so the cascade would resolve them; built, they made
/// `--bin cascade`'s `cascade_deep (markdown x16)` row 3.4x its parent's,
/// far outside the parent's spread (`docs/results-markdown.md`, K87c).
/// The rule's fallback is fixed colour fields, and those are here — but
/// as the *override*, because no single fixed colour reads at AA's 4.5:1
/// on both the showcase's light and dark surfaces (M57's D4 gate,
/// `every_demo_text_reads_in_both_palettes`), so a fixed default would
/// fail that gate in one palette or the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkdownTheme {
    pub h1: (f32, f32),
    pub h2: (f32, f32),
    pub h3: (f32, f32),
    pub body: (f32, f32),
    pub code: (f32, f32),
    pub link: (f32, f32),
    pub quote: (f32, f32),
    /// A link's words and underline (M87/T87.3): `None` derives it from
    /// the node's colour (this type's doc), `Some` fixes it.
    pub link_color: Option<Color>,
    /// A quote's words and its bar: `None` derives it from the node's
    /// colour, `Some` fixes it.
    pub quote_color: Option<Color>,
}

/// A derived quote colour's alpha: the node's own colour at 70%, which
/// reads at AA in both built-in palettes and both of the showcase's
/// (`docs/results-markdown.md`, T87.2).
pub const QUOTE_ALPHA: u8 = 0xb3;

/// What a derived link colour is mixed halfway toward: a blue, so a link
/// reads as one in either palette while keeping most of the text colour's
/// contrast.
pub const LINK_TINT: Color = Color(0x31, 0x5c, 0xde, 0xff);

impl MarkdownTheme {
    /// A quote's colour over a node whose own colour is `fg`.
    pub fn quote_ink(&self, fg: Color) -> Color {
        self.quote_color.unwrap_or(Color(fg.0, fg.1, fg.2, (fg.3 as u16 * QUOTE_ALPHA as u16 / 255) as u8))
    }

    /// A link's colour over a node whose own colour is `fg`.
    pub fn link_ink(&self, fg: Color) -> Color {
        let mix = |a: u8, b: u8| ((a as u16 + b as u16) / 2) as u8;
        self.link_color.unwrap_or(Color(mix(fg.0, LINK_TINT.0), mix(fg.1, LINK_TINT.1), mix(fg.2, LINK_TINT.2), fg.3))
    }
}

impl MarkdownTheme {
    /// What `Markdown` drew before it had a theme: headings at 1.5, 1.25
    /// and 1.1 at weight 700, everything else 1.0 at 400. **Pinned**: the
    /// default display list is byte-identical to the one before T87.2
    /// (`tests/markdown_m69.rs`, `default_theme_is_todays`). The two
    /// colours are new, and derived from the node's — the type's doc says
    /// why.
    pub const DEFAULT: MarkdownTheme = MarkdownTheme {
        h1: (1.5, 700.0),
        h2: (1.25, 700.0),
        h3: (1.1, 700.0),
        body: (1.0, 400.0),
        code: (1.0, 400.0),
        link: (1.0, 400.0),
        quote: (1.0, 400.0),
        link_color: None,
        quote_color: None,
    };
}

impl Default for MarkdownTheme {
    fn default() -> Self {
        MarkdownTheme::DEFAULT
    }
}

/// `RichText` with a parser in front of it.
///
/// The subset is what a UI renders: `# heading` (three levels),
/// `**bold**`, `` `code` ``, `- bullet`, `> quote`, `[link](target)`,
/// `![image](name)`, and blank-line-separated paragraphs. **Not italic** — see this module's header: the
/// painter has a weight axis and no slant, and markup that renders
/// identically to plain text claims to have done something it did not.
///
/// Size, weight and the quote and link colours per element are a
/// [`MarkdownTheme`] ([`Markdown::set_theme`]); the body colour is the
/// node's own, from the cascade, and the quote and link colours derive
/// from it unless the theme fixes them. A blockquote is its words in the
/// quote colour behind a 2 px bar of the same colour, indented like a
/// bullet; a quote that wraps hangs.
///
/// # Links — M87/T87.3
///
/// `[text](target)` is a link: its words are drawn in the link colour,
/// underlined, and the markup and target are not drawn. A target holds no
/// whitespace, and `![` is an image's (T87.4), not a link's. A plain
/// `Markdown` draws links and does nothing with them; [`LinkedMarkdown`]
/// makes them **activate**: a click on a link, or Enter on the link the
/// arrows made active, runs its `on_link` handler with the link's index
/// in `ctx.row` — positional detail in the context, rule 2, the shape a
/// repeater's row handler has. The handler finds the target with
/// [`link_target`] over the application's own source, because a handler
/// receives the application's state and, under a `#[state]` split, the
/// tree is not reachable from it at all.
///
/// # Images — M87/T87.4
///
/// `![alt](name)` names an [`ImageRef`] the application hands to
/// [`Markdown::set_images`] (from a `#[bind(.., with = ..)]`, typically).
/// A name that resolves is a **block**: a line of its own at the picture's
/// size, scaled down to the content width with its aspect kept, painted
/// with `Painter::image` at frame 0 — a still, since an image here changes
/// by id. Its alt text is the label of an `Image` child (`observe`). A
/// name that does not resolve draws its alt words in the quote colour,
/// inline. Loading an image is rebinding its name to the id its pixels are
/// registered under, and damages exactly the image's rect
/// (`docs/results-markdown.md`, T87.4).
pub struct Markdown<S: 'static = (), const LINKS: bool = false> {
    inner: RichText,
    theme: MarkdownTheme,
    /// Each link's markup and target byte ranges and the spans its text
    /// became, in source order. Sized at parse time, like the span store.
    links: Vec<Link>,
    /// The link the arrows are on (`LINKS` only) — what Enter activates,
    /// and what `paint` underlines at 2 px.
    active: Option<u16>,
    on_link: fn(&mut S, &fulgurite::event::EventCtx),
    /// What `![alt](name)` can name — M87/T87.4, set by
    /// [`Markdown::set_images`]. Copied, so a binding's slice need not
    /// outlive the call.
    refs: Vec<ImageRef>,
    /// Each image's markup in source order, as the last parse found it.
    images: Vec<Img>,
}

/// One entry a `Markdown` image can name — M87/T87.4: `![alt](name)`
/// draws the picture registered under `id` (an `ImageSource`'s, such as a
/// program's `ImageStore`) at `size`, scaled down to the content width
/// with its aspect kept. **Data the application binds**: handed to
/// [`Markdown::set_images`], typically from a `#[bind(.., with = ..)]`.
///
/// An image loads by **changing id**, never by replacing pixels under one
/// already painted: `Cmd::Image` folds its rect, id, tint and frame, so
/// new pixels under an old id are no command change, no damage and no
/// frame, while a binding that names the id the pixels were registered
/// under changes the command and damages exactly the image's rect
/// (`docs/plan-markdown.md`, Decisions; `docs/results-markdown.md`, T87.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageRef {
    /// What the markup's parentheses hold: `![alt](name)`.
    pub name: &'static str,
    pub id: ImageId,
    /// The picture's natural size, in logical px.
    pub size: Size,
}

/// One image's markup — M87/T87.4. Byte offsets into the source: `!` at
/// `open`, `]` at `close`, one past `)` at `end`; `name` is what is
/// between the parentheses, and `alt` what is between the brackets.
/// `resolved` is the `ImageRef` it named, if any.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Img {
    open: u32,
    close: u32,
    end: u32,
    alt: (u32, u32),
    resolved: Option<u16>,
}

/// A `Markdown` whose links activate: `on_link` runs with the link's
/// index in `ctx.row` — M87/T87.3. Built with [`Markdown::linked`].
pub type LinkedMarkdown<S> = Markdown<S, true>;

/// One link — M87/T87.3. Byte offsets into the source: `[` at `open`, `]`
/// at `close`, the markup ends at `end` (one past `)`), and `target` is
/// what is between the parentheses. `spans` is the half-open range of
/// spans its text became.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Link {
    open: u32,
    close: u32,
    end: u32,
    target: (u32, u32),
    spans: (u32, u32),
}

/// Links past this many are drawn as text: a link index is a `u16`, and
/// [`Span::NO_LINK`] is the last one.
const MAX_LINKS: usize = Span::NO_LINK as usize;

fn no_link<S>(_: &mut S, _: &fulgurite::event::EventCtx) {}

impl<S: 'static, const LINKS: bool> Default for Markdown<S, LINKS> {
    fn default() -> Self {
        Markdown {
            inner: RichText::default(),
            theme: MarkdownTheme::DEFAULT,
            links: Vec::new(),
            active: None,
            on_link: no_link::<S>,
            refs: Vec::new(),
            images: Vec::new(),
        }
    }
}

impl Markdown<(), false> {
    /// **On the concrete type, not the generic one**: a default type
    /// parameter does not drive inference in expression position, so
    /// `Markdown::new(DOC)` must name exactly one `new`. Which is also why
    /// the linked constructor is [`Markdown::linked`] and not a second
    /// `new` — two would make every `Markdown::new` ambiguous (E0034).
    pub fn new(src: &str) -> Self {
        let mut m = Markdown::default();
        m.set_str(src);
        m
    }
}

impl<S: 'static> Markdown<S, true> {
    /// A `Markdown` whose links run `on_link` — M87/T87.3.
    pub fn linked(src: &str, on_link: fn(&mut S, &fulgurite::event::EventCtx)) -> Self {
        let mut m = Markdown { on_link, ..Markdown::default() };
        m.set_str(src);
        m
    }

    /// The link the arrows are on.
    pub fn active(&self) -> Option<u16> {
        self.active
    }
}

/// Each link in `line`, in order, as four byte offsets: where `[` is,
/// where `](` is, where the target starts, and one past `)` (the target
/// ends just before `)`). **The one scanner**, which
/// `Markdown::set_str` and [`link_target`] both call, so the index a
/// handler is handed and the target it looks up cannot disagree.
fn scan_links(line: &str, mut f: impl FnMut(usize, usize, usize, usize)) {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'[' && (i == 0 || b[i - 1] != b'!') {
            if let Some((close, t, end)) = link_from(b, i) {
                f(i, close, t, end);
                i = end;
                continue;
            }
        }
        i += 1;
    }
}

/// Each image in `line`, in order: where `!` is, where `](` is, where the
/// name starts, and one past `)` — M87/T87.4. The same shape as a link
/// after its `!`, found by the same [`link_from`]; [`scan_links`] skips a
/// `[` straight after `!`, so the two never claim one bracket.
fn scan_images(line: &str, mut f: impl FnMut(usize, usize, usize, usize)) {
    let b = line.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'!' && b[i + 1] == b'[' {
            if let Some((close, t, end)) = link_from(b, i + 1) {
                f(i, close, t, end);
                i = end;
                continue;
            }
        }
        i += 1;
    }
}

/// The link opening at `open`, if one does: a non-empty text with no `[`
/// in it, `](` straight after, and a non-empty target with no whitespace
/// up to `)`.
fn link_from(b: &[u8], open: usize) -> Option<(usize, usize, usize)> {
    let mut j = open + 1;
    while j < b.len() && b[j] != b']' {
        if b[j] == b'[' {
            return None;
        }
        j += 1;
    }
    if j >= b.len() || j == open + 1 || b.get(j + 1) != Some(&b'(') {
        return None;
    }
    let t = j + 2;
    let mut k = t;
    while k < b.len() && b[k] != b')' {
        if b[k].is_ascii_whitespace() {
            return None;
        }
        k += 1;
    }
    (k < b.len() && k > t).then_some((j, t, k + 1))
}

/// Link `n`'s target in `src` — M87/T87.3: the `n`th `[text](target)` in
/// source order, counted the way `Markdown` counts it (the same scanner
/// over the same lines), so a `LinkedMarkdown` handler handed `n` in
/// `ctx.row` reads the target from the source its application already
/// holds. `None` past the last link.
pub fn link_target(src: &str, n: usize) -> Option<&str> {
    let mut seen = 0usize;
    let mut at = 0usize;
    let mut found = None;
    for line in src.split('\n') {
        let base = at;
        at += line.len() + 1;
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if found.is_some() || trimmed.is_empty() {
            continue;
        }
        scan_links(trimmed, |_, _, t, end| {
            if seen == n && found.is_none() && seen < MAX_LINKS {
                found = Some((base + indent + t, base + indent + end - 1));
            }
            seen += 1;
        });
    }
    found.and_then(|(a, b)| src.get(a..b))
}

impl<S: 'static, const LINKS: bool> Markdown<S, LINKS> {

    /// The rendered text and its spans.
    pub fn rich(&self) -> &RichText {
        &self.inner
    }

    /// How many links the source holds.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Link `n`'s target — the same answer [`link_target`] gives over the
    /// source, for a test or an `after_update` that holds the widget.
    pub fn link(&self, n: usize) -> Option<&str> {
        let l = self.links.get(n)?;
        self.inner.text.get(l.target.0 as usize..l.target.1 as usize)
    }

    /// Link `n`'s text as drawn, words joined as the layout spaces them.
    pub fn link_text(&self, n: usize) -> String {
        let mut out = String::new();
        if let Some(l) = self.links.get(n) {
            for k in l.spans.0 as usize..(l.spans.1 as usize).min(self.inner.spans.len()) {
                if k > l.spans.0 as usize && self.inner.spans[k - 1].space {
                    out.push(' ');
                }
                out.push_str(self.inner.span_str(k));
            }
        }
        out
    }

    /// How many images the source holds, resolved or not.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Image `n`'s alt text, as the source wrote it.
    pub fn image_alt(&self, n: usize) -> Option<&str> {
        let m = self.images.get(n)?;
        self.inner.text.get(m.alt.0 as usize..m.alt.1 as usize)
    }

    /// Whether image `n` named an [`ImageRef`] and is drawn as a picture
    /// (otherwise its alt is drawn as words).
    pub fn image_resolved(&self, n: usize) -> bool {
        self.images.get(n).is_some_and(|m| m.resolved.is_some())
    }

    /// Image `n`'s box from the node's origin, as the last layout placed
    /// it ([`RichText::image_rect`]). `None` for an unresolved image, whose
    /// alt is drawn as words and has no box of its own.
    pub fn image_rect(&self, n: usize) -> Option<[f32; 4]> {
        if !self.image_resolved(n) {
            return None;
        }
        let drawn = self.images[..n].iter().filter(|m| m.resolved.is_some()).count();
        self.inner.image_rect(drawn)
    }

    /// Replace what `![alt](name)` can name, and re-parse if it changed —
    /// M87/T87.4. **Resolution happens here as well as in `set_str`**: a
    /// binding that names a new id for an image already parsed must change
    /// the span store, or the command is the old one and loading the image
    /// is no frame (`tests/markdown_images.rs` in the shell). The same refs
    /// again are free, so a binding that runs on every update costs a
    /// comparison; the store keeps its capacity, so a change of id
    /// allocates nothing after the first.
    pub fn set_images(&mut self, refs: &[ImageRef]) {
        if self.refs[..] == *refs {
            return;
        }
        self.refs.clear();
        self.refs.extend_from_slice(refs);
        let src = core::mem::take(&mut self.inner.text);
        self.set_str(&src);
    }

    /// The theme in force.
    pub fn theme(&self) -> MarkdownTheme {
        self.theme
    }

    /// Replace the theme, and re-parse if it changed — a span's scale and
    /// weight are decided when it is parsed, so a theme that only took
    /// effect at the next `set_str` would be a setter that did nothing
    /// visible. The same theme again is free.
    pub fn set_theme(&mut self, theme: MarkdownTheme) {
        if theme == self.theme {
            return;
        }
        self.theme = theme;
        let src = core::mem::take(&mut self.inner.text);
        self.set_str(&src);
    }

    /// Re-parse. The editor calls this on every keystroke, which is why
    /// the parser reuses its stores' capacity and **walks once**: each
    /// line's byte offset is carried from the last, where until M87/T87.1
    /// it was found by re-scanning from byte 0 for every line — lines ×
    /// bytes, T45.10's quadratic again (`tests/markdown_m69.rs`,
    /// `set_str_is_linear`).
    pub fn set_str(&mut self, src: &str) {
        self.inner.text.clear();
        self.inner.text.push_str(src);
        self.inner.clear();
        self.links.clear();
        self.images.clear();
        let text = core::mem::take(&mut self.inner.text);
        let theme = self.theme;
        let mut blank_pending = false;
        let mut next = 0usize;
        for line in text.split('\n') {
            let base = next;
            next += line.len() + 1;
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if trimmed.is_empty() {
                // A blank line is a blank line: remembered rather than
                // emitted, so a document that ends with one does not
                // render a trailing empty row.
                blank_pending = !self.inner.spans.is_empty();
                continue;
            }
            if blank_pending {
                // An empty span whose `br` opens a line nothing is
                // drawn on — which is what a paragraph gap *is*. It
                // measures zero, so it cannot push anything over the
                // width.
                let mut gap = Span::plain((0, 0));
                gap.br = true;
                gap.space = false;
                self.inner.push(gap);
                blank_pending = false;
            }
            // The line's kind: its theme entry, the bytes its marker
            // takes, whether its first word is indented (a bullet's dot,
            // a quote's bar), and the span kind its words carry.
            let ((scale, weight), skip, lead, kind) = match heading(trimmed, &theme) {
                Some((sw, skip)) => (sw, skip, false, Span::TEXT),
                None if trimmed.starts_with("- ") => (theme.body, 2, true, Span::TEXT),
                None if trimmed.starts_with("> ") => (theme.quote, 2, true, Span::QUOTE),
                None => (theme.body, 0, false, Span::TEXT),
            };
            let body = &trimmed[skip..];
            let body_at = base + indent + skip;
            // This line's links, found before its words are: a link's text
            // can span words, and its markup is stripped wherever a word
            // reaches it, like a `**` pair (M87/T87.3).
            let mut next_link = self.links.len();
            let line_at = base + indent;
            let links = &mut self.links;
            scan_links(trimmed, |o, c, t, e| {
                if links.len() < MAX_LINKS {
                    links.push(Link {
                        open: (line_at + o) as u32,
                        close: (line_at + c) as u32,
                        end: (line_at + e) as u32,
                        target: ((line_at + t) as u32, (line_at + e - 1) as u32),
                        spans: (0, 0),
                    });
                }
            });
            // And its images (M87/T87.4): each resolved against the refs
            // now, so a `set_images` that re-parses re-resolves.
            let mut next_image = self.images.len();
            let (images, refs, txt) = (&mut self.images, &self.refs, text.as_str());
            scan_images(trimmed, |o, c, t, e| {
                let name = &txt[line_at + t..line_at + e - 1];
                images.push(Img {
                    open: (line_at + o) as u32,
                    close: (line_at + c) as u32,
                    end: (line_at + e) as u32,
                    alt: ((line_at + o + 2) as u32, (line_at + c) as u32),
                    resolved: refs.iter().position(|r| r.name == name).map(|n| n as u16),
                });
            });
            let mut in_image: Option<usize> = None;
            // Bytes inside a resolved image's markup, which became one
            // span and draw nothing else; and whether the next span must
            // start a line because a block came before it.
            let mut skip_until = 0usize;
            let mut after_block = false;
            let mut in_link: Option<usize> = None;
            // **Carried across words, because a span is a word.**
            // `**two whole words**` opens on the first and closes on
            // the last, and every word between them is bold too — which
            // a per-word strip cannot express and which the first
            // version of this got wrong for any run longer than two.
            let mut bold = false;
            let mut code = false;
            let mut first_in_line = true;
            let mut at = 0usize;
            while at < body.len() {
                let ws = body[at..].len() - body[at..].trim_start().len();
                at += ws;
                if at >= body.len() {
                    break;
                }
                let end =
                    body[at..].find(char::is_whitespace).map(|k| at + k).unwrap_or(body.len());
                let word = &body[at..end];
                // **Markers are found *inside* the word, not stripped
                // off its ends.** `**bold**,` is the ordinary case —
                // emphasis followed by punctuation — and an end-strip
                // never finds the closing pair, so the markers render
                // and every word after stays bold. The first render of
                // this demo said `Headings, bold**, code and bullets
                // are...` in bold to the end of the paragraph, with
                // sixteen green tests behind it, because every fixture
                // wrote `**bold** word` and none wrote `**bold**,`.
                //
                // Scanning also means a marker *toggles*: one loop
                // handles opening and closing, and `a**b**c` works
                // without a rule about which end a marker belongs to.
                let mut seg = 0usize;
                let mut k = 0usize;
                let mut first_seg = true;
                loop {
                    let p = (body_at + at + k) as u32;
                    if (p as usize) < skip_until {
                        if k >= word.len() {
                            break;
                        }
                        k += word[k..].chars().next().map(char::len_utf8).unwrap_or(1);
                        seg = k;
                        continue;
                    }
                    // An image's `![` opens it; an unresolved one's
                    // `](name)` closes it. A resolved one is a single span
                    // pushed at its `![`, and the rest of its markup is
                    // skipped (M87/T87.4).
                    let img_mark = match in_image {
                        Some(n) if self.images[n].close == p => Some((self.images[n].end - p) as usize),
                        None if in_link.is_none()
                            && next_image < self.images.len()
                            && self.images[next_image].open == p => Some(2),
                        _ => None,
                    };
                    // A link's `[` opens it and its `](target)` closes it:
                    // both are markers, never drawn.
                    let link_mark = match in_link {
                        _ if img_mark.is_some() => None,
                        Some(n) if self.links[n].close == p => Some((self.links[n].end - p) as usize),
                        None if next_link < self.links.len() && self.links[next_link].open == p => Some(1),
                        _ => None,
                    };
                    let marker = if img_mark.is_some() {
                        img_mark
                    } else if link_mark.is_some() {
                        link_mark
                    } else if word[k..].starts_with("**") {
                        Some(2usize)
                    } else if word[k..].starts_with('`') {
                        Some(1usize)
                    } else {
                        None
                    };
                    let done = k >= word.len();
                    if done || marker.is_some() {
                        if k > seg {
                            let mut sp = Span::plain(((body_at + at + seg) as u32,
                                                      (body_at + at + k) as u32));
                            // Inline elements are relative to their line
                            // (`MarkdownTheme`): a code word multiplies
                            // the line's scale and is at least its weight.
                            let mut w = weight;
                            sp.scale = scale;
                            if code {
                                sp.scale = scale * theme.code.0;
                                w = w.max(theme.code.1);
                            }
                            if let Some(n) = in_link {
                                sp.scale *= theme.link.0;
                                w = w.max(theme.link.1);
                                sp.link = n as u16;
                            }
                            if bold {
                                w = w.max(700.0);
                            }
                            sp.weight = w;
                            sp.code = code;
                            sp.kind = if in_image.is_some() { Span::ALT } else { kind };
                            sp.br = (first_seg && first_in_line || after_block) && !self.inner.spans.is_empty();
                            sp.bullet = lead && first_seg && first_in_line && !after_block;
                            after_block = false;
                            // Only the word's last segment takes the
                            // gap: `**bold**,` is one word and must not
                            // render as `bold ,`.
                            sp.space = false;
                            self.inner.push(sp);
                            first_seg = false;
                        }
                        match (img_mark, link_mark, marker) {
                            (Some(_), _, _) => match in_image {
                                Some(_) => in_image = None,
                                None => {
                                    let m = self.images[next_image];
                                    next_image += 1;
                                    match m.resolved {
                                        Some(r) => {
                                            // One block span for the
                                            // whole markup; its range is
                                            // the alt text.
                                            let mut sp = Span::plain((m.alt.0, m.alt.1));
                                            sp.kind = Span::IMAGE;
                                            sp.space = false;
                                            sp.br = !self.inner.spans.is_empty();
                                            self.inner.push(sp);
                                            let r = self.refs[r as usize];
                                            self.inner.images.push((r.id, r.size));
                                            skip_until = m.end as usize;
                                            after_block = true;
                                            first_seg = false;
                                        }
                                        None => in_image = Some(next_image - 1),
                                    }
                                }
                            },
                            (None, Some(_), _) => match in_link {
                                Some(n) => {
                                    self.links[n].spans.1 = self.inner.spans.len() as u32;
                                    in_link = None;
                                }
                                None => {
                                    self.links[next_link].spans = (self.inner.spans.len() as u32, self.inner.spans.len() as u32);
                                    in_link = Some(next_link);
                                    next_link += 1;
                                }
                            },
                            (None, None, Some(2)) => bold = !bold,
                            (None, None, Some(_)) => code = !code,
                            (None, None, None) => break,
                        }
                        k += marker.unwrap_or(0);
                        seg = k;
                        continue;
                    }
                    k += word[k..].chars().next().map(char::len_utf8).unwrap_or(1);
                }
                // The gap belongs to the word, so it is set on whatever
                // segment ended up last.
                if end < body.len() {
                    if let Some(last) = self.inner.spans.last_mut() {
                        last.space = true;
                    }
                }
                first_in_line = false;
                at = end;
            }
        }
        self.inner.text = text;
        self.inner.fit();
        if self.active.is_some_and(|a| a as usize >= self.links.len()) {
            self.active = None;
        }
    }
}

/// `#`, `##`, `###` — the theme's entry for the level, and the bytes the
/// marker takes.
fn heading(line: &str, theme: &MarkdownTheme) -> Option<((f32, f32), usize)> {
    for (marker, sw) in [("### ", theme.h3), ("## ", theme.h2), ("# ", theme.h1)] {
        if line.starts_with(marker) {
            return Some((sw, marker.len()));
        }
    }
    None
}

/// `Markdown`'s table and seed, shared by both of its forms: one node,
/// focusable when its links activate (the arrows walk them, Enter runs
/// one).
const MD_TABLE: [NodeDesc; 1] = [NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
const MD_TABLE_LINKED: [NodeDesc; 1] = [NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
const MD_SEEDS: [Seed; 1] = [Seed::new(
    "markdown",
    <RichText as Widget>::type_defaults, Fields::of(&[
    Fields::WIDTH,
    Fields::HEIGHT,
    Fields::PADDING,
    Fields::BACKGROUND,
    Fields::BORDER,
    Fields::COLOR,
    Fields::FONT_SIZE,
    Fields::FONT_WEIGHT,
    Fields::RADIUS,
    Fields::VISIBLE,
])).with_roles(fulgurite::style::SeedRoles::NONE.color(fulgurite::style::Role::Text))];

impl<S: 'static, const LINKS: bool> Widget for Markdown<S, LINKS> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = if LINKS { &MD_TABLE_LINKED } else { &MD_TABLE };
    const KIND: &'static str = "markdown";
    const SEEDS: &'static [Seed] = &MD_SEEDS;

    fn type_defaults() -> Style {
        <RichText as Widget>::type_defaults()
    }
}

impl<S: 'static, const LINKS: bool> Node for Markdown<S, LINKS> {
    fulgurite::node_count!();
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.inner.layout(cx, limits, style, st, i)
    }

    /// Forwarded — M41/T41.7. `Node::probe`'s provided body is this
    /// wrapper's `layout`, which lays the inner `RichText` out and so
    /// never reached its `probe`: a provided method acts on the wrapper.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.inner.probe(cx, limits, style, st, i)
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        self.inner.place(at, style, st, i)
    }

    /// The inner `RichText`'s paint, with a quote and a link in the
    /// theme's colours over this node's own (`MarkdownTheme::quote_ink`,
    /// `link_ink`), and the active link underlined at 2 px.
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let fg = st[i].fg;
        let active = if LINKS { self.active } else { None };
        self.inner.draw(st, i, out, self.theme.quote_ink(fg), self.theme.link_ink(fg), active)
    }

    /// The node, and — for a `LinkedMarkdown`, over a link's words — the
    /// link: `inner: Some(Inner { row: n, node: 0 })`, so the pass sets
    /// `ctx.row = Some(n)` and asks [`Handlers::handler`] with it, and the
    /// press latch refuses a press on one link released on another (rows
    /// differ). Over plain text, `inner` is `None`, no handler answers,
    /// and the press bubbles as it always did.
    ///
    /// [`Handlers::handler`]: fulgurite::event::Handlers::handler
    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<fulgurite::Hit> {
        let h = self.inner.hit(st, i, x, y)?;
        if LINKS {
            if let Some(n) = self.inner.link_at(h.local.x, h.local.y) {
                return Some(fulgurite::Hit {
                    inner: Some(fulgurite::hit::Inner { row: n as u64, node: 0 }),
                    ..h
                });
            }
        }
        Some(h)
    }

    /// `Pointer` over a link of a `LinkedMarkdown` (M57/B48's hook, from
    /// the same [`RichText::link_at`] the hit asks), the style's cursor
    /// elsewhere.
    fn cursor_at(&self, local: Point) -> Option<fulgurite::style::Cursor> {
        (LINKS && self.inner.link_at(local.x, local.y).is_some()).then_some(fulgurite::style::Cursor::Pointer)
    }

    /// Up and Down walk a `LinkedMarkdown`'s links, so the arrows never
    /// move focus off it (`owns_arrows`).
    fn owns_arrows(&self) -> bool {
        LINKS
    }

    /// The link the arrows made active, as the row Enter activates —
    /// M44/T44.10's path: the pass builds the selection a click on that
    /// link would have and sets `ctx.row` to it. `None` for a plain
    /// `Markdown`, and before the first arrow.
    fn active_row(&self) -> Option<fulgurite::hit::Inner> {
        if !LINKS {
            return None;
        }
        self.active.map(|n| fulgurite::hit::Inner { row: n as u64, node: 0 })
    }

    /// Down to the next link (the first, from none), Up to the previous.
    /// Answers whether the active link moved, which is also the mark.
    fn on_event(&mut self, ev: &fulgurite::Event, _cx: &fulgurite::event::EventCtx) -> bool {
        use fulgurite::event::Key;
        if !LINKS || self.links.is_empty() {
            return false;
        }
        let last = (self.links.len() - 1) as u16;
        let next = match ev {
            fulgurite::Event::Key { key: Key::Down, down: true } => {
                Some(self.active.map_or(0, |a| (a + 1).min(last)))
            }
            fulgurite::Event::Key { key: Key::Up, down: true } => {
                Some(self.active.map_or(0, |a| a.saturating_sub(1)))
            }
            _ => return false,
        };
        let moved = next != self.active;
        self.active = next;
        moved
    }

    fn node(&self, _k: usize) -> &dyn Node {
        self
    }

    fn node_mut(&mut self, _k: usize) -> &mut dyn Node {
        self
    }

    /// The rendered text as one `Paragraph` value; one `Image` child per
    /// drawn image (M87/T87.4: its alt as the label, its box as the
    /// bounds, id `instance_id(id, n, 1)`); and for a
    /// `LinkedMarkdown` one `Link` child per link: its text as the label,
    /// the bounds of its first word, a `Click` action, and the id
    /// `instance_id(id, n, 0)` — so the agent's `~=` finds a link by its
    /// text and a platform click lands on it.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        st: &[NodeState],
        i: usize,
        id: u64,
        node: &mut accesskit::Node,
        extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        self.inner.observe(st, i, id, node, extra);
        let s = &st[i];
        if s.size.w <= 0.0 || s.size.h <= 0.0 {
            return;
        }
        // M87/T87.4: each drawn image is an `Image` child, for either form
        // — its alt text the label, its placed box the bounds — so a reader
        // meets the picture where it is, and the paragraph's value (which
        // leaves image spans out) does not say the alt twice. An
        // unresolved image draws its alt as words, which the value holds.
        for n in 0..self.images.len() {
            let Some(r) = self.image_rect(n) else { continue };
            let mut c = accesskit::Node::new(accesskit::Role::Image);
            c.set_label(self.image_alt(n).unwrap_or(""));
            c.set_bounds(accesskit::Rect {
                x0: (s.pos.x + r[0]) as f64,
                y0: (s.pos.y + r[1]) as f64,
                x1: (s.pos.x + r[2]) as f64,
                y1: (s.pos.y + r[3]) as f64,
            });
            let cid = accesskit::NodeId(fulgurite::observe::instance_id(id, n as u64, 1));
            extra.push((cid, c));
            node.push_child(cid);
        }
        if !LINKS {
            return;
        }
        for (n, l) in self.links.iter().enumerate() {
            if l.spans.1 <= l.spans.0 || l.spans.0 as usize >= self.inner.placed.len() {
                continue;
            }
            let r = self.inner.span_rect(l.spans.0 as usize);
            let mut c = accesskit::Node::new(accesskit::Role::Link);
            c.set_label(self.link_text(n));
            c.set_bounds(accesskit::Rect {
                x0: (s.pos.x + r[0]) as f64,
                y0: (s.pos.y + r[1]) as f64,
                x1: (s.pos.x + r[2]) as f64,
                y1: (s.pos.y + r[3]) as f64,
            });
            c.add_action(accesskit::Action::Click);
            let cid = accesskit::NodeId(fulgurite::observe::instance_id(id, n as u64, 0));
            extra.push((cid, c));
            node.push_child(cid);
        }
    }
}

impl<S> fulgurite::event::Handlers<S> for Markdown<(), false> {
    fn handler(
        &self,
        _sel: fulgurite::event::NodeSel,
    ) -> Option<fn(&mut S, &fulgurite::event::EventCtx)> {
        None
    }
}

/// `on_link`, for a selection that names one of this widget's links —
/// never for the node itself (a click on plain text), and never for a
/// dismissal or visibility question, whose sentinel rows are past any
/// link.
impl<S: 'static> fulgurite::event::Handlers<S> for Markdown<S, true> {
    fn handler(
        &self,
        sel: fulgurite::event::NodeSel,
    ) -> Option<fn(&mut S, &fulgurite::event::EventCtx)> {
        match sel.inner {
            Some((row, 0)) if (row as usize) < self.links.len() => Some(self.on_link),
            _ => None,
        }
    }
}

// ---- the editor ------------------------------------------------------

pub const PART_INPUT: &str = "editor-input";
pub const PART_PREVIEW: &str = "editor-preview";

/// Table slots: this node, the `TextArea`, the `Markdown` preview.
pub const MAX_EDITOR_NODES: usize = 3;

/// A markup editor: a text area over a live preview.
///
/// **It edits Markdown, it does not edit a document model.** A rich
/// editor that stored styled runs and mutated them on a selection needs
/// a whole second data structure and an undo stack over it; an editor
/// that edits *markup* has one source of truth, round-trips through a
/// file, and survives a hot reload as a `String`. The formatting
/// commands are therefore text edits — [`RichTextEditor::bold`] wraps
/// the selection in `**` — which is what every Markdown editor does and
/// what makes this one testable without a document model to assert
/// against.
#[derive(Default)]
pub struct RichTextEditor {
    pub input: crate::textarea::TextArea,
    pub preview: Markdown,
    /// What the preview was last built from, so a re-parse happens when
    /// the text changes and not on every frame.
    seen: String,
}

impl RichTextEditor {
    const INPUT: usize = 1;
    const PREVIEW: usize = 2;

    pub fn new(src: &str) -> Self {
        let mut e = RichTextEditor {
            input: crate::textarea::TextArea::new(src),
            ..Default::default()
        };
        e.sync();
        e
    }

    /// Re-parse the preview if the text changed. Answers whether it did.
    pub fn sync(&mut self) -> bool {
        if self.seen == self.input.as_str() {
            return false;
        }
        self.seen = self.input.as_str().to_string();
        self.preview.set_str(&self.seen);
        true
    }

    /// Wrap the selection in `**`, or insert an empty pair at the caret.
    ///
    /// Returns whether anything was written. A command that reports
    /// success for having done nothing is the shape
    /// `docs/plan-agent-parity.md` calls finished-but-unreachable, and a
    /// toolbar button that lies about it is the same bug with a face.
    pub fn bold(&mut self) -> bool {
        self.wrap("**")
    }

    /// The same with backticks.
    pub fn code(&mut self) -> bool {
        self.wrap("`")
    }

    fn wrap(&mut self, marker: &str) -> bool {
        match self.input.selected_str() {
            Some(sel) => {
                let wrapped = alloc::format!("{marker}{sel}{marker}");
                self.input.insert(&wrapped)
            }
            None => self.input.insert(&alloc::format!("{marker}{marker}")),
        }
    }
}

pub struct EditorTable;

impl EditorTable {
    const ARRAY: [NodeDesc; MAX_EDITOR_NODES] = {
        let mut t = [NodeDesc::EMPTY; MAX_EDITOR_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        t[1] = NodeDesc::new(0, 0, 2, 0, NodeDesc::FOCUSABLE).with_id(PART_INPUT);
        t[2] = NodeDesc::new(0, 0, 0, 0, 0).with_id(PART_PREVIEW);
        t
    };
    pub const SLICE: &'static [NodeDesc] = &Self::ARRAY;

    const SEED_ARRAY: [Seed; MAX_EDITOR_NODES] = {
        let mut s = [Seed::WIDGET; MAX_EDITOR_NODES];
        s[0] = Seed::new(
            <RichTextEditor as Widget>::KIND,
            <RichTextEditor as Widget>::type_defaults, Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::SPACING, Fields::VISIBLE]));
        s[1] = <crate::textarea::TextArea as Widget>::SEEDS[0];
        s[2] = <Markdown as Widget>::SEEDS[0];
        s
    };
    pub const SEED_SLICE: &'static [Seed] = &Self::SEED_ARRAY;
}

impl Widget for RichTextEditor {
    const NODE_COUNT: usize = MAX_EDITOR_NODES;
    const DEPTH: usize = 2;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = EditorTable::SLICE;
    const KIND: &'static str = "richtexteditor";
    const SEEDS: &'static [Seed] = EditorTable::SEED_SLICE;

    fn type_defaults() -> Style {
        Style { width: Length::FILL, height: Length::Shrink, spacing: 8.0, ..Style::default() }
    }
}

impl Node for RichTextEditor {
    fulgurite::node_count!();
    fn kind_name(&self) -> &'static str {
        <Self as Widget>::KIND
    }

    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return fulgurite::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        // **The preview is rebuilt here, not by whoever typed.** Every
        // path into the text — a keystroke, a paste, `bold()`, the
        // application setting it — lands in the same place, which is
        // the argument `Host::pump`'s derived-presentation step makes
        // one layer up: doing it in the loop is what makes it
        // impossible to forget.
        self.sync();
        let a = self.input.layout(cx, limits, style, st, i + Self::INPUT);
        let b = self.preview.layout(cx, limits, style, st, i + Self::PREVIEW);
        let s = fulgurite::ops::resolve_self(
            limits,
            &me,
            Size { w: a.w.max(b.w), h: a.h + me.spacing + b.h },
        );
        fulgurite::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.input.place(origin, style, st, i + Self::INPUT);
        let below = st[i + Self::INPUT].size.h + style[i].spacing;
        self.preview.place(
            Point { x: origin.x, y: origin.y + below },
            style,
            st,
            i + Self::PREVIEW,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.input.paint(st, i + Self::INPUT, out);
        self.preview.paint(st, i + Self::PREVIEW, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<fulgurite::Hit> {
        self.input.hit(st, i + Self::INPUT, x, y)
    }

    /// **Formatting from the keyboard** — M57/B8. [`RichTextEditor::bold`]
    /// and [`RichTextEditor::code`] existed and nothing called them: the
    /// editor had no formatting UI at all. Ctrl+B and Ctrl+E (⌘ on a Mac)
    /// reach here because the text area declines a shortcut it does not
    /// know, and the pass walks up to its container.
    fn on_event(&mut self, ev: &fulgurite::Event, cx: &fulgurite::event::EventCtx) -> bool {
        use fulgurite::event::{Key, Mods};
        let short = cx.mods.has(Mods::CTRL) || cx.mods.has(Mods::META);
        match ev {
            fulgurite::Event::Key { key: Key::Char('b'), down: true } if short => self.bold(),
            fulgurite::Event::Key { key: Key::Char('e'), down: true } if short => self.code(),
            _ => false,
        }
    }

    fn node(&self, k: usize) -> &dyn Node {
        match k {
            0 => self,
            1 => &self.input,
            _ => &self.preview,
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        match k {
            0 => self,
            1 => &mut self.input,
            _ => &mut self.preview,
        }
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::TextInput);
        node.set_value(self.input.as_str().to_string());
    }
}

impl<S> fulgurite::event::Handlers<S> for RichTextEditor {
    fn handler(
        &self,
        _sel: fulgurite::event::NodeSel,
    ) -> Option<fn(&mut S, &fulgurite::event::EventCtx)> {
        None
    }
}

// ---- PaintBox --------------------------------------------------------

/// A box the application draws into.
///
/// **A plain `fn` pointer, not a closure** — the rule the whole
/// architecture turns on (`docs/events-decision.md`): a `Box<dyn FnMut>`
/// is a pointer into the old dylib after a reload, and a callback that
/// captured its data would get whatever `fresh()` handed it. A `fn` that
/// takes the painter and the rect can be re-supplied by name, and
/// everything it needs to draw arrives as an argument.
///
/// Named `PaintBox` and not `Canvas`: `fulgurite::Canvas` is the
/// software rasterizer and one name for two things is how a reviewer
/// ends up reading the wrong file (M27/T27.2).
pub struct PaintBox {
    /// Called with the widget's rect, in window coordinates.
    ///
    /// **Clipped to that rect, not to its rounded corners** — M57/B9. The
    /// ground is drawn with the style's `radius` (8 by default), and the
    /// clip is the box: the painter's clip is a rectangle, and every
    /// backend would need a rounded one for this one widget. So ink drawn
    /// into a corner shows past the ground's curve. A drawing that reaches
    /// its edges should inset itself by the radius, or the application
    /// should set `radius: 0`.
    pub draw: fn(&mut dyn Painter, [f32; 4]),
}

fn draw_nothing(_: &mut dyn Painter, _: [f32; 4]) {}

impl Default for PaintBox {
    fn default() -> Self {
        PaintBox { draw: draw_nothing }
    }
}

impl PaintBox {
    pub fn new(draw: fn(&mut dyn Painter, [f32; 4])) -> Self {
        PaintBox { draw }
    }
}

impl Widget for PaintBox {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "paintbox";
    const SEEDS: &'static [Seed] = &[Seed::new(
        <PaintBox as Widget>::KIND,
        <PaintBox as Widget>::type_defaults, Fields::of(&[
        Fields::WIDTH,
        Fields::HEIGHT,
        Fields::BACKGROUND,
        Fields::BORDER,
        Fields::BACKGROUND_FILL,
        Fields::RADIUS,
        Fields::VISIBLE,
    ])).with_roles(fulgurite::style::SeedRoles::NONE.background(fulgurite::style::Role::Raised))];

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(160.0),
            background: Color(0xff, 0xff, 0xff, 0xff),
            radius: 8.0,
            ..Style::default()
        }
    }
}

impl Node for PaintBox {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return fulgurite::ops::hide_subtree(st, i, limits, 1);
        }
        let s = fulgurite::ops::resolve_self(
            limits,
            &me,
            Size { w: limits.max_w, h: limits.max_h },
        );
        fulgurite::ops::record_painted(st, i, limits, s, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = fulgurite::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        fulgurite::ops::fill(st, i, out);
        if s.bg.3 > 0 {
            out.rounded_rect(r, s.bg, s.radius);
        }
        fulgurite::ops::border(st, i, out);
        // **Clipped, because the callback is the application's.** It is
        // handed a rect and nothing stops it drawing outside one; a
        // `Tree` that painted 116 px past its own box is the same
        // mistake made by code this repository owns (M27/T27.11). The
        // clip is the rectangle, not the rounded ground: see `draw`.
        out.push_clip(r);
        (self.draw)(out, r);
        out.pop_clip();
    }

    fulgurite::leaf_common!(PaintBox, 0);

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Canvas);
    }
}

impl<S> fulgurite::event::Handlers<S> for PaintBox {
    fn handler(
        &self,
        _sel: fulgurite::event::NodeSel,
    ) -> Option<fn(&mut S, &fulgurite::event::EventCtx)> {
        None
    }
}
