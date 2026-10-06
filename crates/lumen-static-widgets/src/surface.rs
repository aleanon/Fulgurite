//! M27/T27.4 — the three widgets that are mostly *a box with a look*:
//! `Card`, `Toast` and `Tooltip` — and M85/T85.2's `Pin`, the same
//! one-child splice with an offset and a clip instead of a look, and
//! M85/T85.4's `Pop`, the same splice with a visibility flag instead.
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
/// The same bound and the same reasoning as [`crate::glass::MAX_GLASS_NODES`]:
/// `SUB_TABLE` is a slice, so its length has to be a value, and the
/// array it slices has to be sized at compile time.
pub const MAX_SURFACE_NODES: usize = 256;

// ---- Card ------------------------------------------------------------

/// A raised panel: padding, a radius, a shadow, and whatever is on it.
pub struct Card<W: Widget> {
    pub content: W,
    /// How far it stands off the page — M57/B33. `None` is the style's
    /// `shadow` (8 by default), so a stylesheet stays in charge; `Some(0)`
    /// is flat and outlined in `Role::Border`, and `Some(1..=3)` are
    /// [`ELEVATION`]'s shadows. A card had one look, and a dark page shows
    /// a shadow barely at all: level 0's outline is what separates it
    /// there.
    pub elevation: Option<u8>,
}

/// Shadow spread per elevation level, 0..=3 (M57/B33).
pub const ELEVATION: [f32; 4] = [0.0, 4.0, 8.0, 16.0];

impl<W: Widget + Default> Default for Card<W> {
    fn default() -> Self {
        Card { content: W::default(), elevation: None }
    }
}

impl<W: Widget> Card<W> {
    /// Content index, relative to this node.
    const AT: usize = 1;

    pub fn new(content: W) -> Self {
        Card { content, elevation: None }
    }

    /// The same card at elevation `level` (clamped to 3).
    pub fn with_elevation(mut self, level: u8) -> Self {
        self.elevation = Some(level.min(3));
        self
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
                Fields::BACKGROUND_FILL,
                Fields::RADIUS,
                Fields::SHADOW,
                Fields::CURSOR,
                Fields::VISIBLE,
            ]),).with_roles(
                // The border role also colours elevation 0's outline (M57/B33).
                lumen_static::style::SeedRoles::NONE.background(lumen_static::style::Role::Raised).border(lumen_static::style::Role::Border),
            );
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
    lumen_static::node_count!();
    /// A group: the card announces its content, which describes itself —
    /// M48/T48.6. `Card` has no title of its own to label it with.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::Group);
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
        let inner = lumen_static::ops::inner_limits(lumen_static::ops::within_own_size(limits, &me), me.padding);
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
        let shadow = match self.elevation {
            Some(k) => ELEVATION[(k as usize).min(3)],
            None => st[i].shadow,
        };
        if shadow > 0.0 {
            out.shadow(r, st[i].radius, shadow);
        }
        lumen_static::ops::fill(st, i, out);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        lumen_static::ops::border(st, i, out);
        // Level 0: flat, so an outline says where it is (unless the style
        // already draws one).
        if self.elevation == Some(0) && st[i].border_width == 0 {
            out.stroked_rect(r, st[i].border_color, st[i].radius, 1.0);
        }
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

// ---- Pin -------------------------------------------------------------

/// **One child at an offset inside the pin's own box** — M85/T85.2,
/// iced's `pin`.
///
/// The child is laid out loose (it may shrink to its content) inside the
/// pin's box less the offset, and placed at the pin's position plus
/// `(x, y)`. The pin fills its parent by default, and clips what it holds
/// to its own rect in `paint` and in `hit` alike, so a child pinned past
/// the edge is neither drawn nor pressable there — paint and hit agree
/// (`docs/text-and-scrolling-decisions.md` A1), and it answers
/// [`Node::clips_children`] so the audit agrees too.
///
/// **Not out-of-flow positioning**, which `docs/plan-capabilities.md`
/// rejects: the pin takes its place in the flow like any container, and
/// the offset moves the child only within it. What sits over another
/// widget is a `stack` with a `Pin` in it.
///
/// It draws nothing of its own and claims no press: a press inside the
/// pin but outside its child belongs to whatever is under the pin.
pub struct Pin<W: Widget> {
    pub content: W,
    /// The child's offset from the pin's left edge, px.
    pub x: f32,
    /// The child's offset from the pin's top edge, px.
    pub y: f32,
}

impl<W: Widget + Default> Default for Pin<W> {
    fn default() -> Self {
        Pin { content: W::default(), x: 0.0, y: 0.0 }
    }
}

impl<W: Widget> Pin<W> {
    /// Content index, relative to this node.
    const AT: usize = 1;

    /// `content` pinned at `(x, y)` from the pin's top-left corner.
    pub fn new(content: W, x: f32, y: f32) -> Self {
        Pin { content, x, y }
    }
}

pub struct PinTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> PinTable<W> {
    const ARRAY: [NodeDesc; MAX_SURFACE_NODES] = {
        assert!(W::NODE_COUNT < MAX_SURFACE_NODES, "Pin content exceeds MAX_SURFACE_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SURFACE_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_SURFACE_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SURFACE_NODES];
        // A pin paints nothing of its own, so its size is all it reads.
        s[0] = Seed::new(
            <Pin<W> as Widget>::KIND,
            <Pin<W> as Widget>::type_defaults,
            Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::VISIBLE]),
        );
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

impl<W: Widget> Widget for Pin<W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// As rigid as its content, `Card`'s answer: a `Shrink` pin is its
    /// content plus the offset.
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = PinTable::<W>::SLICE;
    const KIND: &'static str = "pin";
    const SEEDS: &'static [Seed] = PinTable::<W>::SEED_SLICE;

    /// Fills its parent on both axes: the box the child is pinned in.
    fn type_defaults() -> Style {
        Style { width: Length::FILL, height: Length::FILL, ..Style::default() }
    }
}

impl<W: Widget> Node for Pin<W> {
    lumen_static::node_count!();

    /// Its child is clipped to its rect, in paint and in hit
    /// (M55/T55.6's question, answered for the audit).
    fn clips_children(&self) -> bool {
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
            return lumen_static::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        // Inside its own size less the offset, loose: the maximum is
        // inherited and the minimum is not, so the child may shrink to
        // its content (M27/T27.5's wrapper rule).
        let own = lumen_static::ops::within_own_size(limits, &me);
        let inner = Constraints {
            min_w: 0.0,
            max_w: (own.max_w - self.x).max(0.0),
            min_h: 0.0,
            max_h: (own.max_h - self.y).max(0.0),
        };
        let c = self.content.layout(cx, inner, style, st, i + Self::AT);
        let content = Size { w: (c.w + self.x).max(0.0), h: (c.h + self.y).max(0.0) };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.content.place(
            Point { x: origin.x + self.x, y: origin.y + self.y },
            style,
            st,
            i + Self::AT,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if st[i].size.w <= 0.0 || st[i].size.h <= 0.0 {
            return;
        }
        out.push_clip(lumen_static::ops::rect_of(&st[i]));
        self.content.paint(st, i + Self::AT, out);
        out.pop_clip();
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        // The clip's other half: outside the pin is outside its child,
        // whatever the child's own rect says.
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        // Transparent: what the child does not take is not the pin's.
        self.content.hit(st, i + Self::AT, x, y)
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

// ---- Pop -------------------------------------------------------------

/// **One child that is told when it enters or leaves the view** —
/// M85/T85.4, iced's `pop` (its sensor).
///
/// The pop's own table entry carries `NodeDesc::WATCHES_VIEW`, so a
/// `Host` lists it once at construction and, after every `place`,
/// decides whether it is in view: not gone, and its rect meets the window
/// and every ancestor that clips (a `ScrollView`'s viewport, so content
/// below the fold is *not* in view however much of the window it is in).
/// A change reaches the pop on the next pump as `Event::Visibility` —
/// which it records, so [`Pop::visible`] answers — and then runs
/// [`Pop::on_view`] with the event as `ctx.event`. That handler is asked
/// with `NodeSel::visibility`, a sentinel no activation sends, so a click
/// on the content can never run it, and it never answers a click.
///
/// The write reaches the screen **one frame after** the frame that placed
/// the pop in view, because the frame that finds the change is already
/// painted; the host's deadline answers `Some(0.0)` while one waits, so a
/// waiting window asks for that frame at once
/// (`crates/lumen-static-shell/tests/pop_visibility.rs`).
///
/// A lazily loaded image is the shape it was built for: the pop holds a
/// placeholder of the image's size, and `on_view` starts the load.
///
/// **Not inside a `Repeat`, a `List` or a `Dyn`**: their instances live
/// in an arena the host's pass never sees, so each refuses one at
/// construction (`lumen_static::widget::table_has_flag`). In a repeater
/// the signal is its window range, `first_row()..first_row() +
/// window_len()`.
///
/// It draws nothing of its own and claims no press: its box is its
/// content's, and a press on it is its content's or nobody's.
pub struct Pop<S: 'static, W: Widget> {
    pub content: W,
    /// Run when the pop enters or leaves the view; `ctx.event` is
    /// `Event::Visibility { visible }`. A no-op by default.
    pub on_view: fn(&mut S, &lumen_static::event::EventCtx),
    visible: bool,
}

impl<S: 'static, W: Widget + Default> Default for Pop<S, W> {
    fn default() -> Self {
        Pop { content: W::default(), on_view: crate::ops::ignore_click::<S>, visible: false }
    }
}

impl<S: 'static, W: Widget> Pop<S, W> {
    /// Content index, relative to this node.
    const AT: usize = 1;

    /// `content`, with `on_view` run as it enters and leaves the view.
    pub fn new(content: W, on_view: fn(&mut S, &lumen_static::event::EventCtx)) -> Self {
        Pop { content, on_view, visible: false }
    }

    /// Whether the last visibility the host delivered was "in view".
    /// `false` until the first delivery: a pop never shown is never told.
    pub fn visible(&self) -> bool {
        self.visible
    }
}

pub struct PopTable<S, W>(core::marker::PhantomData<(S, W)>);

impl<S: 'static, W: Widget> PopTable<S, W> {
    const ARRAY: [NodeDesc; MAX_SURFACE_NODES] = {
        assert!(W::NODE_COUNT < MAX_SURFACE_NODES, "Pop content exceeds MAX_SURFACE_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SURFACE_NODES];
        // The flag on its own entry is the whole registration: the host
        // scans the table once for it (M85/T85.3).
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER | NodeDesc::WATCHES_VIEW);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_SURFACE_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SURFACE_NODES];
        // A pop paints nothing of its own, so its size is all it reads.
        s[0] = Seed::new(
            <Pop<S, W> as Widget>::KIND,
            <Pop<S, W> as Widget>::type_defaults,
            Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::VISIBLE]),
        );
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

impl<S: 'static, W: Widget> Widget for Pop<S, W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    /// Its box is its content's, so it is as rigid as its content.
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = PopTable::<S, W>::SLICE;
    const KIND: &'static str = "pop";
    const SEEDS: &'static [Seed] = PopTable::<S, W>::SEED_SLICE;

    /// Shrinks to its content on both axes: a pop is its content's box.
    fn type_defaults() -> Style {
        Style::default()
    }
}

impl<S: 'static, W: Widget> Node for Pop<S, W> {
    lumen_static::node_count!();

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
        // Inside its own size, loose: the maximum is inherited and the
        // minimum is not, so the content may shrink (M27/T27.5's rule).
        let own = lumen_static::ops::within_own_size(limits, &me);
        let inner = Constraints { min_w: 0.0, max_w: own.max_w, min_h: 0.0, max_h: own.max_h };
        let c = self.content.layout(cx, inner, style, st, i + Self::AT);
        let s = lumen_static::ops::resolve_self(limits, &me, c);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.content.place(origin, style, st, i + Self::AT);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.content.paint(st, i + Self::AT, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        // Transparent: what the content does not take is not the pop's.
        self.content.hit(st, i + Self::AT, x, y)
    }

    /// Records what the host told it, and consumes it: the event is the
    /// pop's alone, and its handler runs next (`pass::tell_visibility`).
    fn on_event(
        &mut self,
        ev: &lumen_static::event::Event,
        _cx: &lumen_static::event::EventCtx,
    ) -> bool {
        if let lumen_static::event::Event::Visibility { visible } = *ev {
            self.visible = visible;
            return true;
        }
        false
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

// ---- Toast -----------------------------------------------------------

/// A transient message: text in a raised pill.
///
/// **A leaf, and it does not own its own lifetime.** A toast that
/// dismissed itself would need a clock, and there is no per-node tick
/// here — the same reason `Spinner` does not drive its own phase. The
/// application decides when it is up, through `Style::visible` or by not
/// building it; this draws one.
///
/// # Showing, timing out and closing — M57/B34
///
/// A toast drew a pill in the flow and could do nothing else
/// (`docs/widget-audit.md`, Toast). It now:
///
/// - **owns whether it is shown**: [`Toast::show`] shows it, optionally
///   for a time, and [`Toast::hide`] hides it; hidden, it lays out as a
///   hidden subtree. Shown by default, which is what a tooltip's tip
///   (`Popup<Toast>`) relies on.
/// - **times out on the application's clock.** Widgets have no clock
///   here, which is why a `Spinner` takes its phase; [`Toast::tick`] is
///   that, for a countdown, and answers whether the toast went away so
///   the application knows to ask for a frame. The plan's alternative, a
///   M28 job returned by `show`, needs the application's timer set and a
///   message type for one widget's countdown.
/// - **can be closed**: with `closable`, a × at its right hides it.
/// - **floats**: put it in a [`crate::popup::Popup`] with
///   [`crate::popup::Anchor::WindowBottom`] to overlay the bottom centre
///   of the window rather than take a place in the flow. Keep the popup's
///   `open` in step with [`Toast::is_shown`], or an empty overlay stays
///   open and takes Escape; the showcase's `refresh` is the three lines.
pub struct Toast<S: TextStorage = &'static str> {
    pub text: S,
    /// Draw a × that hides it.
    pub closable: bool,
    hidden: bool,
    /// Milliseconds left, when it was shown for a time.
    remaining: Option<f32>,
    /// A press began on the ×.
    on_close: bool,
    width: f32,
}

impl<S: TextStorage + Default> Default for Toast<S> {
    fn default() -> Self {
        Toast::new(S::default())
    }
}

impl<S: TextStorage> Toast<S> {
    /// The ×'s column, inside the right padding.
    pub const CLOSE_W: f32 = 20.0;

    pub fn new(text: S) -> Self {
        Toast { text, closable: false, hidden: false, remaining: None, on_close: false, width: 0.0 }
    }

    /// Show it — for `ms` milliseconds, or until hidden when `None`.
    pub fn show(&mut self, ms: Option<f32>) {
        self.hidden = false;
        self.remaining = ms.filter(|m| m.is_finite() && *m > 0.0);
    }

    /// Hide it now. Answers whether it was showing.
    pub fn hide(&mut self) -> bool {
        self.remaining = None;
        !core::mem::replace(&mut self.hidden, true)
    }

    pub fn is_shown(&self) -> bool {
        !self.hidden
    }

    /// Whether a countdown is running — a host should keep ticking.
    pub fn is_counting(&self) -> bool {
        !self.hidden && self.remaining.is_some()
    }

    /// Advance the countdown by `dt_ms`. Answers whether the toast just
    /// went away.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        let Some(left) = self.remaining.as_mut() else { return false };
        *left -= dt_ms.max(0.0);
        if *left <= 0.0 {
            return self.hide();
        }
        false
    }

    fn extra(&self) -> f32 {
        if self.closable { Self::CLOSE_W } else { 0.0 }
    }

    fn on_x(&self, x: f32) -> bool {
        self.closable && x >= self.width - self.extra() - 12.0
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
        Fields::BACKGROUND_FILL,
        Fields::COLOR,
        Fields::RADIUS,
        Fields::SHADOW,
        Fields::FONT_SIZE,
        Fields::FONT_WEIGHT,
        Fields::VISIBLE,
    ])).with_roles(lumen_static::style::SeedRoles::NONE.color(lumen_static::style::Role::Page).background(lumen_static::style::Role::Text))];

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
    // `kind_name` and `defaults` come from `lumen_static::leaf_common!` below, which
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
        if !me.visible || self.hidden {
            return lumen_static::ops::hide_subtree(st, i, limits, 1);
        }
        let t = cx.text.measure_weighted(
            self.text.as_ref(),
            me.font_size,
            me.font_weight,
            limits.max_w,
        );
        let content = Size { w: t.w + me.padding.horizontal() + self.extra(), h: t.h + me.padding.vertical() };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        self.width = s.w;
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
        lumen_static::ops::fill(st, i, out);
        out.rounded_rect(r, st[i].bg, st[i].radius);
        lumen_static::ops::border(st, i, out);
        let p = st[i].pad;
        out.text_weighted(
            [r[0] + p.0, r[1] + p.1, r[2] - p.2 - self.extra(), r[3] - p.3],
            self.text.as_ref(),
            st[i].font_size,
            st[i].font_weight,
            st[i].fg,
        );
        if self.closable {
            let (cx, cy, d) = (r[2] - p.2 - Self::CLOSE_W / 2.0 + 2.0, (r[1] + r[3]) / 2.0, 4.0);
            let c = st[i].fg;
            out.line(Point { x: cx - d, y: cy - d }, Point { x: cx + d, y: cy + d }, 1.5, c);
            out.line(Point { x: cx - d, y: cy + d }, Point { x: cx + d, y: cy - d }, 1.5, c);
        }
    }

    /// A press and release on the × hide it; anything else is declined,
    /// so a tip or a toast without one takes nothing from what is under.
    fn on_event(&mut self, ev: &lumen_static::Event, cx: &lumen_static::EventCtx) -> bool {
        match ev {
            lumen_static::Event::PointerDown { .. } => {
                self.on_close = self.on_x(cx.local.x);
                self.on_close
            }
            lumen_static::Event::PointerUp { .. } => {
                let hit = core::mem::take(&mut self.on_close) && self.on_x(cx.local.x);
                hit && self.hide()
            }
            _ => false,
        }
    }

    /// A status message, with its text — M48/T48.6.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::Status);
        node.set_value(self.text.as_ref());
        // Announced when it appears, without taking focus — M54/T54.3.
        node.set_live(lumen_static::observe::accesskit::Live::Polite);
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
    pub tip: crate::popup::Popup<Toast<S>>,
    /// The pointer has rested on the content (`Event::Linger`) — M57/B35.
    lingered: bool,
    /// Keyboard focus is on the content: the tip shows for it too, which a
    /// keyboard user had no way to see (M57/B35).
    focused: bool,
    /// Closed by a press or by Escape, until the pointer leaves: the tip
    /// re-opened on every hovered frame, so neither closed it (M57/B35).
    suppressed: bool,
    /// Whether the last layout showed the tip, to see it closed from
    /// outside — the pass's Escape dismisses the popup directly.
    shown: bool,
}

impl<W: Widget + Default, S: TextStorage + Default> Default for Tooltip<W, S> {
    fn default() -> Self {
        let mut tip = crate::popup::Popup::default();
        tip.passive = true;
        Tooltip { content: W::default(), tip, lingered: false, focused: false, suppressed: false, shown: false }
    }
}

impl<W: Widget, S: TextStorage> Tooltip<W, S> {
    /// Content index, and the tip's, relative to this node.
    const AT: usize = 1;
    const TIP: usize = 1 + W::NODE_COUNT;
    /// Gap between the control and its tip.
    const GAP: f32 = 6.0;

    pub fn new(content: W, text: S) -> Self {
        let mut tip = crate::popup::Popup::<Toast<S>>::new(Toast::new(text));
        tip.open = false;
        // Passive: a press on the content closes the tip and still
        // reaches the content (M33/T33.3).
        tip.passive = true;
        Tooltip { content, tip, lingered: false, focused: false, suppressed: false, shown: false }
    }

    /// Whether the tip is showing.
    pub fn is_open(&self) -> bool {
        self.tip.open
    }
}

pub struct TooltipTable<W, S>(core::marker::PhantomData<(W, S)>);

impl<W: Widget, S: TextStorage> TooltipTable<W, S> {
    const N: usize =
        1 + W::NODE_COUNT + <crate::popup::Popup<Toast<S>> as Widget>::NODE_COUNT;

    const ARRAY: [NodeDesc; MAX_SURFACE_NODES] = {
        assert!(Self::N <= MAX_SURFACE_NODES, "Tooltip exceeds MAX_SURFACE_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SURFACE_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        let tip_at = 1 + W::NODE_COUNT;
        lumen_static::widget::splice_child(
            &mut t,
            tip_at,
            <crate::popup::Popup<Toast<S>> as Widget>::SUB_TABLE,
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
            <crate::popup::Popup<Toast<S>> as Widget>::SEEDS,
        );
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

impl<W: Widget, S: TextStorage> Widget for Tooltip<W, S> {
    const NODE_COUNT: usize = TooltipTable::<W, S>::N;
    const DEPTH: usize =
        1 + max_of(W::DEPTH, <crate::popup::Popup<Toast<S>> as Widget>::DEPTH);
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
    lumen_static::node_count!();
    /// A group described by its tip's text, whether or not the tip is
    /// showing — what a screen reader reads as the control's hint (M48/T48.6).
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        node.set_role(lumen_static::observe::accesskit::Role::Group);
        node.set_description(self.tip.content.text.as_ref());
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
        // **The whole content subtree, not this node.** `dispatch` marks
        // the deepest node it hit, so for `Tooltip<Button>` the button
        // carries `hovered` and this node never does. Checking `st[i]`
        // would be a tip that never appears — and a test asserting "the
        // tip opened" against a bare `Tooltip<Label>` would still pass,
        // because a label *is* the deepest node.
        let hovered = (i + Self::AT..i + Self::TIP).any(|k| st[k].hovered);
        // **After a rest, or for keyboard focus; not after a press** —
        // M57/B35. This was `open = hovered`: it opened on the first frame
        // the pointer crossed the control, never for keyboard focus, and a
        // press or Escape could not close it, since the next layout
        // re-opened it (its own comment promised a press closed it).
        if self.shown && !self.tip.open {
            // Closed from outside since the last layout: Escape.
            self.suppressed = true;
        }
        if !hovered {
            self.lingered = false;
            if !self.focused {
                self.suppressed = false;
            }
        }
        self.tip.open = ((hovered && self.lingered) || self.focused) && !self.suppressed;
        self.shown = self.tip.open;

        let c = self.content.layout(cx, limits, style, st, i + Self::AT);
        // The tip measures against the same limits and is placed out of
        // flow; it contributes nothing to this node's size.
        // Flipping above clears the control and both gaps (M57/A18).
        self.tip.above_by = c.h + 2.0 * Self::GAP;
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
        // **Centred under the control** (M57/B35), not left-aligned to it;
        // the popup keeps it on the window.
        let below = Point {
            x: origin.x + (st[i + Self::AT].size.w - self.tip.measured().w) / 2.0,
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

    /// Hover intent, focus and presses, from the content — M57/B35. None of
    /// these is *consumed*: the content keeps its own press and focus.
    fn on_event(&mut self, ev: &lumen_static::event::Event, cx: &lumen_static::event::EventCtx) -> bool {
        use lumen_static::event::Event;
        let mine = cx.target != cx.node;
        match ev {
            Event::Linger if mine && !self.lingered => {
                self.lingered = true;
                true
            }
            Event::PointerDown { .. } if mine => {
                self.suppressed = true;
                false
            }
            Event::Focus { gained } if mine => {
                self.focused = *gained;
                if !*gained {
                    self.suppressed = false;
                }
                false
            }
            _ => false,
        }
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

/// A pop's own handler answers only the host's visibility question
/// (`NodeSel::visibility`, a sentinel no click sends); its content owns
/// every other handler.
impl<S: 'static, W> lumen_static::event::Handlers<S> for Pop<S, W>
where
    W: Widget + lumen_static::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &lumen_static::event::EventCtx)> {
        if sel.i == 0 {
            return sel.is_visibility().then_some(self.on_view);
        }
        self.content
            .handler(lumen_static::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}

/// A pin is a box; its content owns every handler.
impl<W, S> lumen_static::event::Handlers<S> for Pin<W>
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
