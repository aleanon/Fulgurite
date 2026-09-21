//! The built-in widget vocabulary, and the layout helpers the derive
//! calls.
//!
//! Two audiences in one module, which is a wart the name records: `ops`
//! is called that because it once held the per-kind op tables M0 deleted.
//! [`Label`], [`Fixed`], [`Spacer`] and [`Button`] are the vocabulary and
//! are re-exported from [`crate::prelude`]; the rest —
//! `inner_limits`, `loose_child`, `fill_child`, `align_offset`,
//! `spaced`, `hide_subtree` — exist so derive-generated code has a
//! public path to call and are `#[doc(hidden)]`. `resolve_self`,
//! `record` and `rect_of` are visible because tests and widgets outside
//! this crate genuinely use them.
//!
//! Splitting the two into a widgets module and a hidden private one is
//! the honest end state (`docs/plan-review-fixes.md` F6.1); hiding the
//! machinery is most of the benefit for a fraction of the churn.

// `alloc`, not the std prelude: this crate is `no_std` when the
// `std` feature is off (M13/T13.1).
// Only the accesskit `extra` arg needs it, and that is observe-gated.
#[cfg(feature = "observe")]
use alloc::vec::Vec;

use crate::cx::LayoutCx;
use crate::desc::{Constraints, NodeDesc, NodeState, Size, NO_PARENT};
use crate::display::{Color, Painter};
use crate::event::{EventCtx, Handlers, NodeSel};
use crate::style::{Align, Length, Seed, Style};
use crate::text::{Dirty, MeasureKey, Measured, TextStorage};
use crate::widget::{Node, Widget};

/// A node's absolute box, for emitting commands.
#[inline]
/// Zero a whole subtree's geometry and return the zero size.
///
/// What `Style::visible = false` means, for every widget that honours
/// it. Zeroing only the node itself is not enough: descendants keep the
/// previous frame's `size` and `pos`, and `paint` walks into them
/// unconditionally, so a hidden container drew its children at their old
/// boxes. `hatch::hide` did this correctly for `Option`; the derive did
/// not, and the two mechanisms disagreed
/// (`docs/plan-review-fixes.md` F1.6).
///
/// A zero-area node paints nothing a renderer can see and fails every
/// `contains` test, which is what makes this sufficient without threading
/// `&[Style]` into `paint` and `hit`.
#[doc(hidden)]
pub fn hide_subtree(st: &mut [NodeState], i: usize, limits: Constraints, count: usize) -> Size {
    for s in st.iter_mut().skip(i).take(count) {
        s.size = Size::default();
        s.constraints = limits;
    }
    Size::default()
}

/// Draw the focus ring behind `i`, if it has focus and a ring width.
///
/// One helper rather than five copies: `Button`, `TextInput`,
/// `TextArea`, `Toggle` and `Slider` all need the same thing, and a
/// ring that differs between widgets is a ring a user cannot learn.
///
/// **A larger rounded rect *underneath*, not an outline on top.** The
/// first version drew four `Cmd::Line`s around the box, and the live
/// gate caught it within a frame: a square ring over a rounded button
/// covers the antialiased corner, and `ui_check.py`'s corner test
/// dropped to five blended pixels. Drawing behind reuses the rounded
/// rect the renderer already antialiases, follows the control's own
/// radius, and cannot cover the content it is marking.
pub fn focus_ring(st: &[crate::desc::NodeState], i: usize, out: &mut dyn crate::Painter) {
    let s = &st[i];
    if !s.focused || s.focus_ring == 0 {
        return;
    }
    let w = s.focus_ring as f32;
    let r = rect_of(s);
    out.rounded_rect(
        [r[0] - w, r[1] - w, r[2] + w, r[3] + w],
        s.fg,
        // The control's own radius plus the ring's width, so the ring
        // stays concentric rather than squaring off at the corners.
        s.radius + w,
    );
}

/// Draw `i`'s border, if it has one.
///
/// One helper rather than a copy in every widget that fills, for the
/// reason `focus_ring` gives two functions up: a border that differs
/// between widgets is a border a user cannot learn. Every call site is
/// immediately after the node's own `rounded_rect`, because a border is
/// drawn over the fill it encloses.
///
/// **Inset, so it costs the node no space.** A border drawn as a larger
/// rect underneath — which is how `focus_ring` does it, correctly, for a
/// ring that means "this has focus" — would make a bordered control
/// wider than an unbordered one beside it.
pub fn border(st: &[NodeState], i: usize, out: &mut dyn crate::Painter) {
    let s = &st[i];
    if s.border_width == 0 || s.border_color.3 == 0 {
        return;
    }
    out.stroked_rect(rect_of(s), s.border_color, s.radius, s.border_width as f32);
}

pub fn rect_of(st: &NodeState) -> crate::display::Rect {
    [st.pos.x, st.pos.y, st.pos.x + st.size.w, st.pos.y + st.size.h]
}

/// Space consumed by gaps between `n` children.
#[inline]
#[doc(hidden)]
pub const fn spaced(n: usize, spacing: f32) -> f32 {
    if n > 1 {
        spacing * (n - 1) as f32
    } else {
        0.0
    }
}

/// A container's own axis: which of width/height is the main one.
#[derive(Clone, Copy, PartialEq)]
pub enum Axis {
    Row,
    Column,
}

impl Axis {
    #[inline]
    pub fn main(&self, s: Size) -> f32 {
        match self {
            Axis::Row => s.w,
            Axis::Column => s.h,
        }
    }
    #[inline]
    pub fn cross(&self, s: Size) -> f32 {
        match self {
            Axis::Row => s.h,
            Axis::Column => s.w,
        }
    }
    #[inline]
    pub fn size(&self, main: f32, cross: f32) -> Size {
        match self {
            Axis::Row => Size { w: main, h: cross },
            Axis::Column => Size { w: cross, h: main },
        }
    }
    /// The child's `Length` along this container's main axis.
    #[inline]
    pub fn main_len(&self, s: &Style) -> Length {
        match self {
            Axis::Row => s.width,
            Axis::Column => s.height,
        }
    }

    /// …and along the cross axis.
    #[inline]
    pub fn cross_len(&self, s: &Style) -> Length {
        match self {
            Axis::Row => s.height,
            Axis::Column => s.width,
        }
    }
}

/// The content box: the node's limits minus its own padding.
#[inline]
#[doc(hidden)]
pub fn inner_limits(limits: Constraints, pad: crate::style::Edges) -> Constraints {
    Constraints {
        min_w: (limits.min_w - pad.horizontal()).max(0.0),
        max_w: (limits.max_w - pad.horizontal()).max(0.0),
        min_h: (limits.min_h - pad.vertical()).max(0.0),
        max_h: (limits.max_h - pad.vertical()).max(0.0),
    }
}

/// Limits for a non-`Fill` child: it may take up to whatever is left.
#[inline]
#[doc(hidden)]
pub fn loose_child(
    axis: Axis,
    inner: Constraints,
    remaining_main: f32,
    align: Align,
) -> Constraints {
    let cross_min = if align == Align::Fill { axis.cross_of(inner) } else { 0.0 };
    match axis {
        Axis::Row => Constraints {
            min_w: 0.0,
            max_w: remaining_main.max(0.0),
            min_h: cross_min,
            max_h: inner.max_h,
        },
        Axis::Column => Constraints {
            min_w: cross_min,
            max_w: inner.max_w,
            min_h: 0.0,
            max_h: remaining_main.max(0.0),
        },
    }
}

/// Tight main-axis limits for a `Fill` child's share.
#[inline]
#[doc(hidden)]
pub fn fill_child(axis: Axis, inner: Constraints, share: f32, align: Align) -> Constraints {
    let cross_min = if align == Align::Fill { axis.cross_of(inner) } else { 0.0 };
    match axis {
        Axis::Row => Constraints {
            min_w: share.max(0.0),
            max_w: share.max(0.0),
            min_h: cross_min,
            max_h: inner.max_h,
        },
        Axis::Column => Constraints {
            min_w: cross_min,
            max_w: inner.max_w,
            min_h: share.max(0.0),
            max_h: share.max(0.0),
        },
    }
}

impl Axis {
    #[inline]
    pub fn main_of(&self, c: Constraints) -> f32 {
        match self {
            Axis::Row => c.max_w,
            Axis::Column => c.max_h,
        }
    }

    #[inline]
    fn cross_of(&self, c: Constraints) -> f32 {
        match self {
            Axis::Row => c.max_h,
            Axis::Column => c.max_w,
        }
    }
}

/// Resolve a container's own size from its content, honouring its own
/// `Length`s: `Fill` takes the whole limit, `Fixed` takes its value,
/// `Shrink` takes the content size — then clamped.
#[inline]
pub fn resolve_self(limits: Constraints, style: &Style, content: Size) -> Size {
    // Every leaf resolves its own box here, so this is the one place a
    // leaf can honour `visible`. Before 2026-09-02 no leaf read the flag
    // at all: `ops.rs` contained zero occurrences of it, so hiding a
    // `Label` changed nothing — it kept its size and kept painting
    // (`docs/plan-review-fixes.md` F1.6).
    if !style.visible {
        return Size::default();
    }
    // An unbounded axis is a scroll region's (A5). `Fill` there cannot
    // mean the maximum — it is infinite — so it means the content, and
    // the enclosing scroller's viewport reaches the *children* through
    // `LayoutCx::fill_extent`. Either way no infinity reaches a `Size`.
    let w = match style.width {
        Length::Fixed(v) => v,
        Length::Shrink => content.w,
        Length::FillPortion(_) if !limits.max_w.is_finite() => content.w,
        Length::FillPortion(_) => limits.max_w,
    };
    let h = match style.height {
        Length::Fixed(v) => v,
        Length::Shrink => content.h,
        Length::FillPortion(_) if !limits.max_h.is_finite() => content.h,
        Length::FillPortion(_) => limits.max_h,
    };
    limits.clamp(Size { w, h })
}

/// Limits for a child of a `stack` container — M11/T11.6.
///
/// **A stack has no main axis**, which is why this cannot be
/// [`loose_child`] with an argument and why `Axis` gained no third
/// variant: every method on that enum presupposes that one of
/// width/height is the main one.
///
/// Every child is offered the container's whole inner box, because a
/// stack's children *share* its rect instead of dividing it. Nothing
/// else is needed for the two `Length` cases: [`resolve_self`] already
/// answers `FillPortion` with `limits.max_w`/`max_h`, so a `Fill` child
/// takes the box and a `Shrink` child measures its content.
///
/// `Align::Fill` is the one that also sets the *minimum*, so a child
/// stretches to the box rather than merely being allowed to — the same
/// meaning it has on a row's cross axis, applied to both axes here.
#[inline]
#[doc(hidden)]
pub fn stack_child(inner: Constraints, align: Align) -> Constraints {
    let (min_w, min_h) = if align == Align::Fill { (inner.max_w, inner.max_h) } else { (0.0, 0.0) };
    Constraints { min_w, max_w: inner.max_w, min_h, max_h: inner.max_h }
}

/// Whether an axis is the vertical one — for [`LayoutCx::fill_extent`].
#[inline]
#[doc(hidden)]
pub fn is_vertical(axis: Axis) -> bool {
    matches!(axis, Axis::Column)
}

/// Cross-axis offset for a child inside a container of `cross` extent.
#[inline]
#[doc(hidden)]
pub fn align_offset(align: Align, cross: f32, child_cross: f32) -> f32 {
    match align {
        Align::Start | Align::Fill => 0.0,
        Align::Center => ((cross - child_cross) / 2.0).max(0.0),
        Align::End => (cross - child_cross).max(0.0),
    }
}

/// Only the root of a subtree is stored by its parent; this records a
/// node's own laid-out box.
#[inline]
pub fn record(st: &mut [NodeState], i: usize, limits: Constraints, size: Size) -> Size {
    st[i].constraints = limits;
    st[i].size = size;
    size
}

/// `record`, plus the resolved paint attributes.
///
/// Every widget that draws calls this instead, so `paint` — which has no
/// `&[Style]` — can read the cascade's answer out of the state array it
/// already has.
pub fn record_painted(
    st: &mut [NodeState],
    i: usize,
    limits: Constraints,
    size: Size,
    style: &Style,
) -> Size {
    st[i].fg = style.color;
    // C6: the pointer affordance is resolved here, with the rest of the
    // paint attributes, so `paint` — which has no `&[Style]` — needs no
    // second array and no per-frame cascade. Pressed wins over hovered:
    // a control under a held pointer is pressed, not merely hovered.
    st[i].bg = if st[i].pressed && style.pressed_dim > 0.0 {
        dim(style.background, style.pressed_dim)
    } else if st[i].hovered && style.hover_dim > 0.0 {
        dim(style.background, style.hover_dim)
    } else {
        style.background
    };
    st[i].radius = style.radius;
    st[i].focus_ring = style.focus_ring;
    st[i].border_color = style.border_color;
    st[i].border_width = style.border_width;
    st[i].font_size = style.font_size;
    st[i].font_weight = style.font_weight;
    st[i].shadow = style.shadow;
    st[i].pad = (style.padding.left, style.padding.top, style.padding.right, style.padding.bottom);
    record(st, i, limits, size)
}

/// `c` darkened by `f` (0.0–1.0), alpha untouched.
///
/// Multiplicative rather than a fixed subtraction so the affordance
/// scales with the colour: 12% off a near-black surface is a shade, 12%
/// off a bright accent is a shade, and neither clips.
#[inline]
fn dim(c: crate::display::Color, f: f32) -> crate::display::Color {
    // Nothing to darken — the common case, a control at rest. Kept from
    // M25/T25.5's pilot, where it was measured to matter; the output is
    // identical, since a factor of 1.0 truncates to the same byte.
    if f <= 0.0 {
        return c;
    }
    let k = 1.0 - crate::num::clamp(f, 0.0, 1.0);
    let ch = |v: u8| (v as f32 * k) as u8;
    crate::display::Color(ch(c.0), ch(c.1), ch(c.2), c.3)
}

// ---- leaves ----------------------------------------------------------------

/// A leaf's boilerplate: one node, no children, `node`/`node_mut` are
/// self, `place` writes a position, `hit` is a bounds test, and
/// `kind_name`/`defaults` forward to the type-level [`crate::Widget`]
/// halves so the two spellings of one fact cannot drift (M24/T24.0).
///
/// **Exported, since M23.** `Node` has four methods that every leaf
/// answers identically, and a widget crate outside this one had no way
/// to reach them — so the first out-of-core widget set would have
/// written the same twenty lines per widget, which is the shape that let
/// `Shadow` and `Image` hash as the same tag for a year. Every path
/// inside is `$crate::`-qualified so it resolves wherever it is called
/// from.
#[macro_export]
macro_rules! leaf_common {
    ($ty:ty, $flags:expr) => {
        $crate::leaf_common!($ty, $flags, hit);
    };
    // `nohit`: the leaf is never a click target, whatever its box says.
    ($ty:ty, $flags:expr, nohit) => {
        fn place(
            &mut self,
            at: $crate::Point,
            _style: &[$crate::Style],
            st: &mut [$crate::NodeState],
            i: usize,
        ) {
            st[i].pos = at;
        }
        fn hit(
            &self,
            _st: &[$crate::NodeState],
            _i: usize,
            _x: f32,
            _y: f32,
        ) -> Option<$crate::Hit> {
            None
        }
        fn kind_name(&self) -> &'static str {
            <$ty as $crate::widget::Widget>::KIND
        }
        fn defaults(&self) -> $crate::Style {
            <$ty as $crate::widget::Widget>::type_defaults()
        }
        fn node(&self, _i: usize) -> &dyn $crate::Node {
            self
        }
        fn node_mut(&mut self, _i: usize) -> &mut dyn $crate::Node {
            self
        }
    };
    ($ty:ty, $flags:expr, hit) => {
        fn place(
            &mut self,
            at: $crate::Point,
            _style: &[$crate::Style],
            st: &mut [$crate::NodeState],
            i: usize,
        ) {
            st[i].pos = at;
        }
        fn hit(&self, st: &[$crate::NodeState], i: usize, x: f32, y: f32) -> Option<$crate::Hit> {
            if $crate::hit::contains(&st[i], x, y) {
                Some($crate::Hit {
                    node: i as u32,
                    inner: None,
                    local: $crate::Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                })
            } else {
                None
            }
        }
        fn kind_name(&self) -> &'static str {
            <$ty as $crate::widget::Widget>::KIND
        }
        fn defaults(&self) -> $crate::Style {
            <$ty as $crate::widget::Widget>::type_defaults()
        }
        fn node(&self, _i: usize) -> &dyn $crate::Node {
            self
        }
        fn node_mut(&mut self, _i: usize) -> &mut dyn $crate::Node {
            self
        }
    };
}

/// A run of text.
///
/// Generic over storage so each profile can choose (`docs/
/// layout-and-storage-decisions.md` §1); defaults to `&'static str` so
/// the `no_std` profile is the path of least resistance. `text` carries
/// a dirty bit and `measure` memoizes, because shaping is the expensive
/// part of a real frame — the reason text lives here rather than in a
/// sibling field reached by a getter.
#[derive(Default)]
pub struct Label<S: TextStorage = &'static str> {
    pub text: Dirty<S>,
    measure: Measured,
}

impl<S: crate::text::TextBuffer> crate::bind::TextTarget for Label<S> {
    /// The binding seam — M30. `#[bind]` writes through this rather
    /// than through the inherent method, because core cannot name this
    /// type since the widgets moved out.
    fn set_str(&mut self, text: &str) -> bool {
        Label::set_str(self, text)
    }
}

impl<S: TextStorage> Label<S> {
    pub fn new(text: S) -> Self {
        Label { text: Dirty::new(text), measure: Measured::new() }
    }

    /// Replace the text, marking it for re-measurement.
    pub fn set(&mut self, text: S) {
        self.text.set(text);
    }

    /// Replace the text from a borrowed string, reusing the storage's
    /// allocation where it can. **Compares first**: identical text does
    /// not touch the dirty bit, so the measurement memo survives.
    /// Returns whether the text changed.
    ///
    /// The compare used to live only in `crate::bind`, which meant the
    /// obvious way to write a repeater's bind function —
    /// `row.label.set_str(..)` — dirtied every visible row on every
    /// layout and defeated the memo entirely: 62 needless text
    /// measurements per *idle* frame for a 31-row window. Nothing in
    /// `fn(&mut T, usize)` steered an author away from it. Comparing
    /// here is the only sane default for a memoized field
    /// (`docs/plan-review-fixes.md` F4.6).
    ///
    /// Requires [`TextBuffer`](crate::text::TextBuffer), so
    /// `Label<&'static str>` does not have this method: that storage can
    /// only accept a new value by leaking.
    pub fn set_str(&mut self, text: &str) -> bool
    where
        S: crate::text::TextBuffer,
    {
        if self.text.get().as_ref() == text {
            return false;
        }
        self.text.get_mut().set_from_str(text);
        true
    }

    pub fn as_str(&self) -> &str {
        self.text.get().as_ref()
    }

    /// Whether the memo would serve a layout under `limits` and `style`
    /// without re-measuring.
    ///
    /// Takes the style since M26/T26.3, and not for symmetry: a memo that
    /// is warm at 16 px is cold at 32, so "is it warm" has no answer
    /// without the style being asked about. A version that assumed the
    /// default would have been the defect again, in the test API.
    pub fn measure_is_warm(&self, limits: Constraints, style: &Style, scale: f32) -> bool {
        !self.text.is_dirty() && self.measure.is_warm(MeasureKey::of(limits, style, scale))
    }
}

impl<S: TextStorage> Widget for Label<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];

    const KIND: &'static str = "label";

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::PADDING,
            crate::style::Fields::CURSOR,
            crate::style::Fields::VISIBLE,
            crate::style::Fields::COLOR,
            crate::style::Fields::BACKGROUND,
            crate::style::Fields::BORDER,
            crate::style::Fields::RADIUS,
            crate::style::Fields::FONT_SIZE,
            crate::style::Fields::FONT_WEIGHT,
            crate::style::Fields::PRESSED_DIM,
            crate::style::Fields::HOVER_DIM,
        ]))];
}

impl<S: TextStorage> Node for Label<S> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // The memo is the whole reason text is owned here: with real
        // shaping (M5) this call is the expensive one, and it must not
        // run for text that has not changed under limits that have not
        // changed.
        //
        // The key holds the size and weight as well as the limits
        // (M26/T26.3). Until then only the limits were in it, and the size
        // and weight were read in the closure below — so a restyle that
        // changed either, under the same limits, was served the old box.
        let text = self.text.get().as_ref();
        let dirty = self.text.is_dirty();
        let px = style[i].font_size;
        let key = MeasureKey::of(limits, &style[i], cx.scale());
        let content = self.measure.get_or(key, dirty, || {
            cx.text.measure_weighted(text, px, style[i].font_weight, limits.max_w)
        });
        self.text.clear_dirty();
        let pad = style[i].padding;
        let boxed = Size { w: content.w + pad.horizontal(), h: content.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], boxed), &style[i])
    }

    /// Measure without touching the memo — see [`Node::probe`]. The
    /// dirty bit is left alone too: a probe is a question, not a frame.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // The same measurement `layout` makes — size, weight, padding —
        // or the audit compares the frame against a different label.
        // Until 2026-09-03 this measured at 16 px regular with no
        // padding, so a probe of the tally's 76 px value asked for a
        // box a fifth its size.
        let text = self.text.get().as_ref();
        let content =
            cx.text.measure_weighted(text, style[i].font_size, style[i].font_weight, limits.max_w);
        let pad = style[i].padding;
        let boxed = Size { w: content.w + pad.horizontal(), h: content.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], boxed), &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        border(st, i, out);
        // Text sits inside the padding box; `layout` sized the node to
        // include it.
        let p = st[i].pad;
        out.text_weighted(
            [r[0] + p.0, r[1] + p.1, r[2] - p.2, r[3] - p.3],
            self.text.get().as_ref(),
            st[i].font_size,
            st[i].font_weight,
            st[i].fg,
        );
    }
    leaf_common!(Label<S>, 0);

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Label);
        node.set_value(self.text.get().as_ref());
    }
}

/// Fixed-size box: the rigid leaf (§2's relayout boundary, const-known).
pub struct Fixed {
    pub w: f32,
    pub h: f32,
}

impl Default for Fixed {
    fn default() -> Self {
        Fixed { w: 24.0, h: 24.0 }
    }
}

impl Widget for Fixed {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::RIGID)];

    const KIND: &'static str = "fixed";

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::CURSOR,
            crate::style::Fields::VISIBLE,
            crate::style::Fields::COLOR,
        ]))];
}

impl Node for Fixed {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let content = Size { w: self.w, h: self.h };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        out.rect(rect_of(&st[i]), st[i].fg);
    }
    leaf_common!(Fixed, NodeDesc::RIGID);
}

/// Fills whatever its parent hands it.
#[derive(Default)]
pub struct Spacer;

impl Widget for Spacer {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];

    const KIND: &'static str = "spacer";

    /// The one leaf that is filler rather than content.
    fn type_defaults() -> Style {
        Style { width: Length::FILL, height: Length::FILL, ..Style::default() }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::VISIBLE,
        ]))];
}

impl Node for Spacer {
    fn layout(
        &mut self,
        _cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        // A spacer is filler by default; its style says so, and
        // `resolve_self` turns that into "take the whole limit".
        record_painted(st, i, limits, resolve_self(limits, &style[i], Size::default()), &style[i])
    }
    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn Painter) {
        // A spacer is empty space; it draws nothing.
    }

    // `nohit`: a spacer is not a click target.
    // Hit testing follows painting — you cannot click what was never
    // drawn. A row or a column hides the difference, because its
    // children cannot overlap and claiming a transparent gap only ever
    // returns the parent. A `stack` does not: a `Spacer` at `FILL` in a
    // layer above the content is invisible, covers the whole window,
    // and is hit *first* because a stack walks its children in reverse.
    // It swallowed every click and every wheel event in
    // `examples/glass` (found 2026-09-11).
    leaf_common!(Spacer, 0, nohit);
}

/// A clickable leaf. `on_click` is a plain `fn` pointer, never a closure:
/// it cannot capture, so `fresh()` re-supplies it across a hot reload
/// (`docs/events-decision.md`). Payload a closure would have captured
/// arrives in [`EventCtx`] — notably `ctx.row` inside a repeater.
pub struct Button<S: 'static, T: TextStorage = &'static str> {
    pub label: T,
    pub on_click: fn(&mut S, &EventCtx),
    /// The label's measured box and what it was measured under. `label`
    /// is a plain field, not a `Dirty<_>`, so the key carries a content
    /// hash instead of a dirty bit — a few bytes per frame against a
    /// shaping call that allocates. Until 2026-09-03 a button never
    /// measured at all: width was `0.62 em × chars`, the text box was
    /// `font_size` tall, and every label sat 4 px low
    /// (`docs/results-frame-cost.md`).
    ///
    /// Keyed on a [`MeasureKey`] and a fold of the label's bytes. The
    /// fold stands in for the dirty bit a `Label` has and a `Button` does
    /// not. This was a private `ButtonKey` holding the same three fields
    /// the label's memo left out, so the right key and the wrong one sat
    /// in the same file until M26/T26.3 gave both widgets the one type.
    measured: crate::text::FoldedText,
}

pub fn ignore_click<S>(_: &mut S, _: &EventCtx) {}

impl<S, T: TextStorage> Default for Button<S, T> {
    fn default() -> Self {
        Button {
            label: T::default(),
            on_click: ignore_click::<S>,
            measured: crate::text::FoldedText::new(),
        }
    }
}

impl<S: 'static, T: TextStorage> Button<S, T> {}

impl<S: 'static, T: TextStorage> Widget for Button<S, T> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];

    const KIND: &'static str = "button";

    fn type_defaults() -> Style {
        Style {
            padding: crate::style::Edges { left: 14.0, right: 14.0, top: 9.0, bottom: 9.0 },
            // A control that does not change while held reads as broken,
            // and it is the one GUI-theory check `scripts/ui_check.py`
            // could not make before C6.
            pressed_dim: 0.18,
            hover_dim: 0.07,
            // A hand over something clickable (T14.6).
            cursor: crate::style::Cursor::Pointer,
            background: Color(226, 232, 240, 255),
            color: Color(15, 23, 42, 255),
            radius: 8.0,
            ..Style::default()
        }
    }

    const SEEDS: &'static [Seed] =
        &[Seed::new(Self::KIND, Self::type_defaults, crate::style::Fields::of(&[
            crate::style::Fields::WIDTH,
            crate::style::Fields::HEIGHT,
            crate::style::Fields::PADDING,
            crate::style::Fields::CURSOR,
            crate::style::Fields::FOCUS_RING,
            crate::style::Fields::VISIBLE,
            crate::style::Fields::COLOR,
            crate::style::Fields::BACKGROUND,
            crate::style::Fields::BORDER,
            crate::style::Fields::RADIUS,
            crate::style::Fields::FONT_SIZE,
            crate::style::Fields::FONT_WEIGHT,
            crate::style::Fields::PRESSED_DIM,
            crate::style::Fields::HOVER_DIM,
        ]))];
}

impl<S: 'static, T: TextStorage> Node for Button<S, T> {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let label = self.label.as_ref();
        let px = style[i].font_size;
        let weight = style[i].font_weight;
        let key = MeasureKey::of(limits, &style[i], cx.scale());
        // The fold stands in for the dirty bit a `Label` has and a
        // `Button` does not — in `FoldedText` since M26/T26.8, because
        // `Badge` and `Chip` needed the same thing and had none.
        let text = self.measured.get_or(key, label, || {
            cx.text.measure_weighted(label, px, weight, limits.max_w)
        });
        let pad = style[i].padding;
        let content = Size { w: text.w + pad.horizontal(), h: text.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }
    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        focus_ring(st, i, out);
        let r = rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        border(st, i, out);
        // Centre the *measured* line box in the button. The line box,
        // not the ink: that is what every toolkit centres, and it puts
        // capitals a hair below centre in faces with a deep descent
        // (Noto Sans: 0.03 em) — the same hair a browser leaves.
        let text = self.measured.size();
        let x = r[0] + ((r[2] - r[0]) - text.w).max(0.0) / 2.0;
        let y = r[1] + ((r[3] - r[1]) - text.h).max(0.0) / 2.0;
        out.text_weighted(
            [x, y, (x + text.w).max(r[2]), y + text.h],
            self.label.as_ref(),
            st[i].font_size,
            st[i].font_weight,
            st[i].fg,
        );
    }
    /// Measure without touching the memo — see [`Node::probe`].
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let text = cx.text.measure_weighted(
            self.label.as_ref(),
            style[i].font_size,
            style[i].font_weight,
            limits.max_w,
        );
        let pad = style[i].padding;
        let content = Size { w: text.w + pad.horizontal(), h: text.h + pad.vertical() };
        record_painted(st, i, limits, resolve_self(limits, &style[i], content), &style[i])
    }
    leaf_common!(Button<S, T>, NodeDesc::FOCUSABLE);

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Button);
        node.set_label(self.label.as_ref());
        node.add_action(accesskit::Action::Click);
    }
}

macro_rules! no_handlers {
    (Label<S2>) => {
        impl<S, S2: TextStorage> Handlers<S> for Label<S2> {
            fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
                None
            }
        }
    };
    ($ty:ty) => {
        impl<S> Handlers<S> for $ty {
            fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
                None
            }
        }
    };
}

no_handlers!(Label<S2>);
no_handlers!(Fixed);
no_handlers!(Spacer);

/// Only for its *own* state type — a `Button<A>` inside a `B`-rooted
/// tree fails to satisfy the derive's `Handlers<S>` bound, which is the
/// compile-time form of the old dispatch safety contract.
impl<S: 'static, T: TextStorage> Handlers<S> for Button<S, T> {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        Some(self.on_click)
    }
}
