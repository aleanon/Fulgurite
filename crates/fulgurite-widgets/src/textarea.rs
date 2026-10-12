//! A multi-line text area — plan M11/T11.3.
//!
//! # Why a separate type, and what that revises
//!
//! `docs/text-and-scrolling-decisions.md` B7 decided "no multi-line in
//! the first cut" and said the mechanism would be **one widget type
//! with a `wrap` field reserved**. M9 shipped [`TextInput`](crate::input::TextInput) with no such
//! field — an unused field is a claim the code has not earned — and this
//! is the other half of that: multi-line is a **separate widget**.
//!
//! The reason is not taste. Look at what actually differs:
//! [`TextInput`](crate::input::TextInput) shapes against an *unbounded* width because it must not
//! wrap, scrolls **horizontally** to keep the caret visible, and moves
//! the caret by byte boundaries. This shapes against the box, scrolls
//! **vertically**, and moves the caret up and down *geometrically* —
//! "the cluster on the previous line nearest my x" is not a byte step at
//! all. A `wrap: bool` would branch `layout`, `paint` and every caret
//! method, and the two caret models would live in one type pretending to
//! be one thing.
//!
//! What *is* shared is shared properly: the storage and every edit go
//! through [`TextBuffer`], so `insert_str` and `remove_range` are
//! written once.
//!
//! # The size it is built for
//!
//! A note, a message, a form's comment box — not a document editor.
//! Every edit reshapes the whole value, which is linear in its length:
//! about 0.06 ms at 1 KB, 5 ms at 100 KB and 82 ms at 1 MB with the
//! system face (`docs/results-text.md` T45.10, `--bin textarea`). Until
//! M45/T45.10 it was quadratic, 35 ms at 100 KB. Past a few hundred KB a
//! keystroke misses a frame; a paragraph cache is what would change that,
//! and nothing here has one.
//!
//! # Keys and clicks
//!
//! Selection (M14/T14.4) and composition (M14/T14.8) work as in the
//! single-line field; this header said neither existed until M45/T45.6.
//!
//! | input | effect |
//! |---|---|
//! | Left, Right | one grapheme (M45/T45.5) |
//! | word modifier + Left, Right | to the start of the word, to the end of the next (M45/T45.6) |
//! | Up, Down | the nearest cluster on the line above or below |
//! | Home, End | to the ends of the visual line (M45/T45.6) |
//! | jump modifier + Home, End | to the ends of the text (M45/T45.6) |
//! | Shift + any of the above | extends the selection |
//! | Enter | a line break |
//! | Ctrl+A, Cmd+A | selects everything |
//! | a double click | selects the word under it (M45/T45.6) |
//! | a triple click | selects the `\n`-delimited line, without its break |
//!
//! The word modifier is Ctrl and the jump modifier Ctrl too, except on
//! macOS, where they are Alt and Cmd. A triple click takes the logical
//! line and Home and End the visual one, which is what desktop editors do.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
// Only the accesskit `extra` arg needs it, and that is observe-gated.
#[cfg(feature = "a11y")]
use alloc::vec::Vec;

use fulgurite::cx::{Cluster, LayoutCx};
use fulgurite::desc::{NodeDesc, NodeState, NO_PARENT};
use fulgurite::display::Painter;
use fulgurite::event::{Event, EventCtx, Key};
use fulgurite::hit::Hit;
use fulgurite::ops::{rect_of, resolve_self};
use fulgurite::style::{Seed, Style};
use fulgurite::text::{ClusterBuf, TextBuffer};
use fulgurite::widget::{Node, Widget};
use fulgurite::{Constraints, Point, Size};

/// A wrapping, editable, multi-line text box.
pub struct TextArea<S: TextBuffer = fulgurite::text::DefaultTextStorage> {
    text: S,
    pub(crate) caret: usize,
    /// The provisional text an input method is composing, as a byte
    /// range into `text` (M14/T14.8).
    ///
    /// The text *is* in the buffer, so it measures, positions and
    /// scrolls like any other; what makes it provisional is that the
    /// next `Preedit` replaces exactly this range.
    preedit: Option<core::ops::Range<usize>>,
    /// The fixed end of a selection, if there is one (M14/T14.4).
    /// Same model as `TextInput`'s, so the two widgets cannot drift.
    anchor: Option<usize>,
    /// Vertical offset in logical px so the caret stays visible (B5,
    /// one dimension over from `TextInput`'s horizontal one).
    scroll: f32,
    clusters: S::Clusters,
    /// What `clusters` was shaped from: the text's length beside the
    /// [`MeasureKey`](fulgurite::text::MeasureKey) every text memo
    /// builds (M45/T45.2), with the *inner* width as its limit — a text
    /// area wraps, so unlike `TextInput`'s the width stays in. The inner
    /// width and not the node's: padding is not in the key, and the
    /// paragraph wraps at what is left inside it.
    shaped_key: Option<(fulgurite::text::MeasureKey, u32)>,
    /// The height that shaping returned, cached with the key that
    /// produced it (M26/T26.7). `TextInput` keeps the width; a text
    /// area wraps, so what it needs back is the height.
    shaped_h: f32,
    /// One line's height: the measurer's spacing, or the pitch when
    /// `Style::line_height` is set (M80/T80.2). Every row this widget
    /// places — a painted line, the caret, the selection, a click — is
    /// at its index times this.
    line_h: f32,
    pad: (f32, f32),
    /// The caret's colour; the text colour when `None` — M25/T25.10.
    pub caret_color: Option<fulgurite::display::Color>,
    /// The selection highlight's colour; `input::selection_tint` of the
    /// text colour when `None` — M25/T25.10.
    pub selection_color: Option<fulgurite::display::Color>,
}

impl<S: TextBuffer> Default for TextArea<S> {
    fn default() -> Self {
        TextArea {
            text: S::default(),
            caret: 0,
            anchor: None,
            preedit: None,
            scroll: 0.0,
            clusters: S::Clusters::default(),
            shaped_key: None,
            shaped_h: 0.0,
            line_h: 0.0,
            pad: (0.0, 0.0),
            caret_color: None,
            selection_color: None,
        }
    }
}

impl<S: TextBuffer> TextArea<S> {
    pub fn new(text: &str) -> Self {
        let mut t = TextArea::default();
        t.set_str(text);
        t
    }

    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Which visual line the caret is on, from the last shaping.
    pub fn caret_line(&self) -> u16 {
        self.cluster_at(self.caret).map(|c| c.line).unwrap_or(0)
    }

    pub fn set_str(&mut self, s: &str) {
        self.text.set_from_str(s);
        self.caret = self.text.as_ref().len();
        self.shaped_key = None;
    }

    /// Insert at the caret (B3: methods, never a raw `&mut`).
    pub fn insert(&mut self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        self.delete_selection();
        // The caret advances only if the storage took the text. A
        // bounded storage can refuse, and advancing anyway would put
        // the caret past the end (M13/T13.5).
        if !self.text.insert_str(self.caret, s) {
            return false;
        }
        self.caret += s.len();
        self.shaped_key = None;
        true
    }

    /// Paste `text` as one edit — M45/T45.7. Line endings are normalised
    /// (`\r\n` and a lone `\r` become `\n`) and kept; every other
    /// control character is dropped; the rest goes through
    /// [`TextArea::insert`], the path typing takes. Returns the characters
    /// kept, a newline counting as one.
    pub fn paste(&mut self, text: &str) -> usize {
        let kept = crate::edit::printable(text, true);
        let n = kept.chars().count();
        if n > 0 && self.insert(&kept) {
            n
        } else {
            0
        }
    }

    /// The selected byte range, ordered, or `None`.
    pub fn selection(&self) -> Option<core::ops::Range<usize>> {
        let a = self.anchor?;
        let (lo, hi) = if a <= self.caret { (a, self.caret) } else { (self.caret, a) };
        (lo < hi).then_some(lo..hi)
    }

    /// The selected text, or `None`.
    pub fn selected_str(&self) -> Option<&str> {
        self.text.as_ref().get(self.selection()?)
    }

    /// Select everything. Returns whether there was anything to select.
    pub fn select_all(&mut self) -> bool {
        let end = self.text.as_ref().len();
        if end == 0 {
            return false;
        }
        self.anchor = Some(0);
        self.caret = end;
        true
    }

    /// Drop the selection, keeping the caret.
    pub fn clear_selection(&mut self) -> bool {
        self.anchor.take().is_some()
    }

    /// Delete the selection if there is one; returns whether it did.
    pub fn delete_selection(&mut self) -> bool {
        let Some(r) = self.selection() else {
            self.anchor = None;
            return false;
        };
        self.text.remove_range(r.clone());
        self.caret = r.start;
        self.anchor = None;
        self.shaped_key = None;
        true
    }

    /// The provisional range, if a composition is in progress.
    pub fn preedit(&self) -> Option<core::ops::Range<usize>> {
        self.preedit.clone()
    }

    /// Apply one composition step (T14.8).
    ///
    /// Returns whether anything changed. A bounded storage may
    /// **refuse** the provisional text, which is the T13.5 bug in a new
    /// place: the range is recorded from what the buffer actually took,
    /// not from what was offered, so a refused preedit leaves no range
    /// pointing past the end.
    pub fn compose(&mut self, c: fulgurite::event::Composition<'_>) -> bool {
        // Whatever was provisional goes first, in every arm: a preedit
        // replaces it, a commit supersedes it, a cancel drops it.
        let had = self.preedit.take();
        if let Some(r) = had.clone() {
            self.text.remove_range(r.clone());
            self.caret = r.start;
        }
        let text = match c {
            fulgurite::event::Composition::Cancel => {
                if had.is_some() {
                    self.shaped_key = None;
                }
                return had.is_some();
            }
            fulgurite::event::Composition::Preedit(t) | fulgurite::event::Composition::Commit(t) => t,
        };
        if text.is_empty() {
            if had.is_some() {
                self.shaped_key = None;
            }
            return had.is_some();
        }
        let at = self.caret;
        if !self.insert(text) {
            // The storage refused. No range is recorded, so nothing
            // points past the end.
            return had.is_some();
        }
        if matches!(c, fulgurite::event::Composition::Preedit(_)) {
            // Only what the buffer took: `insert` moved the caret by
            // exactly that much.
            self.preedit = Some(at..self.caret);
        }
        true
    }

    /// Extend or collapse after a caret move. See `TextInput::moved`.
    /// Returns whether a selection was dropped (M45/T45.6).
    fn moved(&mut self, extend: bool, from: usize) -> bool {
        if extend {
            self.anchor.get_or_insert(from);
            false
        } else {
            self.selection().is_some() & self.anchor.take().is_some()
        }
    }

    pub fn backspace(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let Some(prev) = self.prev_boundary(self.caret) else { return false };
        self.text.remove_range(prev..self.caret);
        self.caret = prev;
        self.shaped_key = None;
        true
    }

    pub fn delete(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let Some(next) = self.next_boundary(self.caret) else { return false };
        self.text.remove_range(self.caret..next);
        self.shaped_key = None;
        true
    }

    /// The grapheme boundary before `at` — `crate::edit`, which
    /// `TextInput` steps by too (M45/T45.5).
    fn prev_boundary(&self, at: usize) -> Option<usize> {
        crate::edit::prev_boundary(self.text.as_ref(), at)
    }

    fn next_boundary(&self, at: usize) -> Option<usize> {
        crate::edit::next_boundary(self.text.as_ref(), at)
    }

    /// The cluster that places offset `byte` — **looked up exactly**
    /// (M81/T81.5, [`crate::caret::at`]): the last cluster with that byte,
    /// so a direction change's offset is placed at what follows it and the
    /// text's end at the paragraph direction's end. It was the first with
    /// `byte >= at`, which assumed one place per offset. `caret_at`, the
    /// IME area and Up/Down all read this.
    fn cluster_at(&self, byte: usize) -> Option<Cluster> {
        let cl = self.clusters.as_slice();
        crate::caret::at(cl, byte).copied().or_else(|| cl.last().copied())
    }

    /// Move the caret to the line `delta` away, keeping its x.
    ///
    /// **This is why multi-line is a different widget.** Up and down are
    /// not byte steps: the answer is "the cluster on that line whose x
    /// is nearest mine", which only exists because the shaping seam
    /// reports [`Cluster::line`] (T7.2b).
    pub fn move_line(&mut self, delta: i32) -> bool {
        let Some(here) = self.cluster_at(self.caret) else { return false };
        let want = here.line as i32 + delta;
        if want < 0 {
            return false;
        }
        let want = want as u16;
        let mut best: Option<(f32, u32)> = None;
        for c in self.clusters.as_slice() {
            if c.line != want {
                continue;
            }
            let d = (c.x - here.x).abs();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, c.byte));
            }
        }
        match best {
            Some((_, byte)) if byte as usize != self.caret => {
                self.caret = byte as usize;
                true
            }
            _ => false,
        }
    }

    /// To the start of the caret's line — M57/B8. Home and End were
    /// unhandled, so End did nothing in the editor the audit drove
    /// (`docs/widget-audit.md`, RichTextEditor). The line is the text's
    /// own, between newlines, not the visual line a wrap made: that is
    /// the line a user typed and the one a `\n` ends.
    pub fn move_home(&mut self) -> bool {
        let t = self.text.as_ref();
        let at = t[..self.caret].rfind('\n').map_or(0, |k| k + 1);
        core::mem::replace(&mut self.caret, at) != at
    }

    /// To the end of the caret's line, before its newline.
    pub fn move_end(&mut self) -> bool {
        let t = self.text.as_ref();
        let at = t[self.caret..].find('\n').map_or(t.len(), |k| self.caret + k);
        core::mem::replace(&mut self.caret, at) != at
    }

    pub fn move_left(&mut self) -> bool {
        match self.prev_boundary(self.caret) {
            Some(p) => {
                self.caret = p;
                true
            }
            None => false,
        }
    }

    pub fn move_right(&mut self) -> bool {
        match self.next_boundary(self.caret) {
            Some(n) => {
                self.caret = n;
                true
            }
            None => false,
        }
    }

    /// Put the caret at `to`; returns whether it moved.
    fn move_to(&mut self, to: usize) -> bool {
        core::mem::replace(&mut self.caret, to) != to
    }

    /// Move the caret to the start of the word it is in, or of the one
    /// before it (M45/T45.6).
    pub fn move_word_left(&mut self) -> bool {
        match crate::edit::prev_word(self.text.as_ref(), self.caret) {
            Some(p) => self.move_to(p),
            None => false,
        }
    }

    /// Move the caret to the end of the next word (M45/T45.6).
    pub fn move_word_right(&mut self) -> bool {
        match crate::edit::next_word(self.text.as_ref(), self.caret) {
            Some(n) => self.move_to(n),
            None => false,
        }
    }

    /// The bounds Home and End go to from `at`: the **visual** line, when
    /// the paragraph wraps and the clusters are fresh, and the
    /// `\n`-delimited line otherwise (M45/T45.6).
    ///
    /// A shaper need not report a cluster for the `\n` itself, so the
    /// visual line is looked for only among the clusters inside the
    /// logical line. A wrapped line's End is its last cluster, not the
    /// next line's first: the two are the same byte offset, and a caret
    /// there is drawn on the next line.
    fn line_bounds(&self, at: usize) -> core::ops::Range<usize> {
        let s = self.text.as_ref();
        let logical = crate::edit::line_at(s, at);
        if self.shaped_key.is_none() {
            return logical;
        }
        let inside = |c: &&Cluster| (logical.start..=logical.end).contains(&(c.byte as usize));
        let cl = self.clusters.as_slice();
        let Some(line) = cl.iter().filter(inside).rfind(|c| c.byte as usize <= at).map(|c| c.line) else {
            return logical;
        };
        let (mut first, mut last, mut next) = (None, None, None);
        for c in cl.iter().filter(inside) {
            let b = c.byte as usize;
            if c.line == line {
                first.get_or_insert(b);
                last = Some(b);
            } else if c.line > line {
                next = Some(b);
                break;
            }
        }
        let lo = first.unwrap_or(logical.start);
        let hi = match (next, last) {
            (Some(_), Some(l)) => l,
            _ => logical.end,
        };
        if s.is_char_boundary(lo) && s.is_char_boundary(hi) && lo <= hi {
            lo..hi
        } else {
            logical
        }
    }

    /// Caret position within the run, in logical px: `cluster_at`'s x,
    /// so over a right-to-left line the caret after a letter is at the
    /// letter's left (M81/T81.5).
    fn caret_at(&self) -> (f32, f32) {
        match self.cluster_at(self.caret) {
            Some(c) => (c.x, c.line as f32 * self.line_h),
            None => (0.0, 0.0),
        }
    }

    /// **One text command per visible line** — M45/T45.11, R8-F3.
    ///
    /// This painted the whole value as one run. A backend culls per run
    /// and shapes and rasterises every glyph of a run before clipping, so
    /// a caret move in a long document rasterised the document: 53,595
    /// glyphs for one Left in 64 KB (`docs/results-text.md` T45.0 item
    /// 14). The line starts are already here, in the shaped clusters, and
    /// so is the scroll offset, so the lines that meet the box are cheap
    /// to find and only they are emitted.
    ///
    /// Each line is placed at its index times `line_h` — the spacing the
    /// caret, the selection and a click already use, and the pitch when
    /// `Style::line_height` is set (M80/T80.2) — and laid out in the
    /// inner width, where it fits by construction: the shaper broke the
    /// paragraph there. A line's trailing newline is left out.
    ///
    /// With no clusters (the `BlockMetrics` stand-in reports none) there
    /// are no line starts to use, and the value is one run as before.
    fn paint_lines(&self, inner: [f32; 4], s: &NodeState, out: &mut dyn Painter) {
        let text = self.text.as_ref();
        let cl = self.clusters.as_slice();
        let lh = self.line_h;
        if cl.is_empty() || lh <= 0.0 {
            let y = inner[1] - self.scroll;
            out.text_spaced(
                [inner[0], y, inner[2], y + (inner[3] - inner[1]) + self.scroll],
                text,
                s.font_size,
                s.font_weight,
                s.line_height,
                s.fg,
            );
            return;
        }
        let first = fulgurite::num::floor(self.scroll / lh).max(0.0) as u32;
        let last = fulgurite::num::ceil((self.scroll + (inner[3] - inner[1])) / lh).max(0.0) as u32;
        // Clusters are in byte order, and so in line order: the first of
        // a line is found by bisection, not by a walk from the top.
        let mut k = cl.partition_point(|c| (c.line as u32) < first);
        while k < cl.len() && (cl[k].line as u32) <= last {
            let line = cl[k].line;
            let start = cl[k].byte as usize;
            let next = k + cl[k..].partition_point(|c| c.line == line);
            let end = cl.get(next).map_or(text.len(), |c| c.byte as usize);
            let run = text.get(start..end.max(start)).unwrap_or("").trim_end_matches(['\n', '\r']);
            if !run.is_empty() {
                let top = inner[1] + line as f32 * lh - self.scroll;
                // The byte goes with each line (M80/T80.2): a one-line run
                // at a pitch sits at half-leading inside its `lh`-tall
                // rect, which is where the caret and the selection are.
                out.text_spaced(
                    [inner[0], top, inner[2], top + lh],
                    run,
                    s.font_size,
                    s.font_weight,
                    s.line_height,
                    s.fg,
                );
            }
            k = next;
        }
    }

    /// Byte offset nearest a point in the run — where a click lands.
    /// The selection as rects, `f(x0, x1, line)`.
    ///
    /// A multi-line selection is not one rectangle: it is a partial
    /// first line, whole middle lines, and a partial last one — and
    /// since M81/T81.5 a line that changes direction inside the
    /// selection is one rect per visually contiguous span
    /// ([`crate::caret::spans`]). It was each line's lowest to highest
    /// selected x, which over `abc שלום` tinted the unselected letters
    /// between the two parts. A line's last cluster ends where it starts
    /// (its trailing edge is on no cluster of the line), as before.
    fn selection_runs(&self, sel: &core::ops::Range<usize>, mut f: impl FnMut(f32, f32, u16)) {
        crate::caret::spans(self.clusters.as_slice(), sel.clone(), &[], &mut f);
    }

    /// Where a pointer event landed, in byte offsets.
    fn at_pointer(&self, cx: &EventCtx) -> usize {
        self.offset_at(cx.local.x - self.pad.0, cx.local.y - self.pad.1 + self.scroll)
    }

    /// Run a caret move and update the anchor from Shift — the shape
    /// `TextInput` spells out per arm, factored here because this widget
    /// has four movers rather than four one-liners.
    fn moving(&mut self, cx: &EventCtx, f: impl FnOnce(&mut Self) -> bool) -> bool {
        let from = self.caret;
        let moved = f(self);
        self.moved(cx.mods.has(fulgurite::event::Mods::SHIFT), from) | moved
    }

    /// The byte nearest a point: on the line `y` falls in, **the nearest
    /// leading edge** (M81/T81.5) — a cluster's own x, a direction
    /// change's trailing edge and the text's end each being one. So a
    /// click at the visual left end of a right-to-left line lands at its
    /// logical end: a line a `\n` ends has that separator there (no
    /// width, at the paragraph's level), the last line has the text's
    /// end. A soft-wrapped line has no insertion point at its far end,
    /// and lands on its last letter. **The text's end wins a tie**
    /// (M81/T81.6), as in `TextInput`: on a right-to-left last line that
    /// ends in a left-to-right run, the end and that run's first leading
    /// edge are both at x 0.
    fn offset_at(&self, x: f32, y: f32) -> usize {
        let line = if self.line_h > 0.0 { (y / self.line_h).max(0.0) as u16 } else { 0 };
        let cl = self.clusters.as_slice();
        let len = self.text.as_ref().len();
        let mut best = (f32::INFINITY, len);
        for c in cl {
            // Prefer the right line; fall back to any if it is empty.
            let dl = (c.line as i32 - line as i32).abs() as f32 * 1e4;
            let d = dl + (c.x - x).abs();
            if d < best.0 || (d == best.0 && c.byte as usize == len) {
                best = (d, c.byte as usize);
            }
        }
        best.1
    }
}

impl<S: TextBuffer> Widget for TextArea<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    const KIND: &'static str = "textarea";

    fn type_defaults() -> Style {
        Style {
            padding: fulgurite::style::Edges { left: 8.0, right: 8.0, top: 6.0, bottom: 6.0 },
            height: fulgurite::style::Length::Fixed(96.0),
            // An I-beam over editable text (T14.6).
            cursor: fulgurite::style::Cursor::Text,
            // **A field looks like a field** — M57/B8: an edge and a
            // ground, through the palette's roles below. It had neither,
            // so the editor's text area was text on the page with nothing
            // to say where typing would go.
            border_width: 1,
            radius: 8.0,
            background: fulgurite::display::Color(0xff, 0xff, 0xff, 0xff),
            border_color: fulgurite::display::Color(0x83, 0x8d, 0x9f, 0xff),
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, fulgurite::style::Fields::of(&[
            fulgurite::style::Fields::WIDTH,
            fulgurite::style::Fields::HEIGHT,
            fulgurite::style::Fields::PADDING,
            fulgurite::style::Fields::CURSOR,
            fulgurite::style::Fields::FOCUS_RING,
            fulgurite::style::Fields::VISIBLE,
            fulgurite::style::Fields::COLOR,
            fulgurite::style::Fields::BACKGROUND,
            fulgurite::style::Fields::BORDER,
            fulgurite::style::Fields::BACKGROUND_FILL,
            fulgurite::style::Fields::RADIUS,
            fulgurite::style::Fields::FONT_SIZE,
            fulgurite::style::Fields::FONT_WEIGHT,
            fulgurite::style::Fields::PRESSED_DIM,
            fulgurite::style::Fields::HOVER_DIM,
            fulgurite::style::Fields::TRANSITION,
            fulgurite::style::Fields::LINE_HEIGHT,
        ])).with_roles(
            fulgurite::style::SeedRoles::NONE
                .color(fulgurite::style::Role::Text)
                .background(fulgurite::style::Role::Raised)
                .border(fulgurite::style::Role::Border),
        )];
}

impl<S: TextBuffer> Node for TextArea<S> {
    fulgurite::node_count!();
    /// Arrows move a caret, on two axes, so they never leave this control
    /// (M44/T44.1; [`Node::owns_arrows`]).
    fn owns_arrows(&self) -> bool {
        true
    }

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
            return fulgurite::ops::hide_subtree(st, i, limits, 1);
        }
        let px = me.font_size;
        let inner_w = (limits.max_w - me.padding.horizontal()).max(0.0);
        let key = (
            fulgurite::text::MeasureKey::new(cx, Constraints { min_w: 0.0, max_w: inner_w, min_h: 0.0, max_h: 0.0 }, &me),
            self.text.as_ref().len() as u32,
        );
        // Shaped against the *box*, unlike `TextInput` — wrapping is the
        // whole difference.
        if self.shaped_key != Some(key) {
            self.clusters.clear();
            let run =
                cx.text.shape(self.text.as_ref(), px, me.font_weight, inner_w, &mut self.clusters);
            self.line_h = cx.text.line_height(px);
            self.shaped_h = run.h;
            // **At a line height, the pitch is the row** — M80/T80.2. The
            // caret, the selection, a click and `paint_lines` all place a
            // line at its index times `line_h`, so making `line_h` the
            // pitch moves all four together, and the shaped height becomes
            // `lines × pitch` (plus the deficit below the face's spacing,
            // as `Label` measures it).
            if me.line_height != 0 {
                let lines = self.clusters.as_slice().iter().map(|c| c.line as u32 + 1).max().unwrap_or(0);
                if lines > 0 {
                    let k = cx.scale();
                    let nls = fulgurite::num::ceil(self.line_h * k) / k;
                    self.shaped_h = fulgurite::cx::pitched_height(lines, me.line_height, px, nls);
                }
                self.line_h = fulgurite::cx::pitch(me.line_height, px);
            }
            self.shaped_key = Some(key);
        }

        let content = Size { w: inner_w, h: self.shaped_h.max(self.line_h) };
        let padded =
            Size { w: content.w + me.padding.horizontal(), h: content.h + me.padding.vertical() };
        let size = resolve_self(limits, &me, padded);

        // B5, vertically: the caret into view, after shaping, because
        // this is the first moment its line is known.
        let inner_h = (size.h - me.padding.vertical()).max(0.0);
        let (_, cy) = self.caret_at();
        if cy + self.line_h - self.scroll > inner_h {
            self.scroll = cy + self.line_h - inner_h;
        }
        if cy < self.scroll {
            self.scroll = cy;
        }
        self.scroll = fulgurite::num::clamp(self.scroll, 0.0, (content.h - inner_h).max(0.0));
        self.pad = (me.padding.left, me.padding.top);

        fulgurite::ops::record_painted(st, i, limits, size, &me)
    }

    /// Measure without touching what paint reads — M41/T41.4, R1-F5.
    ///
    /// `layout` writes the vertical scroll, the padding and the shaped
    /// clusters from its limits, and the audit's 1e6 px probe unwraps the
    /// paragraph to one line, so the scroll clamp — computed from the
    /// wrapped height — pinned the scroll to zero on a frame nothing then
    /// laid out again (`audit_is_inert.rs`: the area painted at y = 41.2
    /// against 5.2). The height depends on the width here, unlike
    /// `TextInput`'s run, so the memo answers only for the width it was
    /// shaped at; any other width is measured into a local and not kept.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let me = style[i];
        if !me.visible {
            return fulgurite::ops::hide_subtree(st, i, limits, 1);
        }
        let px = me.font_size;
        let inner_w = (limits.max_w - me.padding.horizontal()).max(0.0);
        let key = (
            fulgurite::text::MeasureKey::new(cx, Constraints { min_w: 0.0, max_w: inner_w, min_h: 0.0, max_h: 0.0 }, &me),
            self.text.as_ref().len() as u32,
        );
        let h = if self.shaped_key == Some(key) {
            self.shaped_h.max(self.line_h)
        } else {
            let scale = cx.scale();
            let run = fulgurite::cx::measure_spaced(
                cx.text,
                scale,
                self.text.as_ref(),
                px,
                me.font_weight,
                me.line_height,
                inner_w,
            );
            let row = if me.line_height != 0 {
                fulgurite::cx::pitch(me.line_height, px)
            } else {
                cx.text.line_height(px)
            };
            run.h.max(row)
        };
        let content = Size { w: inner_w, h };
        let padded =
            Size { w: content.w + me.padding.horizontal(), h: content.h + me.padding.vertical() };
        fulgurite::ops::record_painted(st, i, limits, resolve_self(limits, &me, padded), &me)
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        fulgurite::ops::fill(st, i, out);
        out.rounded_rect(r, s.bg, s.radius);
        fulgurite::ops::border(st, i, out);
        let (pl, pt, pr, pb) = s.pad;
        let inner = [r[0] + pl, r[1] + pt, r[2] - pr, r[3] - pb];
        // The run may be taller than the box; the clip is what keeps it
        // inside, as a scroller's content is (A1, one widget down).
        out.push_clip(inner);
        self.paint_lines(inner, s, out);
        // The selection, behind the run — same rule as `TextInput`: a
        // highlight painted after the glyphs would hide them.
        if let Some(sel) = self.selection() {
            let tint = self.selection_color.unwrap_or_else(|| crate::input::selection_tint(s.fg));
            self.selection_runs(&sel, |x0, x1, line| {
                let top = inner[1] + line as f32 * self.line_h - self.scroll;
                out.rect([inner[0] + x0, top, inner[0] + x1, top + self.line_h], tint);
            });
        }
        if let Some(pre) = self.preedit.clone() {
            self.selection_runs(&pre, |x0, x1, line| {
                let top = inner[1] + (line as f32 + 1.0) * self.line_h - self.scroll;
                out.rect([inner[0] + x0, top - 2.0, inner[0] + x1, top - 1.0], s.fg);
            });
        }
        if s.focused {
            let (cx, cy) = self.caret_at();
            let x = inner[0] + cx;
            let top = inner[1] + cy - self.scroll;
            out.rect([x, top, x + 1.0, top + self.line_h], self.caret_color.unwrap_or(s.fg));
        }
        out.pop_clip();
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        fulgurite::hit::contains(&st[i], x, y).then(|| Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // A newline is the one key a single-line field must refuse
            // and this one must accept. Typed, it arrives as Enter; pasted,
            // it arrives whole through `Node::paste` (M45/T45.7) — never as
            // an `Event::Text`, which refuses control characters here.
            Event::Key { key: Key::Enter, down: true } => {
                self.insert("\n");
                true
            }
            Event::Text { ch } if !ch.is_control() => {
                let mut buf = [0u8; 4];
                self.insert(ch.encode_utf8(&mut buf));
                true
            }
            Event::Key { key: Key::Backspace, down: true } => self.backspace(),
            Event::Key { key: Key::Delete, down: true } => self.delete(),
            // With the word modifier (Ctrl, Alt on macOS) an arrow moves
            // by word; Home and End go to the visual line's bounds, and
            // with the jump modifier (Ctrl, Cmd on macOS) to the buffer's
            // (M45/T45.6).
            Event::Key { key: Key::Left, down: true } if cx.mods.has(crate::edit::WORD) => {
                self.moving(cx, Self::move_word_left)
            }
            Event::Key { key: Key::Right, down: true } if cx.mods.has(crate::edit::WORD) => {
                self.moving(cx, Self::move_word_right)
            }
            Event::Key { key: Key::Left, down: true } => self.moving(cx, Self::move_left),
            Event::Key { key: Key::Right, down: true } => self.moving(cx, Self::move_right),
            Event::Key { key: Key::Home, down: true } if cx.mods.has(crate::edit::JUMP) => {
                self.moving(cx, |s| s.move_to(0))
            }
            Event::Key { key: Key::End, down: true } if cx.mods.has(crate::edit::JUMP) => {
                self.moving(cx, |s| s.move_to(s.text.as_ref().len()))
            }
            Event::Key { key: Key::Home, down: true } => self.moving(cx, |s| s.move_to(s.line_bounds(s.caret).start)),
            Event::Key { key: Key::End, down: true } => self.moving(cx, |s| s.move_to(s.line_bounds(s.caret).end)),
            Event::Key { key: Key::Up, down: true } => self.moving(cx, |s| s.move_line(-1)),
            Event::Key { key: Key::Down, down: true } => self.moving(cx, |s| s.move_line(1)),
            Event::Key { key: Key::Char('a'), down: true }
                if cx.mods.has(fulgurite::event::Mods::CTRL)
                    || cx.mods.has(fulgurite::event::Mods::META) =>
            {
                self.select_all()
            }
            Event::Key { key: Key::Escape, down: true } => self.clear_selection(),
            // A double click selects the word under it; a triple click the
            // `\n`-delimited line, without its break (M45/T45.6).
            Event::PointerDown { .. } if cx.clicks >= 2 => {
                let at = self.at_pointer(cx);
                let s = self.text.as_ref();
                let r = if cx.clicks == 2 { crate::edit::word_at(s, at) } else { crate::edit::line_at(s, at) };
                self.anchor = Some(r.start);
                self.caret = r.end;
                true
            }
            Event::PointerDown { .. } => {
                let at = self.at_pointer(cx);
                let from = self.caret;
                self.caret = at;
                if cx.mods.has(fulgurite::event::Mods::SHIFT) {
                    self.anchor.get_or_insert(from);
                } else {
                    self.anchor = Some(at);
                }
                true
            }
            // Only while this node holds the capture T11.2 added.
            Event::PointerMove { .. } => {
                let at = self.at_pointer(cx);
                core::mem::replace(&mut self.caret, at) != at
            }
            Event::PointerUp { .. } => {
                if self.selection().is_none() {
                    self.anchor = None;
                }
                false
            }
            _ => false,
        }
    }

    fn selection(&self) -> Option<&str> {
        self.selected_str()
    }

    fn compose(&mut self, c: fulgurite::event::Composition<'_>) -> bool {
        TextArea::compose(self, c)
    }

    fn paste(&mut self, text: &str) -> Option<usize> {
        Some(TextArea::paste(self, text))
    }

    /// The caret `paint` draws: its line's row at the pitch, **less the
    /// vertical scroll**, one line tall (M80/T80.5).
    fn caret_rect(&self, st: &[NodeState], i: usize) -> Option<fulgurite::display::Rect> {
        let s = st.get(i)?;
        let (pl, pt, _, _) = s.pad;
        let (cx, cy) = self.caret_at();
        let x = s.pos.x + pl + cx;
        let top = s.pos.y + pt + cy - self.scroll;
        Some([x, top, x + 1.0, top + self.line_h])
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }

    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut fulgurite::observe::accesskit::Node,
        _extra: &mut Vec<(fulgurite::observe::accesskit::NodeId, fulgurite::observe::accesskit::Node)>,
    ) {
        node.set_role(fulgurite::observe::accesskit::Role::MultilineTextInput);
        node.set_value(self.text.as_ref());
    }
}

impl<S: TextBuffer, A> fulgurite::event::Handlers<A> for TextArea<S> {
    fn handler(&self, _sel: fulgurite::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}
