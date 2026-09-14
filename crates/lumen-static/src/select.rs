//! `Select<S>` — a dropdown: a trigger, and a popup list of options.
//!
//! This is **T11.5**, deferred on 2026-09-04 because it needed a layout
//! capability the model did not have, and unblocked in three separate
//! milestones that were not framed as paying that debt:
//!
//! | prerequisite | landed |
//! |---|---|
//! | paint order versus tree order | M15/T15.0 — the overlay phase, [`crate::Painter::defer`] |
//! | dismissal on click-outside | M15/T15.1 — `pass::dispatch_overlaid` |
//! | a stack/absolute layout mode | M17/T11.6 — `#[widget(stack)]` |
//!
//! So what is left is widget code, and most of *that* is already
//! written: [`crate::popup::Popup`] is the out-of-flow half and
//! [`crate::repeat::Repeat`] is the rows.
//!
//! # The option count is data
//!
//! `Repeat`, not a field per option — which matters more here than it
//! looks. A menu's length is a property of what the application is
//! showing, not of the tree's *shape*, and a static tree is very willing
//! to confuse the two: `examples/showcase` wrote eighteen `Row` fields,
//! eighteen `pick0..pick17` handlers and a `[&mut Row; 18]` rebind,
//! ~160 lines whose only content was the number eighteen. A `Repeat` is
//! four nodes whatever the count, and one handler serves every row
//! because [`crate::EventCtx::row`] says which was hit — recomputed by
//! hit testing each frame, so it cannot go stale the way a captured
//! index can.
//!
//! # What it owns and what the application owns
//!
//! `Select` owns **opening**. A press on the trigger is consumed in
//! [`Node::on_event`], which is the hook a widget has for its own state
//! ([`crate::slider::Slider`] uses the same one) — an `on_click` is
//! `fn(&mut S, ..)` and from `&mut S` a widget is only reachable as
//! `&mut dyn Node`, so the application could not do it even if it
//! wanted to.
//!
//! It does **not** own closing, and that is not an omission: a press
//! outside every overlay is dismissed and consumed by
//! `pass::dispatch_overlaid` before any widget sees it, and Escape
//! closes the topmost. So pressing the trigger while the menu is open
//! closes it and does *not* reopen it — the press never arrives. That is
//! what every desktop toolkit does, and getting it by not writing code
//! is the reason `dismiss` was made the pass's job in M15.
//!
//! Choosing a row is the application's: the row's `on_click` runs on
//! release with `ctx.row` set, and closing the menu there is one line.

use crate::cx::LayoutCx;
use crate::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use crate::display::Painter;
use crate::event::{Event, EventCtx};
use crate::hit::Hit;
use crate::ops::Button;
use crate::popup::{Anchor, Popup};
use crate::repeat::Repeat;
use crate::style::{Seed, Style};
use crate::widget::{Node, Widget};

/// One row of the menu. A `Button`, so a row gets hover, press and focus
/// from the core rather than from this file.
pub type Row<S> = Button<S, alloc::string::String>;

/// The menu itself: rows in an out-of-flow popup.
pub type Menu<S> = Popup<Repeat<Row<S>>>;

/// A dropdown.
///
/// See the module docs for what it owns and what the application does.
pub struct Select<S: 'static> {
    /// What the closed control shows. The application writes the label.
    pub trigger: Row<S>,
    /// The rows, and whether they are showing.
    pub menu: Menu<S>,
    /// The trigger's rect, kept from `place`.
    ///
    /// `on_event` gets neither `&[Style]` nor `&[NodeState]` — the same
    /// constraint [`crate::scroll::ScrollView`] records — and it must
    /// still tell a press on the trigger from one on a row that bubbled
    /// up through the popup. So the rect is recorded where it is known.
    trigger_rect: (Point, Size),
}

impl<S: 'static> Default for Select<S> {
    fn default() -> Self {
        let mut menu = Menu::<S>::default();
        menu.anchor = Anchor::Below;
        Select {
            trigger: Row::<S>::default(),
            menu,
            trigger_rect: (Point { x: 0.0, y: 0.0 }, Size::default()),
        }
    }
}

impl<S: 'static> Select<S> {
    /// A dropdown over `count` options, each `row_extent` px tall.
    ///
    /// `bind` configures a recycled row: its label **and** its
    /// `on_click`, which is one `fn` for every row because `ctx.row`
    /// carries the index. It cannot capture — a plain `fn` pointer is
    /// what makes a hot reload able to re-supply it (`docs/step4-hot-reload.md`).
    pub fn new(count: usize, row_extent: f32, bind: fn(&mut Row<S>, usize)) -> Self {
        let mut s = Select::default();
        s.menu.content = Repeat::new(count, row_extent, bind);
        s
    }

    /// How many options the menu lists.
    pub fn count(&self) -> usize {
        self.menu.content.count
    }

    /// Change the option count. The tree's shape does not move.
    pub fn set_count(&mut self, n: usize) {
        self.menu.content.count = n;
    }

    pub fn is_open(&self) -> bool {
        self.menu.open
    }

    /// Close the menu — what a row's handler calls after choosing.
    pub fn close(&mut self) {
        self.menu.open = false;
    }

    /// Open it. Rarely needed: a press on the trigger does this.
    pub fn open(&mut self) {
        self.menu.open = true;
    }

    /// Index of the trigger within this subtree, and of the menu.
    const TRIGGER: usize = 1;
    const MENU: usize = 1 + <Row<S> as Widget>::NODE_COUNT;
}

/// The merged table for one `Select`, built at compile time — the same
/// technique [`crate::popup::PopupTable`] uses.
pub struct SelectTable<S>(core::marker::PhantomData<S>);

impl<S: 'static> SelectTable<S> {
    const N: usize = 1 + <Row<S> as Widget>::NODE_COUNT + <Menu<S> as Widget>::NODE_COUNT;

    const ARRAY: [NodeDesc; MAX_SELECT_NODES] = {
        assert!(Self::N <= MAX_SELECT_NODES, "Select exceeds MAX_SELECT_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SELECT_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        crate::widget::splice_child(&mut t, 1, <Row<S> as Widget>::SUB_TABLE, 0);
        let menu_at = 1 + <Row<S> as Widget>::NODE_COUNT;
        crate::widget::splice_child(&mut t, menu_at, <Menu<S> as Widget>::SUB_TABLE, 0);
        t[1].next_sibling = menu_at as u32;
        // **The parts are addressable.** A `Select` is three `Button`s
        // and a panel, so `Kind("button")` cannot tell the trigger from
        // a row and nothing else can reach inside a composite at all —
        // not a stylesheet, not the agent's `#id` selectors. The derive
        // gives a *field* an id; a widget that is more than one node has
        // to name its own, and these are the names.
        //
        // Namespaced, because an unqualified `trigger` would collide
        // with an application's own `#[widget_id("trigger")]` and style
        // both.
        t[1] = t[1].with_id(PART_TRIGGER);
        t[menu_at] = t[menu_at].with_id(PART_POPUP);
        t[menu_at + 1] = t[menu_at + 1].with_id(PART_MENU);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(Self::N).0;

    const SEED_ARRAY: [Seed; MAX_SELECT_NODES] = {
        let mut s = [Seed::WIDGET; MAX_SELECT_NODES];
        s[0] = Seed::new(<Select<S> as Widget>::KIND, <Select<S> as Widget>::type_defaults);
        crate::widget::splice_seeds(&mut s, 1, <Row<S> as Widget>::SEEDS);
        crate::widget::splice_seeds(
            &mut s,
            1 + <Row<S> as Widget>::NODE_COUNT,
            <Menu<S> as Widget>::SEEDS,
        );
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

/// Upper bound on a `Select`'s node count.
pub const MAX_SELECT_NODES: usize = 8;

/// `#id` of the closed control — what a stylesheet and the agent address.
pub const PART_TRIGGER: &str = "select-trigger";
/// `#id` of the out-of-flow popup.
pub const PART_POPUP: &str = "select-popup";
/// `#id` of the row list — the panel a stylesheet gives a background,
/// a radius and a height to.
pub const PART_MENU: &str = "select-menu";

impl<S: 'static> Widget for Select<S> {
    const NODE_COUNT: usize =
        1 + <Row<S> as Widget>::NODE_COUNT + <Menu<S> as Widget>::NODE_COUNT;
    const DEPTH: usize = 1 + {
        let a = <Row<S> as Widget>::DEPTH;
        let b = <Menu<S> as Widget>::DEPTH;
        if a > b {
            a
        } else {
            b
        }
    };
    /// Its size is the trigger's, and the menu is out of flow — so a
    /// longer option list never resizes anything above it. That is the
    /// property `Popup` exists for, inherited.
    const IS_RIGID: bool = <Row<S> as Widget>::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = SelectTable::<S>::SLICE;

    const KIND: &'static str = "select";

    const SEEDS: &'static [Seed] = SelectTable::<S>::SEED_SLICE;
}

impl<S: 'static> Node for Select<S> {
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
            return crate::ops::hide_subtree(st, i, limits, <Self as Widget>::NODE_COUNT);
        }
        // The trigger is the whole of what is in flow.
        let size = self.trigger.layout(cx, limits, style, st, i + Self::TRIGGER);
        // The menu measures itself loosely and reports zero, so it
        // cannot widen or lengthen this control.
        self.menu.layout(cx, limits, style, st, i + Self::MENU);
        crate::ops::record_painted(st, i, limits, size, &me)
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        self.trigger.place(at, style, st, i + Self::TRIGGER);
        self.trigger_rect = (at, st[i + Self::TRIGGER].size);
        // **Below the trigger, not at the control's origin.** `Popup`
        // anchors on the position its parent gives it, so handing it
        // `at` would put the menu *over* the trigger.
        let under = Point { x: at.x, y: at.y + st[i + Self::TRIGGER].size.h };
        self.menu.place(under, style, st, i + Self::MENU);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.trigger.paint(st, i + Self::TRIGGER, out);
        self.menu.paint(st, i + Self::MENU, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<Hit> {
        // The menu first: it is drawn over the trigger's neighbours and
        // must take their clicks. A closed popup answers `None`.
        self.menu
            .hit(st, i + Self::MENU, x, y)
            .or_else(|| self.trigger.hit(st, i + Self::TRIGGER, x, y))
    }

    /// A press on the trigger opens the menu.
    ///
    /// **Open, not toggle.** While the menu is open a press on the
    /// trigger is *outside* the popup, so `pass::dispatch_overlaid`
    /// dismisses every overlay and consumes it — this is never reached.
    /// A toggle here would therefore close and immediately reopen, which
    /// is the classic dropdown bug, and writing `open` rather than
    /// `!open` is what avoids it.
    ///
    /// The rect test is what keeps a press on a *row* from being eaten:
    /// it bubbles through the popup to here, and it is not on the
    /// trigger.
    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        if !matches!(ev, Event::PointerDown { .. }) || self.menu.open {
            return false;
        }
        let (p, s) = self.trigger_rect;
        let on_trigger =
            cx.pos.x >= p.x && cx.pos.x < p.x + s.w && cx.pos.y >= p.y && cx.pos.y < p.y + s.h;
        if !on_trigger {
            return false;
        }
        self.menu.open = true;
        true
    }

    fn dismiss(&mut self) -> bool {
        self.menu.dismiss()
    }

    #[cfg(feature = "alloc")]
    fn arena_count(&self) -> usize {
        self.menu.arena_count()
    }

    #[cfg(feature = "alloc")]
    fn arena_mut(&mut self, a: usize) -> Option<crate::arena::ArenaMut<'_>> {
        self.menu.arena_mut(a)
    }

    fn node(&self, i: usize) -> &dyn Node {
        if i == 0 {
            self
        } else if i < Self::MENU {
            self.trigger.node(i - Self::TRIGGER)
        } else {
            self.menu.node(i - Self::MENU)
        }
    }

    fn node_mut(&mut self, i: usize) -> &mut dyn Node {
        if i == 0 {
            self
        } else if i < Self::MENU {
            self.trigger.node_mut(i - Self::TRIGGER)
        } else {
            self.menu.node_mut(i - Self::MENU)
        }
    }
}

impl<S: 'static> crate::event::Handlers<S> for Select<S> {
    fn handler(&self, sel: crate::event::NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        if sel.i == 0 {
            return None;
        }
        if sel.i < Self::MENU {
            return self
                .trigger
                .handler(crate::event::NodeSel { i: sel.i - Self::TRIGGER, ..sel });
        }
        self.menu.handler(crate::event::NodeSel { i: sel.i - Self::MENU, ..sel })
    }
}
