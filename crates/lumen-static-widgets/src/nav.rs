//! M27/T27.6 — the navigation group.
//!
//! The plan called these "one shape five times: a strip with a selected
//! index". **Three of the five are**, and the other two are not, which
//! is worth saying plainly rather than forcing:
//!
//! * `Tabs`, `BottomNav` and `NavigationRail` really are one widget with
//!   three presentations — a strip of items, one selected, differing in
//!   axis and in how the selection is marked. They are a single type
//!   with a `const` parameter, the way `Chart` is and for the same
//!   reason: `kind_name` and `defaults` then answer at compile time.
//! * **`AppBar` is not a selector.** It is a title and a raised ground;
//!   nothing in it is selected and there is no index. Giving it a
//!   `selected` field to fit the group would be a field that is always
//!   zero and a keyboard that moves it nowhere.
//! * **`PullToRefresh` is not a strip at all.** It is a gesture wrapper:
//!   content, plus a threshold, plus an indicator that appears while the
//!   content is dragged past its top.
//!
//! The grouping in `docs/plan-widget-parity.md` §6 was a guess made from
//! a widget list, and two-fifths of it did not survive contact. That is
//! the tranche table doing its job — it grouped work plausibly, and the
//! work corrected it.

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

/// A horizontal strip, the selection underlined — `Tabs`.
pub const TABS: u8 = 0;
/// A horizontal strip, the selection filled — `BottomNav`.
pub const BOTTOM: u8 = 1;
/// A vertical strip, the selection filled — `NavigationRail`.
pub const RAIL: u8 = 2;

/// The most items one strip can hold.
///
/// Fixed, like `Series` and `Stops`, so a strip allocates nothing and is
/// `Copy`-friendly. Over-long input is truncated rather than refused: a
/// navigation bar is a view of someone else's list, and refusing to draw
/// is worse than drawing the first eight.
pub const MAX_ITEMS: usize = 8;

/// One entry in a strip.
#[derive(Clone, Copy, Default)]
pub struct Item {
    pub label: &'static str,
}

impl Item {
    pub const fn new(label: &'static str) -> Self {
        Item { label }
    }
}

/// A strip of items with one selected.
///
/// **One node.** The items are drawn and hit-tested by arithmetic over a
/// fixed array, the way `Repeat` does for rows — which keeps a five-tab
/// bar at one node instead of six and makes the selection a field rather
/// than a search.
pub struct Strip<const KIND: u8 = TABS> {
    items: [Item; MAX_ITEMS],
    count: usize,
    selected: usize,
    /// The laid-out extent along the strip's own axis, from the last
    /// frame. `on_event` sees `local` and nothing else —
    /// `Slider::track_w` exists for the same reason and says why.
    extent: f32,
    /// Each label's measured width, from the last layout.
    ///
    /// **Measured in `layout` because `paint` cannot measure.** A
    /// `Painter` takes a string and a size and draws; there is no
    /// metric on it. Centring a label in its cell needs its width, so
    /// the width is taken where a `LayoutCx` exists and kept here — the
    /// same trade `Popup::measured` and `ScrollView::measured_overflow`
    /// make, and for the same reason.
    widths: [f32; MAX_ITEMS],
}

/// `Tabs`: a horizontal strip with the selection underlined.
pub type Tabs = Strip<TABS>;
/// `BottomNav`: a horizontal strip with the selection filled.
pub type BottomNav = Strip<BOTTOM>;
/// `NavigationRail`: a vertical strip with the selection filled.
pub type NavigationRail = Strip<RAIL>;

impl<const KIND: u8> Default for Strip<KIND> {
    fn default() -> Self {
        Strip {
            items: [Item::default(); MAX_ITEMS],
            count: 0,
            selected: 0,
            extent: 0.0,
            widths: [0.0; MAX_ITEMS],
        }
    }
}

impl<const KIND: u8> Strip<KIND> {
    /// Build from a slice, truncating past [`MAX_ITEMS`].
    pub fn new(labels: &[&'static str]) -> Self {
        let mut s = Strip::<KIND>::default();
        for (i, l) in labels.iter().take(MAX_ITEMS).enumerate() {
            s.items[i] = Item::new(l);
            s.count = i + 1;
        }
        s
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn label(&self, i: usize) -> &'static str {
        self.items[i.min(MAX_ITEMS - 1)].label
    }

    /// Select `i`, clamped to what exists. Returns whether it moved.
    ///
    /// **Clamped, not ignored.** A caller that computes an index from
    /// data can hand over one past the end, and a strip that then drew
    /// nothing selected would be a navigation bar with no current page.
    pub fn select(&mut self, i: usize) -> bool {
        if self.count == 0 {
            return false;
        }
        let i = i.min(self.count - 1);
        let moved = i != self.selected;
        self.selected = i;
        moved
    }

    /// Whether this strip runs down the screen rather than across it.
    const fn vertical() -> bool {
        KIND == RAIL
    }

    fn item_at(&self, local: Point) -> Option<usize> {
        if self.count == 0 || self.extent <= 0.0 {
            return None;
        }
        let along = if Self::vertical() { local.y } else { local.x };
        let each = self.extent / self.count as f32;
        if each <= 0.0 {
            return None;
        }
        let k = (along / each) as usize;
        Some(k.min(self.count - 1))
    }
}

impl<const KIND: u8> Widget for Strip<KIND> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] =
        &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::FOCUSABLE)];
    const KIND: &'static str = match KIND {
        TABS => "tabs",
        BOTTOM => "bottomnav",
        _ => "navigationrail",
    };
    const SEEDS: &'static [Seed] =
        &[Seed::new(<Strip<KIND> as Widget>::KIND, <Strip<KIND> as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::FONT_WEIGHT,
                Fields::RADIUS,
                Fields::FOCUS_RING,
                Fields::CURSOR,
                Fields::VISIBLE,
            ]))];

    fn type_defaults() -> Style {
        let base = Style {
            color: Color(0x30, 0x3a, 0x4c, 0xff),
            font_size: 14.0,
            cursor: lumen_static::style::Cursor::Pointer,
            ..Style::default()
        };
        match KIND {
            // A rail runs down the side: fixed width, full height.
            //
            // **With a ground**, like the bottom bar. The selected item
            // is a filled pill and its label is drawn in the *style's
            // background*, so a transparent one makes the current item's
            // label invisible — dark text on a dark fill. The render
            // showed it immediately and no test would have: every
            // assertion about the selection was about geometry.
            // `docs/plan-widgets-gallery.md` §3b's ghost button is the
            // same shape: a transparent default takes the widget's own
            // affordance away.
            RAIL => Style {
                width: Length::Fixed(88.0),
                height: Length::FILL,
                background: Color(0xf3, 0xf5, 0xf9, 0xff),
                ..base
            },
            // Tabs and a bottom bar run across: full width, fixed height.
            BOTTOM => Style {
                width: Length::FILL,
                height: Length::Fixed(56.0),
                background: Color(0xf3, 0xf5, 0xf9, 0xff),
                ..base
            },
            _ => Style { width: Length::FILL, height: Length::Fixed(44.0), ..base },
        }
    }
}

impl<const KIND: u8> Node for Strip<KIND> {
    // `kind_name` and `defaults` come from `lumen_static::leaf_common!` below, which
    // forwards them to `Widget::KIND` and `Widget::type_defaults` — and
    // here both are a `match KIND`, so one type answers for three kinds
    // at compile time. `Chart` does the same and says so.
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
        for k in 0..self.count {
            self.widths[k] = cx
                .text
                .measure_weighted(
                    self.items[k].label,
                    me.font_size,
                    if k == self.selected { 600.0 } else { me.font_weight },
                    limits.max_w,
                )
                .w;
        }
        let natural = if Self::vertical() {
            Size { w: 88.0, h: limits.max_h }
        } else {
            Size { w: limits.max_w, h: 44.0 }
        };
        let s = lumen_static::ops::resolve_self(limits, &me, natural);
        self.extent = if Self::vertical() { s.h } else { s.w };
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    /// `layout` with nothing kept — M41/T41.7, `Pagination`'s shape.
    /// `layout` caches each label's measured width, which `paint`
    /// centres by, and the extent `on_event` divides into items; a probe
    /// at the audit's 1e6 px, with the audit's measurer, must leave both
    /// as the frame's layout left them.
    #[cfg(feature = "observe")]
    fn probe(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        let kept = (self.extent, self.widths);
        let size = self.layout(cx, limits, style, st, i);
        (self.extent, self.widths) = kept;
        size
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        lumen_static::ops::focus_ring(st, i, out);
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 || self.count == 0 {
            return;
        }
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        let each = if Self::vertical() { h } else { w } / self.count as f32;
        for k in 0..self.count {
            let cell = if Self::vertical() {
                [r[0], r[1] + each * k as f32, r[2], r[1] + each * (k as f32 + 1.0)]
            } else {
                [r[0] + each * k as f32, r[1], r[0] + each * (k as f32 + 1.0), r[3]]
            };
            let on = k == self.selected;
            match KIND {
                // **An underline, not a fill.** A filled tab reads as a
                // button; the underline is what says "this is the page
                // you are on" without competing with the content.
                TABS => {
                    if on {
                        let t = 3.0;
                        out.rect([cell[0] + 8.0, cell[3] - t, cell[2] - 8.0, cell[3]], s.fg);
                    }
                }
                _ => {
                    if on {
                        out.rounded_rect(
                            [cell[0] + 4.0, cell[1] + 4.0, cell[2] - 4.0, cell[3] - 4.0],
                            s.fg,
                            8.0,
                        );
                    }
                }
            }
            // The label, centred in its cell. An unselected item in a
            // filled strip keeps the foreground colour; a selected one
            // sits *on* the foreground, so it takes the background's —
            // which is why a filled kind's `type_defaults` must give it
            // an opaque one, and why `a_filled_strip_has_a_ground_for_its_selected_label`
            // asserts that rather than trusting it.
            let col = if on && KIND != TABS { s.bg } else { s.fg };
            // Centred on its cell, from the width `layout` measured.
            let tw = self.widths[k].min(cell[2] - cell[0]);
            let tx = cell[0] + ((cell[2] - cell[0]) - tw) / 2.0;
            out.text_weighted(
                [tx, cell[1] + (cell[3] - cell[1]) / 2.0 - s.font_size * 0.6,
                 tx + tw, cell[3]],
                self.items[k].label,
                s.font_size,
                if on { 600.0 } else { s.font_weight },
                col,
            );
        }
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            Event::PointerDown { .. } => match self.item_at(cx.local) {
                Some(k) => {
                    // **True even when the selection did not move.** A
                    // press on the already-selected tab is still handled
                    // — declining would let it bubble to a parent, and a
                    // scroller above would then take it as a drag.
                    self.select(k);
                    true
                }
                None => false,
            },
            Event::Key { key: Key::Right, down: true } | Event::Key { key: Key::Down, down: true } => {
                self.select(self.selected + 1)
            }
            Event::Key { key: Key::Left, down: true } | Event::Key { key: Key::Up, down: true } => {
                self.select(self.selected.saturating_sub(1))
            }
            Event::Key { key: Key::Home, down: true } => self.select(0),
            Event::Key { key: Key::End, down: true } => self.select(usize::MAX),
            _ => false,
        }
    }

    lumen_static::leaf_common!(Strip<KIND>, 0);

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::TabList);
        node.set_size_of_set(self.count);
        node.set_position_in_set(self.selected + 1);
        // The selected label, so a drive can read the selection without
        // a screenshot — which is what M27/T27.5 found every widget in
        // this crate was missing.
        node.set_value(self.label(self.selected));
    }
}

impl<const KIND: u8, S> lumen_static::event::Handlers<S> for Strip<KIND> {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

// ---- AppBar ----------------------------------------------------------

/// A titled bar across the top of a screen.
///
/// **Not a selector**, which is why it is not a [`Strip`]. Nothing in it
/// is selected, there is no index, and giving it one to fit the group
/// would be a field that is always zero and a keyboard that moves it
/// nowhere. It is a title, a ground and an elevation.
pub struct AppBar<S: lumen_static::text::TextStorage = &'static str> {
    pub title: S,
}

impl<S: lumen_static::text::TextStorage + Default> Default for AppBar<S> {
    fn default() -> Self {
        AppBar { title: S::default() }
    }
}

impl<S: lumen_static::text::TextStorage> AppBar<S> {
    pub fn new(title: S) -> Self {
        AppBar { title }
    }
}

impl<S: lumen_static::text::TextStorage> Widget for AppBar<S> {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    /// **Rigid.** An app bar's height is its style's, never its title's
    /// — a longer title wraps or truncates rather than making the bar
    /// taller, which is what every platform's bar does and what stops a
    /// title change relaying out the whole screen below it.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, NodeDesc::RIGID)];
    const KIND: &'static str = "appbar";
    const SEEDS: &'static [Seed] =
        &[Seed::new(<AppBar<S> as Widget>::KIND, <AppBar<S> as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::FONT_WEIGHT,
                Fields::SHADOW,
                Fields::VISIBLE,
            ]),)];

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::Fixed(56.0),
            padding: Edges { left: 16.0, right: 16.0, top: 0.0, bottom: 0.0 },
            background: Color(0x2f, 0x6b, 0xff, 0xff),
            color: Color(0xff, 0xff, 0xff, 0xff),
            font_size: 18.0,
            font_weight: 600.0,
            shadow: 6.0,
            ..Style::default()
        }
    }
}

impl<S: lumen_static::text::TextStorage> Node for AppBar<S> {
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
        // The title is not measured: the bar's height is its style's.
        // Measuring it would make `IS_RIGID` a lie.
        let s = lumen_static::ops::resolve_self(
            limits,
            &me,
            Size { w: limits.max_w, h: 56.0 },
        );
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        if s.shadow > 0.0 {
            out.shadow(r, s.radius, s.shadow);
        }
        out.rounded_rect(r, s.bg, s.radius);
        lumen_static::ops::border(st, i, out);
        let p = s.pad;
        // Vertically centred on the bar, not on the padding box: an app
        // bar's padding is horizontal and a title pinned to the top of
        // it would sit against the status area.
        let mid = (r[1] + r[3]) / 2.0;
        out.text_weighted(
            [r[0] + p.0, mid - s.font_size * 0.6, r[2] - p.2, mid + s.font_size * 0.8],
            self.title.as_ref(),
            s.font_size,
            s.font_weight,
            s.fg,
        );
    }

    lumen_static::leaf_common!(AppBar<S>, 0, nohit);

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Banner);
        node.set_value(self.title.as_ref());
    }
}

impl<S: lumen_static::text::TextStorage, A> lumen_static::event::Handlers<A> for AppBar<S> {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}

// ---- PullToRefresh ---------------------------------------------------

/// Upper bound on `1 + W::NODE_COUNT` for one [`PullToRefresh`].
pub const MAX_PULL_NODES: usize = 256;

/// Content that can be dragged past its top to ask for a refresh.
///
/// **Not a strip**, and not a selector: it is a gesture wrapper. The
/// grouping in the plan put it here because it is "navigation"; what it
/// shares with `Tabs` is the word and nothing else.
///
/// **It does not refresh anything.** It reports that the gesture
/// happened and shows the indicator while it is in progress; what a
/// refresh *is* belongs to the application, which is the same division
/// `Spinner` makes about its own clock. `take_triggered` is the handoff,
/// and it is a take rather than a read so the event cannot be processed
/// twice — a double refresh is the failure this shape exists to prevent.
pub struct PullToRefresh<W: Widget> {
    pub content: W,
    /// How far the content has been dragged past its top, in px.
    pull: f32,
    /// Distance that arms a refresh.
    pub threshold: f32,
    /// Set when `pull` crosses `threshold`; cleared by `take_triggered`.
    triggered: bool,
}

impl<W: Widget + Default> Default for PullToRefresh<W> {
    fn default() -> Self {
        PullToRefresh::new(W::default())
    }
}

impl<W: Widget> PullToRefresh<W> {
    const AT: usize = 1;

    pub fn new(content: W) -> Self {
        PullToRefresh { content, pull: 0.0, threshold: 64.0, triggered: false }
    }

    /// How far it has been pulled, in px.
    pub fn pull(&self) -> f32 {
        self.pull
    }

    /// Whether the pull has passed the threshold and been released.
    pub fn is_armed(&self) -> bool {
        self.pull >= self.threshold
    }

    /// Take the pending refresh, if there is one.
    ///
    /// Clears the flag, so the same gesture cannot be served twice.
    pub fn take_triggered(&mut self) -> bool {
        core::mem::take(&mut self.triggered)
    }

    /// Let go: arm a refresh if the pull got far enough, and spring back.
    pub fn release(&mut self) -> bool {
        let armed = self.is_armed();
        self.triggered |= armed;
        let moved = self.pull != 0.0;
        self.pull = 0.0;
        moved
    }
}

pub struct PullTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> PullTable<W> {
    const ARRAY: [NodeDesc; MAX_PULL_NODES] = {
        assert!(W::NODE_COUNT < MAX_PULL_NODES, "PullToRefresh content exceeds MAX_PULL_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_PULL_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_PULL_NODES] = {
        let mut s = [Seed::WIDGET; MAX_PULL_NODES];
        s[0] = Seed::new(
            <PullToRefresh<W> as Widget>::KIND,
            <PullToRefresh<W> as Widget>::type_defaults, Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::COLOR, Fields::VISIBLE]));
        lumen_static::widget::splice_seeds(&mut s, 1, W::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(1 + W::NODE_COUNT).0;
}

impl<W: Widget> Widget for PullToRefresh<W> {
    const NODE_COUNT: usize = 1 + W::NODE_COUNT;
    const DEPTH: usize = 1 + W::DEPTH;
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = PullTable::<W>::SLICE;
    const KIND: &'static str = "pulltorefresh";
    const SEEDS: &'static [Seed] = PullTable::<W>::SEED_SLICE;

    fn type_defaults() -> Style {
        Style {
            width: Length::FILL,
            height: Length::FILL,
            color: Color(0x2f, 0x6b, 0xff, 0xff),
            ..Style::default()
        }
    }
}

impl<W: Widget> Node for PullToRefresh<W> {
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
        let c = self.content.layout(cx, limits, style, st, i + Self::AT);
        let s = lumen_static::ops::resolve_self(limits, &me, c);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        // **The content moves with the pull.** An indicator that
        // appeared without the content moving reads as an overlay, not
        // as a drag — and the whole affordance is that the surface
        // itself follows the finger.
        self.content.place(
            Point { x: origin.x, y: origin.y + self.pull },
            style,
            st,
            i + Self::AT,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = lumen_static::ops::rect_of(s);
        if self.pull > 0.0 && r[2] > r[0] {
            // A bar across the top, filling as the pull approaches the
            // threshold — a progress reading, not a spinner, because a
            // spinner would have to be animated and this widget has no
            // clock. `Spinner`'s own note says the same.
            let frac = lumen_static::num::clamp(self.pull / self.threshold.max(1.0), 0.0, 1.0);
            let h = (self.pull * 0.4).min(6.0);
            out.rounded_rect([r[0], r[1], r[0] + (r[2] - r[0]) * frac, r[1] + h], s.fg, h / 2.0);
        }
        self.content.paint(st, i + Self::AT, out);
    }

    fn on_event(&mut self, ev: &Event, _cx: &EventCtx) -> bool {
        match ev {
            // Only a downward wheel *at the top* pulls. Everything else
            // is the content's, including an upward one, or a list could
            // never be scrolled back.
            Event::Scroll { dy, .. } if *dy > 0.0 => {
                self.pull = (self.pull + dy).min(self.threshold * 2.0);
                true
            }
            Event::PointerUp { .. } => self.release(),
            _ => false,
        }
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        if !lumen_static::hit::contains(&st[i], x, y) {
            return None;
        }
        self.content.hit(st, i + Self::AT, x, y).or(Some(lumen_static::Hit {
            node: i as u32,
            inner: None,
            local: Point { x: x - st[i].pos.x, y: y - st[i].pos.y },
        }))
    }

    fn node(&self, k: usize) -> &dyn Node {
        if k == 0 {
            self
        } else {
            self.content.node(k - Self::AT)
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        if k == 0 {
            self
        } else {
            self.content.node_mut(k - Self::AT)
        }
    }

    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::GenericContainer);
        node.set_value(if self.is_armed() { "armed" } else { "idle" });
        node.set_numeric_value(self.pull as f64);
    }
}

/// Handlers pass through: a pull is a gesture, not a control.
impl<W, S> lumen_static::event::Handlers<S> for PullToRefresh<W>
where
    W: Widget + lumen_static::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.content
            .handler(lumen_static::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}
