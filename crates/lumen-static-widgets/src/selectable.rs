//! **Read-only text a reader can select and copy** — M80/T80.4
//! (`docs/plan-text-fit.md`, "Read-only selection is `Selectable<S>`").
//!
//! A [`Label`] that takes focus, answers a press with a caret and a drag
//! with a selection, and hands the selection to a copy. It is **one table
//! entry, sharing its label's index**: focusability is the const
//! `NodeDesc::FOCUSABLE` flag on that entry, and no `Node` method exposes
//! a child's text, so "make any child selectable" has no mechanism. The
//! widget owns its label, lays it out and paints it at the same index,
//! and **answers [`Node::selection`] itself** — `Host::copy` asks the
//! focused index, so the answer is this widget's, as it is
//! `NumberInput`'s for the field it shares an index with.
//!
//! **Ctrl+X is a copy.** A selection makes the window's shortcut decision
//! answer Cut, which sets the clipboard and then posts a `Backspace`; this
//! widget does not handle `Backspace` or `Delete`, so the text reaches the
//! clipboard and nothing changes (`docs/plan-text-fit.md`, "Ctrl+X on a
//! `Selectable` is a copy").
//!
//! **Where the rows are.** The label's run is painted by the renderer, so
//! the tint must land on the rows the renderer used: at a line height
//! (`Style::line_height`) the pitch and the half-deficit shift the placer
//! applies (`lumen_static::cx::pitched_height`); at 0, the content height
//! over the line count, because the painter advances by the face's
//! ceil'd new-line size and `TextMeasure::line_height` does not round.
//!
//! **Allocator tier only**, beside `Select` and `RichText`: its cluster
//! list is a `Vec`, so the leaf firmware does not link it. Not over
//! `RichText` or `Markdown`, which paint word by word, so one cluster list
//! does not describe them (deferred, `docs/plan-text-fit.md`).

use alloc::vec::Vec;

use lumen_static::cx::{Cluster, LayoutCx};
use lumen_static::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key, Mods};
use lumen_static::style::{Seed, Style};
use lumen_static::text::{MeasureKey, TextStorage};
use lumen_static::widget::{Node, Widget};

use crate::ops::Label;

/// A [`Label`] whose text can be selected with the pointer and copied.
///
/// A press puts the caret where it landed, a drag (under the pointer
/// capture a consumed press takes) extends the selection, a double press
/// selects the word under it (`edit::word_at`), a triple press or Ctrl+A
/// (Cmd+A) the whole text, and losing focus clears it. The selection is
/// tinted behind the run (the label's ground first, then the tint, then
/// the glyphs, through `Label`'s paint split).
///
/// **Ctrl+X degrades to copy**: `Backspace` and `Delete` are not handled,
/// so the window's Cut copies and its follow-up delete does nothing.
///
/// Styled as its own kind, `selectable`, reading what a `Label` reads.
pub struct Selectable<S: TextStorage = &'static str> {
    /// The text and its layout. Public, as on every wrapper here; a text
    /// set through it directly is caught by the label's dirty bit.
    pub label: Label<S>,
    /// The highlight's colour; [`tint_of`] the text colour when `None`.
    pub selection_color: Option<Color>,
    /// The shown text's clusters, shaped against the room the label wraps
    /// in, so a cluster's `line` is the painted run's line.
    clusters: Vec<Cluster>,
    /// The right edge of each line's last character, by line.
    line_ends: Vec<f32>,
    /// What `clusters` was shaped under: the key the label's memo uses,
    /// and the shown text's length. A still frame reshapes nothing.
    shaped: Option<(MeasureKey, u32)>,
    /// Row pitch and the first row's offset from the content box's top.
    rows: (f32, f32),
    /// The content box's offset in the node (padding left, top), for a
    /// pointer's local coordinates.
    pad: (f32, f32),
    anchor: usize,
    caret: usize,
}

impl<S: TextStorage + Default> Default for Selectable<S> {
    fn default() -> Self {
        Selectable::new(S::default())
    }
}

impl<S: TextStorage> Selectable<S> {
    pub fn new(text: S) -> Self {
        Selectable {
            label: Label::new(text),
            selection_color: None,
            clusters: Vec::new(),
            line_ends: Vec::new(),
            shaped: None,
            rows: (0.0, 0.0),
            pad: (0.0, 0.0),
            anchor: 0,
            caret: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        self.label.as_str()
    }

    /// The selected byte range, if any (empty is none).
    pub fn selection(&self) -> Option<core::ops::Range<usize>> {
        let (a, b) = (self.anchor.min(self.caret), self.anchor.max(self.caret));
        (a < b).then_some(a..b)
    }

    /// The selected text, if any — what a copy takes.
    pub fn selected_str(&self) -> Option<&str> {
        self.selection().and_then(|r| self.label.shown().0.get(r))
    }

    /// Select the whole shown text. Returns whether the selection changed.
    pub fn select_all(&mut self) -> bool {
        let n = self.label.shown().0.len();
        let changed = self.selection() != (n > 0).then_some(0..n);
        self.anchor = 0;
        self.caret = n;
        changed
    }

    /// Drop the selection. Returns whether there was one.
    pub fn clear_selection(&mut self) -> bool {
        let had = self.selection().is_some();
        self.anchor = self.caret;
        had
    }

    /// Keep the caret and the anchor inside the shown text and on a
    /// character boundary — a bind that shortens the text must not leave
    /// a selection reaching past its end (or a copy slicing mid-character).
    fn clamp(&mut self) {
        let s = self.label.shown().0;
        self.anchor = crate::edit::floor_boundary(s, self.anchor);
        self.caret = crate::edit::floor_boundary(s, self.caret);
    }

    /// The byte nearest a point in the content box: the row it falls in,
    /// then the nearest cluster boundary on that row (a row's end
    /// included, so a press past a line's last character lands after it).
    fn offset_at(&self, x: f32, y: f32) -> usize {
        let cl = &self.clusters;
        let Some(last) = cl.last() else { return 0 };
        let (pitch, top) = self.rows;
        let max_line = last.line;
        let line = if pitch > 0.0 { ((y - top) / pitch).max(0.0) as u32 } else { 0 };
        let line = (line.min(max_line as u32)) as u16;
        let len = self.label.shown().0.len();
        let mut best = (f32::INFINITY, len);
        for (k, c) in cl.iter().enumerate() {
            if c.line != line {
                continue;
            }
            let d = (c.x - x).abs();
            if d < best.0 {
                best = (d, c.byte as usize);
            }
            // The row's end: after its last character, which is where the
            // next row starts.
            let next_line = cl.get(k + 1).is_none_or(|n| n.line != line);
            if next_line {
                let end = self.line_ends.get(line as usize).copied().unwrap_or(c.x);
                let to = cl.get(k + 1).map_or(len, |n| n.byte as usize);
                if (end - x).abs() < best.0 {
                    best = ((end - x).abs(), to);
                }
            }
        }
        best.1
    }

    /// The selection as one rect per row it covers, `(x0, x1, line)` in
    /// the content box: a partial first row, whole middle rows, a partial
    /// last one.
    fn tint_rows(&self, sel: core::ops::Range<usize>, mut f: impl FnMut(f32, f32, u16)) {
        let cl = &self.clusters;
        let lines = cl.last().map_or(0, |c| c.line as usize + 1);
        let mut k = 0;
        for line in 0..lines {
            let start = k;
            while k < cl.len() && cl[k].line as usize == line {
                k += 1;
            }
            let row = &cl[start..k];
            if !row.iter().any(|c| (sel.start..sel.end).contains(&(c.byte as usize))) {
                continue;
            }
            let end = self.line_ends.get(line).copied().unwrap_or(0.0);
            let x_of = |b: usize| row.iter().find(|c| c.byte as usize >= b).map_or(end, |c| c.x);
            let (x0, x1) = (x_of(sel.start), x_of(sel.end));
            if x1 > x0 {
                f(x0, x1, line as u16);
            }
        }
    }
}

/// The highlight when none is set: [`crate::input::selection_tint`]'s
/// colour (the text's own) at **40** alpha, not its 64.
///
/// Measured, not chosen: at 64 the showcase's dark palette put this
/// widget's text at **3.62:1** over its own tint, under AA's 4.5, in
/// `every_demo_text_reads_in_both_palettes` (M57's D4 gate; light
/// light-on-dark text over a light-tinted dark ground is the bad case).
/// At 40 both palettes pass (`docs/results-text.md` M80/T80.4). A field's
/// tint is drawn under text too and was never measured, because no demo
/// opens with a field's text selected; that is recorded, not changed here.
pub fn tint_of(fg: Color) -> Color {
    let t = crate::input::selection_tint(fg);
    Color(t.0, t.1, t.2, TINT_ALPHA)
}

/// [`tint_of`]'s alpha.
pub const TINT_ALPHA: u8 = 40;

impl<S: lumen_static::text::TextBuffer> lumen_static::bind::TextTarget for Selectable<S> {
    /// The binding seam, as `Label`'s, and **the selection follows the
    /// text**: a bind that replaces it clamps the caret and the anchor to
    /// the new text's length and character boundaries, so a copy never
    /// slices past the end or inside a character.
    fn set_str(&mut self, text: &str) -> bool {
        let changed = self.label.set_str(text);
        if changed {
            self.clamp();
        }
        changed
    }
}

impl<S: TextStorage> Widget for Selectable<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    const KIND: &'static str = "selectable";

    /// A label's look with an I-beam: the pointer says the text can be
    /// selected.
    fn type_defaults() -> Style {
        Style { cursor: lumen_static::style::Cursor::Text, ..Style::default() }
    }

    /// What the label it lays out reads — the same list as `Label`'s,
    /// overflow included (this tier always has the cut).
    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, lumen_static::style::Fields::of(&[
            lumen_static::style::Fields::WIDTH,
            lumen_static::style::Fields::HEIGHT,
            lumen_static::style::Fields::PADDING,
            lumen_static::style::Fields::CURSOR,
            lumen_static::style::Fields::VISIBLE,
            lumen_static::style::Fields::COLOR,
            lumen_static::style::Fields::BACKGROUND,
            lumen_static::style::Fields::BORDER,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
            lumen_static::style::Fields::PRESSED_DIM,
            lumen_static::style::Fields::HOVER_DIM,
            lumen_static::style::Fields::LINE_HEIGHT,
            lumen_static::style::Fields::OVERFLOW,
        ]))
        .with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];
}

impl<S: TextStorage> Node for Selectable<S> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // The label's dirty bit is read before its layout clears it: a
        // text set straight through `label` of the same length is still a
        // new text.
        let dirty = self.label.text.is_dirty();
        let size = self.label.layout(cx, limits, style, st, i);
        let s = &style[i];
        self.pad = (s.padding.left, s.padding.top);
        let room = crate::ops::text_room(limits, s);
        let tl = Constraints { max_w: room, ..limits };
        let shown = self.label.shown().0;
        let key = (MeasureKey::new(cx, tl, s), shown.len() as u32);
        if dirty || self.shaped != Some(key) {
            let (px, wt) = (s.font_size, s.font_weight);
            self.clusters.clear();
            let run = cx.text.shape(shown, px, wt, room, &mut self.clusters);
            // Each line's right edge: its last character's start plus that
            // character's width (a cluster carries only its start).
            self.line_ends.clear();
            for (k, c) in self.clusters.iter().enumerate() {
                let last_on_line = self.clusters.get(k + 1).is_none_or(|n| n.line != c.line);
                if !last_on_line {
                    continue;
                }
                let to = self.clusters.get(k + 1).map_or(shown.len(), |n| n.byte as usize);
                let ch = shown.get(c.byte as usize..to).unwrap_or("").trim_end_matches(['\n', '\r']);
                let w = if ch.is_empty() { 0.0 } else { cx.text.measure_weighted(ch, px, wt, f32::INFINITY).w };
                while self.line_ends.len() < c.line as usize {
                    self.line_ends.push(c.x);
                }
                self.line_ends.push(c.x + w);
            }
            let lines = self.clusters.last().map_or(0, |c| c.line as u32 + 1);
            self.rows = if lines == 0 {
                (0.0, 0.0)
            } else if s.line_height != 0 {
                // The placer's rows: `pitch` apart, all shifted down by
                // half the deficit below the face's spacing.
                let k = cx.scale();
                let k = if k.is_finite() && k > 0.0 { k } else { 1.0 };
                let nls = lumen_static::num::ceil(cx.text.line_height(px) * k) / k;
                let p = lumen_static::cx::pitch(s.line_height, px);
                (p, (nls - p).max(0.0) / 2.0)
            } else {
                // The face's own spacing: the run's height over its lines,
                // which is the ceil'd new-line size the painter advances by.
                (run.h / lines as f32, 0.0)
            };
            self.shaped = Some(key);
            self.clamp();
        }
        size
    }

    /// Forwarded to the label's probe, which touches no memo; the shaped
    /// clusters are left as the frame left them.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.label.probe(cx, limits, style, st, i)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        // Ground, then the tint, then the glyphs: a highlight painted after
        // the run would hide the text it marks.
        self.label.paint_ground(st, i, out);
        if let Some(sel) = self.selection() {
            let s = &st[i];
            let r = lumen_static::ops::rect_of(s);
            let (ox, oy) = (r[0] + s.pad.0, r[1] + s.pad.1);
            let (pitch, top) = self.rows;
            let tint = self.selection_color.unwrap_or_else(|| tint_of(s.fg));
            self.tint_rows(sel, |x0, x1, line| {
                let y = oy + top + line as f32 * pitch;
                out.rect([ox + x0, y, ox + x1, y + pitch], tint);
            });
        }
        self.label.paint_run(st, i, out);
    }

    /// Press, drag, double and triple press, Ctrl+A, and a lost focus.
    /// Returns `true` only when the selection or the caret moved (a press
    /// always claims the gesture, so the drag that follows is this node's).
    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        let at = |me: &Self| me.offset_at(cx.local.x - me.pad.0, cx.local.y - me.pad.1);
        match ev {
            Event::PointerDown { .. } if cx.clicks >= 3 => {
                self.select_all();
                true
            }
            Event::PointerDown { .. } if cx.clicks == 2 => {
                let w = crate::edit::word_at(self.label.shown().0, at(self));
                self.anchor = w.start;
                self.caret = w.end;
                true
            }
            Event::PointerDown { .. } => {
                let to = at(self);
                if !cx.mods.has(Mods::SHIFT) {
                    self.anchor = to;
                }
                self.caret = to;
                true
            }
            // Only while this node holds the capture its press took.
            Event::PointerMove { .. } => {
                let to = at(self);
                core::mem::replace(&mut self.caret, to) != to
            }
            Event::Key { key: Key::Char('a'), down: true } if cx.mods.has(Mods::CTRL) || cx.mods.has(Mods::META) => {
                self.select_all();
                true
            }
            Event::Key { key: Key::Escape, down: true } => self.clear_selection(),
            Event::Focus { gained: false } => self.clear_selection(),
            // `Backspace` and `Delete` fall through, unhandled: the text is
            // read-only, and a Cut's follow-up delete must change nothing.
            _ => false,
        }
    }

    /// Its own selection: the host asks the focused index, which is this
    /// node's (it shares the label's).
    fn selection(&self) -> Option<&str> {
        self.selected_str()
    }

    lumen_static::leaf_common!(Selectable<S>, NodeDesc::FOCUSABLE);

    /// A label to a reader, with the whole text as its value — the cut, if
    /// any, is paint's.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::Label);
        node.set_value(self.label.as_str());
    }
}

impl<S, S2: TextStorage> lumen_static::event::Handlers<S> for Selectable<S2> {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}
