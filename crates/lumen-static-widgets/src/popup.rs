//! Content that floats above the tree — `docs/plan-capabilities.md`
//! T15.0/T15.1.
//!
//! `Popup<W>` is the widget half of the overlay phase. It is
//! **out of flow**: it measures its content but reports **zero size**
//! for itself, so opening one does not move its siblings — which is the
//! whole point of a dropdown, a tooltip or a menu.
//!
//! # What it costs when closed
//!
//! A closed popup lays out nothing, paints nothing and hits nothing. Its
//! content's nodes stay in the table (the topology is a `const`), and
//! `layout` zeroes them exactly as a hidden subtree is zeroed.
//!
//! # What happens with no overlay sink
//!
//! [`lumen_static::Painter::defer`] returns `false` and the content paints
//! **inline** — clipped by its ancestors and possibly under its later
//! siblings, which is what it would have looked like before overlays
//! existed. Degraded, not broken, and that is deliberate: a host that
//! has not opted into an overlay sink still gets a working application.

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::Painter;
use lumen_static::hit::Hit;
use lumen_static::style::{Seed, Style};
use lumen_static::widget::{Node, Widget};

/// Where a popup puts itself relative to its anchor.
///
/// The anchor is the popup's **parent**, whose rect is already resolved
/// by the time `place` runs — which is what lets positioning be the
/// widget's job rather than the layout model's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// Below the parent, left edges aligned — a dropdown.
    #[default]
    Below,
    /// Above the parent, left edges aligned.
    Above,
    /// Over the parent, top-left aligned — a modal's backdrop child.
    Over,
    /// The window's bottom centre, 24 px up — a toast (M57/B34). Where
    /// the parent is does not matter; with no known window it falls back
    /// to `Below`.
    WindowBottom,
}

/// A subtree painted in the overlay phase.
pub struct Popup<W: Widget> {
    pub content: W,
    /// Whether it is showing. A closed popup costs a `bool` test.
    pub open: bool,
    pub anchor: Anchor,
    /// A **passive** popup is dismissed by a press outside it without
    /// consuming that press — a tooltip, which must not take the click
    /// from the control it explains. `false` (consuming) by default,
    /// which is what a menu wants (M33/T33.3).
    pub passive: bool,
    /// A **modal** popup traps Tab inside itself — a dialog. `false` by
    /// default, which is what a menu and a tooltip want: Tab dismisses a
    /// menu and passes a tooltip by (M44/T44.9). `Popup` is the general
    /// overlay primitive, so it cannot answer for every use; a `Veil` is
    /// always modal.
    pub modal: bool,
    /// The content's measured size, kept from `layout` so `place` can
    /// position it. The same reason `ScrollView` keeps its overflow:
    /// `place` does not re-measure.
    measured: Size,
    /// How far above its anchor point the popup goes when it has to flip
    /// — the height of whatever it hangs from, plus any gap — M57/A18.
    ///
    /// A `Below` popup is placed *at* its anchor, which its owner puts at
    /// the bottom of its trigger; flipping above has to clear the trigger
    /// too, and only the owner knows how tall it is. `0` (the default)
    /// flips to end at the anchor point itself.
    pub above_by: f32,
    /// The window, as the last layout saw it (`LayoutCx::viewport`).
    viewport: Size,
}

impl<W: Widget + Default> Default for Popup<W> {
    fn default() -> Self {
        Popup {
            content: W::default(),
            open: false,
            anchor: Anchor::default(),
            passive: false,
            modal: false,
            measured: Size::default(),
            above_by: 0.0,
            viewport: Size { w: f32::INFINITY, h: f32::INFINITY },
        }
    }
}

impl<W: Widget> Popup<W> {
    /// Content index, relative to this node.
    const AT: usize = 1;

    /// The content's size as the last layout measured it — so an owner can
    /// place the popup against it, as `Tooltip` centres its tip (M57/B35).
    pub fn measured(&self) -> Size {
        self.measured
    }

    pub fn new(content: W) -> Self {
        Popup {
            content,
            open: false,
            anchor: Anchor::default(),
            passive: false,
            modal: false,
            measured: Size::default(),
            above_by: 0.0,
            viewport: Size { w: f32::INFINITY, h: f32::INFINITY },
        }
    }
}

impl<W: Widget> Widget for Popup<W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// Zero-sized in flow, so its parent never asks it to stretch.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = PopupTable::<W>::SLICE;

    const KIND: &'static str = "popup";

    const SEEDS: &'static [Seed] = PopupTable::<W>::SEED_SLICE;
}

/// The merged table for one `Popup<W>`, built at compile time — the
/// same technique [`crate::scroll::ScrollTable`] uses, and for the same
/// reason: `SUB_TABLE` is a slice, so its length is a value.
pub struct PopupTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> PopupTable<W> {
    const ARRAY: [NodeDesc; MAX_POPUP_NODES] = {
        assert!(W::NODE_COUNT < MAX_POPUP_NODES, "Popup content exceeds MAX_POPUP_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_POPUP_NODES];
        // `DEFERS`: `paint` asks for the overlay phase (M85/T85.6).
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER | NodeDesc::DEFERS);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    /// The seed counterpart of [`Self::ARRAY`], spliced the same way.
    const SEED_ARRAY: [Seed; MAX_POPUP_NODES] = {
        let mut s = [Seed::WIDGET; MAX_POPUP_NODES];
        s[0] = Seed::new(<Popup<W> as Widget>::KIND, <Popup<W> as Widget>::type_defaults, lumen_static::style::Fields::of(&[lumen_static::style::Fields::VISIBLE]));
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

/// Upper bound on `1 + W::NODE_COUNT` for one [`Popup`].
pub const MAX_POPUP_NODES: usize = 256;

impl<W: Widget> Node for Popup<W> {
    lumen_static::node_count!();
    fn defaults(&self) -> Style {
        <Self as Widget>::type_defaults()
    }

    fn kind_name(&self) -> &'static str {
        "popup"
    }

    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        if !self.open || !style[i].visible {
            // Closed is exactly a hidden subtree: zero size, zero area,
            // no hit, no paint — the machinery `Style::visible` already
            // has, reused rather than re-implemented.
            self.measured = Size::default();
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }

        // Measured **loosely**, not against the parent's remaining
        // space: a popup is out of flow, so the space its siblings left
        // is not the space it has. It gets the whole viewport's width as
        // an upper bound and its natural height.
        //
        // **At most the window's height** (M57/A18): content taller than
        // the screen is given the screen, and a list inside scrolls
        // rather than running off the bottom.
        //
        // **And the window's width, not the parent's** (M57/A18): a tip
        // on a 30 px button was given 30 px and wrapped into a column 174
        // px tall. Out of flow means the parent's width is not its limit.
        self.viewport = cx.viewport;
        let max_w = if cx.viewport.w.is_finite() { cx.viewport.w } else { limits.max_w };
        let loose = Constraints { min_w: 0.0, max_w, min_h: 0.0, max_h: cx.viewport.h };
        self.measured = self.content.layout(cx, loose, style, st, i + Self::AT);

        // Zero in flow. The parent lays its siblings out as if this were
        // not here, which is what "out of flow" means and what stops a
        // dropdown from pushing the rest of a form down the screen.
        st[i].size = Size::default();
        st[i].constraints = limits;
        Size::default()
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        if !self.open {
            return;
        }
        // Relative to the anchor — this node's own position, which its
        // parent has just assigned from the flow it is not part of.
        let above = at.y - self.above_by - self.measured.h;
        let mut to = match self.anchor {
            Anchor::Below => Point { x: at.x, y: at.y },
            Anchor::Above => Point { x: at.x, y: above },
            Anchor::Over => at,
            Anchor::WindowBottom if self.viewport.w.is_finite() && self.viewport.h.is_finite() => Point {
                x: (self.viewport.w - self.measured.w) / 2.0,
                y: self.viewport.h - self.measured.h - 24.0,
            },
            Anchor::WindowBottom => at,
        };
        // **Kept on the window** — M57/A18. `Below` flips above its
        // trigger when it would cross the bottom edge and there is more
        // room above; then both axes are clamped inside. Before this a
        // tooltip by the right edge ran off it and a list near the bottom
        // ran off the screen (`docs/widget-audit.md`, Tooltip).
        let (vw, vh) = (self.viewport.w, self.viewport.h);
        if vh.is_finite() && matches!(self.anchor, Anchor::Below) && to.y + self.measured.h > vh {
            let room_below = vh - at.y;
            let room_above = at.y - self.above_by;
            if room_above > room_below {
                to.y = above;
            }
        }
        if vw.is_finite() {
            to.x = to.x.min(vw - self.measured.w).max(0.0);
        }
        if vh.is_finite() {
            to.y = to.y.min(vh - self.measured.h).max(0.0);
        }
        self.content.place(to, style, st, i + Self::AT);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if !self.open {
            return;
        }
        // Phase 1 records it and returns; phase 2 (or a painter with no
        // overlay sink) is told `false` and paints here.
        if out.defer(i as u32) {
            return;
        }
        self.content.paint(st, i + Self::AT, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        if !self.open {
            return None;
        }
        // The popup itself has no area — it is zero-sized in flow — so
        // only its content can be hit. Not `contains(&st[i], ..)` first,
        // which would reject every point.
        self.content.hit(st, i + Self::AT, x, y)
    }

    fn dismiss(&mut self) -> bool {
        core::mem::replace(&mut self.open, false)
    }

    /// Out of flow: zero-sized itself, its content placed at the anchor
    /// (M55; `W1003` exempts it).
    fn floats_children(&self) -> bool {
        true
    }

    /// Closed is on purpose, not a squeeze — see `Node::content_absent`.
    fn content_absent(&self) -> bool {
        !self.open
    }

    fn dismissal_consumes_press(&self) -> bool {
        !self.passive
    }

    fn is_modal(&self) -> bool {
        self.modal
    }

    /// **A window-anchored popup has no outside** — M57/B34. It hangs
    /// from nothing, so a press elsewhere is not a press "outside" it:
    /// the showcase's Save button dismissed the toast it had just asked
    /// for, in the same press. Escape still closes it.
    fn dismissed_by_outside_press(&self) -> bool {
        !matches!(self.anchor, Anchor::WindowBottom)
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

impl<W: Widget + lumen_static::event::Handlers<S>, S> lumen_static::event::Handlers<S> for Popup<W> {
    fn handler(&self, sel: lumen_static::event::NodeSel) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.content.handler(lumen_static::event::NodeSel { i: sel.i - Self::AT, ..sel })
    }
}
