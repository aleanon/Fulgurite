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
//!
//! # Keys and clicks
//!
//! | input | effect |
//! |---|---|
//! | Left, Right | one grapheme (M45/T45.5) |
//! | word modifier + Left, Right | to the start of the word, to the end of the next (M45/T45.6) |
//! | Home, End | to the ends of the value |
//! | Shift + any of the above | extends the selection |
//! | Ctrl+A, Cmd+A | selects everything |
//! | Escape | drops the selection |
//! | a press, a drag | puts the caret, selects to the pointer |
//! | a double click | selects the word under it (M45/T45.6) |
//! | a triple click | selects everything — a single line is its own line |
//!
//! The word modifier is Ctrl, and Alt on macOS. A word is a Unicode word
//! segment holding a letter, a digit or `_` (`crate::edit`). The arrows
//! stay in the field beside a focusable sibling ([`Node::owns_arrows`],
//! M44/T44.1), with a modifier or without.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1), and absent entirely without the
// `alloc` feature (T13.4).
// Only the accesskit `extra` arg needs it, and that is observe-gated.
#[cfg(feature = "a11y")]
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
    /// Caret position as a byte offset into `text`. Always on a grapheme
    /// boundary where there is an allocator, and on a `char` boundary
    /// always — every path that moves it steps through `crate::edit`.
    pub(crate) caret: usize,
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
    /// nothing: the text's length beside the [`MeasureKey`] every text
    /// memo builds (M45/T45.2) — the size, weight, display scale and
    /// measurer — and the text's *content* is covered because every edit
    /// goes through a method here.
    ///
    /// **No width.** A single line shapes the same run at any width, so
    /// the limit only made the agent's 1e6 px probe miss; the key's
    /// limits are [`SINGLE_LINE`]. And a scale change reshapes, which the
    /// hand-written tuple this replaced could not (R2-F6,
    /// `crates/lumen-static-widgets/tests/measure_scale_widgets.rs`).
    ///
    /// [`MeasureKey`]: lumen_static::text::MeasureKey
    shaped_key: Option<(lumen_static::text::MeasureKey, u32)>,
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

    /// Paste `text` as one edit — M45/T45.7. A single line keeps no line
    /// break: `\r\n`, `\r` and `\n` are dropped, as is every other
    /// control character, and the rest goes through [`TextInput::insert`],
    /// the path typing takes, so it replaces a selection and moves the
    /// caret exactly as typing would. Returns the characters kept: 0 when
    /// the storage refused, since a bounded storage takes all or nothing.
    pub fn paste(&mut self, text: &str) -> usize {
        let kept = crate::edit::printable(text, false);
        let n = kept.chars().count();
        if n > 0 && self.insert(&kept) {
            n
        } else {
            0
        }
    }

    /// Move the caret, extending the selection or collapsing it.
    ///
    /// `extend` is "Shift is held". This is the one place the anchor is
    /// created and destroyed, so the rule is stated once: extending from
    /// no selection anchors where the caret *was*; not extending drops
    /// the anchor.
    ///
    /// Returns whether a selection was dropped, which is a change even
    /// when the caret did not move: End at the end of a selected value
    /// collapses it, and an arm answering "unchanged" then would leave
    /// the highlight painted and let the key reach an ancestor
    /// (M45/T45.6).
    fn moved(&mut self, extend: bool, from: usize) -> bool {
        if extend {
            self.anchor.get_or_insert(from);
            false
        } else {
            self.selection().is_some() & self.anchor.take().is_some()
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

    /// Move the caret to the start of the word it is in, or of the one
    /// before it (M45/T45.6); returns whether it moved.
    pub fn move_word_left(&mut self) -> bool {
        match crate::edit::prev_word(self.text.as_ref(), self.caret) {
            Some(p) => core::mem::replace(&mut self.caret, p) != p,
            None => false,
        }
    }

    /// Move the caret to the end of the next word (M45/T45.6).
    pub fn move_word_right(&mut self) -> bool {
        match crate::edit::next_word(self.text.as_ref(), self.caret) {
            Some(n) => core::mem::replace(&mut self.caret, n) != n,
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

    /// The grapheme boundary before `at`, if any — `crate::edit`
    /// (M45/T45.5): a combining accent, a ZWJ sequence and a flag are each
    /// crossed whole, and a multi-byte character is never split, or the
    /// next slice panics. This is the operation the shaping seam's byte
    /// offsets are mapped back to the caller's string for (T7.2b).
    fn prev_boundary(&self, at: usize) -> Option<usize> {
        crate::edit::prev_boundary(self.text.as_ref(), at)
    }

    fn next_boundary(&self, at: usize) -> Option<usize> {
        crate::edit::next_boundary(self.text.as_ref(), at)
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

/// The limits a single-line field's shaped key is built with: none. Its
/// run is shaped unbounded (B7), so no width it is offered changes it.
const SINGLE_LINE: Constraints =
    Constraints { min_w: 0.0, max_w: f32::INFINITY, min_h: 0.0, max_h: f32::INFINITY };

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
        ]))];
}

impl<S: TextBuffer> Node for TextInput<S> {
    lumen_static::node_count!();
    /// Arrows move a caret, so they never leave this control
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
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let px = me.font_size;
        let key = (lumen_static::text::MeasureKey::new(cx, SINGLE_LINE, &me), self.text.as_ref().len() as u32);
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

    /// Measure without touching what paint reads — M41/T41.4, R1-F5.
    ///
    /// The provided probe is `layout`, and `layout` writes the scroll
    /// offset (and the padding it is drawn against, and the shaped
    /// clusters) from the limits it is handed. The agent's audit probes
    /// every leaf at 1e6 and 2e6 px, so after `frame_done` a scrolled
    /// field held a scroll of zero; under `Everything` the next layout put
    /// it back, and under `Boundaries` an unrelated hover left the field
    /// unmarked and paint drew the probe's scroll
    /// (`crates/lumen-static-agent/tests/audit_is_inert.rs`).
    ///
    /// The run is shaped against no width limit, so its width does not
    /// depend on the probe's box: the memo answers when it is warm — its
    /// key has no width since M45/T45.2, so the probe asks it the same
    /// question `layout` does — and a cold one is measured into a local
    /// and not kept.
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
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let px = me.font_size;
        let key = (lumen_static::text::MeasureKey::new(cx, SINGLE_LINE, &me), self.text.as_ref().len() as u32);
        let warm = self.shaped_key == Some(key);
        let content = if warm {
            Size { w: self.shaped_size.0, h: self.shaped_size.1 }
        } else {
            let run = cx.text.measure_weighted(self.text.as_ref(), px, me.font_weight, f32::INFINITY);
            Size { w: run.w, h: cx.text.line_height(px) }
        };
        let padded =
            Size { w: content.w + me.padding.horizontal(), h: content.h + me.padding.vertical() };
        lumen_static::ops::record_painted(st, i, limits, resolve_self(limits, &me, padded), &me)
    }

    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        lumen_static::ops::focus_ring(st, i, out);
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
        // The run's own width, not the field's: a painter wraps inside the
        // rect it is handed, and a field scrolled part-way used to hand it
        // `inner + scroll`, narrower than the run, so the word crossing
        // the right edge moved to a second line below the clip and the
        // visible line ended a word early (M45/T45.1). The clip above is
        // what keeps the text inside the field, not the rect.
        let run_w = self.shaped_size.0.max(inner[2] - inner[0] + self.scroll);
        out.text_weighted(
            [x, inner[1], x + run_w, inner[3]],
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
            // With the word modifier (Ctrl, Alt on macOS) an arrow moves
            // by word, not by grapheme (M45/T45.6).
            Event::Key { key: Key::Left, down: true } => {
                let from = self.caret;
                let moved = if cx.mods.has(crate::edit::WORD) { self.move_word_left() } else { self.move_left() };
                self.moved(cx.mods.has(Mods::SHIFT), from) | moved
            }
            Event::Key { key: Key::Right, down: true } => {
                let from = self.caret;
                let moved = if cx.mods.has(crate::edit::WORD) { self.move_word_right() } else { self.move_right() };
                self.moved(cx.mods.has(Mods::SHIFT), from) | moved
            }
            Event::Key { key: Key::Home, down: true } => {
                let from = self.caret;
                let moved = self.move_home();
                self.moved(cx.mods.has(Mods::SHIFT), from) | moved
            }
            Event::Key { key: Key::End, down: true } => {
                let from = self.caret;
                let moved = self.move_end();
                self.moved(cx.mods.has(Mods::SHIFT), from) | moved
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
            //
            // A double click selects the word under it and a triple click
            // the whole value (M45/T45.6): a single line is its own line.
            Event::PointerDown { .. } if cx.clicks >= 2 => {
                if cx.clicks == 2 {
                    let at = self.offset_at(cx.local.x - self.pad_left + self.scroll);
                    let w = crate::edit::word_at(self.text.as_ref(), at);
                    self.anchor = Some(w.start);
                    self.caret = w.end;
                    true
                } else {
                    self.select_all()
                }
            }
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

    fn paste(&mut self, text: &str) -> Option<usize> {
        Some(TextInput::paste(self, text))
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
