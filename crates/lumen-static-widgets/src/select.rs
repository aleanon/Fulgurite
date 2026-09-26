//! `Select<S>` — a dropdown: a trigger, and a popup list of options.
//!
//! This is **T11.5**, deferred on 2026-09-04 because it needed a layout
//! capability the model did not have, and unblocked in three separate
//! milestones that were not framed as paying that debt:
//!
//! | prerequisite | landed |
//! |---|---|
//! | paint order versus tree order | M15/T15.0 — the overlay phase, [`lumen_static::Painter::defer`] |
//! | dismissal on click-outside | M15/T15.1 — `pass::dispatch_overlaid` |
//! | a stack/absolute layout mode | M17/T11.6 — `#[widget(stack)]` |
//!
//! So what is left is widget code, and most of *that* is already
//! written: [`crate::popup::Popup`] is the out-of-flow half and
//! [`lumen_static::repeat::Repeat`] is the rows.
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
//! because [`lumen_static::EventCtx::row`] says which was hit — recomputed by
//! hit testing each frame, so it cannot go stale the way a captured
//! index can.
//!
//! # It is controlled — the application owns `open` (M33/T33.3)
//!
//! Following `Toggle`'s precedent, which never flips itself: whether the
//! menu is open is the **application's** state, bound into the popup with
//! the value form, `#[bind(ui.picker.menu.open, open)]`.
//!
//! - **Opening** is the trigger's `on_click` (it is a [`Button`]): the
//!   application sets its `open`.
//! - **Choosing** is a row's `on_click`, with `ctx.row` set: the
//!   application records the choice and clears `open`.
//! - **Dismissal** — Escape, or a press outside every overlay — is done by
//!   `pass::dispatch_overlaid`, which then asks the popup's owner for its
//!   [`Select::on_dismiss`] and runs it, so the application can clear
//!   `open`. Without that call the state would still say "open" and the
//!   next binding would reopen the menu.
//!
//! Until M33 the widget owned opening (a press on the trigger, in
//! `on_event`) and the application closed it by reaching into the tree —
//! which a split application, whose handlers cannot reach the tree,
//! cannot do. Pressing the trigger while the menu is open still closes it
//! and leaves it closed: the press is dismissed and consumed, and so is
//! its release (`InputState::swallow_release`), so the trigger's
//! `on_click` never runs.

use lumen_static::cx::LayoutCx;
use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::Painter;
use lumen_static::event::EventCtx;
use lumen_static::hit::Hit;
use crate::ops::Button;
use crate::popup::{Anchor, Popup};
use lumen_static::repeat::Repeat;
use lumen_static::style::{Seed, Style};
use lumen_static::widget::{Node, Widget};

/// One row of the menu. A `Button`, so a row gets hover, press and focus
/// from the core rather than from this file.
///
/// Its own kind, `selectrow` (M57/C1), so a row's look is the widget's and
/// a sheet can restyle rows without reaching every other button.
pub type Row<S> = Button<S, alloc::string::String, true>;

/// The closed control: an ordinary button, which is what it looks like.
pub type Trigger<S> = Button<S, alloc::string::String>;

/// The menu itself: rows in an out-of-flow popup.
///
/// **Named `Menu` until M27/T27.2.** The wider name is wanted for the
/// nested command menu M27 builds, and the narrower one was the more
/// accurate of the two anyway: this is the menu *of a `Select`*, not a
/// menu in general. Renaming the alias rather than the widget is the
/// cheaper half — seven uses, all in this file.
///
/// The collision was never a compile error, since the two would live in
/// different crates and neither is in the prelude. What it costs is
/// prose: `check_doc_identifiers.sh` says of itself that it "does not
/// know scope", so a backticked `Menu` in a doc comment would pass
/// whichever one the writer meant, forever. `Rule` *was* a hard
/// collision — it is in the prelude, so a `Rule` widget makes a glob
/// import ambiguous — which is why tranche 1 shipped `Divider`.
pub type SelectMenu<S> = Popup<Repeat<Row<S>>>;

/// A dropdown.
///
/// See the module docs for what it owns and what the application does.
pub struct Select<S: 'static> {
    /// What the closed control shows. The application writes the label.
    pub trigger: Trigger<S>,
    /// The rows, and whether they are showing. `menu.open` is what the
    /// application binds.
    pub menu: SelectMenu<S>,
    /// Run when the pass dismisses the menu — Escape, or a press outside
    /// it — so the application can clear its `open`. See the module docs.
    pub on_dismiss: fn(&mut S, &EventCtx),
}

impl<S: 'static> Default for Select<S> {
    fn default() -> Self {
        let mut menu = SelectMenu::<S>::default();
        menu.anchor = Anchor::Below;
        Select { trigger: Trigger::<S>::default(), menu, on_dismiss: |_, _| {} }
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

    /// Close the menu directly. A controlled `Select` is closed by its
    /// application's state through the binding; this is for a host that
    /// drives the widget without one (a test, the agent's fixtures).
    pub fn close(&mut self) {
        self.menu.open = false;
    }

    /// Open it directly — see [`Select::close`].
    pub fn open(&mut self) {
        self.menu.open = true;
    }

    /// Index of the trigger within this subtree, and of the menu.
    const TRIGGER: usize = 1;
    const MENU: usize = 1 + <Trigger<S> as Widget>::NODE_COUNT;
}

/// The merged table for one `Select`, built at compile time — the same
/// technique [`crate::popup::PopupTable`] uses.
pub struct SelectTable<S>(core::marker::PhantomData<S>);

impl<S: 'static> SelectTable<S> {
    const N: usize = 1 + <Trigger<S> as Widget>::NODE_COUNT + <SelectMenu<S> as Widget>::NODE_COUNT;

    const ARRAY: [NodeDesc; MAX_SELECT_NODES] = {
        assert!(Self::N <= MAX_SELECT_NODES, "Select exceeds MAX_SELECT_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_SELECT_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        lumen_static::widget::splice_child(&mut t, 1, <Trigger<S> as Widget>::SUB_TABLE, 0);
        let menu_at = 1 + <Trigger<S> as Widget>::NODE_COUNT;
        lumen_static::widget::splice_child(&mut t, menu_at, <SelectMenu<S> as Widget>::SUB_TABLE, 0);
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
        s[0] = Seed::new(<Select<S> as Widget>::KIND, <Select<S> as Widget>::type_defaults, lumen_static::style::Fields::of(&[lumen_static::style::Fields::VISIBLE]));
        lumen_static::widget::splice_seeds(&mut s, 1, <Trigger<S> as Widget>::SEEDS);
        lumen_static::widget::splice_seeds(
            &mut s,
            1 + <Trigger<S> as Widget>::NODE_COUNT,
            <SelectMenu<S> as Widget>::SEEDS,
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
    const NODE_COUNT: usize = 1 + <Trigger<S> as Widget>::NODE_COUNT + <SelectMenu<S> as Widget>::NODE_COUNT;
    const DEPTH: usize = 1 + {
        let a = <Trigger<S> as Widget>::DEPTH;
        let b = <SelectMenu<S> as Widget>::DEPTH;
        if a > b {
            a
        } else {
            b
        }
    };
    /// Its size is the trigger's, and the menu is out of flow — so a
    /// longer option list never resizes anything above it. That is the
    /// property `Popup` exists for, inherited.
    const IS_RIGID: bool = <Trigger<S> as Widget>::IS_RIGID;
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
            return lumen_static::ops::hide_subtree(st, i, limits, <Self as Widget>::NODE_COUNT);
        }
        // The trigger is the whole of what is in flow.
        let size = self.trigger.layout(cx, limits, style, st, i + Self::TRIGGER);
        // The menu measures itself loosely and reports zero, so it
        // cannot widen or lengthen this control.
        // Flipping above clears the trigger (M57/A18).
        self.menu.above_by = size.h;
        self.menu.layout(cx, limits, style, st, i + Self::MENU);
        lumen_static::ops::record_painted(st, i, limits, size, &me)
    }

    fn place(&mut self, at: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = at;
        self.trigger.place(at, style, st, i + Self::TRIGGER);
        // **Below the trigger, not at the control's origin.** `Popup`
        // anchors on the position its parent gives it, so handing it
        // `at` would put the menu *over* the trigger.
        let under = Point { x: at.x, y: at.y + st[i + Self::TRIGGER].size.h };
        self.menu.place(under, style, st, i + Self::MENU);
    }

    /// What a screen reader and the agent are told — M57/A17.
    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut lumen_static::observe::accesskit::Node,
        _extra: &mut alloc::vec::Vec<(lumen_static::observe::accesskit::NodeId, lumen_static::observe::accesskit::Node)>,
    ) {
        // A combo box: its value is what the trigger shows, and whether
        // its menu is open is the part a reader needs to know.
        node.set_role(lumen_static::observe::accesskit::Role::ComboBox);
        node.set_value(AsRef::<str>::as_ref(&self.trigger.label));
        node.set_expanded(self.menu.open);
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

    fn dismiss(&mut self) -> bool {
        self.menu.dismiss()
    }

    #[cfg(feature = "alloc")]
    fn arena_count(&self) -> usize {
        self.menu.arena_count()
    }

    #[cfg(feature = "alloc")]
    fn arena_mut(&mut self, a: usize) -> Option<lumen_static::arena::ArenaMut<'_>> {
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

impl<S: 'static> lumen_static::event::Handlers<S> for Select<S> {
    fn handler(&self, sel: lumen_static::event::NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        if sel.i == 0 {
            // Only the pass's dismissal question reaches this: an
            // activation passes `inner` only at the node it hit, and the
            // trigger and rows have handlers of their own.
            return sel.is_dismissal().then_some(self.on_dismiss);
        }
        if sel.i < Self::MENU {
            return self.trigger.handler(lumen_static::event::NodeSel { i: sel.i - Self::TRIGGER, ..sel });
        }
        self.menu.handler(lumen_static::event::NodeSel { i: sel.i - Self::MENU, ..sel })
    }
}
