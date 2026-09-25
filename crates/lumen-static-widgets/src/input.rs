//! A single-line text field — plan M9, Cluster B of
//! `docs/text-and-scrolling-decisions.md`.
//!
//! # What is here, and what is deliberately not
//!
//! - **The widget owns the buffer** (B1). A `Label`'s text originates in
//!   application state and the widget holds a copy; a field's text
//!   originates *at the widget*, because the user typed it there. Same
//!   rule — whoever can invalidate the shaped cache owns the storage —
//!   pointing the other way.
//! - **Edits go through methods** (B3), never a `&mut String`. Undo, IME
//!   preedit and change notification all need a seam where an edit is
//!   observed; a method boundary costs nothing today and keeps them
//!   possible.
//! - **The caret is a `Cmd::Rect`** (B4). No display-list command was
//!   added for it.
//! - **Caret-into-view happens in `layout`** (B5), after shaping, which
//!   is the first moment the caret's x is known.
//! - **No IME** (B6) and **no multi-line** (B7) in this cut, and the
//!   fields are not reserved for them either — an unused field is a
//!   claim this does not yet earn. Both are named here instead.
//! - **The application reads the value directly**, `field.as_str()`
//!   (B8).
//!
//! # Selection — M14/T14.4
//!
//! This note used to say selection was blocked on two things the
//! framework did not have: modifier state on `Event::Key`, and pointer
//! drag, "`PointerMove` is deliberately not routed". **The second half
//! was already false when it was written.** M11/T11.2 added pointer
//! capture, and `pass::dispatch` routes `PointerMove` *directly* to a
//! captured node — which is how `Slider` has worked since. A comment
//! describing machinery in another file, which `AGENT.md` names as the
//! fragile kind, and it outlived the fact by a milestone.
//!
//! Both halves exist now. `anchor` is the other end of the selection:
//! `None` when there is none, `Some(byte)` when the caret is dragging
//! away from a fixed point. Shift+Arrow sets it from the modifiers
//! (T14.1); a drag sets it from the press and moves the caret with
//! capture. The selection is painted as a `Cmd::Rect` behind the run —
//! B4 decided the rectangle before there was anything to put in it.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
// Only the accesskit `extra` arg needs it, and that is observe-gated.
#[cfg(feature = "observe")]
use alloc::vec::Vec;

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key, Mods};
use lumen_static::hit::Hit;
use lumen_static::ops::{rect_of, resolve_self};
use lumen_static::style::{Seed, Style};
use lumen_static::text::{ClusterBuf, TextBuffer};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, Point, Size};

/// A single-line editable text field.
pub struct TextInput<S: TextBuffer = lumen_static::text::DefaultTextStorage> {
    text: S,
    /// Caret position as a byte offset into `text`. Always on a `char`
    /// boundary — every path that moves it steps by clusters.
    caret: usize,
    /// The provisional text an input method is composing, as a byte
    /// range into `text` (M14/T14.8).
    ///
    /// The text *is* in the buffer, so it measures, positions and
    /// scrolls like any other; what makes it provisional is that the
    /// next `Preedit` replaces exactly this range.
    preedit: Option<core::ops::Range<usize>>,
    /// The fixed end of a selection, if there is one (T14.4).
    ///
    /// `None` means no selection: the caret is a point. B6/B7 refused
    /// to *reserve* this field while nothing could move it; T14.1's
    /// modifiers and T11.2's capture are what can.
    anchor: Option<usize>,
    /// Horizontal offset so the caret stays visible in a field narrower
    /// than its content (B5). Written by `layout`.
    scroll: f32,
    /// The last shaped run's cluster boundaries (B2's separate cache:
    /// `Measured` stays one `Cell` per label, and only fields that need
    /// caret geometry pay for an array).
    clusters: S::Clusters,
    /// What `clusters` was shaped from, so an unchanged frame reshapes
    /// nothing. Text length and the style inputs; the text's *content*
    /// is covered because every edit goes through a method here.
    shaped_key: Option<(u64, u32, u32, u32)>,
    /// What that shaping *returned*: the run's width and the line
    /// height, cached with the key that produced them (M26/T26.7).
    ///
    /// **`shape` already answers this.** It returns a `Size`, and this
    /// file threw it away and called `measure_weighted` on the same run
    /// on the very next line — unconditionally, every layout, including
    /// the frames the shape cache hit. Eight bytes to stop re-measuring
    /// text that has not changed.
    shaped_size: (f32, f32),
    /// The resolved left padding from the last layout.
    ///
    /// `on_event` has no `&[Style]` and no `&[NodeState]` — it gets the
    /// node's own `local` point and nothing else — so the offset from
    /// the node's edge to the first glyph has to be remembered here.
    /// The same reason the scroller keeps its measured overflow.
    pad_left: f32,
    /// The caret's colour; the text colour when `None` — M25/T25.10.
    pub caret_color: Option<Color>,
    /// The selection highlight's colour; [`selection_tint`] of the text
    /// colour when `None` — M25/T25.10.
    pub selection_color: Option<Color>,
}

impl<S: TextBuffer> Default for TextInput<S> {
    fn default() -> Self {
        TextInput {
            text: S::default(),
            caret: 0,
            anchor: None,
            preedit: None,
            scroll: 0.0,
            clusters: S::Clusters::default(),
            shaped_key: None,
            shaped_size: (0.0, 0.0),
            pad_left: 0.0,
            caret_color: None,
            selection_color: None,
        }
    }
}

impl<S: TextBuffer> TextInput<S> {
    pub fn new(text: &str) -> Self {
        let mut f = TextInput::default();
        f.set_str(text);
        f
    }

    /// The current value (B8: the application reads it directly).
    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    /// Caret byte offset.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Replace the whole value and put the caret at the end.
    pub fn set_str(&mut self, s: &str) {
        self.text.set_from_str(s);
        self.caret = self.text.as_ref().len();
        self.anchor = None;
        self.invalidate();
    }

    /// The selected byte range, or `None` when nothing is selected.
    ///
    /// Always ordered: a selection dragged leftwards reports the same
    /// range as one dragged rightwards, because a caller wants to slice
    /// with it.
    pub fn selection(&self) -> Option<core::ops::Range<usize>> {
        let a = self.anchor?;
        let (lo, hi) = if a <= self.caret { (a, self.caret) } else { (self.caret, a) };
        (lo < hi).then_some(lo..hi)
    }

    /// The selected text, or `None`.
    pub fn selected_str(&self) -> Option<&str> {
        let r = self.selection()?;
        self.text.as_ref().get(r)
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

    /// Drop the selection, keeping the caret where it is.
    pub fn clear_selection(&mut self) -> bool {
        self.anchor.take().is_some()
    }

    /// Delete the selection if there is one. Returns whether it deleted.
    ///
    /// Every edit calls this first: typing over a selection replaces it,
    /// which is the behaviour a user expects and the one a field without
    /// it gets subtly wrong.
    pub fn delete_selection(&mut self) -> bool {
        let Some(r) = self.selection() else {
            self.anchor = None;
            return false;
        };
        self.text.remove_range(r.clone());
        self.caret = r.start;
        self.anchor = None;
        self.invalidate();
        true
    }

    /// The provisional range, if a composition is in progress.
    pub fn preedit(&self) -> Option<core::ops::Range<usize>> {
        self.preedit.clone()
    }

    /// Apply one composition step (M14/T14.8).
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
            self.invalidate();
        }
        let text = match c {
            lumen_static::event::Composition::Cancel => return had.is_some(),
            lumen_static::event::Composition::Preedit(t) | lumen_static::event::Composition::Commit(t) => t,
        };
        if text.is_empty() {
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

    /// Move the caret, extending the selection or collapsing it.
    ///
    /// `extend` is "Shift is held". This is the one place the anchor is
    /// created and destroyed, so the rule is stated once: extending from
    /// no selection anchors where the caret *was*; not extending drops
    /// the anchor.
    fn moved(&mut self, extend: bool, from: usize) {
        if extend {
            self.anchor.get_or_insert(from);
        } else {
            self.anchor = None;
        }
    }

    /// Insert at the caret and advance it (B3).
    pub fn insert(&mut self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        // Typing over a selection replaces it.
        self.delete_selection();
        // The caret advances only if the storage took the text. A
        // bounded storage can refuse, and advancing anyway would put
        // the caret past the end (M13/T13.5).
        if !self.text.insert_str(self.caret, s) {
            return false;
        }
        self.caret += s.len();
        self.invalidate();
        true
    }

    /// Delete the character before the caret. Returns whether anything
    /// was removed.
    pub fn backspace(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let Some(prev) = self.prev_boundary(self.caret) else { return false };
        self.text.remove_range(prev..self.caret);
        self.caret = prev;
        self.invalidate();
        true
    }

    /// Delete the character after the caret.
    pub fn delete(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let Some(next) = self.next_boundary(self.caret) else { return false };
        self.text.remove_range(self.caret..next);
        self.invalidate();
        true
    }

    /// Move the caret one character left; returns whether it moved.
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

    pub fn move_home(&mut self) -> bool {
        core::mem::replace(&mut self.caret, 0) != 0
    }

    pub fn move_end(&mut self) -> bool {
        let end = self.text.as_ref().len();
        core::mem::replace(&mut self.caret, end) != end
    }

    /// The `char` boundary before `at`, if any.
    ///
    /// Byte arithmetic, not `- 1`: a caret sits on a boundary and a
    /// multi-byte character must be crossed whole, or the next slice
    /// panics. This is the operation the shaping seam's byte offsets are
    /// mapped back to the caller's string for (T7.2b).
    fn prev_boundary(&self, at: usize) -> Option<usize> {
        let s = self.text.as_ref();
        s[..at.min(s.len())].char_indices().next_back().map(|(i, _)| i)
    }

    fn next_boundary(&self, at: usize) -> Option<usize> {
        let s = self.text.as_ref();
        s.get(at..)?.chars().next().map(|c| at + c.len_utf8())
    }

    fn invalidate(&mut self) {
        self.shaped_key = None;
    }

    /// X of the caret within the run, from the shaped clusters.
    ///
    /// Falls back to the run's end when the measurer reported no
    /// clusters — `BlockMetrics` does, deliberately (T7.2b) — so a field
    /// under the stand-in still shows a caret in a sensible place
    /// instead of at zero.
    fn caret_x(&self, run_w: f32) -> f32 {
        if self.clusters.as_slice().is_empty() {
            return run_w;
        }
        self.x_of(self.caret, run_w)
    }

    /// The x of a byte offset within the run — `caret_x` generalised,
    /// so the selection rect and the caret cannot disagree about where a
    /// boundary is.
    fn x_of(&self, at: usize, run_w: f32) -> f32 {
        let cl = self.clusters.as_slice();
        if cl.is_empty() {
            return if at == 0 { 0.0 } else { run_w };
        }
        cl.iter()
            .find(|c| c.byte as usize >= at)
            .map(|c| c.x)
            .unwrap_or_else(|| cl.last().map(|c| c.x).unwrap_or(0.0))
    }

    /// Byte offset nearest an x within the run — a click's landing spot.
    fn offset_at(&self, x: f32) -> usize {
        let cl = self.clusters.as_slice();
        let mut best = (f32::INFINITY, self.text.as_ref().len());
        for c in cl {
            let d = (c.x - x).abs();
            if d < best.0 {
                best = (d, c.byte as usize);
            }
        }
        best.1
    }
}

/// The selection highlight when a field sets none: the foreground colour
/// at low alpha.
///
/// **A default, not the only answer**, since M25/T25.10: a field that
/// wants a different highlight sets its `selection_color`. It is not a
/// `Style` field because paint only has `NodeState`, which is 76 B and
/// asserted at that size by `fixtures/embedded` — four bytes times every
/// node of every tree for a colour one widget uses. A field on the widget
/// costs bytes only where a text field is.
///
/// The derivation works in both light and dark themes because it tints
/// with the *text* colour, which is by construction the one that
/// contrasts with the field. The canvas blends alpha, so the glyphs —
/// painted after this rect — stay readable through it.
pub fn selection_tint(fg: Color) -> Color {
    Color(fg.0, fg.1, fg.2, 64)
}

impl<S: TextBuffer> Widget for TextInput<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    const KIND: &'static str = "textinput";

    fn type_defaults() -> Style {
        Style {
            padding: lumen_static::style::Edges { left: 8.0, right: 8.0, top: 6.0, bottom: 6.0 },
            // An I-beam over editable text, without the application
            // having to write a rule for it (T14.6).
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
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];
}

impl<S: TextBuffer> Node for TextInput<S> {
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
        let key = (
            self.text.as_ref().len() as u64,
            px.to_bits(),
            me.font_weight.to_bits(),
            limits.max_w.to_bits(),
        );
        // A single line: shaped against no width limit, because wrapping
        // is what a field does not do (B7) and clamping the shape to the
        // box would move the clusters every time the field scrolled.
        if self.shaped_key != Some(key) {
            self.clusters.clear();
            let run = cx.text.shape(
                self.text.as_ref(),
                px,
                me.font_weight,
                f32::INFINITY,
                &mut self.clusters,
            );
            self.shaped_size = (run.w, cx.text.line_height(px));
            self.shaped_key = Some(key);
        }
        let (run_w, line) = self.shaped_size;

        let content = Size { w: run_w, h: line };
        let padded =
            Size { w: content.w + me.padding.horizontal(), h: content.h + me.padding.vertical() };
        let size = resolve_self(limits, &me, padded);

        // B5: caret into view, here, because this is the first moment
        // the caret's x is known. Same place and shape as A6's scroll
        // clamp, and for the same reason.
        let inner_w = (size.w - me.padding.horizontal()).max(0.0);
        let cx_pos = self.caret_x(run_w);
        if cx_pos - self.scroll > inner_w {
            self.scroll = cx_pos - inner_w;
        }
        if cx_pos < self.scroll {
            self.scroll = cx_pos;
        }
        self.scroll = lumen_static::num::clamp(self.scroll, 0.0, (run_w - inner_w).max(0.0));
        self.pad_left = me.padding.left;

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
        // The text may be wider than the field; the clip is what keeps
        // it inside, exactly as a scroller's content is kept inside
        // (A1's scoped containment, one widget down).
        out.push_clip(inner);
        let x = inner[0] - self.scroll;
        // B4 decided the selection is an ordinary rect, and it is
        // painted *before* the run so the text sits on top of it —
        // a selection drawn after would hide the glyphs it highlights.
        if let Some(sel) = self.selection() {
            let run_w = inner[2] - inner[0];
            let (a, b) = (self.x_of(sel.start, run_w), self.x_of(sel.end, run_w));
            out.rect(
                [x + a, inner[1], x + b, inner[3]],
                self.selection_color.unwrap_or_else(|| selection_tint(s.fg)),
            );
        }
        out.text_weighted(
            [x, inner[1], x + (inner[2] - inner[0]) + self.scroll, inner[3]],
            self.text.as_ref(),
            s.font_size,
            s.font_weight,
            s.fg,
        );
        // B4: the caret is an ordinary rect. Only in the focused field —
        // a caret in every field at once is the tell that focus is not
        // actually being tracked.
        // The composing run is underlined, which is how every platform
        // shows "this is not committed yet" (T14.8).
        if let Some(pre) = self.preedit.clone() {
            let run_w = inner[2] - inner[0];
            let (a, b) = (self.x_of(pre.start, run_w), self.x_of(pre.end, run_w));
            out.rect([x + a, inner[3] - 2.0, x + b, inner[3] - 1.0], s.fg);
        }
        if s.focused {
            let cx_pos = inner[0] - self.scroll + self.caret_x(inner[2] - inner[0]);
            out.rect([cx_pos, inner[1], cx_pos + 1.0, inner[3]], self.caret_color.unwrap_or(s.fg));
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

    /// C3: the field edits itself.
    ///
    /// Returns `true` only when something actually changed, so an
    /// unhandled key still reaches an ancestor — the same contract the
    /// scroller's chaining relies on (C4).
    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        let changed = match ev {
            Event::Text { ch } if !ch.is_control() => {
                let mut buf = [0u8; 4];
                self.insert(ch.encode_utf8(&mut buf));
                true
            }
            Event::Key { key: Key::Backspace, down: true } => self.backspace(),
            Event::Key { key: Key::Delete, down: true } => self.delete(),
            // Shift extends the selection, anything else collapses it.
            // `moved` is called with where the caret *was*, because that
            // is the anchor a fresh selection starts from (T14.4).
            Event::Key { key: Key::Left, down: true } => {
                let from = self.caret;
                let moved = self.move_left();
                self.moved(cx.mods.has(Mods::SHIFT), from);
                moved
            }
            Event::Key { key: Key::Right, down: true } => {
                let from = self.caret;
                let moved = self.move_right();
                self.moved(cx.mods.has(Mods::SHIFT), from);
                moved
            }
            Event::Key { key: Key::Home, down: true } => {
                let from = self.caret;
                let moved = self.move_home();
                self.moved(cx.mods.has(Mods::SHIFT), from);
                moved
            }
            Event::Key { key: Key::End, down: true } => {
                let from = self.caret;
                let moved = self.move_end();
                self.moved(cx.mods.has(Mods::SHIFT), from);
                moved
            }
            // Ctrl+A / Cmd+A. Both, because which one is "the" shortcut
            // modifier is the platform's business and `Mods` reports
            // both truthfully (T14.1).
            Event::Key { key: Key::Char('a'), down: true }
                if cx.mods.has(Mods::CTRL) || cx.mods.has(Mods::META) =>
            {
                self.select_all()
            }
            Event::Key { key: Key::Escape, down: true } => self.clear_selection(),
            // A click puts the caret where it landed. `local` is
            // relative to the node's origin, and the run starts one
            // padding in — read from the resolved attributes rather than
            // guessed, because a stylesheet may have changed it.
            Event::PointerDown { .. } => {
                let at = self.offset_at(cx.local.x - self.pad_left + self.scroll);
                let from = self.caret;
                self.caret = at;
                // Shift+click extends from the caret, as every toolkit
                // does; a plain press starts a drag whose anchor is
                // where it landed.
                if cx.mods.has(Mods::SHIFT) {
                    self.anchor.get_or_insert(from);
                } else {
                    self.anchor = Some(at);
                }
                true
            }
            // A drag. This reaches here only while this node holds the
            // pointer capture T11.2 added — the mechanism the old note
            // in this file said did not exist.
            Event::PointerMove { .. } => {
                let at = self.offset_at(cx.local.x - self.pad_left + self.scroll);
                core::mem::replace(&mut self.caret, at) != at
            }
            // A press that never moved is a caret, not an empty
            // selection: `selection()` already reports `None` for a
            // zero-width range, and dropping the anchor here keeps
            // `anchor.is_some()` meaning "there is a selection".
            Event::PointerUp { .. } => {
                if self.selection().is_none() {
                    self.anchor = None;
                }
                false
            }
            _ => false,
        };
        changed
    }

    fn selection(&self) -> Option<&str> {
        self.selected_str()
    }

    fn compose(&mut self, c: lumen_static::event::Composition<'_>) -> bool {
        TextInput::compose(self, c)
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
        node.set_role(lumen_static::observe::accesskit::Role::TextInput);
        node.set_value(self.text.as_ref());
    }
}

impl<S: TextBuffer, A> lumen_static::event::Handlers<A> for TextInput<S> {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}
