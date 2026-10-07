//! **A menu is data** — M90/T90.4 (`docs/plan-desktop-integration.md`,
//! "A menu is data, and every activation is one hook").
//!
//! A context menu, a drawn menu bar and (T90.6) a tray all describe their
//! commands with these types: `&'static` tables an application declares
//! once, and an enabled mask computed from its state. **No code, so no
//! firmware cost**: nothing here is called by dispatch, layout or paint;
//! the widgets that draw a menu (`CommandList`, `ContextMenu` in
//! `lumen-static-widgets`) and the host that runs its hook
//! (`Program::on_menu`) read the tables.
//!
//! What a chosen entry *means* reaches the application as its `id`, one
//! `u32`, through one hook, whichever route chose it: a drawn row taken
//! with [`Node::take_menu`](crate::widget::Node::take_menu), or a native
//! activation posted as [`Event::Menu`](crate::event::Event::Menu). An
//! id is data, so a hot swap keeps it meaningful where a captured closure
//! would not (`docs/events-decision.md` rule 3).
//!
//! **Named `MenuEntry`, not `MenuItem`**: `MenuItem` is AccessKit's role
//! name, and a live token of the same name would make every doc mention
//! of either ambiguous.

use crate::event::{Key, Mods};

/// The most entries one menu holds: one bit each of a [`MenuMask`].
pub const MAX_ENTRIES: usize = 64;

/// A keyboard shortcut a menu entry answers to — Ctrl+S, say. Matched
/// against a key press no widget consumed (T90.5); a context menu only
/// shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accel {
    pub key: Key,
    pub mods: Mods,
}

/// One command: what it says, the `id` the application's hook is handed
/// when it is chosen, and its shortcut, if it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub label: &'static str,
    pub id: u32,
    pub accel: Option<Accel>,
}

impl MenuEntry {
    /// An entry with no shortcut.
    pub const fn new(label: &'static str, id: u32) -> MenuEntry {
        MenuEntry { label, id, accel: None }
    }

    /// The same entry, answering to `key` with `mods`.
    pub const fn accel(self, key: Key, mods: Mods) -> MenuEntry {
        MenuEntry { accel: Some(Accel { key, mods }), ..self }
    }
}

/// A titled group of entries — one menu of a menu bar (T90.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuSection {
    pub title: &'static str,
    pub entries: &'static [MenuEntry],
}

/// **Which entries are enabled**: bit *n* is the *n*-th entry in
/// flattened order (a menu bar's sections one after another). State, not
/// a `fn` per entry: the application computes it from its own state, and
/// a widget gets it through a binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuMask(pub u64);

impl MenuMask {
    /// Every entry enabled.
    pub const ALL: MenuMask = MenuMask(u64::MAX);
    /// None enabled.
    pub const NONE: MenuMask = MenuMask(0);

    /// Whether entry `n` is enabled. An index past [`MAX_ENTRIES`] is not.
    pub const fn enabled(self, n: usize) -> bool {
        n < MAX_ENTRIES && self.0 & (1 << n) != 0
    }

    /// The same mask with entry `n` enabled or not.
    pub const fn with(self, n: usize, on: bool) -> MenuMask {
        if n >= MAX_ENTRIES {
            return self;
        }
        if on {
            MenuMask(self.0 | (1 << n))
        } else {
            MenuMask(self.0 & !(1 << n))
        }
    }
}

impl Default for MenuMask {
    fn default() -> Self {
        MenuMask::ALL
    }
}
