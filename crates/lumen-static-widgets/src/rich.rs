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
//! (`crates/lumen-static-shell/tests/alloc_gate.rs`, the Markdown arm).
//! Until T87.1 they were arrays of 192 words held inline, and `push`
//! refused the 193rd silently: a 64 KB document laid out its first 192
//! words, and any probe of it timed a truncated parse. The widget's
//! inline size and the heap per word are `docs/results-markdown.md`'s
//! (T87.1). This crate's `rich` module is behind `alloc`, and the leaf
//! firmware links none of it.
//!
//! # What the Markdown subset leaves out, and why
//!
//! **Italic.** [`lumen_static::Painter`] has one text method and its
//! only typographic axis is *weight*: there is no italic face and no
//! synthetic slant. A parser that accepted `*emphasis*` and rendered it
//! identically to plain text would be telling the reader it had done
//! something. So `*` is not markup here, and the gap is the text
//! stack's rather than this widget's — a slant belongs beside
//! `FontStack`, next to the weight axis it would join.
//!
//! Headings, `**bold**`, `` `code` ``, bullets and paragraphs are the
//! subset, which is what a UI actually renders: labels, emphasis, an
//! identifier, a list.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

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
    /// What the word is part of: [`Span::TEXT`] or [`Span::QUOTE`] —
    /// M87/T87.2. A quote's words are drawn in the quote colour, which a
    /// `Markdown` reads from its `markdown-quote` part, and a quote line's
    /// first word sets `bullet` for its indent and draws a bar instead of
    /// a dot.
    pub kind: u8,
}

impl Span {
    /// An ordinary word.
    pub const TEXT: u8 = 0;
    /// A word of a blockquote (`> ` at a line's start) — M87/T87.2.
    pub const QUOTE: u8 = 1;

    pub const fn plain(range: (u32, u32)) -> Span {
        Span { range, scale: 1.0, weight: 400.0, code: false, br: false, bullet: false, space: true, kind: Span::TEXT }
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
    widths: Vec<(f32, f32)>,
    measured_under: u64,
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
            eat(&[s.space as u8]);
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

    /// Append a span. **Never refuses** since M87/T87.1: the store grows,
    /// which is a parse-time allocation, never a layout-time one
    /// (`fit` sizes the other two stores to match).
    pub fn push(&mut self, s: Span) {
        self.spans.push(s);
    }

    /// Forget every span, keeping the stores' capacity for the next parse.
    pub fn clear(&mut self) {
        self.spans.clear();
    }

    /// Size `placed` and `widths` to `spans` — called once a parse is
    /// done, and by `layout` only when a span was pushed since, so a still
    /// frame resizes nothing. Shrinking keeps the capacity; growing
    /// allocates, at parse time.
    fn fit(&mut self) {
        let n = self.spans.len();
        if self.placed.len() != n {
            self.placed.resize(n, (0.0, 0.0, 0.0));
        }
        if self.widths.len() != n {
            self.widths.resize(n, (0.0, 0.0));
            // A store that changed length holds no widths for the new
            // spans, so the memo cannot be trusted whatever the fold says.
            self.measured_under = 0;
        }
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
/// (`cap_shift` in `lumen-static-shell`'s text module), so the middle of a
/// lower-case letter is below it by half of cap height less x-height:
/// (0.714 − 0.536) / 2 em for Noto Sans, plus the 0.08 em the run's box
/// sits below the dot's box here. The dot was centred on the box and read
/// 2.5 px high at 15 px; `tests/markdown_bullet.rs` in the shell checks
/// the dot against a rasterised `x`, since this is a fact about ink.
const BULLET_DROP: f32 = 0.17;
/// Padding around a `code` chip.
const CHIP: f32 = 3.0;

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
    ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];

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
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
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
            for k in 0..count {
                let s = self.spans[k];
                let size = me.font_size * s.scale;
                let w = cx.text.measure_weighted(self.span_str(k), size, s.weight, f32::INFINITY).w;
                let gap = if s.space { cx.text.measure_weighted(" ", size, s.weight, f32::INFINITY).w } else { 0.0 };
                self.widths[k] = (w, gap);
            }
            self.measured_under = key;
        }
        let (mut x, mut y) = (0.0f32, 0.0f32);
        // **A wrapped bullet hangs** (M57/B7): its continuation lines
        // start under its text, not under the dot. Set by a bullet span,
        // cleared by the next line break.
        let mut hang = 0.0f32;
        let ratio = line_ratio(me.line_height);
        let mut line_h = me.font_size * ratio;
        self.lines = if count == 0 { 0 } else { 1 };
        for k in 0..count {
            let s = self.spans[k];
            let size = me.font_size * s.scale;
            let (w, gap) = self.widths[k];
            let w = w + if s.code { CHIP * 2.0 } else { 0.0 };
            let lead = if s.bullet { BULLET_X } else { 0.0 };
            if s.br && k > 0 {
                x = 0.0;
                hang = 0.0;
                y += line_h;
                line_h = size * ratio;
                self.lines += 1;
            } else if x > hang && x + w > inner {
                // Wraps *between* spans: a span is a word and never
                // breaks inside itself.
                x = hang;
                y += line_h;
                line_h = size * ratio;
                self.lines += 1;
            }
            line_h = line_h.max(size * ratio);
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
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: content.w, h: content.h + me.padding.vertical() },
        );
        self.height = s.h;
        lumen_static::ops::record_painted(st, i, limits, s, &me)
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
    fn draw(&self, st: &[NodeState], i: usize, out: &mut dyn Painter, quote: Color) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        lumen_static::ops::fill(st, i, out);
        if s.bg.3 > 0 {
            out.rounded_rect(r, s.bg, s.radius);
        }
        lumen_static::ops::border(st, i, out);
        out.push_clip(r);
        let (x0, y0) = (r[0] + s.pad.0, r[1] + s.pad.1);
        for (k, (&sp, &(x, y, w))) in self.spans.iter().zip(&self.placed).enumerate() {
            let size = s.font_size * sp.scale;
            let h = size * line_ratio(s.line_height);
            let ink = if sp.kind == Span::QUOTE { quote } else { s.fg };
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
        self.draw(st, i, out, st[i].fg);
    }

    lumen_static::leaf_common!(RichText, 0);

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
        let mut v = String::new();
        for k in 0..self.spans.len() {
            if k > 0 {
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

impl<S> lumen_static::event::Handlers<S> for RichText {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
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
/// `**bold**`, `` `code` ``, `- bullet`, `> quote`, and blank-line-
/// separated paragraphs. **Not italic** — see this module's header: the
/// painter has a weight axis and no slant, and markup that renders
/// identically to plain text claims to have done something it did not.
///
/// Size, weight and the quote and link colours per element are a
/// [`MarkdownTheme`] ([`Markdown::set_theme`]); the body colour is the
/// node's own, from the cascade, and the quote and link colours derive
/// from it unless the theme fixes them. A blockquote is its words in the
/// quote colour behind a 2 px bar of the same colour, indented like a
/// bullet; a quote that wraps hangs.
#[derive(Default)]
pub struct Markdown {
    inner: RichText,
    theme: MarkdownTheme,
}

impl Markdown {
    pub fn new(src: &str) -> Self {
        let mut m = Markdown::default();
        m.set_str(src);
        m
    }

    /// The rendered text and its spans.
    pub fn rich(&self) -> &RichText {
        &self.inner
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
                    let marker = if word[k..].starts_with("**") {
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
                            if bold {
                                w = w.max(700.0);
                            }
                            sp.weight = w;
                            sp.code = code;
                            sp.kind = kind;
                            sp.br = first_seg && first_in_line && !self.inner.spans.is_empty();
                            sp.bullet = lead && first_seg && first_in_line;
                            // Only the word's last segment takes the
                            // gap: `**bold**,` is one word and must not
                            // render as `bold ,`.
                            sp.space = false;
                            self.inner.push(sp);
                            first_seg = false;
                        }
                        match marker {
                            Some(2) => bold = !bold,
                            Some(_) => code = !code,
                            None => break,
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

impl Widget for Markdown {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "markdown";
    const SEEDS: &'static [Seed] = &[Seed::new(
        <Markdown as Widget>::KIND,
        <Markdown as Widget>::type_defaults, Fields::of(&[
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
    ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];

    fn type_defaults() -> Style {
        <RichText as Widget>::type_defaults()
    }
}

impl Node for Markdown {
    lumen_static::node_count!();
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

    /// The inner `RichText`'s paint, with a quote in the theme's quote
    /// colour over this node's own (`MarkdownTheme::quote_ink`).
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.inner.draw(st, i, out, self.theme.quote_ink(st[i].fg))
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        self.inner.hit(st, i, x, y)
    }

    fn node(&self, _k: usize) -> &dyn Node {
        self
    }

    fn node_mut(&mut self, _k: usize) -> &mut dyn Node {
        self
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        st: &[NodeState],
        i: usize,
        id: u64,
        node: &mut accesskit::Node,
        extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        self.inner.observe(st, i, id, node, extra)
    }
}

impl<S> lumen_static::event::Handlers<S> for Markdown {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
        None
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
    lumen_static::node_count!();
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
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
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
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: a.w.max(b.w), h: a.h + me.spacing + b.h },
        );
        lumen_static::ops::record_painted(st, i, limits, s, &me)
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

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        self.input.hit(st, i + Self::INPUT, x, y)
    }

    /// **Formatting from the keyboard** — M57/B8. [`RichTextEditor::bold`]
    /// and [`RichTextEditor::code`] existed and nothing called them: the
    /// editor had no formatting UI at all. Ctrl+B and Ctrl+E (⌘ on a Mac)
    /// reach here because the text area declines a shortcut it does not
    /// know, and the pass walks up to its container.
    fn on_event(&mut self, ev: &lumen_static::Event, cx: &lumen_static::event::EventCtx) -> bool {
        use lumen_static::event::{Key, Mods};
        let short = cx.mods.has(Mods::CTRL) || cx.mods.has(Mods::META);
        match ev {
            lumen_static::Event::Key { key: Key::Char('b'), down: true } if short => self.bold(),
            lumen_static::Event::Key { key: Key::Char('e'), down: true } if short => self.code(),
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

impl<S> lumen_static::event::Handlers<S> for RichTextEditor {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
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
/// Named `PaintBox` and not `Canvas`: `lumen_static::Canvas` is the
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
    ])).with_roles(lumen_static::style::SeedRoles::NONE.background(lumen_static::style::Role::Raised))];

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
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: limits.max_w, h: limits.max_h },
        );
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        lumen_static::ops::fill(st, i, out);
        if s.bg.3 > 0 {
            out.rounded_rect(r, s.bg, s.radius);
        }
        lumen_static::ops::border(st, i, out);
        // **Clipped, because the callback is the application's.** It is
        // handed a rect and nothing stops it drawing outside one; a
        // `Tree` that painted 116 px past its own box is the same
        // mistake made by code this repository owns (M27/T27.11). The
        // clip is the rectangle, not the rounded ground: see `draw`.
        out.push_clip(r);
        (self.draw)(out, r);
        out.pop_clip();
    }

    lumen_static::leaf_common!(PaintBox, 0);

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

impl<S> lumen_static::event::Handlers<S> for PaintBox {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
        None
    }
}
