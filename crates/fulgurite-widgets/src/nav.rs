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

use fulgurite::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use fulgurite::display::{Color, Painter};
use fulgurite::event::{Event, EventCtx, Key};
use fulgurite::style::{Fields, Length, Seed, Style};
use fulgurite::widget::{Node, Widget};
use fulgurite::{Edges, LayoutCx};

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
    /// The item under the pointer, for hover and press feedback — M57/B36.
    /// Paint ignored `hovered` and `pressed` entirely.
    hot: Option<usize>,
}

/// `Tabs`: a horizontal strip with the selection underlined.
pub type Tabs = Strip<TABS>;

/// A rail item's height (M57/B38).
const RAIL_ITEM: f32 = 56.0;
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
            hot: None,
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

    /// The item under a local point, read along the strip's own axis.
    ///
    /// **A rail reads `local.y` where the other two read `local.x`.** It is
    /// the one line three presentations sharing one implementation can get
    /// wrong with nothing else looking different, and only a press down a
    /// rail can tell — `a_rail_reads_the_press_down_its_own_axis` in
    /// `tests/nav.rs` is that press.
    /// One item's extent along the strip. A rail stacks fixed 56 px items
    /// from its top, as Material's does (M57/B38); it divided its whole
    /// height among them, so four items in a tall rail were 65 px each and
    /// grew with the window. Strips divide their width.
    fn each(&self) -> f32 {
        let even = self.extent / self.count.max(1) as f32;
        if KIND == RAIL { even.min(RAIL_ITEM) } else { even }
    }

    fn item_at(&self, local: Point) -> Option<usize> {
        if self.count == 0 || self.extent <= 0.0 {
            return None;
        }
        let along = if Self::vertical() { local.y } else { local.x };
        let each = self.each();
        if each <= 0.0 || along < 0.0 {
            return None;
        }
        let k = (along / each) as usize;
        if KIND == RAIL && k >= self.count {
            // Below the last item of a rail is not an item.
            return None;
        }
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
                Fields::BACKGROUND_FILL,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::FONT_WEIGHT,
                Fields::RADIUS,
                Fields::FOCUS_RING,
                Fields::CURSOR,
                Fields::VISIBLE,
            ])).with_roles(match KIND { TABS => fulgurite::style::SeedRoles::NONE.color(fulgurite::style::Role::Muted).border(fulgurite::style::Role::Accent), _ => fulgurite::style::SeedRoles::NONE.color(fulgurite::style::Role::Text).background(fulgurite::style::Role::Surface) })];

    fn type_defaults() -> Style {
        let base = Style {
            color: Color(0x30, 0x3a, 0x4c, 0xff),
            font_size: 14.0,
            cursor: fulgurite::style::Cursor::Pointer,
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
    // `kind_name` and `defaults` come from `fulgurite::leaf_common!` below, which
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
            return fulgurite::ops::hide_subtree(st, i, limits, 1);
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
        let s = fulgurite::ops::resolve_self(limits, &me, natural);
        self.extent = if Self::vertical() { s.h } else { s.w };
        fulgurite::ops::record_painted(st, i, limits, s, &me)
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
        let s = &st[i];
        let r = fulgurite::ops::rect_of(s);
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        if w <= 0.0 || h <= 0.0 || self.count == 0 {
            return;
        }
        fulgurite::ops::fill(st, i, out);
        out.rounded_rect(r, s.bg, s.radius);
        fulgurite::ops::border(st, i, out);
        let each = self.each();
        for k in 0..self.count {
            let cell = if Self::vertical() {
                [r[0], r[1] + each * k as f32, r[2], r[1] + each * (k as f32 + 1.0)]
            } else {
                [r[0] + each * k as f32, r[1], r[0] + each * (k as f32 + 1.0), r[3]]
            };
            let on = k == self.selected;
            // **Hover and press** (M57/B36): a tint on the item under the
            // pointer, stronger while it is held.
            if !on && self.hot == Some(k) {
                let a = if s.pressed { 0x2e } else { 0x16 };
                out.rounded_rect(
                    [cell[0] + 4.0, cell[1] + 4.0, cell[2] - 4.0, cell[3] - 4.0],
                    fulgurite::display::Color(s.fg.0, s.fg.1, s.fg.2, a),
                    8.0,
                );
            }
            match KIND {
                // **An underline, not a fill.** A filled tab reads as a
                // button; the underline is what says "this is the page
                // you are on" without competing with the content.
                TABS => {
                    if on {
                        // In the accent (M57/B36): the underline and the
                        // label were the text colour, and only the weight
                        // told the current tab apart.
                        let t = 3.0;
                        out.rect([cell[0] + 8.0, cell[3] - t, cell[2] - 8.0, cell[3]], s.border_color);
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
            // asserts that rather than trusting it. **The guard is the
            // painted colour since M54/T54.5**: that test reads the
            // defaults, and drawing this label in the pill's own colour
            // (the review's fd31a02) left it green; the contrast harness
            // (`tests/text_contrast.rs`) and `just widget-drive` measure
            // the run against the fill it lands on, and read 1.00:1.
            let col = match (on, KIND) {
                (true, TABS) => s.border_color,
                (true, _) => s.bg,
                _ => s.fg,
            };
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
            Event::Hover { .. } => {
                let was = self.hot;
                self.hot = self.item_at(cx.local);
                was != self.hot
            }
            Event::Leave => self.hot.take().is_some(),
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

    fulgurite::leaf_common!(Strip<KIND>, 0);

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

impl<const KIND: u8, S> fulgurite::event::Handlers<S> for Strip<KIND> {
    fn handler(
        &self,
        _sel: fulgurite::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

// ---- AppBar ----------------------------------------------------------

/// Upper bound on one [`AppBar`]'s nodes: itself and its two slots.
pub const MAX_APPBAR_NODES: usize = 64;

/// A titled bar across the top of a screen, with a leading and an
/// actions slot.
///
/// **Not a selector**, which is why it is not a [`Strip`]. Nothing in it
/// is selected, there is no index, and giving it one to fit the group
/// would be a field that is always zero and a keyboard that moves it
/// nowhere. It is a title, a ground and an elevation.
///
/// **Slots** — M57/B39. `L` sits at the start (a back or menu button) and
/// `A` at the end (actions); both default to `Option<Spacer>`, and `None`
/// is a hidden subtree that takes no room. The title fills what is left
/// and **ends in an ellipsis** when it does not fit, which the rustdoc
/// claimed and nothing tested: it drew past the bar.
///
/// **The cut is `Style::overflow`'s** — M80/T80.3: `Ellipsis` by default,
/// `Clip` for a cut with no mark, and `Wrap` reads as `Ellipsis`, because a
/// bar is one line. The search is [`crate::fit::cut`], shared with
/// `Label`, which cuts at a **grapheme** boundary; `fit_title` searched
/// `char` boundaries and kept half a ZWJ family, flag or skin tone before
/// the `…`.
pub struct AppBar<
    S: fulgurite::text::TextStorage = &'static str,
    L: Widget = Option<crate::ops::Spacer>,
    A: Widget = Option<crate::ops::Spacer>,
> {
    pub title: S,
    pub leading: L,
    pub actions: A,
    /// How much of the title fits, from layout — the same pair `Label`
    /// keeps: the prefix's end byte (`u32::MAX` = uncut) and where the `…`
    /// starts from the title's left edge (negative under `Clip`: no mark).
    fit: (u32, f32),
    /// What `fit` was computed under, so a still frame measures nothing:
    /// the title's fold under a [`MeasureKey`](fulgurite::text::MeasureKey)
    /// for the room (M80/T80.3), which adds the measurer's generation and
    /// the overflow byte that the hand-rolled key never had.
    fit_key: fulgurite::text::FoldedText,
    /// The title's box, from `place`.
    title_x: (f32, f32),
}

impl<S, L, A> Default for AppBar<S, L, A>
where
    S: fulgurite::text::TextStorage + Default,
    L: Widget + Default,
    A: Widget + Default,
{
    fn default() -> Self {
        AppBar::with(S::default(), L::default(), A::default())
    }
}

impl<S: fulgurite::text::TextStorage> AppBar<S> {
    pub fn new(title: S) -> Self {
        AppBar::with(title, None, None)
    }
}

impl<S: fulgurite::text::TextStorage, L: Widget, A: Widget> AppBar<S, L, A> {
    const LEAD: usize = 1;

    /// A bar with both slots.
    pub fn with(title: S, leading: L, actions: A) -> Self {
        AppBar { title, leading, actions, fit: (u32::MAX, -1.0), fit_key: fulgurite::text::FoldedText::new(), title_x: (0.0, 0.0) }
    }

    fn act() -> usize {
        1 + L::NODE_COUNT
    }

    /// The part of the title drawn, and whether it was cut.
    pub fn shown_title(&self) -> (&str, bool) {
        let t = self.title.as_ref();
        if self.fit.0 == u32::MAX {
            return (t, false);
        }
        (t.get(..self.fit.0 as usize).unwrap_or(""), true)
    }
}

/// The gap between a slot and the title.
const SLOT_GAP: f32 = 8.0;
const ELLIPSIS: &str = crate::fit::ELLIPSIS;

pub struct AppBarTable<L, A>(core::marker::PhantomData<(L, A)>);

impl<L: Widget, A: Widget> AppBarTable<L, A> {
    const N: usize = 1 + L::NODE_COUNT + A::NODE_COUNT;
    const ARRAY: [NodeDesc; MAX_APPBAR_NODES] = {
        assert!(Self::N <= MAX_APPBAR_NODES, "AppBar slots exceed MAX_APPBAR_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_APPBAR_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER | NodeDesc::RIGID);
        fulgurite::widget::splice_child(&mut t, 1, L::SUB_TABLE, 0);
        fulgurite::widget::splice_child(&mut t, 1 + L::NODE_COUNT, A::SUB_TABLE, 0);
        // The leading slot's next sibling is the actions slot.
        t[1].next_sibling = (1 + L::NODE_COUNT) as u32;
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(Self::N).0;

    const SEED_ARRAY: [Seed; MAX_APPBAR_NODES] = {
        let mut s = [Seed::WIDGET; MAX_APPBAR_NODES];
        s[0] = Seed::new("appbar", appbar_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::BACKGROUND_FILL,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::FONT_WEIGHT,
                Fields::SHADOW,
                // M80/T80.3: the bar's ground and its shadow are rounded by
                // it, and the mask had never been measured to say so.
                Fields::RADIUS,
                Fields::VISIBLE,
                Fields::OVERFLOW,
            ]),).with_roles(fulgurite::style::SeedRoles::NONE.color(fulgurite::style::Role::OnAccent).background(fulgurite::style::Role::Accent));
        fulgurite::widget::splice_seeds(&mut s, 1, L::SEEDS);
        fulgurite::widget::splice_seeds(&mut s, 1 + L::NODE_COUNT, A::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

fn appbar_defaults() -> Style {
    Style {
        width: Length::FILL,
        height: Length::Fixed(56.0),
        padding: Edges { left: 16.0, right: 16.0, top: 0.0, bottom: 0.0 },
        // `Palette::LIGHT`'s accent since M54/T54.6: `#2f6bff` put its
        // white title at 4.497:1 (`docs/results-a11y-platform.md`).
        background: Color(0x31, 0x5c, 0xde, 0xff),
        color: Color(0xff, 0xff, 0xff, 0xff),
        font_size: 18.0,
        font_weight: 600.0,
        shadow: 6.0,
        // M80/T80.3: a bar's title is one line, cut with a mark.
        overflow: fulgurite::style::TextOverflow::Ellipsis,
        ..Style::default()
    }
}

impl<S: fulgurite::text::TextStorage, L: Widget, A: Widget> Widget for AppBar<S, L, A> {
    const NODE_COUNT: usize = AppBarTable::<L, A>::N;
    const DEPTH: usize = 1 + if L::DEPTH > A::DEPTH { L::DEPTH } else { A::DEPTH };
    /// **Rigid.** An app bar's height is its style's, never its title's
    /// — a longer title is cut with an ellipsis rather than making the bar
    /// taller, which is what every platform's bar does and what stops a
    /// title change relaying out the whole screen below it.
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = AppBarTable::<L, A>::SLICE;
    const KIND: &'static str = "appbar";
    const SEEDS: &'static [Seed] = AppBarTable::<L, A>::SEED_SLICE;

    fn type_defaults() -> Style {
        appbar_defaults()
    }
}

impl<S: fulgurite::text::TextStorage, L: Widget, A: Widget> Node for AppBar<S, L, A> {
    fulgurite::node_count!();
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
            return fulgurite::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        // The bar's size is its style's: the title and the slots are
        // fitted into it, never the other way round.
        let s = fulgurite::ops::resolve_self(limits, &me, Size { w: limits.max_w, h: 56.0 });
        let inner = Constraints { min_w: 0.0, max_w: (s.w - me.padding.horizontal()).max(0.0), min_h: 0.0, max_h: s.h };
        let l = self.leading.layout(cx, inner, style, st, i + Self::LEAD);
        let a = self.actions.layout(cx, inner, style, st, i + Self::act());
        let gap = |w: f32| if w > 0.0 { SLOT_GAP } else { 0.0 };
        let room = (inner.max_w - l.w - gap(l.w) - a.w - gap(a.w)).max(0.0);
        self.fit_title(cx, room, &me);
        record_painted_rigid(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        let me = &style[i];
        let h = st[i].size.h;
        let (lw, lh) = (st[i + Self::LEAD].size.w, st[i + Self::LEAD].size.h);
        let (aw, ah) = (st[i + Self::act()].size.w, st[i + Self::act()].size.h);
        let x0 = origin.x + me.padding.left;
        let x1 = origin.x + st[i].size.w - me.padding.right;
        self.leading.place(Point { x: x0, y: origin.y + (h - lh) / 2.0 }, style, st, i + Self::LEAD);
        self.actions.place(Point { x: x1 - aw, y: origin.y + (h - ah) / 2.0 }, style, st, i + Self::act());
        let tx = x0 + lw + if lw > 0.0 { SLOT_GAP } else { 0.0 };
        self.title_x = (tx, x1 - aw - if aw > 0.0 { SLOT_GAP } else { 0.0 });
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = fulgurite::ops::rect_of(s);
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        if s.shadow > 0.0 {
            out.shadow(r, s.radius, s.shadow);
        }
        fulgurite::ops::fill(st, i, out);
        out.rounded_rect(r, s.bg, s.radius);
        fulgurite::ops::border(st, i, out);
        // Vertically centred on the bar, not on the padding box: an app
        // bar's padding is horizontal and a title pinned to the top of
        // it would sit against the status area.
        let mid = (r[1] + r[3]) / 2.0;
        let (y0, y1) = (mid - s.font_size * 0.6, mid + s.font_size * 0.8);
        let (x0, x1) = self.title_x;
        let (shown, cut) = self.shown_title();
        out.text_weighted([x0, y0, x1, y1], shown, s.font_size, s.font_weight, s.fg);
        if cut && self.fit.1 >= 0.0 {
            let e = x0 + self.fit.1;
            out.text_weighted([e, y0, x1, y1], ELLIPSIS, s.font_size, s.font_weight, s.fg);
        }
        self.leading.paint(st, i + Self::LEAD, out);
        self.actions.paint(st, i + Self::act(), out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<fulgurite::Hit> {
        if !fulgurite::hit::contains(&st[i], x, y) {
            return None;
        }
        // The slots only: the bar itself is a ground, and takes nothing.
        self.leading.hit(st, i + Self::LEAD, x, y).or_else(|| self.actions.hit(st, i + Self::act(), x, y))
    }

    fn node(&self, k: usize) -> &dyn Node {
        if k == 0 {
            self
        } else if k < Self::act() {
            self.leading.node(k - Self::LEAD)
        } else {
            self.actions.node(k - Self::act())
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        if k == 0 {
            self
        } else if k < Self::act() {
            self.leading.node_mut(k - Self::LEAD)
        } else {
            self.actions.node_mut(k - Self::act())
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
        // The whole title, not the cut one: a reader is not short of room.
        node.set_role(accesskit::Role::Banner);
        node.set_value(self.title.as_ref());
    }
}

impl<S: fulgurite::text::TextStorage, L: Widget, A: Widget> AppBar<S, L, A> {
    /// How much of the title fits in `room`: [`crate::fit::cut`], with a
    /// mark unless the style says `Clip`, memoised under a
    /// [`MeasureKey`](fulgurite::text::MeasureKey) for the room and the
    /// title's fold, so a still frame measures nothing.
    fn fit_title(&mut self, cx: &mut LayoutCx, room: f32, me: &Style) {
        let t = self.title.as_ref();
        let key = fulgurite::text::MeasureKey::new(
            cx,
            Constraints { min_w: 0.0, max_w: room, min_h: 0.0, max_h: 0.0 },
            me,
        );
        let fit = &mut self.fit;
        self.fit_key.get_or(key, t, || {
            let mark = me.overflow != fulgurite::style::TextOverflow::Clip;
            *fit = match crate::fit::cut(cx.text, t, me.font_size, me.font_weight, room, mark) {
                None => (u32::MAX, -1.0),
                Some((end, w)) => (end as u32, if mark { w } else { -1.0 }),
            };
            Size::default()
        });
    }
}

/// `record_painted` for a rigid container: the same call, named for the
/// reader who looks for why the bar's size ignores its content.
fn record_painted_rigid(st: &mut [NodeState], i: usize, limits: Constraints, s: Size, me: &Style) -> Size {
    fulgurite::ops::record_painted(st, i, limits, s, me)
}

impl<S, L, A, X> fulgurite::event::Handlers<X> for AppBar<S, L, A>
where
    S: fulgurite::text::TextStorage,
    L: Widget + fulgurite::event::Handlers<X>,
    A: Widget + fulgurite::event::Handlers<X>,
{
    fn handler(
        &self,
        sel: fulgurite::event::NodeSel,
    ) -> Option<fn(&mut X, &EventCtx)> {
        let act = 1 + L::NODE_COUNT;
        if sel.i == 0 {
            None
        } else if sel.i < act {
            self.leading.handler(fulgurite::event::NodeSel { i: sel.i - 1, inner: sel.inner })
        } else {
            self.actions.handler(fulgurite::event::NodeSel { i: sel.i - act, inner: sel.inner })
        }
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
    /// Where a pointer drag started, less the pull it started from — set
    /// by a press, cleared by the release (M57/B40).
    drag_from: Option<f32>,
    /// **Held open while the application refreshes** — M57/B40. Set it
    /// when you take the trigger, clear it when the work is done; while it
    /// is set the indicator is full and the content stays pulled down,
    /// which is the "refreshing" state a user waits on. Controlled, like
    /// `Toggle::on`: the widget never clears it.
    pub refreshing: bool,
}

impl<W: Widget + Default> Default for PullToRefresh<W> {
    fn default() -> Self {
        PullToRefresh::new(W::default())
    }
}

impl<W: Widget> PullToRefresh<W> {
    const AT: usize = 1;

    pub fn new(content: W) -> Self {
        PullToRefresh { content, pull: 0.0, threshold: 64.0, triggered: false, drag_from: None, refreshing: false }
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
    /// Whether the content is scrolled to its top, or does not scroll —
    /// the only time a pull may begin (M57/B40).
    fn at_top(&self) -> bool {
        self.content.node(0).scroll_offset().is_none_or(|o| o <= 0.5)
    }

    /// The offset the content is drawn at: the pull, or the threshold while
    /// the application is refreshing.
    fn shown(&self) -> f32 {
        if self.refreshing { self.threshold.max(self.pull) } else { self.pull }
    }

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
        fulgurite::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(1 + W::NODE_COUNT).0;

    const SEED_ARRAY: [Seed; MAX_PULL_NODES] = {
        let mut s = [Seed::WIDGET; MAX_PULL_NODES];
        s[0] = Seed::new(
            <PullToRefresh<W> as Widget>::KIND,
            <PullToRefresh<W> as Widget>::type_defaults, Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::COLOR, Fields::VISIBLE])).with_roles(fulgurite::style::SeedRoles::NONE.color(fulgurite::style::Role::Accent));
        fulgurite::widget::splice_seeds(&mut s, 1, W::SEEDS);
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
    fulgurite::node_count!();
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
            return fulgurite::ops::hide_subtree(st, i, limits, Self::NODE_COUNT);
        }
        // **Within its own box, less its padding** — M57/B40. The content
        // was offered the pull's *limits*, so a 50 px pull laid a scroll
        // view out 300 px tall, and the padding a sheet set was ignored in
        // both layout and place (the showcase's padding did nothing).
        let own = fulgurite::ops::within_own_size(limits, &me);
        let inner = fulgurite::ops::inner_limits(own, me.padding);
        let loose = Constraints { min_w: 0.0, max_w: inner.max_w, min_h: 0.0, max_h: inner.max_h };
        let c = self.content.layout(cx, loose, style, st, i + Self::AT);
        let padded = Size { w: c.w + me.padding.horizontal(), h: c.h + me.padding.vertical() };
        let s = fulgurite::ops::resolve_self(limits, &me, padded);
        fulgurite::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        let pad = style[i].padding;
        // **The content moves with the pull.** An indicator that
        // appeared without the content moving reads as an overlay, not
        // as a drag — and the whole affordance is that the surface
        // itself follows the finger.
        self.content.place(
            Point { x: origin.x + pad.left, y: origin.y + pad.top + self.shown() },
            style,
            st,
            i + Self::AT,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = fulgurite::ops::rect_of(s);
        let pull = self.shown();
        if pull > 0.0 && r[2] > r[0] {
            // A bar across the top, filling as the pull approaches the
            // threshold — a progress reading, not a spinner, because a
            // spinner would have to be animated and this widget has no
            // clock. `Spinner`'s own note says the same.
            let frac = fulgurite::num::clamp(pull / self.threshold.max(1.0), 0.0, 1.0);
            let h = (pull * 0.4).min(6.0);
            out.rounded_rect([r[0], r[1], r[0] + (r[2] - r[0]) * frac, r[1] + h], s.fg, h / 2.0);
        }
        // Clipped to its own box: pulled down, the content drew past the
        // bottom of the widget (M57/B40).
        out.push_clip(r);
        self.content.paint(st, i + Self::AT, out);
        out.pop_clip();
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        let max = self.threshold * 2.0;
        match ev {
            // **Only at the top** (M57/B40): the comment said so, and any
            // downward wheel pulled, so inside a list this took every
            // scroll. Half the wheel's distance, so the pull resists.
            Event::Scroll { dy, .. } if *dy > 0.0 && self.at_top() => {
                self.pull = (self.pull + dy * 0.5).min(max);
                true
            }
            // An upward wheel while pulled takes the pull back first.
            Event::Scroll { dy, .. } if *dy < 0.0 && self.pull > 0.0 => {
                self.pull = (self.pull + dy * 0.5).max(0.0);
                true
            }
            // **A wheel has no release; the gesture going quiet is one**
            // (M57/B40). Released only on a `PointerUp`, a wheel pull stayed
            // armed, its content held down, until an unrelated click.
            Event::ScrollEnd if self.pull > 0.0 => self.release(),
            // **A drag pulls** (M57/B40). There was no arm for it, so the
            // gesture the widget is named for did not exist. A press the
            // content declined starts one — but not a control's, which
            // must keep its own press or never activate.
            Event::PointerDown { .. } if !cx.target_is_control && self.at_top() => {
                self.drag_from = Some(cx.pos.y - self.pull * 2.0);
                true
            }
            Event::PointerMove { .. } => match self.drag_from {
                Some(from) => {
                    self.pull = ((cx.pos.y - from) * 0.5).clamp(0.0, max);
                    true
                }
                None => false,
            },
            Event::PointerUp { .. } => {
                self.drag_from = None;
                self.release()
            }
            _ => false,
        }
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<fulgurite::Hit> {
        if !fulgurite::hit::contains(&st[i], x, y) {
            return None;
        }
        self.content.hit(st, i + Self::AT, x, y).or(Some(fulgurite::Hit {
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
impl<W, S> fulgurite::event::Handlers<S> for PullToRefresh<W>
where
    W: Widget + fulgurite::event::Handlers<S>,
{
    fn handler(
        &self,
        sel: fulgurite::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        self.content
            .handler(fulgurite::event::NodeSel { i: sel.i - Self::AT, inner: sel.inner })
    }
}
