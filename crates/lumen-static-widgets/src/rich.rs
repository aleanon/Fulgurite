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
//! straight loop over placed boxes. What it costs is a fixed ceiling —
//! [`MAX_SPANS`] words — which is the same trade `Tree` makes with
//! `MAX_TREE_NODES` and a `FilePicker` with `MAX_FILES`.
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

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

/// The most words one `RichText` lays out.
pub const MAX_SPANS: usize = 192;

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
}

impl Span {
    pub const fn plain(range: (u32, u32)) -> Span {
        Span { range, scale: 1.0, weight: 400.0, code: false, br: false, bullet: false, space: true }
    }
}

/// Wrapped, styled text.
pub struct RichText {
    text: String,
    spans: [Span; MAX_SPANS],
    count: usize,
    /// Where each span was placed, and how wide it measured.
    placed: [(f32, f32, f32); MAX_SPANS],
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
    widths: [(f32, f32); MAX_SPANS],
    measured_under: u64,
}

impl Default for RichText {
    fn default() -> Self {
        RichText {
            text: String::new(),
            spans: [Span::plain((0, 0)); MAX_SPANS],
            count: 0,
            placed: [(0.0, 0.0, 0.0); MAX_SPANS],
            lines: 0,
            height: 0.0,
            widths: [(0.0, 0.0); MAX_SPANS],
            measured_under: 0,
        }
    }
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
        for s in &self.spans[..self.count] {
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
        &self.spans[..self.count]
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text one span covers.
    pub fn span_str(&self, i: usize) -> &str {
        let s = self.spans[i.min(self.count.saturating_sub(1))];
        self.text.get(s.range.0 as usize..s.range.1 as usize).unwrap_or("")
    }

    /// Lines the last layout produced.
    pub fn lines(&self) -> usize {
        self.lines
    }

    pub fn push(&mut self, s: Span) -> bool {
        if self.count >= MAX_SPANS {
            return false;
        }
        self.spans[self.count] = s;
        self.count += 1;
        true
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }

    fn split_plain(&mut self) {
        self.count = 0;
        let text = core::mem::take(&mut self.text);
        let mut at = 0usize;
        for w in text.split_whitespace() {
            // `split_whitespace` drops the separators, so the offset is
            // found rather than accumulated: a run of two spaces would
            // otherwise put every following span one byte short.
            let start = text[at..].find(w).map(|k| at + k).unwrap_or(at);
            at = start + w.len();
            if !self.push(Span::plain((start as u32, at as u32))) {
                break;
            }
        }
        self.text = text;
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

impl Node for RichText {
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
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        // **The wrap width is the box, decided before the words are
        // placed.** A `Shrink` width would make the answer depend on
        // the wrap and the wrap depend on the answer; text fills the
        // width it is given and shrinks in height, which is what
        // `type_defaults` says and what this relies on.
        let inner = (limits.max_w - me.padding.horizontal()).max(0.0);
        let key = self.fold(me.font_size, cx.scale());
        if key != self.measured_under {
            for k in 0..self.count {
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
        self.lines = if self.count == 0 { 0 } else { 1 };
        for k in 0..self.count {
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
            self.placed[k] = (x + lead, y, w);
            if s.bullet {
                hang = x + lead;
            }
            x += lead + w + gap;
        }
        let content =
            Size { w: limits.max_w, h: if self.count == 0 { 0.0 } else { y + line_h } };
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: content.w, h: content.h + me.padding.vertical() },
        );
        self.height = s.h;
        lumen_static::ops::record_painted(st, i, limits, s, &me)
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
        let kept = (self.placed, self.lines, self.height);
        let size = self.layout(cx, limits, style, st, i);
        (self.placed, self.lines, self.height) = kept;
        size
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
        out.push_clip(r);
        let (x0, y0) = (r[0] + s.pad.0, r[1] + s.pad.1);
        for k in 0..self.count {
            let sp = self.spans[k];
            let (x, y, w) = self.placed[k];
            let size = s.font_size * sp.scale;
            let h = size * line_ratio(s.line_height);
            if sp.bullet {
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
                    Color(s.fg.0, s.fg.1, s.fg.2, 0x1a),
                    4.0,
                );
                tx += CHIP;
            }
            out.text_weighted(
                [tx, y0 + y + h / 2.0 - size * 0.6, x0 + x + w, y0 + y + h],
                self.span_str(k),
                size,
                sp.weight,
                s.fg,
            );
        }
        out.pop_clip();
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
        for k in 0..self.count {
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

/// `RichText` with a parser in front of it.
///
/// The subset is what a UI renders: `# heading` (three levels),
/// `**bold**`, `` `code` ``, `- bullet`, and blank-line-separated
/// paragraphs. **Not italic** — see this module's header: the painter
/// has a weight axis and no slant, and markup that renders identically
/// to plain text claims to have done something it did not.
#[derive(Default)]
pub struct Markdown {
    inner: RichText,
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

    /// Re-parse. The editor calls this on every keystroke, which is why
    /// the parser allocates once and walks once.
    pub fn set_str(&mut self, src: &str) {
        self.inner.text = src.to_string();
        self.inner.clear();
        let text = core::mem::take(&mut self.inner.text);
        let mut blank_pending = false;
        for (ln, line) in text.split('\n').enumerate() {
            let base = line_start(&text, ln);
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if trimmed.is_empty() {
                // A blank line is a blank line: remembered rather than
                // emitted, so a document that ends with one does not
                // render a trailing empty row.
                blank_pending = self.inner.count > 0;
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
            let (scale, weight, skip, bullet) = match heading(trimmed) {
                Some(h) => h,
                None if trimmed.starts_with("- ") => (1.0, 400.0, 2, true),
                None => (1.0, 400.0, 0, false),
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
                            sp.scale = scale;
                            sp.weight = if bold { 700.0 } else { weight };
                            sp.code = code;
                            sp.br = first_seg && first_in_line && self.inner.count > 0;
                            sp.bullet = bullet && first_seg && first_in_line;
                            // Only the word's last segment takes the
                            // gap: `**bold**,` is one word and must not
                            // render as `bold ,`.
                            sp.space = false;
                            if !self.inner.push(sp) {
                                break;
                            }
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
                if end < body.len() && self.inner.count > 0 {
                    let last = self.inner.count - 1;
                    self.inner.spans[last].space = true;
                }
                first_in_line = false;
                at = end;
            }
        }
        self.inner.text = text;
    }
}

/// Byte offset of line `n` in `text`.
fn line_start(text: &str, n: usize) -> usize {
    let mut at = 0usize;
    for _ in 0..n {
        match text[at..].find('\n') {
            Some(k) => at += k + 1,
            None => return text.len(),
        }
    }
    at
}

/// `#`, `##`, `###` — scale, weight, bytes to skip, and never a bullet.
fn heading(line: &str) -> Option<(f32, f32, usize, bool)> {
    for (marker, scale) in [("### ", 1.1f32), ("## ", 1.25), ("# ", 1.5)] {
        if line.starts_with(marker) {
            return Some((scale, 700.0, marker.len(), false));
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

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.inner.paint(st, i, out)
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
