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

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::Painter;
use lumen_static::hit::Hit;
use lumen_static::ops::{Axis, inner_limits, rect_of, resolve_self};
use lumen_static::style::{Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Constraints, LayoutCx, Point, Size};

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
    pub track: Option<lumen_static::display::Color>,
    /// The scrollbar's thumb colour; the text colour when `None` —
    /// M25/T25.10.
    pub thumb: Option<lumen_static::display::Color>,
    /// The viewport's size at the last layout, for `on_event`, which has
    /// no state array: the bar's geometry and a page's length both come
    /// from it (M44/T44.10).
    box_size: Size,
    /// Where along the thumb a bar drag took hold, while one is in
    /// progress (M44/T44.10).
    grab: Option<f32>,
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
            box_size: Size { w: 0.0, h: 0.0 },
            grab: None,
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
        let pos = lumen_static::num::clamp(self.scroll / over, 0.0, 1.0);
        Some(match self.axis {
            Axis::Column => {
                let frac = lumen_static::num::clamp(vh / (vh + over), 0.0, 1.0);
                let thumb_h = lumen_static::num::clamp(vh * frac, Self::MIN_THUMB.min(vh), vh);
                let y = r[1] + (vh - thumb_h) * pos;
                ([r[2] - Self::BAR, r[1], r[2], r[3]], [r[2] - Self::BAR, y, r[2], y + thumb_h])
            }
            Axis::Row => {
                let frac = lumen_static::num::clamp(vw / (vw + over), 0.0, 1.0);
                let thumb_w = lumen_static::num::clamp(vw * frac, Self::MIN_THUMB.min(vw), vw);
                let x = r[0] + (vw - thumb_w) * pos;
                ([r[0], r[3] - Self::BAR, r[2], r[3]], [x, r[3] - Self::BAR, x + thumb_w, r[3]])
            }
        })
    }

    /// Content index, relative to this node.
    const AT: usize = 1;

    /// The bar's track and thumb in this node's own coordinates, from
    /// the last layout — what `on_event` measures a drag against.
    fn local_bar(&self) -> Option<([f32; 4], [f32; 4])> {
        let s = NodeState { size: self.box_size, ..NodeState::default() };
        self.bar_rects(&s)
    }

    /// Scroll to put the thumb's leading edge at `edge` along the track
    /// (local px) — the arithmetic `bar_rects` does, inverted.
    fn scroll_to_thumb(&mut self, edge: f32) -> bool {
        let Some((track, thumb)) = self.local_bar() else { return false };
        let (t0, t1, len) = match self.axis {
            Axis::Column => (track[1], track[3], thumb[3] - thumb[1]),
            Axis::Row => (track[0], track[2], thumb[2] - thumb[0]),
        };
        let room = (t1 - t0 - len).max(0.0);
        let frac = if room > 0.0 { lumen_static::num::clamp((edge - t0) / room, 0.0, 1.0) } else { 0.0 };
        let want = frac * self.measured_overflow;
        let moved = want != self.scroll;
        self.scroll = want;
        moved
    }

    /// Scroll by `by` px, clamped; whether it moved.
    fn scroll_by(&mut self, by: f32) -> bool {
        let want = lumen_static::num::clamp(self.scroll + by, 0.0, self.measured_overflow);
        let moved = want != self.scroll;
        self.scroll = want;
        moved
    }

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
/// the same technique [`lumen_static::hatch::EitherTable`] uses, and for the
/// same reason: `SUB_TABLE` is a slice, so its length is a value.
pub struct ScrollTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> ScrollTable<W> {
    const ARRAY: [NodeDesc; MAX_SCROLL_NODES] = {
        assert!(W::NODE_COUNT < MAX_SCROLL_NODES, "ScrollView content exceeds MAX_SCROLL_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SCROLL_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    /// The seed counterpart of [`Self::ARRAY`], spliced the same way.
    const SEED_ARRAY: [Seed; MAX_SCROLL_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SCROLL_NODES];
        s[0] = Seed::new(<ScrollView<W> as Widget>::KIND, <ScrollView<W> as Widget>::type_defaults, lumen_static::style::Fields::of(&[
                lumen_static::style::Fields::WIDTH,
                lumen_static::style::Fields::HEIGHT,
                lumen_static::style::Fields::PADDING,
                lumen_static::style::Fields::CURSOR,
                lumen_static::style::Fields::VISIBLE,
                lumen_static::style::Fields::BACKGROUND,
                lumen_static::style::Fields::BORDER,
                lumen_static::style::Fields::RADIUS,
                lumen_static::style::Fields::PRESSED_DIM,
                lumen_static::style::Fields::HOVER_DIM,
            ]));
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

/// Upper bound on `1 + W::NODE_COUNT` for one [`ScrollView`]. See
/// [`lumen_static::hatch::MAX_EITHER_NODES`] for what the bound costs.
pub const MAX_SCROLL_NODES: usize = 256;

impl<W: Widget> Node for ScrollView<W> {
    lumen_static::node_count!();
    /// Its content is clipped to the viewport: a row below the fold is
    /// scrolled out, not misplaced (M55/T55.6, `W1002`).
    fn clips_children(&self) -> bool {
        true
    }

    /// How far the content is scrolled — M57/B40 (`Node::scroll_offset`).
    fn scroll_offset(&self) -> Option<f32> {
        Some(self.scroll)
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
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        // Inside its own `Fixed` size, not across the offer (M57/B26:
        // the same defect M56 found in the derive, in a hand-written
        // wrapper; `tests/wrapper_own_size.rs`).
        let inner = inner_limits(lumen_static::ops::within_own_size(limits, &me), me.padding);

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
        self.box_size = size;

        // A6: clamp here, because this is the first moment the content's
        // extent is known.
        let over = (self.axis.main(content) - self.axis.main(viewport)).max(0.0);
        self.measured_overflow = over;
        self.scroll =
            if self.scroll.is_finite() { lumen_static::num::clamp(self.scroll, 0.0, over) } else { 0.0 };

        lumen_static::ops::record_painted(st, i, limits, size, &me)
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
        lumen_static::ops::border(st, i, out);
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
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        // **The bar is this node's, before any content under it**
        // (M44/T44.10): it is drawn over the content's edge, and a press
        // there is a drag of the bar, not a click on a row beneath it.
        if let Some((track, _)) = self.bar_rects(&st[i]) {
            if x >= track[0] && x < track[2] && y >= track[1] && y < track[3] {
                return Some(Hit {
                    node: i as u32,
                    inner: None,
                    local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
                });
            }
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
    fn on_event(&mut self, ev: &lumen_static::event::Event, cx: &lumen_static::event::EventCtx) -> bool {
        use lumen_static::event::{Event, Key};
        let main = |p: Point, axis: Axis| match axis {
            Axis::Column => p.y,
            Axis::Row => p.x,
        };
        match *ev {
            Event::Scroll { dy, .. } => {
                // Wheel-up scrolls content down: the offset moves against
                // `dy`. At an end, or with nothing to scroll, this
                // declines, and the parent gets the rest of the gesture.
                self.scroll_by(-dy)
            }
            // **A drag on the bar** (M44/T44.10). The press takes the
            // thumb where it landed — or, on the track beside it, centres
            // the thumb there first — and consumes, so the pass captures
            // and every move until the release reaches this node.
            Event::PointerDown { .. } => {
                let Some((track, thumb)) = self.local_bar() else { return false };
                let (x, y) = (cx.local.x, cx.local.y);
                if !(x >= track[0] && x < track[2] && y >= track[1] && y < track[3]) {
                    return false;
                }
                let at = main(cx.local, self.axis);
                let (lo, hi) = match self.axis {
                    Axis::Column => (thumb[1], thumb[3]),
                    Axis::Row => (thumb[0], thumb[2]),
                };
                let grab = if at >= lo && at < hi { at - lo } else { (hi - lo) / 2.0 };
                self.grab = Some(grab);
                self.scroll_to_thumb(at - grab);
                true
            }
            Event::PointerMove { .. } => match self.grab {
                Some(grab) => {
                    self.scroll_to_thumb(main(cx.local, self.axis) - grab);
                    true
                }
                None => false,
            },
            Event::PointerUp { .. } | Event::PointerCancel { .. } => self.grab.take().is_some(),
            // **The keyboard scrolls** (M44/T44.10): PageUp and PageDown by
            // most of a viewport, Home and End to the ends. A scroller is
            // not focusable, so these arrive from a focused control inside
            // it, bubbling — and a scroller at its end declines, so an
            // outer one gets the key, as with the wheel.
            Event::Key { key, down: true } => {
                let page = 0.9 * main(Point { x: self.box_size.w, y: self.box_size.h }, self.axis);
                match key {
                    Key::PageDown => self.scroll_by(page),
                    Key::PageUp => self.scroll_by(-page),
                    Key::Home => self.scroll_by(-self.scroll),
                    Key::End => self.scroll_by(self.measured_overflow - self.scroll),
                    _ => false,
                }
            }
            _ => false,
        }
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

    /// A scroll view — M54/T54.3; it reported `GenericContainer`.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        use lumen_static::observe::accesskit::Role;
        node.set_role(Role::ScrollView);
    }
}

impl<W, S> lumen_static::event::Handlers<S> for ScrollView<W>
where
    W: Widget + lumen_static::event::Handlers<S>,
{
    fn handler(&self, sel: lumen_static::event::NodeSel) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.content.handler(lumen_static::event::NodeSel { i: sel.i - 1, inner: sel.inner })
    }
}
