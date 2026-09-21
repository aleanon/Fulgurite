//! M27/T27.9 — the choice group: `OptionList`, `PickList`, `Combobox`,
//! `Menu`.
//!
//! # Why these do not use `Repeat`
//!
//! `Select` — the core's dropdown, which the showcase's own picker is —
//! puts its rows in a `Repeat`, and its rows are bound by a plain `fn`
//! the *application* supplies. That is right for a dropdown over
//! someone else's data, and it is exactly what stops a widget owning its
//! own option list: `Repeat::bind` is `fn(&mut T, usize)` and cannot
//! capture the widget that holds the options.
//!
//! So a `PickList` that owns its options draws them itself, the way
//! [`crate::Strip`] draws its items: one node, a fixed array, rows
//! hit-tested by arithmetic. That also keeps a four-option picker at
//! four nodes instead of an arena, and sidesteps the per-row addressing
//! debt (`docs/plan-select.md` T24.4) rather than inheriting it.
//!
//! **`Select` stays the primitive** — a dropdown over data the
//! application binds. These are the ones that own what they show.

use lumen_static::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{Event, EventCtx, Key};
use crate::popup::{Anchor, Popup};
use lumen_static::style::{Fields, Length, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{Edges, LayoutCx};

/// The most options one of these can hold.
///
/// Fixed, like `Series` and `Strip`'s items, so a chooser allocates
/// nothing. Over-long input is **truncated, not refused**: a picker is a
/// view of someone else's list, and refusing to draw is worse than
/// drawing the first sixteen.
pub const MAX_OPTIONS: usize = 16;

/// A row's height.
const ROW_H: f32 = 30.0;

// ---- OptionList ------------------------------------------------------

/// The rows of a chooser: one node that draws and hits all of them.
///
/// Its whole interface to the widget above it is `take_picked` — a take
/// rather than a read, so one press cannot be served twice. `Veil` and
/// `PullToRefresh` hand their events over the same way, and for the same
/// reason.
pub struct OptionList {
    options: [&'static str; MAX_OPTIONS],
    count: usize,
    /// Which rows pass the current filter. All of them when there is
    /// none.
    shown: [bool; MAX_OPTIONS],
    shown_count: usize,
    /// The row the pointer is over, for the hover affordance.
    hot: Option<usize>,
    picked: Option<usize>,
    /// Laid-out height, so `on_event` can turn `local.y` into a row.
    height: f32,
}

impl Default for OptionList {
    fn default() -> Self {
        OptionList {
            options: [""; MAX_OPTIONS],
            count: 0,
            shown: [true; MAX_OPTIONS],
            shown_count: 0,
            hot: None,
            picked: None,
            height: 0.0,
        }
    }
}

impl OptionList {
    pub fn new(options: &[&'static str]) -> Self {
        let mut l = OptionList::default();
        l.set_options(options);
        l
    }

    pub fn set_options(&mut self, options: &[&'static str]) {
        self.count = 0;
        for (i, o) in options.iter().take(MAX_OPTIONS).enumerate() {
            self.options[i] = o;
            self.shown[i] = true;
            self.count = i + 1;
        }
        self.shown_count = self.count;
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn option(&self, i: usize) -> &'static str {
        self.options[i.min(MAX_OPTIONS - 1)]
    }

    /// Keep only the options containing `needle`, case-insensitively.
    ///
    /// Returns whether the visible set changed, so a caller can skip a
    /// relayout. An **empty needle shows everything** — a filter that
    /// hid every row when the field was cleared would make a combobox
    /// unusable the moment someone deleted their query.
    pub fn filter(&mut self, needle: &str) -> bool {
        let before = self.shown_count;
        let n = needle.trim();
        self.shown_count = 0;
        for i in 0..self.count {
            let keep = n.is_empty() || contains_fold(self.options[i], n);
            self.shown[i] = keep;
            if keep {
                self.shown_count += 1;
            }
        }
        self.shown_count != before
    }

    /// How many rows are currently visible.
    pub fn shown(&self) -> usize {
        self.shown_count
    }

    /// The option index at visible position `k`.
    pub fn nth_shown(&self, k: usize) -> Option<usize> {
        let mut seen = 0;
        for i in 0..self.count {
            if self.shown[i] {
                if seen == k {
                    return Some(i);
                }
                seen += 1;
            }
        }
        None
    }

    /// Take the pending choice, if there is one.
    pub fn take_picked(&mut self) -> Option<usize> {
        self.picked.take()
    }

    fn row_at(&self, y: f32) -> Option<usize> {
        if self.shown_count == 0 || self.height <= 0.0 {
            return None;
        }
        let k = (y / ROW_H) as usize;
        self.nth_shown(k.min(self.shown_count - 1))
    }
}

/// `needle` in `hay`, ASCII-case-insensitively.
///
/// ASCII only, and that is a limit rather than an oversight: a
/// Unicode-correct fold needs tables this crate does not carry, and
/// `docs/plan-embedded.md`'s tiering is why it does not. A filter that
/// missed a non-ASCII letter is a worse answer than one that says so.
fn contains_fold(hay: &str, needle: &str) -> bool {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.is_empty() || n.len() > h.len() {
        return n.is_empty();
    }
    (0..=h.len() - n.len()).any(|i| {
        (0..n.len()).all(|j| h[i + j].eq_ignore_ascii_case(&n[j]))
    })
}

impl Widget for OptionList {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "optionlist";
    const SEEDS: &'static [Seed] =
        &[Seed::new(<OptionList as Widget>::KIND, <OptionList as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::RADIUS,
                Fields::SHADOW,
                Fields::VISIBLE,
            ]),)];

    fn type_defaults() -> Style {
        Style {
            width: Length::Fixed(220.0),
            height: Length::Shrink,
            padding: Edges { left: 12.0, right: 12.0, top: 6.0, bottom: 6.0 },
            background: Color(0xff, 0xff, 0xff, 0xff),
            color: Color(0x30, 0x3a, 0x4c, 0xff),
            font_size: 14.0,
            radius: 10.0,
            shadow: 12.0,
            ..Style::default()
        }
    }
}

impl Node for OptionList {
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
        let content = Size {
            w: limits.max_w,
            h: self.shown_count as f32 * ROW_H + me.padding.vertical(),
        };
        let s = lumen_static::ops::resolve_self(limits, &me, content);
        self.height = s.h;
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
        let mut k = 0usize;
        for idx in 0..self.count {
            if !self.shown[idx] {
                continue;
            }
            let y = r[1] + p.1 + ROW_H * k as f32;
            if Some(idx) == self.hot {
                out.rounded_rect([r[0] + 4.0, y, r[2] - 4.0, y + ROW_H], s.fg, 6.0);
            }
            let col = if Some(idx) == self.hot { s.bg } else { s.fg };
            out.text_weighted(
                [r[0] + p.0, y + ROW_H / 2.0 - s.font_size * 0.6, r[2] - p.2, y + ROW_H],
                self.options[idx],
                s.font_size,
                s.font_weight,
                col,
            );
            k += 1;
        }
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // `Hover`, not `PointerMove` — see `Event::Hover`. A move
            // reaches a widget only under capture, so this arm never ran
            // and an option list never highlighted the row under the
            // pointer (M27/T27.11, finding 3).
            Event::Hover { .. } => {
                let was = self.hot;
                self.hot = self.row_at(cx.local.y - 6.0);
                was != self.hot
            }
            // **On the *up*, not the down.** A press-and-drag that
            // leaves the list must not choose — every menu in every
            // toolkit commits on release, and choosing on the press also
            // closes the list under the finger that is still down.
            Event::PointerUp { .. } => match self.row_at(cx.local.y - 6.0) {
                Some(idx) => {
                    self.picked = Some(idx);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    lumen_static::leaf_common!(OptionList, 0);

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::ListBox);
        node.set_size_of_set(self.shown_count);
        // The visible options, so a drive can assert a filter worked
        // without reading pixels.
        let mut v = alloc::string::String::new();
        for i in 0..self.count {
            if self.shown[i] {
                if !v.is_empty() {
                    v.push(',');
                }
                v.push_str(self.options[i]);
            }
        }
        node.set_value(v);
    }
}

impl<S> lumen_static::event::Handlers<S> for OptionList {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

// ---- the three choosers ----------------------------------------------

/// A `PickList`: a button showing the selection, and a list under it.
pub const PICKLIST: u8 = 0;
/// A `Combobox`: the same, with the trigger editable and filtering.
pub const COMBOBOX: u8 = 1;
/// A `Menu`: a list with no trigger, opened by the application.
pub const MENU: u8 = 2;

/// Upper bound on a chooser's node count.
pub const MAX_CHOOSER_NODES: usize = 8;

/// Part ids. Same caveat as every other composite in this crate: two in
/// one tree collide, and `W1011` says so.
pub const PART_TRIGGER: &str = "chooser-trigger";
pub const PART_POPUP: &str = "chooser-popup";
pub const PART_LIST: &str = "chooser-list";

/// One of `PickList`, `Combobox` or `Menu`.
///
/// They differ in exactly two things — whether there is a trigger, and
/// whether typing in it filters — so they are one type with a `const`
/// parameter, as `Strip` and `Veil` are.
/// **No `S` parameter.** A chooser carries no `on_click`: its rows are
/// its own, and what a choice *means* reaches the application through
/// `selected()` in `after_update`, the same handoff `PullToRefresh` and
/// `Veil` make. `Select` is generic because its rows are `Button<S, _>`
/// the application binds; these are not.
pub struct Chooser<const KIND: u8 = PICKLIST> {
    /// The closed control. A `Menu` has one too, laid out at zero size:
    /// a type per shape would be three tables to keep in step.
    pub trigger: crate::input::TextInput<alloc::string::String>,
    pub list: Popup<OptionList>,
    selected: Option<usize>,
    /// Whether the trigger holds a **typed query** rather than the
    /// committed selection.
    ///
    /// **A combobox must not filter on its own selection.** Choosing
    /// "Apple" puts "Apple" in the trigger; filtering on that leaves one
    /// row, so re-opening the list shows only what is already chosen and
    /// the other options are unreachable. Found by `just widget-drive`,
    /// which opened a combobox and counted one row where six were
    /// seeded — and the headless test could not see it, because it set
    /// the trigger's text directly and so was always in the "typed"
    /// state.
    typed: bool,
    /// The trigger's rect, kept from `place` — `on_event` gets neither
    /// style nor state array. `Select::trigger_rect` says why.
    trigger_rect: (Point, Size),
}

/// A picker over its own options.
pub type PickList = Chooser<PICKLIST>;
/// A picker whose trigger filters as you type.
pub type Combobox = Chooser<COMBOBOX>;
/// A command list, opened by the application.
pub type Menu = Chooser<MENU>;

impl<const KIND: u8> Default for Chooser<KIND> {
    fn default() -> Self {
        let mut list = Popup::<OptionList>::default();
        list.anchor = Anchor::Below;
        Chooser::<KIND> {
            trigger: crate::input::TextInput::default(),
            list,
            selected: None,
            typed: false,
            trigger_rect: (Point { x: 0.0, y: 0.0 }, Size::default()),
        }
    }
}

impl<const KIND: u8> Chooser<KIND> {
    const TRIGGER: usize = 1;
    const POPUP: usize = 2;

    pub fn new(options: &[&'static str]) -> Self {
        let mut c = Chooser::<KIND>::default();
        c.list.content.set_options(options);
        if KIND != MENU && c.list.content.count() > 0 {
            c.select(0);
        }
        c
    }

    /// Whether the list is showing.
    pub fn is_open(&self) -> bool {
        self.list.open
    }

    pub fn open(&mut self) -> bool {
        let was = self.list.open;
        self.list.open = true;
        !was
    }

    pub fn close(&mut self) -> bool {
        let was = self.list.open;
        self.list.open = false;
        was
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Choose option `i`, and put its text in the trigger.
    pub fn select(&mut self, i: usize) -> bool {
        if i >= self.list.content.count() {
            return false;
        }
        let moved = self.selected != Some(i);
        self.selected = Some(i);
        let text = self.list.content.option(i);
        self.trigger.set_str(text);
        // A committed choice is not a query: the trigger now shows the
        // selection, and the list must go back to showing everything.
        self.typed = false;
        self.list.content.filter("");
        moved
    }

    /// What the trigger currently reads.
    pub fn text(&self) -> &str {
        self.trigger.as_str()
    }

    /// Whether the trigger holds a typed query rather than the
    /// selection.
    pub fn is_query(&self) -> bool {
        self.typed
    }
}

pub struct ChooserTable<const KIND: u8>;

impl<const KIND: u8> ChooserTable<KIND> {
    const N: usize = 1 + 1 + <Popup<OptionList> as Widget>::NODE_COUNT;

    const ARRAY: [NodeDesc; MAX_CHOOSER_NODES] = {
        assert!(Self::N <= MAX_CHOOSER_NODES, "Chooser exceeds MAX_CHOOSER_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_CHOOSER_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        t[1] = NodeDesc::new(0, 0, 2, 0, NodeDesc::FOCUSABLE).with_id(PART_TRIGGER);
        lumen_static::widget::splice_child(
            &mut t,
            2,
            <Popup<OptionList> as Widget>::SUB_TABLE,
            0,
        );
        t[2] = t[2].with_id(PART_POPUP);
        t[3] = t[3].with_id(PART_LIST);
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(Self::N).0;

    const SEED_ARRAY: [Seed; MAX_CHOOSER_NODES] = {
        let mut s = [Seed::WIDGET; MAX_CHOOSER_NODES];
        s[0] = Seed::new(
            <Chooser<KIND> as Widget>::KIND,
            <Chooser<KIND> as Widget>::type_defaults, Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::VISIBLE]));
        s[1] = <crate::input::TextInput<alloc::string::String> as Widget>::SEEDS[0];
        lumen_static::widget::splice_seeds(&mut s, 2, <Popup<OptionList> as Widget>::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

impl<const KIND: u8> Widget for Chooser<KIND> {
    const NODE_COUNT: usize = ChooserTable::<KIND>::N;
    const DEPTH: usize = 1 + <Popup<OptionList> as Widget>::DEPTH;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = ChooserTable::<KIND>::SLICE;
    const KIND: &'static str = match KIND {
        PICKLIST => "picklist",
        COMBOBOX => "combobox",
        _ => "menu",
    };
    const SEEDS: &'static [Seed] = ChooserTable::<KIND>::SEED_SLICE;

    fn type_defaults() -> Style {
        Style { width: Length::Fixed(220.0), height: Length::Shrink, ..Style::default() }
    }
}

impl<const KIND: u8> Node for Chooser<KIND> {
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
        // **The list's choice is taken here**, where `&mut self` and the
        // state array are both in hand. `on_event` reaches only the node
        // that was hit, and the node hit inside an open list is the
        // *list*, not this — the same reason `Tooltip` reads hover here
        // and `Glass` reads its press here.
        if let Some(idx) = self.list.content.take_picked() {
            self.select(idx);
            self.list.open = false;
        }
        // **A query is what the trigger holds when it is no longer
        // showing the selection** — derived, not tracked.
        //
        // The flag was set from `on_event` first, and it never fired: a
        // `Text` event goes to the *focused* node, which is the trigger
        // itself, and the trigger consumes it — so the wrapper's arm was
        // only ever reached for events the field had declined. Deriving
        // it needs no plumbing and cannot go stale.
        if KIND == COMBOBOX {
            self.typed = match self.selected {
                Some(k) => self.trigger.as_str() != self.list.content.option(k),
                None => !self.trigger.as_str().is_empty(),
            };
        }
        // A combobox filters on what the trigger reads — but only once
        // it has been *typed in*. See `typed`.
        if KIND == COMBOBOX && self.typed {
            self.list.content.filter(self.trigger.as_str());
        }
        // **It opens on focus, not on a press.** The trigger is a
        // `TextInput` and consumes the `PointerDown` itself — the
        // bubbling walk stops at the node that took the event, so an
        // arm here never ran and the list never opened. Focus is the
        // signal that survives: `dispatch` sets it from the *hit chain*
        // before any node is asked, so it is true by the time this runs.
        //
        // It also reads better than a press arm would: a chooser reached
        // by Tab opens too, which a keyboard user needs and a press arm
        // could not give them.
        //
        // A dismissing press clears focus (`dispatch_overlaid`), so this
        // cannot re-open what that press just closed — the dropdown bug
        // `Select` avoids, avoided here by the same accident of ordering.
        if KIND != MENU && st[i + Self::TRIGGER].focused {
            self.list.open = true;
        }
        let t = if KIND == MENU {
            // A menu has no trigger to show. Zeroed rather than absent,
            // so the table's shape does not depend on the kind.
            lumen_static::ops::hide_subtree(st, i + Self::TRIGGER, limits, 1)
        } else {
            self.trigger.layout(cx, limits, style, st, i + Self::TRIGGER)
        };
        self.list.layout(cx, limits, style, st, i + Self::POPUP);
        let s = lumen_static::ops::resolve_self(limits, &me, t);
        self.trigger_rect = (Point { x: 0.0, y: 0.0 }, t);
        lumen_static::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.trigger.place(origin, style, st, i + Self::TRIGGER);
        self.trigger_rect = (origin, st[i + Self::TRIGGER].size);
        // Below the trigger — `Anchor::Below` places the content *at*
        // the point it is given, as `Select` does with its trigger's
        // bottom edge.
        self.list.place(
            Point { x: origin.x, y: origin.y + st[i + Self::TRIGGER].size.h },
            style,
            st,
            i + Self::POPUP,
        );
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        if KIND != MENU {
            self.trigger.paint(st, i + Self::TRIGGER, out);
        }
        self.list.paint(st, i + Self::POPUP, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<lumen_static::Hit> {
        // The open list first: it is an overlay and is above the trigger.
        if let Some(h) = self.list.hit(st, i + Self::POPUP, x, y) {
            return Some(h);
        }
        if KIND != MENU {
            if let Some(h) = self.trigger.hit(st, i + Self::TRIGGER, x, y) {
                return Some(h);
            }
        }
        None
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            Event::Key { key: Key::Escape, down: true } if self.list.open => {
                self.list.open = false;
                true
            }
            // Anything else reaching a combobox is the field's. It
            // normally will not: the field is the focused node and gets
            // its own events first. This is the path for an event that
            // bubbled past it.
            _ if KIND == COMBOBOX => self.trigger.on_event(ev, cx),
            _ => false,
        }
    }

    fn node(&self, k: usize) -> &dyn Node {
        match k {
            0 => self,
            1 => &self.trigger,
            _ => self.list.node(k - Self::POPUP),
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        match k {
            0 => self,
            1 => &mut self.trigger,
            _ => self.list.node_mut(k - Self::POPUP),
        }
    }

    #[cfg(feature = "observe")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut alloc::vec::Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::ComboBox);
        node.set_value(self.trigger.as_str());
        node.set_expanded(self.list.open);
    }
}

impl<const KIND: u8, A> lumen_static::event::Handlers<A> for Chooser<KIND> {
    fn handler(
        &self,
        _sel: lumen_static::event::NodeSel,
    ) -> Option<fn(&mut A, &EventCtx)> {
        None
    }
}
