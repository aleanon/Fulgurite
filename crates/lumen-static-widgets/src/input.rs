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
    /// What a screen reader announces this control as — M54/T54.3
    /// (R15-F3). Empty by default, and an empty one is not published: a
    /// control with no visible text of its own needs its application to
    /// say what it is for. A field rather than a relation to a sibling
    /// `Label`, so it works inside a `Repeat` row.
    ///
    /// **Only with `a11y`**: a build nobody can read aloud — the firmware
    /// — does not carry it (its `Ui` is a const-asserted size). Write it
    /// through `set_label`, which every build has.
    #[cfg(feature = "a11y")]
    pub label: &'static str,
    /// Shows its text and takes focus, but **edits nothing** — M57/B20.
    ///
    /// For a control that displays a value it owns elsewhere: a pick list's
    /// trigger was an editable field, so typing into one appended to the
    /// shown option ("Applexy") while the selection did not move
    /// (`docs/widget-audit.md`, PickList). Read-only, it declines text,
    /// deletion, a paste and an input method's composition (the last two
    /// since M80/T80.5), draws no caret and reports none
    /// (`Node::caret_rect`), and publishes AccessKit's read-only state
    /// (the agent's `readOnly`).
    pub read_only: bool,
    /// Shown, dimmed, while the field is empty — M57/B15. The text colour
    /// at [`PLACEHOLDER_ALPHA`], so it follows the palette and stays above
    /// 4.5:1 on the field's ground in both (`tests/text_input_b15.rs`
    /// prints the ratios).
    pub placeholder: &'static str,
    /// Set by Enter, taken by the application — M57/B15. The field has no
    /// application-state parameter to carry a handler, so a submission is
    /// a flag, the shape `PullToRefresh::take_triggered` and
    /// `Menu::take_chosen` already have.
    submitted: bool,
    /// Presses taken, wrapping. A counter, not a flag, so an owner that
    /// reads it once a frame sees a press whose release landed in the same
    /// frame — which a `pressed` bit does not survive (`Chooser` reopens on
    /// a press of its field).
    presses: u32,
    /// The placeholder's width, measured with the run while the text is
    /// empty: an empty field's content is otherwise zero wide, and a
    /// shrink field then had no room to show it in.
    placeholder_w: f32,
}

/// A placeholder's alpha over the text colour, out of 255: 68%. 62% read
/// 4.55:1 on white — over the bound by a hair a slightly lighter text
/// colour would take away.
pub const PLACEHOLDER_ALPHA: u16 = 0xad;

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
            #[cfg(feature = "a11y")]
            label: "",
            read_only: false,
            placeholder: "",
            submitted: false,
            presses: 0,
            placeholder_w: 0.0,
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
        // **Read-only edits nothing, composed text included** (M57/B20's
        // rule, which this did not follow): a pick list's trigger took an
        // input method's preedit into its shown option, and answered no
        // caret for it. Found by `tests/caret_rect.rs` (M80/T80.5).
        if self.read_only {
            return false;
        }
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
        // Read-only keeps nothing, for `compose`'s reason (M80/T80.5).
        if self.read_only {
            return 0;
        }
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

    /// How many presses this field has taken, wrapping: compare with the
    /// last count seen to learn that a press happened since.
    pub fn presses(&self) -> u32 {
        self.presses
    }

    /// Whether Enter was pressed since the last call — and clears it.
    pub fn take_submitted(&mut self) -> bool {
        core::mem::take(&mut self.submitted)
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

    /// The text line's box inside the field, in logical px: across, the
    /// padded box; down, one line.
    ///
    /// **The line, centred; the padding is horizontal room, not a
    /// vertical clip.** The text used to start at the top padding and be
    /// clipped to the padded box, so a field squeezed shorter than
    /// padding + line cut its own text: a `SearchField`'s pill gives its
    /// field 22 px, the field kept 6 + 6 of it, and a 20 px line showed a
    /// 10 px slice of its tops with the field's own fill below. The line
    /// sits in the middle of the padded box when it fits there, and of
    /// the whole box when it does not. `paint` draws the run and the
    /// caret in it, and `caret_rect` reports the caret from it
    /// (M80/T80.5).
    #[inline]
    fn line_box(&self, s: &NodeState) -> [f32; 4] {
        let r = rect_of(s);
        let (pl, pt, pr, pb) = s.pad;
        let line = if self.shaped_size.1 > 0.0 { self.shaped_size.1 } else { (r[3] - r[1] - pt - pb).max(0.0) };
        let (bh, ih) = (r[3] - r[1], r[3] - r[1] - pt - pb);
        let top = if ih >= line { r[1] + pt + (ih - line) / 2.0 } else { r[1] + (bh - line) / 2.0 };
        [r[0] + pl, top, r[2] - pr, top + line]
    }

    /// X of the caret within the run, from the shaped clusters.
    ///
    /// Falls back to the run's end when the measurer reported no
    /// clusters — `BlockMetrics` does, deliberately (T7.2b) — so a field
    /// under the stand-in still shows a caret in a sensible place
    /// instead of at zero.
    fn caret_x(&self) -> f32 {
        self.x_of(self.caret)
    }

    /// The x of a byte offset within the run — `caret_x` generalised,
    /// so the selection rect and the caret cannot disagree about where a
    /// boundary is.
    ///
    /// **The byte is looked up exactly** — M81/T81.5. It was the first
    /// cluster with `byte >= at`, which is the same thing while every
    /// offset has one place; at a direction change an offset has two (the
    /// ending run's trailing edge, then what follows), and the caret is
    /// the second, as [`crate::caret::at`] says for all three text
    /// widgets. So the end of the text is the trailing insertion point,
    /// at the paragraph direction's end: the run's right end for a
    /// left-to-right paragraph (M57/B15: the last cluster's *start* put
    /// the caret before the last glyph), its left end for a right-to-left
    /// one. With no clusters at all (the `BlockMetrics` stand-in) the end
    /// is the run's width (`shaped_size`), not the box's.
    fn x_of(&self, at: usize) -> f32 {
        let run_w = self.shaped_size.0;
        let cl = self.clusters.as_slice();
        if cl.is_empty() {
            return if at == 0 { 0.0 } else { run_w };
        }
        crate::caret::at(cl, at).map_or(run_w, |c| c.x)
    }

    /// Byte offset nearest an x within the run — a click's landing spot:
    /// **the nearest leading edge** (M81/T81.5), a direction change's
    /// trailing edge and the text's end included, each a cluster.
    ///
    /// **The text's end wins a tie** (M81/T81.6). In a right-to-left
    /// paragraph whose last run is left to right (`שלום world 123`) the
    /// end sits at x 0, where the first Latin letter's leading edge is
    /// too: one place, two offsets. A click there — or in the padding
    /// beyond it — is a click past the line's end, as it is past a
    /// left-to-right line's right end, so it lands at the end.
    fn offset_at(&self, x: f32) -> usize {
        let cl = self.clusters.as_slice();
        let len = self.text.as_ref().len();
        // The end of the text is a place a click can land too: only
        // cluster *starts* were candidates, so a click past the last
        // character put the caret before it (M57/B15). A measurer reports
        // that end as a cluster, at the paragraph direction's end; the
        // run's right end stands in only where none does — as the
        // candidate it was, a click at a right-to-left run's right end
        // tied its first letter and won (M81/T81.5).
        let mut best = match cl.last() {
            Some(c) if c.byte as usize == len => (f32::INFINITY, len),
            _ => ((self.shaped_size.0 - x).abs(), len),
        };
        for c in cl {
            let d = (c.x - x).abs();
            if d < best.0 || (d == best.0 && c.byte as usize == len) {
                best = (d, c.byte as usize);
            }
        }
        best.1
    }

    /// The x spans `range` covers in the run, through
    /// [`crate::caret::spans`]: each selected cluster from its leading to
    /// its trailing edge, contiguous ones joined. With no clusters (the
    /// stand-in), the one span between the range's ends.
    fn spans(&self, range: core::ops::Range<usize>, mut f: impl FnMut(f32, f32)) {
        let cl = self.clusters.as_slice();
        if cl.is_empty() {
            return f(self.x_of(range.start), self.x_of(range.end));
        }
        crate::caret::spans(cl, range, &[], &mut |a, b, _| f(a, b));
    }
}

/// The limits a single-line field's shaped key is built with: none. Its
/// run is shaped unbounded (B7), so no width it is offered changes it.
const SINGLE_LINE: Constraints =
    Constraints { min_w: 0.0, max_w: f32::INFINITY, min_h: 0.0, max_h: f32::INFINITY };

/// The caret's width in logical px: `paint` draws it and lets its clip
/// run this far into the right padding, `caret_rect` reports it, and
/// `place` reserves what the padding cannot hold (M80/T80.5).
pub(crate) const CARET_W: f32 = 1.0;

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
            lumen_static::style::Fields::BACKGROUND_FILL,
            lumen_static::style::Fields::RADIUS,
            lumen_static::style::Fields::FONT_SIZE,
            lumen_static::style::Fields::FONT_WEIGHT,
            lumen_static::style::Fields::PRESSED_DIM,
            lumen_static::style::Fields::HOVER_DIM,
            lumen_static::style::Fields::TRANSITION,
        ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Text))];
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
            self.placeholder_w = if self.text.as_ref().is_empty() && !self.placeholder.is_empty() {
                cx.text.measure_weighted(self.placeholder, px, me.font_weight, f32::INFINITY).w
            } else {
                0.0
            };
            self.shaped_key = Some(key);
        }
        let (run_w, line) = self.shaped_size;

        let content = Size { w: run_w.max(self.placeholder_w), h: line };
        let padded =
            Size { w: content.w + me.padding.horizontal(), h: content.h + me.padding.vertical() };
        let size = resolve_self(limits, &me, padded);

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

    /// **The scroll is decided here, against the final box — never in
    /// `layout`.** B5 brought the caret into view in `layout`, which runs
    /// for every *measure* as well as for the frame: a parent that sizes
    /// its children loosely before placing them, and the agent's audit
    /// under a million-pixel box, each laid the field out wider than it
    /// is, so the run "fitted", the scroll clamped to 0, and the frame's
    /// own layout could only put the caret at the right edge again. The
    /// view jumped on every click of a full field (reported from the live
    /// window) — the `Repeat` defect of M27/T27.10 in a text field, with
    /// the same answer: layout derives, `place` decides.
    ///
    /// **The caret's own width is shown** — M80/T80.5. The view kept the
    /// caret's *left edge* in the padded box and the clip ended there, so
    /// a caret at the end of a full field was drawn one pixel wide wholly
    /// outside the clip: a full field showed no caret at all. Found by
    /// `shell/tests/ime_area.rs`, whose reference is the painted caret
    /// column. `paint` now lets the clip run `CARET_W` into the right
    /// padding, and only what that padding cannot hold (`spill`, zero for
    /// any padding of a pixel or more) is reserved here, so a field sized
    /// to its text does not scroll when its caret reaches the end.
    fn place(&mut self, at: Point, _style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        let (pl, _, pr, _) = st[i].pad;
        let inner_w = (st[i].size.w - pl - pr).max(0.0);
        let spill = (CARET_W - pr).max(0.0);
        let room = (inner_w - spill).max(0.0);
        let cx_pos = self.caret_x();
        if cx_pos - self.scroll > room {
            self.scroll = cx_pos - room;
        }
        if cx_pos < self.scroll {
            self.scroll = cx_pos;
        }
        // The run's end is the further of its measured width and the
        // caret's x: a shaper's last cluster boundary is the unrounded pen
        // (580.14 against a 580 width for the gate's long run), and a
        // clamp at the width alone cut that fraction of the caret off.
        let end = self.shaped_size.0.max(cx_pos);
        self.scroll = lumen_static::num::clamp(self.scroll, 0.0, (end + spill - inner_w).max(0.0));
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = rect_of(s);
        lumen_static::ops::fill(st, i, out);
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);

        let inner = self.line_box(s);
        // The text may be wider than the field; the clip is what keeps
        // it inside, exactly as a scroller's content is kept inside
        // (A1's scoped containment, one widget down). Across, the
        // padding, plus the caret's width into the right padding so a
        // caret at the end of a full field shows (M80/T80.5, `place`);
        // down, the box — a glyph is never cut by padding.
        out.push_clip([inner[0], r[1], (inner[2] + CARET_W).min(r[2]), r[3]]);
        let x = inner[0] - self.scroll;
        // B4 decided the selection is an ordinary rect, and it is
        // painted *before* the run so the text sits on top of it —
        // a selection drawn after would hide the glyphs it highlights.
        // One rect per visually contiguous span (M81/T81.5,
        // `caret::spans`): a selection across a direction change is two.
        if let Some(sel) = self.selection() {
            let tint = self.selection_color.unwrap_or_else(|| selection_tint(s.fg));
            self.spans(sel, |a, b| out.rect([x + a, inner[1], x + b, inner[3]], tint));
        }
        // **The run's rect holds the whole run, never just the visible part
        // of it.** A text rect's width is the width the glyphs are laid
        // out in, and a run wider than it *wraps*. The field passed the
        // visible width, from the scrolled start to the right padding, which
        // with the caret at the end *is* the run's width, to the float: a
        // 235 px word in a 234.84 px rect went to a second line, under the
        // clip, and a full field went blank on every other keystroke
        // (reported from the live window). M45/T45.1 had widened it to
        // `max(run, visible)`, which is the same number in that case. A field
        // never wraps (B7), so the
        // rect is the shaped run rounded up with a pixel to spare, and the
        // clip alone decides what shows.
        let whole = |w: f32| x + lumen_static::num::ceil(w) + 1.0;
        if self.text.as_ref().is_empty() && !self.placeholder.is_empty() {
            let dim = Color(s.fg.0, s.fg.1, s.fg.2, (s.fg.3 as u16 * PLACEHOLDER_ALPHA / 0xff) as u8);
            out.text_weighted([x, inner[1], whole(self.placeholder_w).max(inner[2]), inner[3]], self.placeholder, s.font_size, s.font_weight, dim);
        } else {
            out.text_weighted(
                [x, inner[1], whole(self.shaped_size.0).max(inner[2]), inner[3]],
                self.text.as_ref(),
                s.font_size,
                s.font_weight,
                s.fg,
            );
        }
        // B4: the caret is an ordinary rect. Only in the focused field —
        // a caret in every field at once is the tell that focus is not
        // actually being tracked.
        // The composing run is underlined, which is how every platform
        // shows "this is not committed yet" (T14.8).
        if let Some(pre) = self.preedit.clone() {
            self.spans(pre, |a, b| out.rect([x + a, inner[3] - 2.0, x + b, inner[3] - 1.0], s.fg));
        }
        if s.focused && !self.read_only {
            let cx_pos = inner[0] - self.scroll + self.caret_x();
            out.rect([cx_pos, inner[1], cx_pos + CARET_W, inner[3]], self.caret_color.unwrap_or(s.fg));
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
            Event::Text { .. } | Event::Key { key: Key::Backspace | Key::Delete, down: true } if self.read_only => false,
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
            // Enter submits (M57/B15), and **is not consumed**: a
            // composite whose part is a field — a combobox choosing its
            // highlighted row — acts on the Enter its field declines.
            // (Word movement is the plain arrows' `edit::WORD` arm below,
            // M45/T45.6.)
            Event::Key { key: Key::Enter, down: true } => {
                self.submitted = true;
                false
            }
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
                self.presses = self.presses.wrapping_add(1);
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
                self.presses = self.presses.wrapping_add(1);
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

    /// The caret `paint` draws, **less the scroll**: a full field shows
    /// the end of its run, and the candidate window goes where the caret
    /// is on screen (M80/T80.5). `None` when read-only, which paints no
    /// caret.
    fn caret_rect(&self, st: &[NodeState], i: usize) -> Option<lumen_static::display::Rect> {
        if self.read_only {
            return None;
        }
        let inner = self.line_box(st.get(i)?);
        let x = inner[0] - self.scroll + self.caret_x();
        Some([x, inner[1], x + CARET_W, inner[3]])
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
        if !self.label.is_empty() {
            node.set_label(self.label);
        }
        node.set_value(self.text.as_ref());
        // Published, so a reader (and the drive's IME check) knows this
        // field opens no candidate window: `caret_rect` is `None` here.
        if self.read_only {
            node.set_read_only();
        }
    }
}

impl<S: TextBuffer, A> lumen_static::event::Handlers<A> for TextInput<S> {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}

impl<S: TextBuffer> TextInput<S> {
    /// Name this control for a screen reader (M54/T54.3). A no-op in a build without `a11y`, which
    /// carries no name.
    #[allow(unused_variables)]
    pub fn set_label(&mut self, label: &'static str) {
        #[cfg(feature = "a11y")]
        {
            self.label = label;
        }
    }
}
