//! A scroll region — plan M8, Cluster A of
//! `docs/text-and-scrolling-decisions.md`.
//!
//! One node plus its content's subtree, both on the table: the content
//! is a fixed shape, so there is nothing to virtualize and no arena.
//! What makes it a scroller is that the content is measured against the
//! viewport but *allowed to exceed it*, placed at an offset, and clipped
//! back to the viewport when painted and when hit.
//!
//! # A5 — what constraints the content gets, and why there is no NaN
//!
//! The content gets **the viewport's own constraints, unchanged**. That
//! is A5's decision ("`Fill` inside a scroller means the viewport, never
//! the content") and it is also what avoids the hazard A5 was raised
//! about. The tempting implementation is to loosen the main axis to
//! infinity so the content can exceed the viewport; then a `FillPortion`
//! child inside computes `used += ∞`, `free = (∞ − ∞).max(0.0)` =
//! **NaN**, and NaN sizes land in `NodeState`, make every `contains`
//! false, hash by bit pattern into the journal and slip past the audit's
//! `EPS` comparisons.
//!
//! No infinity is needed. `resolve_self` gives a `Shrink` node its
//! *content* size and does not clamp it to `max`, so content already
//! exceeds a finite viewport by construction. `tests/scroll_view.rs`
//! pins both halves: content taller than the viewport, and not one
//! non-finite number anywhere in the state array.
//!
//! # A6 — who clamps the offset
//!
//! `layout` does, after measuring, because that is the first moment the
//! content's extent is known — the same reasoning B5 uses to put
//! caret-into-view in `layout`. A wheel handler writes `scroll` freely;
//! an offset past the end is corrected on the next frame rather than
//! rejected at the setter, which no setter could do correctly anyway.
//!
//! # A7 — a `Repeat` inside a `ScrollView`
//!
//! Forbidden, and **reported rather than refused**. `Repeat` derives its
//! window from the constraint it is given; a scroller hands its content
//! an unbounded main axis, so the repeater's window becomes "all of it"
//! and a million-row list materializes every row. A `Repeat` *is* a
//! scroller; nesting them asks one of them to be wrong.
//!
//! `ui.lint` reports it as `W1008`, naming both nodes. The first
//! implementation also had `layout` assert against it in debug, and that
//! was wrong twice over: it turned a slow frame into a crash, and — the
//! part that settled it — it made the release-path lint impossible to
//! test, because the assert fired before any frame the lint could
//! examine. A check that prevents its own verification is not a check.
//! `crates/lumen-static-agent/tests/lint.rs` covers it now.

use crate::desc::{NodeDesc, NodeState, NO_PARENT};
use crate::display::Painter;
use crate::hit::Hit;
use crate::ops::{inner_limits, rect_of, resolve_self, Axis};
use crate::style::{Length, Seed, Style};
use crate::widget::{Node, Widget};
use crate::{Constraints, LayoutCx, Point, Size};

/// A clipped, offset window onto a taller (or wider) content subtree.
pub struct ScrollView<W: Widget> {
    pub content: W,
    /// Offset along [`ScrollView::axis`], in px. Clamped in `layout`
    /// (A6); a handler may write anything.
    pub scroll: f32,
    /// Which way it scrolls. Vertical is the default and the common
    /// case; the cross axis is not scrolled, it is fitted.
    pub axis: Axis,
    /// How far the content exceeded the viewport at the last layout.
    ///
    /// Kept because `on_event` has no state array and must still know
    /// whether it is at an end — which is what decides chaining. Not
    /// `pub`: it is a cache of the last frame, and a caller that sets it
    /// is lying to the chaining rule.
    measured_overflow: f32,
    /// The scrollbar's track colour; a tint of the text colour when `None`
    /// — M25/T25.10.
    pub track: Option<crate::display::Color>,
    /// The scrollbar's thumb colour; the text colour when `None` —
    /// M25/T25.10.
    pub thumb: Option<crate::display::Color>,
}

impl<W: Widget + Default> Default for ScrollView<W> {
    fn default() -> Self {
        ScrollView::new(W::default())
    }
}

impl<W: Widget> ScrollView<W> {
    pub fn new(content: W) -> Self {
        ScrollView {
            content,
            scroll: 0.0,
            axis: Axis::Column,
            measured_overflow: 0.0,
            track: None,
            thumb: None,
        }
    }

    /// A horizontal scroller.
    pub fn horizontal(content: W) -> Self {
        ScrollView { axis: Axis::Row, ..ScrollView::new(content) }
    }

    /// Thickness of the scrollbar, in logical px.
    pub const BAR: f32 = 8.0;
    /// Shortest a thumb may get, so a very long list still has one a
    /// person can see and grab.
    pub const MIN_THUMB: f32 = 24.0;

    /// Track and thumb rects, or `None` when there is nothing to scroll.
    ///
    /// A scroller with no overflow paints **no bar at all** rather than
    /// a full-length one: a bar that is always present says "this
    /// scrolls" about something that does not.
    fn bar_rects(&self, s: &NodeState) -> Option<([f32; 4], [f32; 4])> {
        // Spelled positively rather than as `!(over > 0.0)`: NaN must
        // land in the "no bar" arm, and `is_finite` says so where a
        // negated comparison only implies it — the same rule
        // `visible_range` already follows two methods down.
        let over = self.measured_overflow;
        if !over.is_finite() || over <= 0.0 {
            return None;
        }
        let r = rect_of(s);
        let (vw, vh) = (r[2] - r[0], r[3] - r[1]);
        // The thumb is as long a fraction of the track as the viewport
        // is of the content -- the conventional proportion, and the one
        // that makes the bar readable as "how much of this am I seeing".
        let pos = crate::num::clamp(self.scroll / over, 0.0, 1.0);
        Some(match self.axis {
            Axis::Column => {
                let frac = crate::num::clamp(vh / (vh + over), 0.0, 1.0);
                let thumb_h = crate::num::clamp(vh * frac, Self::MIN_THUMB.min(vh), vh);
                let y = r[1] + (vh - thumb_h) * pos;
                ([r[2] - Self::BAR, r[1], r[2], r[3]], [r[2] - Self::BAR, y, r[2], y + thumb_h])
            }
            Axis::Row => {
                let frac = crate::num::clamp(vw / (vw + over), 0.0, 1.0);
                let thumb_w = crate::num::clamp(vw * frac, Self::MIN_THUMB.min(vw), vw);
                let x = r[0] + (vw - thumb_w) * pos;
                ([r[0], r[3] - Self::BAR, r[2], r[3]], [x, r[3] - Self::BAR, x + thumb_w, r[3]])
            }
        })
    }

    /// Content index, relative to this node.
    const AT: usize = 1;

    /// How far the content exceeds the viewport, from the last layout —
    /// the range `scroll` is clamped to.
    pub fn overflow(&self, st: &[NodeState], i: usize) -> f32 {
        let (viewport, content) = (st[i].size, st[i + Self::AT].size);
        (self.axis.main(content) - self.axis.main(viewport)).max(0.0)
    }
}

impl<W: Widget> Widget for ScrollView<W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// A scroller's size comes from its parent, never from its content —
    /// that is what it is for. Rigid on both axes, so a content change
    /// inside can never resize anything above it.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = ScrollTable::<W>::SLICE;

    const KIND: &'static str = "scrollview";

    const SEEDS: &'static [Seed] = ScrollTable::<W>::SEED_SLICE;

    /// Fills its parent on both axes: a scroller with a `Shrink` height
    /// would size to its content and never scroll.
    fn type_defaults() -> Style {
        Style { width: Length::FILL, height: Length::FILL, ..Style::default() }
    }
}

/// The merged table for one `ScrollView<W>`, built at compile time —
/// the same technique [`crate::hatch::EitherTable`] uses, and for the
/// same reason: `SUB_TABLE` is a slice, so its length is a value.
pub struct ScrollTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> ScrollTable<W> {
    const ARRAY: [NodeDesc; MAX_SCROLL_NODES] = {
        assert!(W::NODE_COUNT < MAX_SCROLL_NODES, "ScrollView content exceeds MAX_SCROLL_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SCROLL_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        crate::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    /// The seed counterpart of [`Self::ARRAY`], spliced the same way.
    const SEED_ARRAY: [Seed; MAX_SCROLL_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SCROLL_NODES];
        s[0] = Seed::new(<ScrollView<W> as Widget>::KIND, <ScrollView<W> as Widget>::type_defaults)
            .reading(crate::style::Fields::of(&[
                crate::style::Fields::WIDTH,
                crate::style::Fields::HEIGHT,
                crate::style::Fields::PADDING,
                crate::style::Fields::CURSOR,
                crate::style::Fields::VISIBLE,
                crate::style::Fields::BACKGROUND,
                crate::style::Fields::RADIUS,
                crate::style::Fields::PRESSED_DIM,
                crate::style::Fields::HOVER_DIM,
            ]));
        crate::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

/// Upper bound on `1 + W::NODE_COUNT` for one [`ScrollView`]. See
/// [`crate::hatch::MAX_EITHER_NODES`] for what the bound costs.
pub const MAX_SCROLL_NODES: usize = 256;

impl<W: Widget> Node for ScrollView<W> {
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
            return crate::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        let inner = inner_limits(limits, me.padding);

        // The viewport, resolved before the content: `Fill` inside needs
        // it, and it does not depend on the content — a scroller is
        // rigid, which is what makes this order possible at all.
        let outer = resolve_self(limits, &me, Size::default());
        let viewport = Size {
            w: (outer.w - me.padding.horizontal()).max(0.0),
            h: (outer.h - me.padding.vertical()).max(0.0),
        };

        // A5. The main axis is unbounded so `Shrink` content can exceed
        // the viewport — `resolve_self` clamps every node to the limits
        // it is given, so a finite maximum would simply squeeze the
        // content and there would be nothing to scroll. `Fill` inside
        // resolves against the viewport instead, published through the
        // context; no infinity reaches a `Size`.
        let loosened = match self.axis {
            Axis::Column => Constraints { max_h: f32::INFINITY, ..inner },
            Axis::Row => Constraints { max_w: f32::INFINITY, ..inner },
        };
        let content = cx.with_fill_span(viewport, |cx| {
            self.content.layout(cx, loosened, style, st, i + Self::AT)
        });

        let size = outer;

        // A6: clamp here, because this is the first moment the content's
        // extent is known.
        let over = (self.axis.main(content) - self.axis.main(viewport)).max(0.0);
        self.measured_overflow = over;
        self.scroll =
            if self.scroll.is_finite() { crate::num::clamp(self.scroll, 0.0, over) } else { 0.0 };

        crate::ops::record_painted(st, i, limits, size, &me)
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        let pad = style[i].padding;
        let origin = Point { x: at.x + pad.left, y: at.y + pad.top };
        let shifted = match self.axis {
            Axis::Column => Point { x: origin.x, y: origin.y - self.scroll },
            Axis::Row => Point { x: origin.x - self.scroll, y: origin.y },
        };
        self.content.place(shifted, style, st, i + Self::AT);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let r = rect_of(&st[i]);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        crate::ops::border(st, i, out);
        // The scoped containment invariant: content may extend past the
        // viewport by construction, and the clip is what stops it
        // (`docs/text-and-scrolling-decisions.md` A1). A renderer that
        // ignores the clip would draw rows no click can reach.
        out.push_clip(r);
        self.content.paint(st, i + Self::AT, out);
        out.pop_clip();

        // The scrollbar, *outside* the clip: it belongs to the viewport,
        // not to the content, so clipping it would be right by accident
        // rather than by construction (M14/T14.7).
        if let Some((track, thumb)) = self.bar_rects(&st[i]) {
            out.rect(track, self.track.unwrap_or_else(|| crate::input::selection_tint(st[i].fg)));
            out.rect(thumb, self.thumb.unwrap_or(st[i].fg));
        }
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        // Outside the viewport is outside the scroller, whatever the
        // content's own geometry says — the half of A1 that keeps paint
        // and hit agreeing.
        if !crate::hit::contains(&st[i], x, y) {
            return None;
        }
        self.content.hit(st, i + Self::AT, x, y).or(Some(Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        }))
    }

    /// Consume a wheel event, or decline it so a parent scroller gets
    /// it (C3, and C4's chaining).
    ///
    /// Declining at the end of travel is the whole of scroll chaining:
    /// an inner list scrolled to its bottom hands the rest of the
    /// gesture to the page, which is what every platform does and what
    /// people notice immediately when it is missing. `dispatch` walks
    /// the chain and stops at the first `true`, so nothing else is
    /// needed for it.
    ///
    /// Uses the overflow `layout` measured, not the state array: an
    /// event handler has no `&[NodeState]`, and the last frame's extent
    /// is the right answer anyway — it is the frame the user is looking
    /// at while they turn the wheel.
    fn on_event(&mut self, ev: &crate::event::Event, _cx: &crate::event::EventCtx) -> bool {
        let crate::event::Event::Scroll { dy, .. } = ev else { return false };
        // Wheel-up scrolls content down: the offset moves against `dy`.
        let want = crate::num::clamp(self.scroll - dy, 0.0, self.measured_overflow);
        if want == self.scroll {
            // At an end, or nothing to scroll: decline, and the parent
            // gets the rest of the gesture.
            return false;
        }
        self.scroll = want;
        true
    }

    fn node(&self, i: usize) -> &dyn Node {
        if i == 0 {
            self
        } else {
            self.content.node(i - Self::AT)
        }
    }

    fn node_mut(&mut self, i: usize) -> &mut dyn Node {
        if i == 0 {
            self
        } else {
            self.content.node_mut(i - Self::AT)
        }
    }
}

impl<W, S> crate::event::Handlers<S> for ScrollView<W>
where
    W: Widget + crate::event::Handlers<S>,
{
    fn handler(&self, sel: crate::event::NodeSel) -> Option<fn(&mut S, &crate::event::EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.content.handler(crate::event::NodeSel { i: sel.i - 1, inner: sel.inner })
    }
}
