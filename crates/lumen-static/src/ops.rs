//! The layout helpers the derive calls — M30.
//!
//! `ops` is called that because it once held the per-kind op tables M0
//! deleted. It then held two audiences at once: the widget vocabulary
//! (`Label`, `Fixed`, `Spacer`, `Button`) and the machinery
//! derive-generated code calls. This module's own rustdoc named the fix
//! — *"splitting the two into a widgets module and a hidden private one
//! is the honest end state (`docs/plan-review-fixes.md` F6.1)"* — and
//! deferred it for churn.
//!
//! M30 moved the widgets to `lumen-static-widgets` and the split came
//! with them, because the crate boundary made it compulsory rather than
//! merely honest: `inner_limits`, `loose_child`, `fill_child`,
//! `align_offset`, `spaced` and `hide_subtree` are what the derive emits
//! calls to, so they cannot live in an optional crate. `resolve_self`,
//! `record`, `record_painted` and `rect_of` are public because widgets
//! outside this crate genuinely use them — which is now every widget.

use crate::desc::{Constraints, NodeState, Size};
use crate::style::{Align, Length, Style};

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

/// Draw the keyboard focus ring around `s` — **called once per frame by
/// the host, not by widgets** (M57/A6, `docs/plan-widget-fixes.md`).
///
/// **An outline with a gap, outside the node, in the palette's `focus`
/// colour, drawn after the tree.** The ring used to be a larger rounded
/// rect *underneath* each widget, in `s.fg`, relying on the widget to
/// paint an opaque full-box body over its middle. The widget audit
/// (`docs/widget-audit.md` X4) found every way that fails: a `Tabs`
/// strip has no body and turned into a solid dark block; a `Slider`
/// paints only a thin track and became a solid blue bar with the knob
/// gone; a primary button's `fg` is white, so its ring was white on a
/// white card; a checked toggle's ring was its own blue with no gap.
///
/// Before that, a ring of four `Cmd::Line`s over a rounded button covered
/// the antialiased corner (`ui_check.py`'s corner test). A
/// `stroked_rect` follows the radius the renderer already antialiases,
/// and the `gap` keeps it off the control's own pixels, so it can neither
/// cover the content nor merge with a body of the same colour.
///
/// There is one focus, so there is one ring, and the host — which knows
/// `input.focus`, whether focus is keyboard-visible, and the palette —
/// draws it. A widget opts out with `Style::focus_ring == 0`.
pub fn focus_ring(s: &crate::desc::NodeState, color: crate::display::Color, out: &mut dyn crate::Painter) {
    if s.focus_ring == 0 || s.size.w <= 0.0 || s.size.h <= 0.0 {
        return;
    }
    let (w, gap) = (s.focus_ring as f32, FOCUS_GAP);
    let r = rect_of(s);
    let o = gap + w;
    // `stroked_rect` strokes *inside* its rect (T27.1), so the rect is
    // the ring's outer edge and the stroke fills `[gap, gap + w]`.
    out.stroked_rect([r[0] - o, r[1] - o, r[2] + o, r[3] + o], color, s.radius + o, w);
}

/// The transparent space between a control and its focus ring, so a
/// ring the colour of the control's body still reads as a ring.
pub const FOCUS_GAP: f32 = 2.0;

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

/// Draw `i`'s fill, if it has one — M82/T82.3
/// (`docs/plan-paint-vocabulary.md`).
///
/// [`NodeState::fill`] is an index into the table the painter answers
/// ([`crate::Painter::fills`]): `0` draws nothing, `n` draws entry
/// `n - 1` as an ordinary inline [`crate::Painter::gradient`] over the
/// node's box, rounded by its radius. So what reaches `Recording`,
/// `hash_into`, the journal and the window is the resolved stops, never
/// the index (`docs/plan-iced-roadmap.md` §2.9). An index past the table's
/// end draws nothing, which is also what a painter with no table (`&[]`,
/// the firmware's `Checksum`) does: it paints the flat `bg` alone.
///
/// Called at every one of [`border`]'s call sites, **before** the node's
/// flat `bg`, so the fill shows where the background is transparent and a
/// background colour still covers it; `scripts/check_fill_paint.sh` keeps
/// the two call sites paired. A disabled node's stops fade to 40% alpha,
/// as [`record_painted`] fades `bg` (M57/A16).
pub fn fill(st: &[NodeState], i: usize, out: &mut dyn crate::Painter) {
    let s = &st[i];
    if s.fill == 0 {
        return;
    }
    let Some(f) = out.fills().get(s.fill as usize - 1).copied() else {
        return;
    };
    let mut stops = f.stops;
    if s.disabled {
        for c in stops.color.iter_mut() {
            c.3 = (c.3 as u16 * 2 / 5) as u8;
        }
    }
    out.gradient(rect_of(s), stops, f.shape, s.radius);
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

/// **What a container offers its own content**: on an axis where its
/// own length is `Fixed`, that length — clamped into the offer exactly
/// as [`resolve_self`] will clamp it — and not the whole offer.
///
/// Found by M56 (`docs/plan-showcase-design.md`,
/// `crates/lumen-static/tests/fixed_container.rs`): the derive computed
/// a container's content box from what it was *offered* and applied its
/// own `Fixed` size only when it resolved its box afterwards, so a
/// `Fixed(60)` column offered 400 px gave a `FILL` child 380 px of height
/// and a `Fixed(320)` column holding a grid laid the grid's rows out
/// 727 px wide. It stayed hidden while every such container sat in a
/// parent offering less than its fixed size.
#[inline]
#[doc(hidden)]
pub fn within_own_size(limits: Constraints, style: &Style) -> Constraints {
    let mut c = limits;
    if let Length::Fixed(v) = style.width {
        let w = limits.clamp(Size { w: v, h: 0.0 }).w;
        c.min_w = w;
        c.max_w = w;
    }
    if let Length::Fixed(v) = style.height {
        let h = limits.clamp(Size { w: 0.0, h: v }).h;
        c.min_h = h;
        c.max_h = h;
    }
    c
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
    st[i].disabled = style.disabled;
    if style.disabled {
        // **Unavailable reads as faded** — M57/A16. Both colours at 40%
        // alpha, and no hover or press shade: nothing will happen.
        let fade = |c: crate::display::Color| crate::display::Color(c.0, c.1, c.2, (c.3 as u16 * 2 / 5) as u8);
        st[i].fg = fade(style.color);
        st[i].bg = fade(style.background);
        st[i].border_color = fade(style.border_color);
        st[i].radius = style.radius;
        st[i].focus_ring = style.focus_ring;
        st[i].border_width = style.border_width;
        st[i].font_size = style.font_size;
        st[i].font_weight = style.font_weight;
        st[i].line_height = style.line_height;
        // M82/T82.3: the index; `ops::fill` fades the stops it resolves.
        st[i].fill = style.background_fill;
        st[i].shadow = style.shadow;
        st[i].pad = (style.padding.left, style.padding.top, style.padding.right, style.padding.bottom);
        return record(st, i, limits, size);
    }
    st[i].fg = style.color;
    // C6: the pointer affordance is resolved here, with the rest of the
    // paint attributes, so `paint` — which has no `&[Style]` — needs no
    // second array and no per-frame cascade. Pressed wins over hovered:
    // a control under a held pointer is pressed, not merely hovered.
    st[i].bg = shaded(style.background, &st[i], style);
    st[i].radius = style.radius;
    st[i].focus_ring = style.focus_ring;
    st[i].border_color = style.border_color;
    st[i].border_width = style.border_width;
    st[i].font_size = style.font_size;
    st[i].font_weight = style.font_weight;
    // M80/T80.2: the line pitch, for a `Label`'s `text_spaced`.
    st[i].line_height = style.line_height;
    // M82/T82.3: the fill table index, for `ops::fill` at paint.
    st[i].fill = style.background_fill;
    st[i].shadow = style.shadow;
    st[i].pad = (style.padding.left, style.padding.top, style.padding.right, style.padding.bottom);
    record(st, i, limits, size)
}

/// `c` as the pointer affordance shows it on `n`: darkened by the style's
/// `pressed_dim` while pressed, else by `hover_dim` while hovered.
///
/// What [`record_painted`] does to `background`, public — M57/B11 —
/// for a widget whose visible body is another colour. A toggle that is
/// on is drawn in `color`, so it showed no hover and no press at all:
/// `record_painted` shades only the background, which an on toggle does
/// not paint.
pub fn shaded(c: crate::display::Color, n: &NodeState, style: &Style) -> crate::display::Color {
    if n.pressed && style.pressed_dim > 0.0 {
        dim(c, style.pressed_dim)
    } else if n.hovered && style.hover_dim > 0.0 {
        dim(c, style.hover_dim)
    } else {
        c
    }
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
        $crate::node_count!();
        $crate::__a11y_items! {
            fn observe_walk(
                &self,
                _table: &[$crate::desc::NodeDesc],
                st: &[$crate::NodeState],
                id_of: &dyn Fn(usize) -> u64,
                out: &mut $crate::observe::ObsVec<($crate::observe::NodeId, $crate::observe::AkNode)>,
            ) {
                // A leaf is one node; its `observe` is called statically,
                // so the leaf is never coerced to `&dyn Node` (M35/T35.1).
                $crate::observe::emit_node(
                    <$ty as $crate::widget::Widget>::SUB_TABLE,
                    st,
                    0,
                    id_of,
                    out,
                    |n, o| $crate::widget::Node::observe(self, st, 0, id_of(0), n, o),
                );
            }
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
        $crate::node_count!();
        $crate::__a11y_items! {
            fn observe_walk(
                &self,
                _table: &[$crate::desc::NodeDesc],
                st: &[$crate::NodeState],
                id_of: &dyn Fn(usize) -> u64,
                out: &mut $crate::observe::ObsVec<($crate::observe::NodeId, $crate::observe::AkNode)>,
            ) {
                // A leaf is one node; its `observe` is called statically,
                // so the leaf is never coerced to `&dyn Node` (M35/T35.1).
                $crate::observe::emit_node(
                    <$ty as $crate::widget::Widget>::SUB_TABLE,
                    st,
                    0,
                    id_of,
                    out,
                    |n, o| $crate::widget::Node::observe(self, st, 0, id_of(0), n, o),
                );
            }
        }
    };
}

/// `Node::node_count` for a hand-written impl — M48/T48.7. (The method
/// exists only when core is built with `observe`, so it is named here
/// rather than linked: a link would not resolve in the default build.) One line inside `impl Node for X`, where `X:
/// Widget`: it answers `X::NODE_COUNT` and names no child, so it cannot
/// forward wrongly. Expands to nothing without core's `observe`, whichever
/// crate it is written in.
#[macro_export]
macro_rules! node_count {
    () => {
        $crate::__observe_items! {
            fn node_count(&self) -> usize {
                <Self as $crate::widget::Widget>::NODE_COUNT
            }
        }
    };
}

/// Expands its items only when `lumen-static` is built with `observe` —
/// M35/T35.1. Derive- and macro-emitted code lands in the *user's* crate,
/// where a `cfg(feature = "observe")` would test the user's features, not
/// this crate's; routing through a macro defined twice here decides it
/// where the feature lives.
#[cfg(feature = "observe")]
#[doc(hidden)]
#[macro_export]
macro_rules! __observe_items {
    ($($t:tt)*) => { $($t)* };
}

/// See the `observe` build of this macro.
#[cfg(not(feature = "observe"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __observe_items {
    ($($t:tt)*) => {};
}

/// Expands its items only when `lumen-static` is built with `a11y` —
/// M54/T54.1. The accessibility half of what `__observe_items!` gated
/// until then: the derive's and `leaf_common!`'s static walk
/// ([`Node::observe_walk`](crate::widget::Node::observe_walk)) follows the
/// screen-reader tree, and `observe` implies `a11y`, so an agent build
/// still has it.
#[cfg(feature = "a11y")]
#[doc(hidden)]
#[macro_export]
macro_rules! __a11y_items {
    ($($t:tt)*) => { $($t)* };
}

/// See the `a11y` build of this macro.
#[cfg(not(feature = "a11y"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __a11y_items {
    ($($t:tt)*) => {};
}
