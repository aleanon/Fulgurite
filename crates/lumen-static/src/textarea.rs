//! A multi-line text area — plan M11/T11.3.
//!
//! # Why a separate type, and what that revises
//!
//! `docs/text-and-scrolling-decisions.md` B7 decided "no multi-line in
//! the first cut" and said the mechanism would be **one widget type
//! with a `wrap` field reserved**. M9 shipped [`crate::TextInput`] with no such
//! field — an unused field is a claim the code has not earned — and this
//! is the other half of that: multi-line is a **separate widget**.
//!
//! The reason is not taste. Look at what actually differs:
//! [`crate::TextInput`] shapes against an *unbounded* width because it must not
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

use crate::cx::{Cluster, LayoutCx};
use crate::desc::{NodeDesc, NodeState, NO_PARENT};
use crate::display::Painter;
use crate::event::{Event, EventCtx, Key};
use crate::hit::Hit;
use crate::ops::{rect_of, resolve_self};
use crate::style::Style;
use crate::text::{ClusterBuf, TextBuffer};
use crate::widget::{Node, Widget};
use crate::{Constraints, Point, Size};

/// A wrapping, editable, multi-line text box.
pub struct TextArea<S: TextBuffer = crate::text::DefaultTextStorage> {
    text: S,
    caret: usize,
    /// Vertical offset in logical px so the caret stays visible (B5,
    /// one dimension over from `TextInput`'s horizontal one).
    scroll: f32,
    clusters: S::Clusters,
    shaped_key: Option<(u64, u32, u32, u32)>,
    line_h: f32,
    pad: (f32, f32),
}

impl<S: TextBuffer> Default for TextArea<S> {
    fn default() -> Self {
        TextArea {
            text: S::default(),
            caret: 0,
            scroll: 0.0,
            clusters: S::Clusters::default(),
            shaped_key: None,
            line_h: 0.0,
            pad: (0.0, 0.0),
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

    pub fn backspace(&mut self) -> bool {
        let Some(prev) = self.prev_boundary(self.caret) else { return false };
        self.text.remove_range(prev..self.caret);
        self.caret = prev;
        self.shaped_key = None;
        true
    }

    pub fn delete(&mut self) -> bool {
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
}

impl<S: TextBuffer> Node for TextArea<S> {
    fn kind_name(&self) -> &'static str {
        "textarea"
    }

    fn defaults(&self) -> Style {
        Style {
            padding: crate::style::Edges { left: 8.0, right: 8.0, top: 6.0, bottom: 6.0 },
            height: crate::style::Length::Fixed(96.0),
            ..Style::default()
        }
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
            return crate::ops::hide_subtree(st, i, limits, 1);
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
            cx.text.shape(self.text.as_ref(), px, me.font_weight, inner_w, &mut self.clusters);
            self.shaped_key = Some(key);
        }
        self.line_h = cx.text.line_height(px);
        let run = cx.text.measure_weighted(self.text.as_ref(), px, me.font_weight, inner_w);

        let content = Size { w: inner_w, h: run.h.max(self.line_h) };
        let padded = Size {
            w: content.w + me.padding.horizontal(),
            h: content.h + me.padding.vertical(),
        };
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
        self.scroll = self.scroll.clamp(0.0, (content.h - inner_h).max(0.0));
        self.pad = (me.padding.left, me.padding.top);

        crate::ops::record_painted(st, i, limits, size, &me)
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        out.rounded_rect(r, s.bg, s.radius);
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
        if s.focused {
            let (cx, cy) = self.caret_at();
            let x = inner[0] + cx;
            let top = inner[1] + cy - self.scroll;
            out.rect([x, top, x + 1.0, top + self.line_h], s.fg);
        }
        out.pop_clip();
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        crate::hit::contains(&st[i], x, y).then(|| Hit {
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
            Event::Key { key: Key::Left, down: true } => self.move_left(),
            Event::Key { key: Key::Right, down: true } => self.move_right(),
            Event::Key { key: Key::Up, down: true } => self.move_line(-1),
            Event::Key { key: Key::Down, down: true } => self.move_line(1),
            Event::PointerDown { .. } => {
                let at = self.offset_at(
                    cx.local.x - self.pad.0,
                    cx.local.y - self.pad.1 + self.scroll,
                );
                core::mem::replace(&mut self.caret, at) != at
            }
            _ => false,
        }
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
        node: &mut crate::observe::accesskit::Node,
        _extra: &mut Vec<(crate::observe::accesskit::NodeId, crate::observe::accesskit::Node)>,
    ) {
        node.set_role(crate::observe::accesskit::Role::MultilineTextInput);
        node.set_value(self.text.as_ref());
    }
}

impl<S: TextBuffer, A> crate::event::Handlers<A> for TextArea<S> {
    fn handler(&self, _sel: crate::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}
