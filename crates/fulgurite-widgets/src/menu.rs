//! M90/T90.4 — context menus: `ContextMenu<W>` and its row list,
//! `CommandList` (`docs/plan-desktop-integration.md`, "`ContextMenu<W>`
//! and `CommandList`"); M90/T90.5 — the drawn menu bar, `MenuBar`.
//!
//! # What a context menu is here
//!
//! **A wrapper that answers one event.** Every route to a context menu —
//! a right press, Shift+F10 or the Menu key on a focused control, a touch
//! held still, an assistive technology's `ShowContextMenu` — reaches the
//! tree as one [`Event::ContextRequest`] at a window point, bubbling from
//! the node under it (M90/T90.2, T90.3). `ContextMenu<W>` sits around a
//! subtree, consumes the request that bubbles to it, and opens a
//! [`Popup`]`<CommandList>` with its top-left **at that point**
//! ([`Anchor::At`]), kept on the window by `Popup::place`'s clamp.
//!
//! # A menu is data; a choice is an id
//!
//! The rows are a `&'static [MenuEntry]` (`fulgurite::menu`), which
//! entries are enabled is a [`MenuMask`] (state, written by the
//! application or bound), and a chosen row is its entry's `id`, taken
//! **once** by the host through [`Node::take_menu`] from the list that
//! consumed the release or the Enter — never from this wrapper, so no
//! wrapper forwards it — and handed to the program's `Program::on_menu`
//! hook. A native menu's activation (`Event::Menu`) ends in the same hook.
//! An application without the hook is never asked, and pays nothing.
//!
//! # Opening, choosing, dismissing
//!
//! - **Open**: the request; the list then takes focus (the host's
//!   overlay focus, M57/A12), so the keyboard can choose at once.
//! - **Choose**: a release over an enabled row, or Up/Down/Home/End and
//!   Enter or Space. A disabled row is drawn dimmed, skipped by the
//!   keyboard, and a release over it chooses nothing and leaves the list
//!   open. A choice closes the list on the next layout, and focus goes
//!   back to where it was.
//! - **Dismiss**: Escape, or a press outside — left or right — through
//!   the pass's overlay dismissal, which closes the popup directly. A
//!   right press elsewhere inside the wrapper both dismisses and asks
//!   again, so the menu moves there.
//!
//! Not built (`docs/plan-desktop-integration.md`): submenus, check and
//! radio entries, mnemonics.

use alloc::string::String;
use alloc::vec::Vec;

use fulgurite::desc::{Constraints, NodeDesc, NodeState, Point, Size, NO_PARENT};
use fulgurite::display::{Color, Painter};
use fulgurite::event::{Event, EventCtx, Key, Mods};
use fulgurite::menu::{MenuEntry, MenuMask, MenuSection, MAX_ENTRIES};
use fulgurite::style::{Fields, Length, Seed, Style};
use fulgurite::widget::{Node, Widget};
use fulgurite::{Edges, LayoutCx};

use crate::popup::{Anchor, Popup};

/// A row's height — `OptionList`'s, so a menu and a chooser read alike.
const ROW_H: f32 = 30.0;

// ---- CommandList -----------------------------------------------------

/// **A menu's rows**: one node that draws and hits all of them, as
/// `OptionList` does for a chooser — the rows are the entries'.
///
/// Its interface upward is two takes: [`Node::take_menu`], which the host
/// asks for the chosen entry's id, and [`CommandList::take_done`], which
/// its owner asks to close the popup. Both are take-once, so one release
/// runs one command.
pub struct CommandList {
    entries: &'static [MenuEntry],
    /// Each entry's shortcut as drawn ("Ctrl+S"), built once with the
    /// entries so paint allocates nothing.
    accels: Vec<String>,
    enabled: MenuMask,
    /// The row under the pointer, for the hover affordance.
    hot: Option<usize>,
    /// The keyboard's row.
    highlight: Option<usize>,
    /// The chosen entry's id, until the host takes it.
    picked: Option<u32>,
    /// A row was chosen, until the owner takes it and closes.
    done: bool,
    /// Laid-out height, so `on_event` can turn `local.y` into a row.
    height: f32,
    /// The top padding, for the same conversion.
    pad_top: f32,
}

impl Default for CommandList {
    fn default() -> Self {
        CommandList {
            entries: &[],
            accels: Vec::new(),
            enabled: MenuMask::ALL,
            hot: None,
            highlight: None,
            picked: None,
            done: false,
            height: 0.0,
            pad_top: 0.0,
        }
    }
}

impl CommandList {
    /// A list of `entries`, all enabled.
    ///
    /// # Panics
    ///
    /// With more than [`MAX_ENTRIES`] entries, naming the count: the
    /// enabled mask has one bit per entry.
    pub fn new(entries: &'static [MenuEntry]) -> Self {
        let mut l = CommandList::default();
        l.set_entries(entries);
        l
    }

    /// Replace the entries. The keyboard's row and the hover are cleared.
    pub fn set_entries(&mut self, entries: &'static [MenuEntry]) {
        assert!(
            entries.len() <= MAX_ENTRIES,
            "a menu holds at most {MAX_ENTRIES} entries (one bit each of its MenuMask); this one has {}",
            entries.len()
        );
        self.entries = entries;
        self.accels = entries.iter().map(|e| e.accel.map_or_else(String::new, |a| accel_label(a.key, a.mods))).collect();
        self.hot = None;
        self.highlight = None;
    }

    pub fn entries(&self) -> &'static [MenuEntry] {
        self.entries
    }

    /// Which entries are enabled (bit *n*, entry *n*).
    pub fn set_enabled(&mut self, mask: MenuMask) {
        self.enabled = mask;
        if self.highlight.is_some_and(|h| !mask.enabled(h)) {
            self.highlight = None;
        }
    }

    pub fn enabled(&self) -> MenuMask {
        self.enabled
    }

    /// The keyboard's row.
    pub fn highlight(&self) -> Option<usize> {
        self.highlight
    }

    /// Whether a row was chosen since the last take — the owner closes on
    /// it. Taking clears it.
    pub fn take_done(&mut self) -> bool {
        core::mem::take(&mut self.done)
    }

    /// Forget the hover and the keyboard's row — a menu opens fresh.
    pub fn reset(&mut self) {
        self.hot = None;
        self.highlight = None;
    }

    /// Fresh, with the keyboard's row on the first enabled entry — a menu
    /// opened from the keyboard (M90/T90.5's `MenuBar`), so Enter chooses
    /// at once.
    pub fn reset_to_first(&mut self) {
        self.reset();
        self.step(1);
    }

    fn row_at(&self, y: f32) -> Option<usize> {
        if self.entries.is_empty() || y < 0.0 {
            return None;
        }
        let k = (y / ROW_H) as usize;
        (k < self.entries.len()).then_some(k)
    }

    fn choose(&mut self, k: usize) -> bool {
        if !self.enabled.enabled(k) {
            return false;
        }
        self.picked = Some(self.entries[k].id);
        self.done = true;
        true
    }

    /// Move the keyboard's row by `d` among the enabled entries, stopping
    /// at the ends; from nothing, down starts at the first and up at the
    /// last.
    fn step(&mut self, d: i32) -> bool {
        let on: Vec<usize> = (0..self.entries.len()).filter(|&k| self.enabled.enabled(k)).collect();
        if on.is_empty() {
            return false;
        }
        let at = self.highlight.and_then(|h| on.iter().position(|&k| k == h));
        let next = match at {
            Some(p) => (p as i32 + d).clamp(0, on.len() as i32 - 1) as usize,
            None if d >= 0 => 0,
            None => on.len() - 1,
        };
        let was = self.highlight;
        self.highlight = Some(on[next]);
        was != self.highlight
    }

    fn key(&mut self, key: Key) -> bool {
        match key {
            Key::Enter | Key::Space => self.highlight.is_some_and(|k| self.choose(k)),
            Key::Down => self.step(1),
            Key::Up => self.step(-1),
            Key::Home => {
                self.highlight = None;
                self.step(1)
            }
            Key::End => {
                self.highlight = None;
                self.step(-1)
            }
            _ => false,
        }
    }
}

/// "Ctrl+Shift+S" — a shortcut as a menu row shows it.
fn accel_label(key: Key, mods: Mods) -> String {
    let mut s = String::new();
    for (bit, name) in [(Mods::CTRL, "Ctrl+"), (Mods::ALT, "Alt+"), (Mods::SHIFT, "Shift+"), (Mods::META, "Super+")] {
        if mods.has(bit) {
            s.push_str(name);
        }
    }
    match key {
        Key::Char(c) => s.extend(c.to_uppercase()),
        Key::F(n) => {
            s.push('F');
            s.push_str(&alloc::format!("{n}"));
        }
        Key::Enter => s.push_str("Enter"),
        Key::Delete => s.push_str("Del"),
        Key::Backspace => s.push_str("Backspace"),
        Key::Insert => s.push_str("Ins"),
        Key::Escape => s.push_str("Esc"),
        Key::Space => s.push_str("Space"),
        Key::Tab => s.push_str("Tab"),
        Key::Home => s.push_str("Home"),
        Key::End => s.push_str("End"),
        Key::PageUp => s.push_str("PgUp"),
        Key::PageDown => s.push_str("PgDn"),
        Key::Left => s.push_str("Left"),
        Key::Right => s.push_str("Right"),
        Key::Up => s.push_str("Up"),
        Key::Down => s.push_str("Down"),
        Key::Menu => s.push_str("Menu"),
    }
    s
}

impl Widget for CommandList {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "commandlist";
    const SEEDS: &'static [Seed] =
        &[Seed::new(<CommandList as Widget>::KIND, <CommandList as Widget>::type_defaults, Fields::of(&[
                Fields::WIDTH,
                Fields::HEIGHT,
                Fields::PADDING,
                Fields::BACKGROUND,
                Fields::BORDER,
                Fields::BACKGROUND_FILL,
                Fields::COLOR,
                Fields::FONT_SIZE,
                Fields::RADIUS,
                Fields::SHADOW,
                Fields::VISIBLE,
            ]),).with_roles(fulgurite::style::SeedRoles::NONE.color(fulgurite::style::Role::Text).background(fulgurite::style::Role::Raised))];

    /// `OptionList`'s look: a raised panel with a shadow.
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

impl Node for CommandList {
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
            return fulgurite::ops::hide_subtree(st, i, limits, 1);
        }
        let content = Size { w: limits.max_w, h: self.entries.len() as f32 * ROW_H + me.padding.vertical() };
        let s = fulgurite::ops::resolve_self(limits, &me, content);
        self.height = s.h;
        self.pad_top = me.padding.top;
        fulgurite::ops::record_painted(st, i, limits, s, &me)
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
        let p = s.pad;
        let muted = Color(s.fg.0, s.fg.1, s.fg.2, 0xa0);
        out.push_clip(r);
        for (k, e) in self.entries.iter().enumerate() {
            let y = r[1] + p.1 + ROW_H * k as f32;
            if y > r[3] {
                break;
            }
            let on = self.enabled.enabled(k);
            let lit = on && (Some(k) == self.hot || Some(k) == self.highlight);
            if lit {
                out.rounded_rect([r[0] + 4.0, y, r[2] - 4.0, y + ROW_H], s.fg, 6.0);
            }
            // A disabled row is dimmed, never hidden: what the menu can do
            // is still worth reading.
            let col = if lit { s.bg } else if on { s.fg } else { muted };
            let text = [r[0] + p.0, y + ROW_H / 2.0 - s.font_size * 0.6, r[2] - p.2, y + ROW_H];
            out.text_weighted(text, e.label, s.font_size, s.font_weight, col);
            let accel = self.accels.get(k).map_or("", String::as_str);
            if !accel.is_empty() {
                // Right-aligned by estimate: a menu row has no measurer
                // in paint, and the label is left-aligned well clear.
                let w = accel.chars().count() as f32 * s.font_size * 0.55;
                let ac = if lit { s.bg } else { muted };
                out.text_weighted([r[2] - p.2 - w, text[1], r[2] - p.2, text[3]], accel, s.font_size, s.font_weight, ac);
            }
        }
        out.pop_clip();
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        match ev {
            // `Hover`, not `PointerMove`: a move reaches a widget only under
            // capture (M27/T27.11).
            Event::Leave => self.hot.take().is_some(),
            Event::Hover { .. } => {
                let was = self.hot;
                self.hot = self.row_at(cx.local.y - self.pad_top);
                was != self.hot
            }
            // **On the release**, as every menu commits; a release over a
            // disabled row (or the padding) is the menu's and chooses
            // nothing, so the list stays open.
            Event::PointerUp { .. } => {
                if let Some(k) = self.row_at(cx.local.y - self.pad_top) {
                    self.choose(k);
                }
                true
            }
            Event::Key { key, down: true } => self.key(*key),
            _ => false,
        }
    }

    /// Up, Down, Home and End walk the rows (M44/T44.1).
    fn owns_arrows(&self) -> bool {
        true
    }

    fn take_menu(&mut self) -> Option<u32> {
        self.picked.take()
    }

    fulgurite::leaf_common!(CommandList, 0);

    /// A menu: its entries' labels as its value, comma-separated as
    /// `OptionList`'s are, so a drive reads what it offers without pixels.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        _st: &[NodeState],
        _i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::Menu);
        node.set_size_of_set(self.entries.len());
        let mut v = String::new();
        for e in self.entries {
            if !v.is_empty() {
                v.push(',');
            }
            v.push_str(e.label);
        }
        node.set_value(v);
    }
}

impl<S> fulgurite::event::Handlers<S> for CommandList {
    fn handler(&self, _sel: fulgurite::event::NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

// ---- ContextMenu -----------------------------------------------------

/// Part ids — the wrapped content, the popup and its row list, addressed
/// through the owner (`#owner > #context-menu-list`), since two context
/// menus in one tree carry the same ones (`W1011`). The content's id
/// replaces whatever its own table gave its root, as `Tooltip`'s does.
pub const PART_CONTENT: &str = "context-menu-content";
pub const PART_POPUP: &str = "context-menu-popup";
pub const PART_LIST: &str = "context-menu-list";

/// Upper bound on `1 + W::NODE_COUNT + 2` for one [`ContextMenu`].
pub const MAX_CONTEXT_NODES: usize = 256;

/// **A subtree with a context menu** — see the module docs.
///
/// Transparent: it is as big as its content, hits only what its content
/// hits, and paints nothing of its own. What it adds is the answer to
/// `Event::ContextRequest`.
pub struct ContextMenu<W: Widget> {
    pub content: W,
    pub menu: Popup<CommandList>,
    /// Which entries are enabled — the application's state, written here
    /// or through a binding (M33's value form), and handed to the list at
    /// layout.
    pub enabled: MenuMask,
}

impl<W: Widget + Default> Default for ContextMenu<W> {
    fn default() -> Self {
        ContextMenu::new(W::default(), &[])
    }
}

impl<W: Widget> ContextMenu<W> {
    /// Content index, and the popup's, relative to this node.
    const AT: usize = 1;
    const POPUP: usize = 1 + W::NODE_COUNT;

    /// `content`, with a menu of `entries`, all enabled.
    pub fn new(content: W, entries: &'static [MenuEntry]) -> Self {
        let mut menu = Popup::new(CommandList::new(entries));
        menu.open = false;
        ContextMenu { content, menu, enabled: MenuMask::ALL }
    }

    /// Whether the menu is showing.
    pub fn is_open(&self) -> bool {
        self.menu.open
    }

    /// Open the menu with its top-left at window point `(x, y)` — what the
    /// request does; for a host that drives the widget directly.
    pub fn open_at(&mut self, x: f32, y: f32) {
        self.menu.anchor = Anchor::At { x: x.round() as i32, y: y.round() as i32 };
        self.menu.open = true;
        self.menu.content.reset();
    }

    /// Close it directly.
    pub fn close(&mut self) {
        self.menu.open = false;
    }
}

pub struct ContextMenuTable<W>(core::marker::PhantomData<W>);

impl<W: Widget> ContextMenuTable<W> {
    const N: usize = 1 + W::NODE_COUNT + <Popup<CommandList> as Widget>::NODE_COUNT;

    const ARRAY: [NodeDesc; MAX_CONTEXT_NODES] = {
        assert!(Self::N <= MAX_CONTEXT_NODES, "ContextMenu exceeds MAX_CONTEXT_NODES");
        let mut t = [NodeDesc::EMPTY; MAX_CONTEXT_NODES];
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 2, NodeDesc::CONTAINER);
        fulgurite::widget::splice_child(&mut t, 1, W::SUB_TABLE, 0);
        let pop = 1 + W::NODE_COUNT;
        fulgurite::widget::splice_child(&mut t, pop, <Popup<CommandList> as Widget>::SUB_TABLE, 0);
        t[1].next_sibling = pop as u32;
        t[1] = t[1].with_id(PART_CONTENT);
        t[pop] = t[pop].with_id(PART_POPUP);
        // **The list takes focus**, so a menu opened from the keyboard is
        // chosen from the keyboard: the host moves focus into an overlay
        // that opens (M57/A12) and back when it closes. A closed popup has
        // no area, so it is a Tab stop only while open.
        t[pop + 1] = t[pop + 1].with_id(PART_LIST);
        t[pop + 1].flags |= NodeDesc::FOCUSABLE;
        t
    };
    pub const SLICE: &'static [NodeDesc] = Self::ARRAY.split_at(Self::N).0;

    const SEED_ARRAY: [Seed; MAX_CONTEXT_NODES] = {
        let mut s = [Seed::WIDGET; MAX_CONTEXT_NODES];
        s[0] = Seed::new(
            <ContextMenu<W> as Widget>::KIND,
            <ContextMenu<W> as Widget>::type_defaults,
            Fields::of(&[Fields::WIDTH, Fields::HEIGHT, Fields::VISIBLE]),
        );
        fulgurite::widget::splice_seeds(&mut s, 1, W::SEEDS);
        fulgurite::widget::splice_seeds(&mut s, 1 + W::NODE_COUNT, <Popup<CommandList> as Widget>::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = Self::SEED_ARRAY.split_at(Self::N).0;
}

impl<W: Widget> Widget for ContextMenu<W> {
    const NODE_COUNT: usize = ContextMenuTable::<W>::N;
    const DEPTH: usize = 1 + {
        let (a, b) = (W::DEPTH, <Popup<CommandList> as Widget>::DEPTH);
        if a > b {
            a
        } else {
            b
        }
    };
    /// As rigid as its content: the menu is out of flow.
    const IS_RIGID: bool = W::IS_RIGID;
    const SUB_TABLE: &'static [NodeDesc] = ContextMenuTable::<W>::SLICE;
    const KIND: &'static str = "contextmenu";
    const SEEDS: &'static [Seed] = ContextMenuTable::<W>::SEED_SLICE;

    /// **Shrink on both axes**, as `Tooltip`: adding a menu to a control
    /// must not change the layout it sits in.
    fn type_defaults() -> Style {
        Style { width: Length::Shrink, height: Length::Shrink, ..Style::default() }
    }
}

impl<W: Widget> Node for ContextMenu<W> {
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
        // **A choice closes the menu** — taken here, where `&mut self` is
        // in hand; the release that chose reached the list, not this node
        // (`Chooser` takes its list's choice the same way).
        if self.menu.content.take_done() {
            self.menu.open = false;
        }
        self.menu.content.set_enabled(self.enabled);
        let c = self.content.layout(cx, limits, style, st, i + Self::AT);
        self.menu.layout(cx, limits, style, st, i + Self::POPUP);
        fulgurite::ops::record_painted(st, i, limits, c, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.content.place(origin, style, st, i + Self::AT);
        // `Anchor::At` ignores the point it is handed; the popup places
        // itself at the request's.
        self.menu.place(origin, style, st, i + Self::POPUP);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        self.content.paint(st, i + Self::AT, out);
        // Defers itself into the overlay phase.
        self.menu.paint(st, i + Self::POPUP, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<fulgurite::Hit> {
        // The open menu first: it is drawn over the content. Transparent
        // otherwise — what the content does not take is not the wrapper's.
        self.menu.hit(st, i + Self::POPUP, x, y).or_else(|| self.content.hit(st, i + Self::AT, x, y))
    }

    /// **The request that bubbled here opens the menu at its point** —
    /// from inside the content, or from the open menu itself (a right
    /// press on it moves it). Consumed, so it goes no further up.
    fn on_event(&mut self, ev: &Event, _cx: &EventCtx) -> bool {
        match *ev {
            Event::ContextRequest { x, y } => {
                self.open_at(x, y);
                true
            }
            _ => false,
        }
    }

    fn node(&self, k: usize) -> &dyn Node {
        match k {
            0 => self,
            _ if k < Self::POPUP => self.content.node(k - Self::AT),
            _ => self.menu.node(k - Self::POPUP),
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        match k {
            0 => self,
            _ if k < Self::POPUP => self.content.node_mut(k - Self::AT),
            _ => self.menu.node_mut(k - Self::POPUP),
        }
    }

    #[cfg(feature = "alloc")]
    fn arena_count(&self) -> usize {
        self.content.arena_count()
    }

    #[cfg(feature = "alloc")]
    fn arena_mut(&mut self, a: usize) -> Option<fulgurite::arena::ArenaMut<'_>> {
        self.content.arena_mut(a)
    }

    /// **A container that claims a context menu** — `GenericContainer`,
    /// since its content speaks for itself, with AccessKit's
    /// `ShowContextMenu` in its actions: a reader's request reaches
    /// dispatch as `Action::ContextMenu`, which asks at this node's centre
    /// and bubbles to this node (M90/T90.3). A claim, like
    /// `PullToRefresh`'s.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        st: &[NodeState],
        i: usize,
        _id: u64,
        node: &mut accesskit::Node,
        _extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::GenericContainer);
        // None on a node with no area: dispatch refuses an action on a node
        // that is not on screen (`Chooser::observe`'s rule).
        if st[i].size.w > 0.0 && st[i].size.h > 0.0 {
            node.add_action(accesskit::Action::ShowContextMenu);
        }
    }
}

impl<W, S> fulgurite::event::Handlers<S> for ContextMenu<W>
where
    W: Widget + fulgurite::event::Handlers<S>,
{
    fn handler(&self, sel: fulgurite::event::NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        // A wrapper is not a control, and the list's rows run no handler:
        // a choice is the program's menu hook.
        if sel.i == 0 || sel.i >= Self::POPUP {
            return None;
        }
        self.content.handler(fulgurite::event::NodeSel { i: sel.i - Self::AT, ..sel })
    }
}

// ---- MenuBar ---------------------------------------------------------

/// Part ids — the bar's popup and its row list, addressed through the
/// owner (`#owner > #menu-bar-list`), as a context menu's are.
pub const PART_BAR_POPUP: &str = "menu-bar-popup";
pub const PART_BAR_LIST: &str = "menu-bar-list";

/// The horizontal padding inside each title.
const TITLE_PAD: f32 = 12.0;

/// **The menu bar, drawn by the framework** — M90/T90.5
/// (`docs/plan-desktop-integration.md`, "The menu bar is drawn by the
/// framework, on every platform"; owner decision D2: on Linux the bar is
/// drawn from the same menu data, and native menus are deferred).
///
/// A row of titles from the same `&'static [MenuSection]` the program
/// declares with `Program::menu`, each opening a [`Popup`]`<CommandList>`
/// of its section's entries below it. The application places it: a static
/// tree has no slot the framework could insert it into.
///
/// - **Pointer.** A press on a title opens its menu; a press on the open
///   title (or anywhere outside the menu) closes it, through the pass's
///   outside-press dismissal. With a menu open, hovering another title
///   switches to its menu.
/// - **Keyboard.** The bar is **one Tab stop**, as WAI-ARIA's menubar
///   pattern has it, with a roving title: Left and Right move between
///   titles, Down, Enter or Space open the current one with its first
///   enabled row highlighted, and the list takes focus. With a menu open,
///   Left and Right move to the neighbouring title's menu (the list
///   declines them and they bubble here). Escape closes it and focus goes
///   back to the bar on the title it opened from (M44/T44.9's path).
/// - **Choosing** is the `CommandList`'s: a release on an enabled row or
///   Enter; the host takes the entry's id ([`Node::take_menu`]) and runs
///   the program's `Program::on_menu` hook, as for a context menu.
/// - **Enabled** entries are [`MenuBar::enabled`], bit *n* for the *n*-th
///   entry in flattened order — the mask `Program::menu`'s function
///   computes, written here by the application from the same function.
///   A disabled row is dimmed and chooses nothing.
///
/// The accelerators the rows show are matched by the host, not the bar
/// (`Program::menu`): a bar is not needed for Ctrl+S to work.
///
/// Not built (reopen: an application that needs one): F10 or Alt focusing
/// the bar, mnemonics, submenus, check and radio entries.
pub struct MenuBar {
    sections: &'static [MenuSection],
    pub menu: Popup<CommandList>,
    /// Which entries are enabled, in flattened order.
    pub enabled: MenuMask,
    /// The roving title: the keyboard's, and the open menu's.
    title: usize,
    /// The title under the pointer, for the hover affordance.
    hot: Option<usize>,
    /// Each title's `[x0, x1)` from the bar's left, from layout.
    spans: Vec<(f32, f32)>,
    /// The bar's top-left and height, from `place`/`layout`, so a menu
    /// hangs below its title.
    origin: Point,
    height: f32,
}

impl Default for MenuBar {
    fn default() -> Self {
        MenuBar::new(&[])
    }
}

impl MenuBar {
    const POPUP: usize = 1;

    /// A bar of `sections`, every entry enabled.
    ///
    /// # Panics
    ///
    /// With more than [`MAX_ENTRIES`] entries across its sections, naming
    /// the count: the enabled mask has one bit per entry.
    pub fn new(sections: &'static [MenuSection]) -> Self {
        let n: usize = sections.iter().map(|s| s.entries.len()).sum();
        assert!(
            n <= MAX_ENTRIES,
            "a menu bar holds at most {MAX_ENTRIES} entries (one bit each of its MenuMask); this one has {n}"
        );
        let mut menu = Popup::new(CommandList::default());
        menu.open = false;
        MenuBar {
            sections,
            menu,
            enabled: MenuMask::ALL,
            title: 0,
            hot: None,
            spans: Vec::new(),
            origin: Point { x: 0.0, y: 0.0 },
            height: 0.0,
        }
    }

    pub fn sections(&self) -> &'static [MenuSection] {
        self.sections
    }

    /// The open menu's title, if one is open.
    pub fn open_title(&self) -> Option<usize> {
        self.menu.open.then_some(self.title)
    }

    /// The roving title — the keyboard's, and the open menu's.
    pub fn title(&self) -> usize {
        self.title
    }

    /// Title `k`'s `[x0, x1)` from the bar's left edge, as the last layout
    /// measured it.
    pub fn title_span(&self, k: usize) -> Option<(f32, f32)> {
        self.spans.get(k).copied()
    }

    /// Open title `k`'s menu; `keyboard` highlights its first enabled row.
    pub fn open(&mut self, k: usize, keyboard: bool) {
        let Some(sec) = self.sections.get(k) else { return };
        self.title = k;
        self.menu.content.set_entries(sec.entries);
        self.menu.content.set_enabled(self.section_mask(k));
        if keyboard {
            self.menu.content.reset_to_first();
        } else {
            self.menu.content.reset();
        }
        self.menu.open = true;
    }

    /// Close the open menu, if any.
    pub fn close(&mut self) {
        self.menu.open = false;
    }

    /// Section `k`'s slice of [`MenuBar::enabled`].
    fn section_mask(&self, k: usize) -> MenuMask {
        let off: usize = self.sections[..k].iter().map(|s| s.entries.len()).sum();
        MenuMask(self.enabled.0.checked_shr(off as u32).unwrap_or(0))
    }

    /// The title under bar-local `x`.
    fn title_at(&self, x: f32) -> Option<usize> {
        self.spans.iter().position(|&(a, b)| x >= a && x < b)
    }

    /// The title `d` steps from the current one, wrapping.
    fn step(&self, d: i32) -> usize {
        let n = self.sections.len().max(1) as i32;
        (self.title as i32 + d).rem_euclid(n) as usize
    }
}

impl Widget for MenuBar {
    const NODE_COUNT: usize = 1 + <Popup<CommandList> as Widget>::NODE_COUNT;
    const DEPTH: usize = 1 + <Popup<CommandList> as Widget>::DEPTH;
    const IS_RIGID: bool = false;
    const SUB_TABLE: &'static [NodeDesc] = MenuBarTable::SLICE;
    const KIND: &'static str = "menubar";
    const SEEDS: &'static [Seed] = MenuBarTable::SEED_SLICE;

    /// A raised strip as wide as its titles; an application that wants it
    /// across the window styles its width `fill`.
    fn type_defaults() -> Style {
        Style {
            width: Length::Shrink,
            height: Length::Shrink,
            padding: Edges { left: 4.0, right: 4.0, top: 2.0, bottom: 2.0 },
            background: Color(0xff, 0xff, 0xff, 0xff),
            color: Color(0x30, 0x3a, 0x4c, 0xff),
            font_size: 14.0,
            radius: 6.0,
            // A hairline, so the strip reads as one on a surface of its own
            // colour; the palette's `border` role.
            border_width: 1,
            border_color: Color(0xd5, 0xdb, 0xe4, 0xff),
            ..Style::default()
        }
    }
}

pub struct MenuBarTable;

impl MenuBarTable {
    const N: usize = <MenuBar as Widget>::NODE_COUNT;
    const ARRAY: [NodeDesc; 3] = {
        assert!(Self::N == 3, "a MenuBar is its strip, a popup and a list");
        let mut t = [NodeDesc::EMPTY; 3];
        // **Focusable**: the bar is one Tab stop with a roving title.
        t[0] = NodeDesc::new(1, NO_PARENT, 0, 1, NodeDesc::CONTAINER | NodeDesc::FOCUSABLE);
        fulgurite::widget::splice_child(&mut t, 1, <Popup<CommandList> as Widget>::SUB_TABLE, 0);
        t[1] = t[1].with_id(PART_BAR_POPUP);
        // The list takes focus when it opens (M57/A12), so a menu opened
        // from the keyboard is chosen from the keyboard; focus goes back
        // to the bar when it closes.
        t[2] = t[2].with_id(PART_BAR_LIST);
        t[2].flags |= NodeDesc::FOCUSABLE;
        t
    };
    pub const SLICE: &'static [NodeDesc] = &Self::ARRAY;

    const SEED_ARRAY: [Seed; 3] = {
        let mut s = [Seed::WIDGET; 3];
        s[0] = Seed::new(<MenuBar as Widget>::KIND, <MenuBar as Widget>::type_defaults, Fields::of(&[
            Fields::WIDTH,
            Fields::HEIGHT,
            Fields::PADDING,
            Fields::BACKGROUND,
            Fields::BORDER,
            Fields::BACKGROUND_FILL,
            Fields::COLOR,
            Fields::FONT_SIZE,
            Fields::RADIUS,
            Fields::VISIBLE,
        ]))
        .with_roles(
            fulgurite::style::SeedRoles::NONE
                .color(fulgurite::style::Role::Text)
                .background(fulgurite::style::Role::Raised)
                .border(fulgurite::style::Role::Border),
        );
        fulgurite::widget::splice_seeds(&mut s, 1, <Popup<CommandList> as Widget>::SEEDS);
        s
    };
    pub const SEED_SLICE: &'static [Seed] = &Self::SEED_ARRAY;
}

impl Node for MenuBar {
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
        // **A choice closes the menu**, taken here as `ContextMenu` takes
        // it: the release reached the list, not this node.
        if self.menu.content.take_done() {
            self.menu.open = false;
        }
        if self.menu.open {
            let mask = self.section_mask(self.title);
            self.menu.content.set_enabled(mask);
        }
        // Each title's span, measured; the vector keeps its capacity, so a
        // steady frame allocates nothing.
        self.spans.clear();
        let mut x = me.padding.left;
        for sec in self.sections {
            let w = cx.text.measure_weighted(sec.title, me.font_size, me.font_weight, f32::INFINITY).w + 2.0 * TITLE_PAD;
            self.spans.push((x, x + w));
            x += w;
        }
        let content = Size { w: x + me.padding.right, h: ROW_H + me.padding.vertical() };
        let s = fulgurite::ops::resolve_self(limits, &me, content);
        self.height = s.h;
        // Flipping above clears the bar (M57/A18).
        self.menu.above_by = s.h;
        self.menu.layout(cx, limits, style, st, i + Self::POPUP);
        fulgurite::ops::record_painted(st, i, limits, s, &me)
    }

    fn place(&mut self, origin: Point, style: &[Style], st: &mut [NodeState], i: usize) {
        st[i].pos = origin;
        self.origin = origin;
        // Below the open title: `Anchor::Below` places the list *at* the
        // point, flipping and clamping like every dropdown.
        let x = self.spans.get(self.title).map_or(0.0, |s| s.0);
        self.menu.place(Point { x: origin.x + x, y: origin.y + st[i].size.h }, style, st, i + Self::POPUP);
    }

    fn paint(&self, st: &[NodeState], i: usize, out: &mut dyn Painter) {
        let s = &st[i];
        let r = fulgurite::ops::rect_of(s);
        if r[2] > r[0] && r[3] > r[1] {
            fulgurite::ops::fill(st, i, out);
            out.rounded_rect(r, s.bg, s.radius);
            fulgurite::ops::border(st, i, out);
            let tint = |a: u8| Color(s.fg.0, s.fg.1, s.fg.2, a);
            for (k, sec) in self.sections.iter().enumerate() {
                let Some(&(a, b)) = self.spans.get(k) else { break };
                let cell = [r[0] + a, r[1] + s.pad.1, r[0] + b, r[3] - s.pad.3];
                // The open title reads as pressed; the hovered one, or the
                // focused bar's roving one, as lit.
                let open = self.menu.open && k == self.title;
                let lit = Some(k) == self.hot || (s.focused && k == self.title);
                if open {
                    out.rounded_rect(cell, tint(0x2e), 6.0);
                } else if lit {
                    out.rounded_rect(cell, tint(0x16), 6.0);
                }
                let y = (cell[1] + cell[3]) / 2.0 - s.font_size * 0.6;
                out.text_weighted([cell[0] + TITLE_PAD, y, cell[2], cell[3]], sec.title, s.font_size, s.font_weight, s.fg);
            }
        }
        // Defers itself into the overlay phase.
        self.menu.paint(st, i + Self::POPUP, out);
    }

    fn hit(&self, st: &[NodeState], i: usize, x: f32, y: f32) -> Option<fulgurite::Hit> {
        // The open menu first: it is drawn over everything.
        if let Some(h) = self.menu.hit(st, i + Self::POPUP, x, y) {
            return Some(h);
        }
        if !fulgurite::hit::contains(&st[i], x, y) {
            return None;
        }
        let o = st[i].pos;
        Some(fulgurite::Hit { node: i as u32, inner: None, local: Point { x: x - o.x, y: y - o.y } })
    }

    fn on_event(&mut self, ev: &Event, cx: &EventCtx) -> bool {
        let own = cx.target == cx.node;
        match *ev {
            // A press on a title opens its menu. With a menu open a press
            // on the bar never arrives: it is outside the menu, so the
            // pass's dismissal closes it and consumes the press.
            Event::PointerDown { .. } if own => match self.title_at(cx.local.x) {
                Some(k) => {
                    self.open(k, false);
                    true
                }
                None => false,
            },
            // `Hover`, not `PointerMove` (M27/T27.11). With a menu open,
            // the pointer moving onto another title switches to its menu.
            Event::Hover { .. } if own => {
                let was = self.hot;
                self.hot = self.title_at(cx.local.x);
                if let Some(k) = self.hot {
                    if self.menu.open && k != self.title {
                        self.open(k, false);
                        return true;
                    }
                }
                was != self.hot
            }
            Event::Leave => self.hot.take().is_some(),
            // Left and Right: the neighbouring title — its menu, when one
            // is open (from the list, which declines them), or the roving
            // title on a focused bar.
            Event::Key { key: k @ (Key::Left | Key::Right), down: true } if !self.sections.is_empty() => {
                let to = self.step(if k == Key::Right { 1 } else { -1 });
                if self.menu.open {
                    self.open(to, true);
                } else {
                    self.title = to;
                }
                true
            }
            Event::Key { key: Key::Home, down: true } if own && !self.menu.open => {
                self.title = 0;
                true
            }
            Event::Key { key: Key::End, down: true } if own && !self.menu.open => {
                self.title = self.sections.len().saturating_sub(1);
                true
            }
            Event::Key { key: Key::Down | Key::Enter | Key::Space, down: true } if own && !self.menu.open && !self.sections.is_empty() => {
                self.open(self.title.min(self.sections.len() - 1), true);
                true
            }
            _ => false,
        }
    }

    /// Left, Right, Home and End move the roving title (M44/T44.1).
    fn owns_arrows(&self) -> bool {
        true
    }

    fn node(&self, k: usize) -> &dyn Node {
        match k {
            0 => self,
            _ => self.menu.node(k - Self::POPUP),
        }
    }

    fn node_mut(&mut self, k: usize) -> &mut dyn Node {
        match k {
            0 => self,
            _ => self.menu.node_mut(k - Self::POPUP),
        }
    }

    /// A menu bar: its titles, comma-separated, as its value, and one
    /// `MenuItem` child per title — its text the label, its drawn cell the
    /// bounds, expanded while its menu is open — so a reader meets each
    /// title where it is and the agent's `~=` finds one by its text. The
    /// open menu speaks for itself as a `Menu`.
    #[cfg(feature = "a11y")]
    fn observe(
        &self,
        st: &[NodeState],
        i: usize,
        id: u64,
        node: &mut accesskit::Node,
        extra: &mut Vec<(accesskit::NodeId, accesskit::Node)>,
    ) {
        node.set_role(accesskit::Role::MenuBar);
        let mut v = String::new();
        for s in self.sections {
            if !v.is_empty() {
                v.push(',');
            }
            v.push_str(s.title);
        }
        node.set_value(v);
        let s = &st[i];
        if s.size.w <= 0.0 || s.size.h <= 0.0 {
            return;
        }
        for (k, sec) in self.sections.iter().enumerate() {
            let Some(&(a, b)) = self.spans.get(k) else { break };
            let mut c = accesskit::Node::new(accesskit::Role::MenuItem);
            c.set_label(sec.title);
            c.set_bounds(accesskit::Rect {
                x0: (s.pos.x + a) as f64,
                y0: s.pos.y as f64,
                x1: (s.pos.x + b) as f64,
                y1: (s.pos.y + s.size.h) as f64,
            });
            c.set_expanded(self.menu.open && k == self.title);
            let cid = accesskit::NodeId(fulgurite::observe::instance_id(id, k as u64, 0));
            extra.push((cid, c));
            node.push_child(cid);
        }
    }
}

impl<S> fulgurite::event::Handlers<S> for MenuBar {
    fn handler(&self, _sel: fulgurite::event::NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        // A choice is the program's menu hook, not a handler.
        None
    }
}
