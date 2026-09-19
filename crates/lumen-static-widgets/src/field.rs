//! M27/T27.5 — the fields: `NumberInput`, `RangeSlider`, `SearchField`.
//!
//! All three are composites over things the core already has, which is
//! what makes them the cheap tranche. What they add is the part nobody
//! wants to write twice: a number field that refuses letters and clamps
//! to a range, a slider with two handles that cannot cross, and a text
//! field that looks like a search box.
//!
//! # `NumberInput` is not Lumen's
//!
//! It is `docs/plan-widget-parity.md`'s addition, and it is a different
//! widget from the Stepper the ask drops precisely by having **no
//! `+`/`−` buttons**: a text field constrained to a number, with min,
//! max, step, keyboard increment and commit-on-blur.
//!
//! **One node, not two.** It owns a `TextInput` and forwards every
//! `Node` method to it *at the same index*, so a number field costs
//! exactly what a text field costs and a stylesheet rule reaches it
//! without a part id. `on_event` gets first refusal on the keys it cares
//! about and hands the rest over.
//!
//! **Commit on blur is read in `layout`, not from an event**, because
//! there is no blur event — focus lives in `NodeState`, and `layout` is
//! the one method holding both `&mut self` and the state array. That is
//! `Tooltip`'s hover trick and `Glass`'s press trick, for the third
//! time.

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

use crate::trim::{Icon, Symbol};

// ---- NumberInput -----------------------------------------------------

/// A one-line field constrained to a number.
pub struct NumberInput {
    field: lumen_static::input::TextInput<alloc::string::String>,
    /// The committed value. The text may differ while it is being typed.
    value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    /// Digits after the point when the value is written back.
    pub decimals: u8,
    /// Whether it had focus last frame, so `layout` can see a blur.
    was_focused: bool,
}

impl Default for NumberInput {
    fn default() -> Self {
        NumberInput::new(0.0, 0.0, 100.0, 1.0)
    }
}

impl NumberInput {
    pub fn new(value: f32, min: f32, max: f32, step: f32) -> Self {
        let mut n = NumberInput {
            field: lumen_static::input::TextInput::default(),
            value: 0.0,
            min,
            max,
            step,
            decimals: 2,
            was_focused: false,
        };
        n.set_value(value);
        n
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn as_str(&self) -> &str {
        self.field.as_str()
    }

    /// Select the whole value, so the next keystroke replaces it.
    ///
    /// Inventory row 10 (`docs/plan-widget-parity.md` §4). **Typing into
    /// a number field appends at the caret**, like any other field —
    /// which is right, and is also why "type 5 and get 5" needs this
    /// first. A test that assumed replacement found `0.05` where it
    /// wanted `5`, which is the widget being correct and the test being
    /// wrong about it.
    pub fn select_all(&mut self) -> bool {
        self.field.select_all()
    }

    /// Clamp, store, and rewrite the text to match.
    pub fn set_value(&mut self, v: f32) -> bool {
        let v = clamp_finite(v, self.min, self.max);
        let text = format_fixed(v, self.decimals);
        let changed = v != self.value || self.field.as_str() != text;
        self.value = v;
        if self.field.as_str() != text {
            self.field.set_str(&text);
        }
        changed
    }

    /// Parse what is typed and commit it, or put the last good value
    /// back.
    ///
    /// **Reverting, not clearing.** A field left holding `-` is
    /// mid-thought, not an error; blanking it loses the number the user
    /// had before they started editing, which is the one thing they
    /// cannot retype from the screen.
    pub fn commit(&mut self) -> bool {
        match parse_decimal(self.field.as_str()) {
            Some(v) => self.set_value(v),
            None => self.set_value(self.value),
        }
    }

    fn nudge(&mut self, by: f32) -> bool {
        self.set_value(self.value + by)
    }
}

/// `clamp` that keeps a NaN out of the stored value.
///
/// `f32::clamp` panics when the bounds are unordered and returns NaN for
/// a NaN input; a field whose value is NaN formats as "NaN" and never
/// parses back, so it is a state with no way out. The last good value is
/// the honest answer.
fn clamp_finite(v: f32, lo: f32, hi: f32) -> f32 {
    if !v.is_finite() {
        return lo;
    }
    lumen_static::num::clamp(v, lo, hi.max(lo))
}

/// `{:.n}` without `std`'s formatter — `alloc` has one, but the rounding
/// has to be explicit anyway to keep the text and the value in step.
fn format_fixed(v: f32, decimals: u8) -> alloc::string::String {
    use alloc::format;
    match decimals {
        0 => format!("{:.0}", v),
        1 => format!("{:.1}", v),
        2 => format!("{:.2}", v),
        _ => format!("{:.3}", v),
    }
}

/// Parse a decimal, refusing everything `is_number_char` would have let
/// through that is not actually a number.
fn parse_decimal(s: &str) -> Option<f32> {
    let t = s.trim();
    if t.is_empty() || t == "-" || t == "." || t == "-." {
        return None;
    }
    t.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// The characters a number field accepts at all.
///
/// Checked here rather than left to `parse` on commit, because a field
/// that accepts `abc` and silently reverts on blur has taught the user
/// nothing and lost their keystrokes twice.
fn is_number_char(ch: char) -> bool {
    ch.is_ascii_digit() || ch == '.' || ch == '-'
}

impl Widget for NumberInput {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
    const KIND: &'static str = "numberinput";
    const SEEDS: &'static [Seed] =
        &[Seed::new(<NumberInput as Widget>::KIND, <NumberInput as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::COLOR,
                Fields::RADIUS,
                Fields::FONT_SIZE,
                Fields::FONT_WEIGHT,
                Fields::FOCUS_RING,
                Fields::CURSOR,
                Fields::VISIBLE,
            ]))];

    /// A field's look, plus the border M27/T27.1 made possible. Narrower
    /// than a text field, because a number is short and a wide box
    /// invites a sentence.
    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(120.0),
            height: Length::Fixed(36.0),
            padding: Edges { left: 10.0, right: 10.0, top: 8.0, bottom: 8.0 },
            background: Color(0xf3, 0xf5, 0xf9, 0xff),
            border_width: 1,
            border_color: Color(0xc6, 0xcd, 0xd8, 0xff),
            radius: 8.0,
            cursor: lumen_static::style::Cursor::Text,
            ..Style::default()
        }
    }
}

impl Node for NumberInput {
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
        // **Blur commits.** There is no blur event; focus is state, and
        // this is the one method that can see it change.
        let focused = st[i].focused;
        if self.was_focused && !focused {
            self.commit();
        }
        self.was_focused = focused;
        self.field.layout(cx, limits, style, st, i)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        self.field.place(origin, style, st, i);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.field.paint(st, i, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        self.field.hit(st, i, x, y)
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // A digit, a point or a sign goes to the field; anything
            // else is refused *here*, so the keystroke is never lost and
            // then silently undone on blur.
            Event::Text { ch } => {
                if !is_number_char(*ch) {
                    // Consumed: the field must not see it, and the
                    // parent must not treat it as unhandled text.
                    return true;
                }
                self.field.on_event(ev, cx)
            }
            Event::Key { key: Key::Up, down: true } => self.nudge(self.step),
            Event::Key { key: Key::Down, down: true } => self.nudge(-self.step),
            Event::Key { key: Key::PageUp, down: true } => self.nudge(self.step * 10.0),
            Event::Key { key: Key::PageDown, down: true } => self.nudge(self.step * -10.0),
            Event::Key { key: Key::Enter, down: true } => self.commit(),
            _ => self.field.on_event(ev, cx),
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
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::SpinButton);
        node.set_value(self.field.as_str());
        node.set_numeric_value(self.value as f64);
        node.set_min_numeric_value(self.min as f64);
        node.set_max_numeric_value(self.max as f64);
        node.set_numeric_value_step(self.step as f64);
    }

}

impl<S> lumen_static::event::Handlers<S> for NumberInput {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

// ---- RangeSlider -----------------------------------------------------

/// Two handles on one track: a low bound and a high one.
///
/// **They cannot cross**, and that is the whole widget. A pair of
/// `Slider`s side by side is not a range: nothing stops the low one
/// passing the high one, and the moment it does the filled span between
/// them is inverted and the control lies about what it selects.
///
/// One node, like `Slider`, and for the same reason: two handles on one
/// track is one thing to click, and splitting it would make the track a
/// container whose children overlap — which is what `W1004` reports.
pub struct RangeSlider {
    low: f32,
    high: f32,
    /// How far an arrow key moves the focused handle. A range only
    /// draggable by mouse is not an accessible range — `Slider::step`'s
    /// argument, unchanged.
    pub step: f32,
    /// Which handle the keyboard and the current drag address.
    high_active: bool,
    /// Track width from the last layout; `on_event` sees only `local`.
    /// `Slider::track_w` exists for exactly this and says why.
    track_w: f32,
}

impl Default for RangeSlider {
    fn default() -> Self {
        RangeSlider { low: 0.0, high: 1.0, step: 0.05, high_active: false, track_w: 0.0 }
    }
}

impl RangeSlider {
    pub fn new(low: f32, high: f32) -> Self {
        let mut r = RangeSlider::default();
        r.set_low(low);
        r.set_high(high);
        r
    }

    pub fn low(&self) -> f32 {
        self.low
    }

    pub fn high(&self) -> f32 {
        self.high
    }

    /// Set the low bound. Clamped to `0.0..=high`, so it can meet the
    /// high handle but never pass it.
    pub fn set_low(&mut self, v: f32) -> bool {
        let v = clamp_finite(v, 0.0, self.high);
        let changed = v != self.low;
        self.low = v;
        changed
    }

    /// Set the high bound. Clamped to `low..=1.0`.
    pub fn set_high(&mut self, v: f32) -> bool {
        let v = clamp_finite(v, self.low, 1.0);
        let changed = v != self.high;
        self.high = v;
        changed
    }

    /// Which handle a press at `x` (a fraction of the track) should take.
    ///
    /// The nearer one, and the **high** one when they are equal — so a
    /// range collapsed to a point can still be opened. Taking the low
    /// one there would pin it: `set_low` clamps to `high`, so dragging
    /// right would move nothing.
    fn nearest(&self, at: f32) -> bool {
        let (dl, dh) = ((at - self.low).abs(), (at - self.high).abs());
        dh <= dl
    }

    fn drag_to(&mut self, at: f32) -> bool {
        if self.high_active {
            self.set_high(at)
        } else {
            self.set_low(at)
        }
    }

    fn nudge(&mut self, by: f32) -> bool {
        if self.high_active {
            self.set_high(self.high + by)
        } else {
            self.set_low(self.low + by)
        }
    }
}

impl Widget for RangeSlider {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
    const KIND: &'static str = "rangeslider";
    const SEEDS: &'static [Seed] =
        &[Seed::new(<RangeSlider as Widget>::KIND, <RangeSlider as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::COLOR,
                Fields::FOCUS_RING,
                Fields::CURSOR,
                Fields::VISIBLE,
            ]))];

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(24.0),
            background: Color(0xd6, 0xdc, 0xe6, 0xff),
            color: Color(0x2f, 0x6b, 0xff, 0xff),
            ..Style::default()
        }
    }
}

impl Node for RangeSlider {
    // `kind_name` and `defaults` come from `leaf_common!` below.
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
        let s = lumen_static::ops::resolve_self(limits, &me, Size { w: limits.max_w, h: 24.0 });
        self.track_w = s.w;
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        lumen_static::ops::focus_ring(st, i, out);
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        // `Slider`'s geometry, with the fill between the handles rather
        // than from the left edge — which is the only visual difference
        // and the one that says "range" rather than "amount".
        let t = (h * 0.25).max(2.0);
        let mid = r[1] + h / 2.0;
        out.rounded_rect([r[0], mid - t / 2.0, r[2], mid + t / 2.0], s.bg, t / 2.0);
        let (xl, xh) = (r[0] + w * self.low, r[0] + w * self.high);
        if xh > xl {
            out.rounded_rect([xl, mid - t / 2.0, xh, mid + t / 2.0], s.fg, t / 2.0);
        }
        let d = h * 0.75;
        for x in [xl, xh] {
            let kx = lumen_static::num::clamp(x - d / 2.0, r[0], r[2] - d);
            out.rounded_rect([kx, mid - d / 2.0, kx + d, mid + d / 2.0], s.fg, d / 2.0);
        }
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        let at = |x: f32, w: f32| if w > 0.0 { lumen_static::num::clamp(x / w, 0.0, 1.0) } else { 0.0 };
        match ev {
            Event::PointerDown { .. } => {
                let f = at(cx.local.x, self.track_w);
                self.high_active = self.nearest(f);
                self.drag_to(f);
                true
            }
            // Only while a press is held: `dispatch` gives the captured
            // node every move, and a range that followed an unpressed
            // pointer would be unusable.
            Event::PointerMove { .. } => self.drag_to(at(cx.local.x, self.track_w)),
            Event::Key { key: Key::Right, down: true } => self.nudge(self.step),
            Event::Key { key: Key::Left, down: true } => self.nudge(-self.step),
            Event::Key { key: Key::Home, down: true } => {
                self.high_active = false;
                self.set_low(0.0)
            }
            Event::Key { key: Key::End, down: true } => {
                self.high_active = true;
                self.set_high(1.0)
            }
            // Tab between the two handles, so a keyboard can reach both.
            Event::Key { key: Key::Tab, down: true } => {
                self.high_active = !self.high_active;
                true
            }
            _ => false,
        }
    }

    lumen_static::leaf_common!(RangeSlider, 0);

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Slider);
        // Both bounds as a readable value, so a drive need not compare
        // screenshots to see a handle move.
        node.set_value(alloc::format!("{:.2}..{:.2}", self.low, self.high));
        node.set_numeric_value(self.low as f64);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
    }

}

impl<S> lumen_static::event::Handlers<S> for RangeSlider {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

// ---- SearchField -----------------------------------------------------

/// The icon's index within a `SearchField`, and the field's.
const SF_ICON: usize = 1;
const SF_FIELD: usize = 2;

/// Part ids — a composite has to publish its insides or nothing can
/// address them (`docs/plan-select.md`). Same caveat as
/// [`crate::surface::PART_CONTENT`]: two in one tree collide, and
/// `W1011` says so.
pub const PART_SEARCH_ICON: &str = "search-icon";
/// The editable field inside a `SearchField`.
pub const PART_SEARCH_FIELD: &str = "search-field";

/// A text field that looks like one: a magnifier, then the text.
pub struct SearchField {
    pub icon: Icon,
    pub field: lumen_static::input::TextInput<alloc::string::String>,
}

impl Default for SearchField {
    fn default() -> Self {
        SearchField {
            icon: Icon::new(Symbol::Search),
            field: lumen_static::input::TextInput::default(),
        }
    }
}

impl SearchField {
    pub fn new(text: &str) -> Self {
        let mut s = SearchField::default();
        s.field.set_str(text);
        s
    }

    pub fn as_str(&self) -> &str {
        self.field.as_str()
    }

    /// Empty the query. Returns whether anything changed, so a caller
    /// can skip a re-search.
    pub fn clear(&mut self) -> bool {
        if self.field.as_str().is_empty() {
            return false;
        }
        self.field.set_str("");
        true
    }

    /// Gap between the magnifier and the text.
    const GAP: f32 = 8.0;
}

pub struct SearchFieldTable;

impl SearchFieldTable {
    const N: usize = 3;
    const ARRAY: [NodeDesc; Self::N] = {
        let mut t = [NodeDesc::EMPTY; Self::N];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        t[SF_ICON] = NodeDesc::new(0, 0, SF_FIELD as u32, 0, 0).with_id(PART_SEARCH_ICON);
        t[SF_FIELD] =
            NodeDesc::new(0, 0, 0, 0, NodeDesc::FOCUSABLE).with_id(PART_SEARCH_FIELD);
        t
    };
    pub const SLICE: &'static [NodeDesc] = &Self::ARRAY;

    const SEED_ARRAY: [Seed; Self::N] = {
        let mut s = [Seed::WIDGET; Self::N];
        s[0] = Seed::new(<SearchField as Widget>::KIND, <SearchField as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::RADIUS,
                Fields::CURSOR,
                Fields::VISIBLE,
            ]));
        s[SF_ICON] = <Icon as Widget>::SEEDS[0];
        s[SF_FIELD] = <lumen_static::input::TextInput<alloc::string::String> as Widget>::SEEDS[0];
        s
    };
    pub const SEED_SLICE: &'static [Seed] = &Self::SEED_ARRAY;
}

impl Widget for SearchField {
    const NODE_COUNT: usize = SearchFieldTable::N;
    const DEPTH: usize = 2;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = SearchFieldTable::SLICE;
    const KIND: &'static str = "searchfield";
    const SEEDS: &'static [Seed] = SearchFieldTable::SEED_SLICE;

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(38.0),
            padding: Edges { left: 12.0, right: 12.0, top: 8.0, bottom: 8.0 },
            background: Color(0xf3, 0xf5, 0xf9, 0xff),
            border_width: 1,
            border_color: Color(0xc6, 0xcd, 0xd8, 0xff),
            radius: 19.0,
            cursor: lumen_static::style::Cursor::Text,
            ..Style::default()
        }
    }
}

impl Node for SearchField {
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
        let inner = lumen_static::ops::inner_limits(limits, me.padding);
        // **Loose limits for the icon.** A `SearchField` is `FILL`, so
        // its own `min_w` is the width its parent insisted on — and
        // `resolve_self` clamps a `Fixed` *up* to `min_w`. Handing the
        // icon `inner` unchanged made a 20 px magnifier resolve to 296,
        // centred in the pill, with the text pushed off the right edge:
        // `ui.getAppliedStyles` reported `Fixed(20.0)` while the box was
        // 296 wide, which is the tell.
        //
        // A wrapper that wants a child to shrink has to say so; the
        // maximum is inherited, the minimum is not.
        let loose = Constraints { min_w: 0.0, max_w: inner.max_w, min_h: 0.0, max_h: inner.max_h };
        let ic = self.icon.layout(cx, loose, style, st, i + SF_ICON);
        // The field takes what is left after the icon and the gap.
        let rest = Constraints {
            min_w: 0.0,
            max_w: (inner.max_w - ic.w - Self::GAP).max(0.0),
            min_h: 0.0,
            max_h: inner.max_h,
        };
        let f = self.field.layout(cx, rest, style, st, i + SF_FIELD);
        let content = Size {
            w: limits.max_w,
            h: ic.h.max(f.h) + me.padding.vertical(),
        };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        let p = style[i].padding;
        let h = st[i].size.h;
        // Both centred on the field's own middle, so the magnifier sits
        // on the text's line rather than on the box's top edge.
        let iw = st[i + SF_ICON].size.w;
        let ih = st[i + SF_ICON].size.h;
        let fh = st[i + SF_FIELD].size.h;
        self.icon.place(
            Point { x: origin.x + p.left, y: origin.y + (h - ih) / 2.0 },
            style,
            st,
            i + SF_ICON,
        );
        self.field.place(
            Point { x: origin.x + p.left + iw + Self::GAP, y: origin.y + (h - fh) / 2.0 },
            style,
            st,
            i + SF_FIELD,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if st[i].size.w <= 0.0 || st[i].size.h <= 0.0 {
            return;
        }
        let r = lumen_static::ops::rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        lumen_static::ops::border(st, i, out);
        self.icon.paint(st, i + SF_ICON, out);
        self.field.paint(st, i + SF_FIELD, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        // **The field, wherever inside the box the click landed.** The
        // icon is decoration and `Icon` declines a hit anyway; falling
        // through to the *container* would focus nothing, so clicking
        // the padding would do nothing and the control would feel dead
        // at its edges.
        self.field.hit(st, i + SF_FIELD, x, y).or(Some(lumen_static::Hit {
            node: (i + SF_FIELD) as u32,
            inner: None,
            local: Point {
                x: x - st[i + SF_FIELD].pos.x,
                y: y - st[i + SF_FIELD].pos.y,
            },
        }))
    }

    fn node(&self, k: usize) -> &dyn Node {
        match k {
            SF_ICON => &self.icon,
            SF_FIELD => &self.field,
            _ => self,
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        match k {
            SF_ICON => &mut self.icon,
            SF_FIELD => &mut self.field,
            _ => self,
        }
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::SearchInput);
        node.set_value(self.field.as_str());
    }
}

impl<S> lumen_static::event::Handlers<S> for SearchField {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}
