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
//! # Still not here
//!
//! No selection and no IME, for the same reasons as the single-line
//! field:
//! selection needs modifier state on `Event::Key` or a pointer drag, and
//! IME needs a preedit event. Both are named in
//! `docs/plan-architecture-v2.md`'s Phase D paragraph, and neither is a
//! field reserved here.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
// Only the accesskit `extra` arg needs it, and that is observe-gated.
#[cfg(feature = "observe")]
use alloc::vec::Vec;

use lumen_static::cx::{Cluster, LayoutCx};
use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::Painter;
use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::hit::Hit;
use lumen_static::ops::{rect_of, resolve_self};
use lumen_static::style::{Seed, Style};
use lumen_static::text::{ClusterBuf, TextBuffer};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, Point, Size};

/// A wrapping, editable, multi-line text box.
pub struct TextArea<S: TextBuffer = lumen_static::text::DefaultTextStorage> {
    text: S,
    caret: usize,
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
    shaped_key: Option<(u64, u32, u32, u32)>,
    /// The height that shaping returned, cached with the key that
    /// produced it (M26/T26.7). `TextInput` keeps the width; a text
    /// area wraps, so what it needs back is the height.
    shaped_h: f32,
    line_h: f32,
    pad: (f32, f32),
    /// The caret's colour; the text colour when `None` — M25/T25.10.
    pub caret_color: Option<lumen_static::display::Color>,
    /// The selection highlight's colour; `input::selection_tint` of the
    /// text colour when `None` — M25/T25.10.
    pub selection_color: Option<lumen_static::display::Color>,
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
    pub fn compose(&mut self, c: lumen_static::event::Composition<'_>) -> bool {
        // Whatever was provisional goes first, in every arm: a preedit
        // replaces it, a commit supersedes it, a cancel drops it.
        let had = self.preedit.take();
        if let Some(r) = had.clone() {
            self.text.remove_range(r.clone());
            self.caret = r.start;
        }
        let text = match c {
            lumen_static::event::Composition::Cancel => {
                if had.is_some() {
                    self.shaped_key = None;
                }
                return had.is_some();
            }
            lumen_static::event::Composition::Preedit(t) | lumen_static::event::Composition::Commit(t) => t,
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
        if matches!(c, lumen_static::event::Composition::Preedit(_)) {
            // Only what the buffer took: `insert` moved the caret by
            // exactly that much.
            self.preedit = Some(at..self.caret);
        }
        true
    }

    /// Extend or collapse after a caret move. See `TextInput::moved`.
    fn moved(&mut self, extend: bool, from: usize) {
        if extend {
            self.anchor.get_or_insert(from);
        } else {
            self.anchor = None;
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

    fn prev_boundary(&self, at: usize) -> Option<usize> {
        let s = self.text.as_ref();
        s[..at.min(s.len())].char_indices().next_back().map(|(i, _)| i)
    }

    fn next_boundary(&self, at: usize) -> Option<usize> {
        let s = self.text.as_ref();
        s.get(at..)?.chars().next().map(|c| at + c.len_utf8())
    }

    fn cluster_at(&self, byte: usize) -> Option<Cluster> {
        let cl = self.clusters.as_slice();
        cl.iter().find(|c| c.byte as usize >= byte).copied().or_else(|| cl.last().copied())
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

    /// Caret position within the run, in logical px.
    fn caret_at(&self) -> (f32, f32) {
        match self.cluster_at(self.caret) {
            Some(c) => (c.x, c.line as f32 * self.line_h),
            None => (0.0, 0.0),
        }
    }

    /// Byte offset nearest a point in the run — where a click lands.
    /// The selection as one rect per line it covers.
    ///
    /// A multi-line selection is not one rectangle: it is a partial
    /// first line, whole middle lines, and a partial last one. Yielding
    /// per line keeps `paint` from having to know that, and keeps the
    /// geometry in the one place that has the clusters.
    fn selection_runs(
        &self,
        sel: &core::ops::Range<usize>,
    ) -> impl Iterator<Item = (f32, f32, u16)> + '_ {
        let cl = self.clusters.as_slice();
        let (lo, hi) = (sel.start, sel.end);
        let lines = cl.iter().map(|c| c.line).max().unwrap_or(0);
        (0..=lines).filter_map(move |line| {
            let mut bounds: Option<(f32, f32)> = None;
            for c in cl.iter().filter(|c| c.line == line) {
                let b = c.byte as usize;
                if b < lo || b > hi {
                    continue;
                }
                bounds = Some(match bounds {
                    None => (c.x, c.x),
                    Some((a, z)) => (a.min(c.x), z.max(c.x)),
                });
            }
            bounds.filter(|(a, z)| z > a).map(|(a, z)| (a, z, line))
        })
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
        self.moved(cx.mods.has(lumen_static::event::Mods::SHIFT), from);
        moved
    }

    fn offset_at(&self, x: f32, y: f32) -> usize {
        let line = if self.line_h > 0.0 { (y / self.line_h).max(0.0) as u16 } else { 0 };
        let cl = self.clusters.as_slice();
        let mut best = (f32::INFINITY, self.text.as_ref().len());
        for c in cl {
            // Prefer the right line; fall back to any if it is empty.
            let dl = (c.line as i32 - line as i32).abs() as f32 * 1e4;
            let d = dl + (c.x - x).abs();
            if d < best.0 {
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
            padding: lumen_static::style::Edges { left: 8.0, right: 8.0, top: 6.0, bottom: 6.0 },
            height: lumen_static::style::Length::Fixed(96.0),
            // An I-beam over editable text (T14.6).
            cursor: lumen_static::style::Cursor::Text,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::PADDING,
            lumen_static::style::Fields::CURSOR,
            lumen_static::style::Fields::FOCUS_RING,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
            lumen_static::style::Fields::PRESSED_DIM,
            lumen_static::style::Fields::HOVER_DIM,
        ]))];
}

impl<S: TextBuffer> Node for TextArea<S> {
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
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let px = me.font_size;
        let inner_w = (limits.max_w - me.padding.horizontal()).max(0.0);
        let key = (
            self.text.as_ref().len() as u64,
            px.to_bits(),
            me.font_weight.to_bits(),
            inner_w.to_bits(),
        );
        // Shaped against the *box*, unlike `TextInput` — wrapping is the
        // whole difference.
        if self.shaped_key != Some(key) {
            self.clusters.clear();
            let run =
                cx.text.shape(self.text.as_ref(), px, me.font_weight, inner_w, &mut self.clusters);
            self.line_h = cx.text.line_height(px);
            self.shaped_h = run.h;
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
        self.scroll = lumen_static::num::clamp(self.scroll, 0.0, (content.h - inner_h).max(0.0));
        self.pad = (me.padding.left, me.padding.top);

        lumen_static::ops::record_painted(st, i, limits, size, &me)
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        let (pl, pt, pr, pb) = s.pad;
        let inner = [r[0] + pl, r[1] + pt, r[2] - pr, r[3] - pb];
        // The run may be taller than the box; the clip is what keeps it
        // inside, as a scroller's content is (A1, one widget down).
        out.push_clip(inner);
        let y = inner[1] - self.scroll;
        out.text_weighted(
            [inner[0], y, inner[2], y + (inner[3] - inner[1]) + self.scroll],
            self.text.as_ref(),
            s.font_size,
            s.font_weight,
            s.fg,
        );
        // The selection, behind the run — same rule as `TextInput`: a
        // highlight painted after the glyphs would hide them.
        if let Some(sel) = self.selection() {
            for (x0, x1, line) in self.selection_runs(&sel) {
                let top = inner[1] + line as f32 * self.line_h - self.scroll;
                out.rect(
                    [inner[0] + x0, top, inner[0] + x1, top + self.line_h],
                    self.selection_color.unwrap_or_else(|| crate::input::selection_tint(s.fg)),
                );
            }
        }
        if let Some(pre) = self.preedit.clone() {
            for (x0, x1, line) in self.selection_runs(&pre) {
                let top = inner[1] + (line as f32 + 1.0) * self.line_h - self.scroll;
                out.rect([inner[0] + x0, top - 2.0, inner[0] + x1, top - 1.0], s.fg);
            }
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
        lumen_static::hit::contains(&st[i], x, y).then(|| Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        })
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // A newline is the one key a single-line field must refuse
            // and this one must accept.
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
            Event::Key { key: Key::Left, down: true } => self.moving(cx, Self::move_left),
            Event::Key { key: Key::Right, down: true } => self.moving(cx, Self::move_right),
            Event::Key { key: Key::Up, down: true } => self.moving(cx, |s| s.move_line(-1)),
            Event::Key { key: Key::Down, down: true } => self.moving(cx, |s| s.move_line(1)),
            Event::Key { key: Key::Char('a'), down: true }
                if cx.mods.has(lumen_static::event::Mods::CTRL)
                    || cx.mods.has(lumen_static::event::Mods::META) =>
            {
                self.select_all()
            }
            Event::Key { key: Key::Escape, down: true } => self.clear_selection(),
            Event::PointerDown { .. } => {
                let at = self.at_pointer(cx);
                let from = self.caret;
                self.caret = at;
                if cx.mods.has(lumen_static::event::Mods::SHIFT) {
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

    fn compose(&mut self, c: lumen_static::event::Composition<'_>) -> bool {
        TextArea::compose(self, c)
    }

    fn node(&self, _i: usize) -> &dyn Node {
        self
    }

    fn node_mut(&mut self, _i: usize) -> &mut dyn Node {
        self
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::MultilineTextInput);
        node.set_value(self.text.as_ref());
    }
}

impl<S: TextBuffer, A> lumen_static::event::Handlers<A> for TextArea<S> {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}
