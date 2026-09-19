//! M27/T27.4 — the three widgets that are mostly *a box with a look*:
//! `Card`, `Toast` and `Tooltip`.
//!
//! # Why a `Card` type at all
//!
//! A derived container already paints a shadow, a fill, a radius and —
//! since M27/T27.1 — a border, all from its style. So a card *is* a
//! styled container, and the fair question is what a type adds.
//!
//! It adds `type_defaults`. `docs/plan-widgets-gallery.md` §3b records
//! what happens without them: a fully transparent control has nothing
//! for `pressed_dim` to scale, so "clicking does nothing" was a third of
//! the showcase's reported defects. A `Card<W>` looks like a card with no
//! stylesheet at all, which is the difference between a widget set and a
//! naming convention.
//!
//! It is also the wrapper shape this crate had none of: every widget in
//! tranche 1 was a leaf. `Card<W>` merges `W`'s table into its own at
//! compile time, the way `Glass<W>` and `ScrollView<W>` do.

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::text::TextStorage;
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

/// Upper bound on `1 + W::NODE_COUNT` for one [`Card`] or [`Tooltip`].
///
/// The same bound and the same reasoning as [`lumen_static::glass::MAX_GLASS_NODES`]:
/// `SUB_TABLE` is a slice, so its length has to be a value, and the
/// array it slices has to be sized at compile time.
pub const MAX_SURFACE_NODES: usize = 256;

// ---- Card ------------------------------------------------------------

/// A raised panel: padding, a radius, a shadow, and whatever is on it.
pub struct Card<W: Widget> {
    pub content: W,
}

impl<W: Widget + Default> Default for Card<W> {
    fn default() -> Self {
        Card { content: W::default() }
    }
}

impl<W: Widget> Card<W> {
    /// Content index, relative to this node.
    const AT: usize = 1;

    pub fn new(content: W) -> Self {
        Card { content }
    }
}

pub struct CardTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> CardTable<W> {
    const ARRAY: [NodeDesc; MAX_SURFACE_NODES] = {
        assert!(W::NODE_COUNT < MAX_SURFACE_NODES, "Card content exceeds MAX_SURFACE_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SURFACE_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_SURFACE_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SURFACE_NODES];
        s[0] = Seed::new(<Card<W> as Widget>::KIND, <Card<W> as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::RADIUS,
                Fields::SHADOW,
                Fields::CURSOR,
                Fields::VISIBLE,
            ]),);
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

impl<W: Widget> Widget for Card<W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// A card is as rigid as what is on it — `Glass`'s answer, for the
    /// same reason: its height is `Shrink`, so taller content makes a
    /// taller card and a content change *does* need the relayout.
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = CardTable::<W>::SLICE;
    const KIND: &'static str = "card";
    const SEEDS: &'static [Seed] = CardTable::<W>::SEED_SLICE;

    /// **A visible surface, not a transparent one.** `Style::default`
    /// has a transparent background, and a card that has to be styled
    /// before it looks like anything is a container with a different
    /// name. The palette is not reachable from a `const fn`, so this is
    /// a neutral light surface; a sheet overrides it with a role.
    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Shrink,
            padding: Edges::all(16.0),
            background: Color(0xff, 0xff, 0xff, 0xff),
            radius: 12.0,
            shadow: 8.0,
            ..Style::default()
        }
    }
}

impl<W: Widget> Node for Card<W> {
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
        let c = self.content.layout(cx, inner, style, st, i + Self::AT);
        let content =
            Size { w: c.w + me.padding.horizontal(), h: c.h + me.padding.vertical() };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        let p = style[i].padding;
        self.content.place(
            Point { x: origin.x + p.left, y: origin.y + p.top },
            style,
            st,
            i + Self::AT,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if st[i].size.w <= 0.0 || st[i].size.h <= 0.0 {
            return;
        }
        let r = lumen_static::ops::rect_of(&st[i]);
        if st[i].shadow > 0.0 {
            out.shadow(r, st[i].radius, st[i].shadow);
        }
        out.rounded_rect(r, st[i].bg, st[i].radius);
        lumen_static::ops::border(st, i, out);
        self.content.paint(st, i + Self::AT, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        // Content first, then the card itself: a card *is* a painted
        // surface, so it claims what its content does not — unlike a
        // transparent container, which must not.
        // Content first: what is *on* the card is above the card.
        self.content.hit(st, i + Self::AT, x, y).or(Some(lumen_static::Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        }))
    }

    fn node(&self, k: usize) -> &dyn Node {
        if k == i_zero() {
            self
        } else {
            self.content.node(k - Self::AT)
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        if k == i_zero() {
            self
        } else {
            self.content.node_mut(k - Self::AT)
        }
    }

}

/// `0`, named. `node(k)` compares against the card's own index within
/// its merged table, which is always zero — spelled out because
/// `k == 0` beside `k - Self::AT` reads like a typo for `k == Self::AT`.
const fn i_zero() -> usize {
    0
}

// ---- Toast -----------------------------------------------------------

/// A transient message: text in a raised pill.
///
/// **A leaf, and it does not own its own lifetime.** A toast that
/// dismissed itself would need a clock, and there is no per-node tick
/// here — the same reason `Spinner` does not drive its own phase. The
/// application decides when it is up, through `Style::visible` or by not
/// building it; this draws one.
pub struct Toast<S: TextStorage = &'static str> {
    pub text: S,
}

impl<S: TextStorage + Default> Default for Toast<S> {
    fn default() -> Self {
        Toast { text: S::default() }
    }
}

impl<S: TextStorage> Toast<S> {
    pub fn new(text: S) -> Self {
        Toast { text }
    }
}

impl<S: TextStorage> Widget for Toast<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "toast";
    const SEEDS: &'static [Seed] = &[Seed::new(
        <Toast<S> as Widget>::KIND,
        <Toast<S> as Widget>::type_defaults, Fields::of(&[
        Fields::WIDTH,
        Fields::HEIGHT,
        Fields::PADDING,
        Fields::BACKGROUND,
        Fields::BORDER,
        Fields::COLOR,
        Fields::RADIUS,
        Fields::SHADOW,
        Fields::FONT_SIZE,
        Fields::FONT_WEIGHT,
        Fields::VISIBLE,
    ]))];

    /// Dark on light, which is the convention every toolkit shares and
    /// the inverse of everything else in this crate — a toast is a
    /// notification, not a surface.
    fn type_defaults() -> Style {
        Style {
            width: Length::Shrink,
            height: Length::Shrink,
            padding: Edges { left: 16.0, right: 16.0, top: 10.0, bottom: 10.0 },
            background: Color(0x26, 0x2c, 0x3a, 0xff),
            color: Color(0xf5, 0xf7, 0xfa, 0xff),
            radius: 8.0,
            shadow: 10.0,
            font_size: 14.0,
            ..Style::default()
        }
    }
}

impl<S: TextStorage> Node for Toast<S> {
    // `kind_name` and `defaults` come from `leaf_common!` below, which
    // forwards them to `Widget::KIND` and `Widget::type_defaults` — the
    // agreement `crates/lumen-static/tests/seeds.rs` holds every leaf to.
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
        let t = cx.text.measure_weighted(
            self.text.as_ref(),
            me.font_size,
            me.font_weight,
            limits.max_w,
        );
        let content = Size { w: t.w + me.padding.horizontal(), h: t.h + me.padding.vertical() };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if st[i].size.w <= 0.0 || st[i].size.h <= 0.0 {
            return;
        }
        let r = lumen_static::ops::rect_of(&st[i]);
        if st[i].shadow > 0.0 {
            out.shadow(r, st[i].radius, st[i].shadow);
        }
        out.rounded_rect(r, st[i].bg, st[i].radius);
        lumen_static::ops::border(st, i, out);
        let p = st[i].pad;
        out.text_weighted(
            [r[0] + p.0, r[1] + p.1, r[2] - p.2, r[3] - p.3],
            self.text.as_ref(),
            st[i].font_size,
            st[i].font_weight,
            st[i].fg,
        );
    }

    lumen_static::leaf_common!(Toast<S>, 0);

}

// ---- Tooltip ---------------------------------------------------------

/// Part ids, so a composite's insides can be reached.
///
/// The same argument `Select` makes (`docs/plan-select.md`): the derive
/// names a *field*, so a widget that is several nodes has to publish its
/// own parts or nothing can address them — not a stylesheet, not the
/// agent's `#id` selectors, not a drive script. Namespaced, because a
/// bare `tip` would collide with an application's own id and style both.
pub const PART_TIP: &str = "tooltip-tip";
/// The popup wrapper around the tip.
pub const PART_TIP_POPUP: &str = "tooltip-popup";
/// The wrapped content, so a sheet can reach the control being explained.
///
/// **A known limitation, made visible rather than hidden.** A part id
/// lives on the *type's* table, so two `Tooltip`s in one tree carry this
/// id twice and `W1011` reports both as unaddressable — which is exactly
/// what it is for, and is the same root cause as the showcase's
/// `PairCheckA`/`PairCheckB` split (M27/T27.3). `Select` has had the
/// same property since M24 and nothing said so. The real answer is
/// instance-scoped addressing, which is `docs/plan-widget-parity.md`
/// T27.0's third landing; until then a second one is a lint finding, not
/// a silent wrong answer.
pub const PART_CONTENT: &str = "tooltip-content";

/// Content, plus a tip that appears while the pointer is over it.
///
/// # Why the tip opens in `layout`
///
/// `dispatch` records `hovered` on the **deepest** node it hits, so for
/// `Tooltip<Button>` the *button* carries the flag and this node never
/// does. Reading the subtree in `layout` is `Glass`'s answer to exactly
/// the same problem (M22/T22.4): `layout` is the one method holding both
/// `&mut self` and the state array, and `dispatch` has already run this
/// pump, so the flag it reads is this frame's.
pub struct Tooltip<W: Widget, S: TextStorage = &'static str> {
    pub content: W,
    pub tip: lumen_static::popup::Popup<Toast<S>>,
}

impl<W: Widget + Default, S: TextStorage + Default> Default for Tooltip<W, S> {
    fn default() -> Self {
        Tooltip {
            content: W::default(),
            tip: lumen_static::popup::Popup::default(),
        }
    }
}

impl<W: Widget, S: TextStorage> Tooltip<W, S> {
    /// Content index, and the tip's, relative to this node.
    const AT: usize = 1;
    const TIP: usize = 1 + W::NODE_COUNT;
    /// Gap between the control and its tip.
    const GAP: f32 = 6.0;

    pub fn new(content: W, text: S) -> Self {
        let mut tip = lumen_static::popup::Popup::<Toast<S>>::new(Toast::new(text));
        tip.open = false;
        Tooltip { content, tip }
    }

    /// Whether the tip is showing.
    pub fn is_open(&self) -> bool {
        self.tip.open
    }
}

pub struct TooltipTable<W, S>(core::marker::PhantomData<(W, S)>);

impl<W: Widget, S: TextStorage> TooltipTable<W, S> {
    const N: usize =
        1 + W::NODE_COUNT + <lumen_static::popup::Popup<Toast<S>> as Widget>::NODE_COUNT;

    const ARRAY: [NodeDesc; MAX_SURFACE_NODES] = {
        assert!(Self::N <= MAX_SURFACE_NODES, "Tooltip exceeds MAX_SURFACE_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SURFACE_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        let tip_at = 1 + W::NODE_COUNT;
        lumen_static::widget::splice_child(
            &mut t,
            tip_at,
            <lumen_static::popup::Popup<Toast<S>> as Widget>::SUB_TABLE,
            0,
        );
        t[1].next_sibling = tip_at as u32;
        t[1] = t[1].with_id(PART_CONTENT);
        t[tip_at] = t[tip_at].with_id(PART_TIP_POPUP);
        t[tip_at + 1] = t[tip_at + 1].with_id(PART_TIP);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(Self::N).0;

    const SEED_ARRAY: [Seed; MAX_SURFACE_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SURFACE_NODES];
        s[0] = Seed::new(
            <Tooltip<W, S> as Widget>::KIND,
            <Tooltip<W, S> as Widget>::type_defaults, Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::VISIBLE]));
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        lumen_static::widget::splice_seeds(
            &mut s,
            1 + W::NODE_COUNT,
            <lumen_static::popup::Popup<Toast<S>> as Widget>::SEEDS,
        );
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

impl<W: Widget, S: TextStorage> Widget for Tooltip<W, S> {
    const NODE_COUNT: usize = TooltipTable::<W, S>::N;
    const DEPTH: usize =
        1 + max_of(W::DEPTH, <lumen_static::popup::Popup<Toast<S>> as Widget>::DEPTH);
    /// As rigid as its content: the tip is out of flow and cannot change
    /// this node's size.
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = TooltipTable::<W, S>::SLICE;
    const KIND: &'static str = "tooltip";
    const SEEDS: &'static [Seed] = TooltipTable::<W, S>::SEED_SLICE;

    /// **Shrink on both axes, unlike every other container here.** A
    /// tooltip is a transparent wrapper around one control; making it
    /// `FILL` would stretch the control it wraps and change a layout
    /// merely by adding a tip to it.
    fn type_defaults() -> Style {
        Style { width: Length::Shrink, height: Length::Shrink, ..Style::default() }
    }
}

const fn max_of(a: usize, b: usize) -> usize {
    if a > b {
        a
    } else {
        b
    }
}

impl<W: Widget, S: TextStorage> Node for Tooltip<W, S> {
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
        // **The whole content subtree, not this node.** `dispatch` marks
        // the deepest node it hit, so for `Tooltip<Button>` the button
        // carries `hovered` and this node never does. Checking `st[i]`
        // would be a tip that never appears — and a test asserting "the
        // tip opened" against a bare `Tooltip<Label>` would still pass,
        // because a label *is* the deepest node.
        let hovered = (i + Self::AT..i + Self::TIP).any(|k| st[k].hovered);
        self.tip.open = hovered;

        let c = self.content.layout(cx, limits, style, st, i + Self::AT);
        // The tip measures against the same limits and is placed out of
        // flow; it contributes nothing to this node's size.
        self.tip.layout(cx, limits, style, st, i + Self::TIP);
        lumen_static::ops::record_painted(st, i, limits, c, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.content.place(origin, style, st, i + Self::AT);
        // **Below the content, not over it.** `Anchor::Below` places the
        // tip *at* the point it is given, so the caller passes the point
        // below the control — which is what `Select` does with its
        // trigger's bottom edge.
        //
        // This is not cosmetic. A tip drawn over its own control is an
        // **overlay**, and the overlay pass hit-tests before
        // `Tooltip::hit` ever runs — so the tip took every click on the
        // button it was explaining, and `Tooltip::hit` declining to
        // offer it made no difference at all. The headless test could
        // not see this (it calls `Node::hit` directly, below the overlay
        // pass); `just widget-drive` reported the press counter not
        // moving, which is the same defect from the outside.
        let below = Point {
            x: origin.x,
            y: origin.y + st[i + Self::AT].size.h + Self::GAP,
        };
        self.tip.place(below, style, st, i + Self::TIP);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.content.paint(st, i + Self::AT, out);
        // The tip defers itself into the overlay phase; painting it here
        // is what makes that offer.
        self.tip.paint(st, i + Self::TIP, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        // **The tip is not a hit target.** A tooltip that swallowed the
        // pointer would close itself the instant it opened, because the
        // content underneath would stop being hovered. `Spacer`'s
        // `nohit` exists for the same reason and the same bug.
        self.content.hit(st, i + Self::AT, x, y)
    }

    fn node(&self, k: usize) -> &dyn Node {
        match k {
            0 => self,
            _ if k < Self::TIP => self.content.node(k - Self::AT),
            _ => self.tip.node(k - Self::TIP),
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        match k {
            0 => self,
            _ if k < Self::TIP => self.content.node_mut(k - Self::AT),
            _ => self.tip.node_mut(k - Self::TIP),
        }
    }
}

// ---- handler routing -------------------------------------------------
//
// **The half that makes a wrapper usable rather than merely visible.**
// `Dyn` shipped without it: every button, checkbox and field inside an
// island could be hit-tested, hovered and pressed, and no handler ever
// ran, because the application type is exactly what the erasure erased.
// Nothing that stopped at "the demo paints" could see it, and the first
// round of tests stopped there (`docs/plan-widgets-gallery.md` §3).
//
// A wrapper is not a control, so node 0 answers `None` and everything
// else is the content's, re-based.

/// A card is a surface; its content owns every handler.
impl<W, S> lumen_static::event::Handlers<S> for Card<W>
where
    W: Widget + lumen_static::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.content
            .handler(lumen_static::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}

/// A toast is a leaf with no handler of its own.
impl<S2, S: TextStorage> lumen_static::event::Handlers<S2> for Toast<S> {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S2, &lumen_static::event::EventCtx)> {
        None
    }
}

/// A tooltip routes to its content. The tip is not a hit target
/// (see `Node::hit`), so it can carry none.
impl<W, S, A> lumen_static::event::Handlers<A> for Tooltip<W, S>
where
    W: Widget + lumen_static::event::Handlers<A>,
    S: TextStorage,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut A, &lumen_static::event::EventCtx)> {
        if sel.i == 0 || sel.i >= Self::TIP {
            return None;
        }
        self.content
            .handler(lumen_static::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}
