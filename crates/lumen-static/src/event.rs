//! Input events and the one queue — plan E0 (`docs/plan-events.md`).
//!
//! **One queue.** OS input, agent-synthesized input, and test input all
//! enter [`InputQueue`], so anything an agent does replays as a test.
//! The queue is host-owned and exists before any shell does: a winit (or
//! headless, or agent) front end *feeds* it, it does not own it.
//!
//! Events are plain data and serialize under `observe` — recording a
//! session is a dev concern, but the vocabulary itself is not, so the
//! types are always present.


use crate::desc::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
///
/// **Deliberately not exhaustive.** Media and browser keys, numpad
/// distinctions, and the modifier keys themselves are excluded
/// (`docs/plan-desktop.md` T14.2). Modifiers are *state* — [`Mods`] —
/// not keys: a `Key::Shift` would let a widget see a shortcut
/// half-pressed.
pub enum Key {
    Enter,
    /// Activates the focused control. Added M11/T11.1: every desktop
    /// toolkit toggles a checkbox with Space, and until then `dispatch`
    /// carried a comment claiming it already did.
    Space,
    Escape,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    /// Added M14/T14.2, with the rest of this block.
    PageUp,
    PageDown,
    /// Completes the editing set beside Delete and Backspace.
    Insert,
    /// A printable key *as a shortcut*: Ctrl+C is
    /// `Key::Char('c')` with [`Mods::CTRL`], not [`Event::Text`].
    ///
    /// Typing still arrives as `Event::Text`; the windowing layer
    /// suppresses that when [`Mods::is_shortcut`] holds, or Ctrl+C would
    /// both fire the shortcut and type a "c" — a bug invisible until a
    /// text field has focus.
    ///
    /// Lower-cased by the windowing layer, so a shortcut table needs one
    /// entry rather than two: Shift is reported in the modifiers.
    Char(char),
    /// F1 to F12, as `F(1)` to `F(12)`. One variant rather than twelve:
    /// F1 help, F2 rename, F5 refresh and F11 fullscreen are the common
    /// ones, and a table is a worse fit than a number.
    F(u8),
}

/// Which modifier keys are held — `docs/plan-desktop.md` T14.1.
///
/// A bitflag rather than four `bool`s so it fits in one byte and
/// `Event` stays 16 B, which is what keeps [`InputQueue`] at 1 KB.
///
/// **`META`, not `CMD` or `SUPER`.** One name for the platform's own
/// modifier — Command on macOS, Super elsewhere — mapped per platform
/// by the windowing layer, so an application writes one shortcut table.
/// Whether `CTRL` or `META` is "the" shortcut modifier is the
/// *application's* choice; this reports both truthfully and decides
/// nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub struct Mods(pub u8);

impl Mods {
    pub const NONE: Mods = Mods(0);
    pub const SHIFT: Mods = Mods(1 << 0);
    pub const CTRL: Mods = Mods(1 << 1);
    pub const ALT: Mods = Mods(1 << 2);
    pub const META: Mods = Mods(1 << 3);

    /// Whether every bit of `m` is held. `has(Mods::NONE)` is always
    /// true, which is the right answer to "does this need no modifiers".
    pub const fn has(self, m: Mods) -> bool {
        self.0 & m.0 == m.0
    }

    /// Whether *no* modifier is held. A shortcut table wants this to
    /// tell a bare Enter from Ctrl+Enter.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether any modifier but Shift is held.
    ///
    /// The rule the windowing layer applies to suppress [`Event::Text`]:
    /// Shift is excluded because Shift+A *is* how a capital arrives,
    /// while Ctrl+C must not also type a "c" (T14.2).
    pub const fn is_shortcut(self) -> bool {
        self.0 & !Mods::SHIFT.0 != 0
    }

    /// Add `m`.
    pub const fn with(self, m: Mods) -> Mods {
        Mods(self.0 | m.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "observe", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    PointerDown { x: f32, y: f32 },
    PointerUp { x: f32, y: f32 },
    PointerMove { x: f32, y: f32 },
    Scroll { x: f32, y: f32, dy: f32 },
    Key { key: Key, down: bool },
    Text { ch: char },
    /// The modifier keys changed — T14.1.
    ///
    /// **In the queue, not beside it.** `docs/plan-events.md` E0 is
    /// "one queue, so anything an agent does replays as a test", and
    /// modifier state held outside the queue makes a recorded session
    /// unreplayable exactly when modifiers matter. `dispatch` tracks
    /// the current value and puts it in every [`EventCtx`], so a
    /// handler reads `ctx.mods` and never sees this event.
    Modifiers(Mods),
}

impl Event {
    /// Pointer position, for the events that carry one.
    pub fn position(&self) -> Option<Point> {
        match *self {
            Event::PointerDown { x, y }
            | Event::PointerUp { x, y }
            | Event::PointerMove { x, y }
            | Event::Scroll { x, y, .. } => Some(Point { x, y }),
            Event::Key { .. } | Event::Text { .. } | Event::Modifiers(_) => None,
        }
    }
}

/// What dispatch learned about where an event landed. Handlers read
/// their payload from here (and from their widget's own fields) — never
/// from a capture, which is what keeps them `fn` pointers across a hot
/// reload (`docs/events-decision.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventCtx {
    /// What happened (C2).
    ///
    /// A handler is one `fn` per node and every event now reaches it —
    /// keys, text and scroll as well as clicks (C1) — so it has to be
    /// able to tell them apart. The ctx already carries data *derived*
    /// from the event (`pos`, `local`, `row`); carrying the event itself
    /// is one field and removes the guessing.
    pub event: Event,
    /// Main-table index of the node that received the event.
    pub node: u32,
    /// For virtualized content: which row. `None` for ordinary nodes.
    ///
    /// This is the field that replaces `move |s| s.remove(idx)`: the row
    /// is recomputed by hit testing every time, so it can never go stale
    /// the way a captured index does.
    pub row: Option<u64>,
    /// Node index *within the template*, when `row` is set.
    pub inner: u32,
    /// Absolute pointer position, when the event carries one.
    pub pos: Point,
    /// Pointer position relative to the receiving node's origin.
    pub local: Point,
    /// Which modifiers were held when this event happened (T14.1).
    ///
    /// Tracked by `dispatch` from [`Event::Modifiers`] rather than
    /// carried on each event, so `Event` stays 16 B.
    pub mods: Mods,
}

impl EventCtx {
    pub fn at(node: u32) -> Self {
        EventCtx::for_event(node, Event::PointerUp { x: 0.0, y: 0.0 })
    }

    /// The ctx a non-pointer event produces at `node`.
    pub fn for_event(node: u32, event: Event) -> Self {
        EventCtx {
            event,
            node,
            row: None,
            inner: 0,
            pos: Point::default(),
            local: Point::default(),
            mods: Mods::NONE,
        }
    }

    /// The same, with the modifiers that were held.
    pub fn for_event_with(node: u32, event: Event, mods: Mods) -> Self {
        EventCtx { mods, ..EventCtx::for_event(node, event) }
    }
}

/// The caller-owned input state `dispatch` reads and updates.
///
/// Bundled as of M14/T14.1, when modifier tracking would have made
/// `dispatch` a seven-argument function. It is also the thing a session
/// recording snapshots: three loose `&mut` were three things to
/// remember, and this is one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputState {
    /// The focused node's main-table index, if any.
    pub focus: Option<u32>,
    /// The node holding the pointer for a drag, if any (T11.2).
    ///
    /// Checked *before* the hit test: a drag that leaves its control
    /// must keep reaching it, and the first version checked after, so
    /// the slider stopped following the pointer the moment it left the
    /// track.
    pub capture: Option<u32>,
    /// Modifiers currently held, updated from [`Event::Modifiers`].
    pub mods: Mods,
}

/// The single input queue (see the module note).
///
/// **Fixed capacity, in every build.** This was a `VecDeque` until
/// M13/T13.4, which needed the core to compile with no allocator at
/// all. A second, embedded-only queue would have forked the one thing
/// `docs/plan-events.md` E0 set out to unify, so instead there is one
/// ring and every build uses it — which is also the better-tested
/// arrangement, since the existing suite exercises the embedded path
/// without knowing it.
///
/// `Event` is `Copy` and 16 bytes, so [`CAP`](Self::CAP) events cost
/// 1 KB inline — affordable on the M13 device's 128 KB
/// (`docs/results-embedded.md`) and one less heap structure per host
/// on the desktop.
#[derive(Debug)]
pub struct InputQueue {
    q: [Event; Self::CAP],
    head: usize,
    len: usize,
    dropped: u32,
}

impl Default for InputQueue {
    fn default() -> Self {
        InputQueue {
            // Any value works: `len` is what makes a slot live.
            q: [Event::Key { key: Key::Enter, down: false }; Self::CAP],
            head: 0,
            len: 0,
            dropped: 0,
        }
    }
}

impl InputQueue {
    /// Queue depth. At 60 fps and human input rates this is about a
    /// second of continuous input without a drain, and the frame loop
    /// drains every frame.
    pub const CAP: usize = 64;

    pub fn new() -> Self {
        Self::default()
    }

    /// Queue `ev`, or count it as dropped if the queue is full.
    ///
    /// Overflow rejects the **newest** event rather than evicting the
    /// oldest. Either choice breaks gesture pairing, but evicting the
    /// oldest can strand a `PointerUp` whose `PointerDown` is gone —
    /// a click that was never pressed. Rejecting the newest leaves the
    /// queued prefix a coherent history.
    ///
    /// Returns whether it was queued. The drop is *counted*, not
    /// silent: see [`dropped`](Self::dropped). Same rule as the change
    /// journal's `truncated` flag — a gate that hides its own overflow
    /// is worse than no gate.
    pub fn push(&mut self, ev: Event) -> bool {
        if self.len == Self::CAP {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        self.q[(self.head + self.len) % Self::CAP] = ev;
        self.len += 1;
        true
    }

    pub fn pop(&mut self) -> Option<Event> {
        if self.len == 0 {
            return None;
        }
        let ev = self.q[self.head];
        self.head = (self.head + 1) % Self::CAP;
        self.len -= 1;
        Some(ev)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How many events this queue has ever refused for lack of room.
    /// Non-zero means input was lost and the app should be told, not
    /// that the queue is currently full.
    pub fn dropped(&self) -> u32 {
        self.dropped
    }

    /// Everything queued, oldest first — the recording seed.
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        (0..self.len).map(|i| &self.q[(self.head + i) % Self::CAP])
    }
}

/// A node selector for handler lookup: a main-table index, plus the
/// off-table row a hit resolved to, if any.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeSel {
    pub i: usize,
    pub inner: Option<(u64, u32)>,
}

/// Which handler, if any, a node carries.
///
/// Generic over the application state so that a `Button<A>` placed in a
/// `B`-rooted tree is a **compile error**: the state-type contract that
/// used to be a safety comment is now checked by the type system.
pub trait Handlers<S> {
    fn handler(&self, sel: NodeSel) -> Option<fn(&mut S, &EventCtx)>;
}

/// The application root: state plus the tree that presents it.
///
/// Splitting the tree out of `S` is what lets dispatch hold `&mut S`
/// while a handler runs: the handler `fn` pointer is copied out through
/// `&*self` first, that borrow ends, and only then does `&mut S` exist.
/// The borrow checker enforces the sequence that used to be a documented
/// rule (`docs/plan-events.md` E2).
pub trait App: Sized {
    type Tree: crate::widget::Widget + Handlers<Self>;
    fn tree(&self) -> &Self::Tree;
    fn tree_mut(&mut self) -> &mut Self::Tree;

    /// Refresh the tree's *presentation copies* from the state they are
    /// derived from. Called by the frame loop after input is drained and
    /// before layout, so a handler that changes state cannot leave a
    /// stale value on screen.
    ///
    /// Widget fields hold the presentation copy and sibling fields are
    /// the source of truth (`docs/layout-and-storage-decisions.md` §1);
    /// this is the seam where the copy is brought back into agreement.
    /// `#[derive(App)]` writes the body from `#[bind(...)]` declarations
    /// — see `crate::bind`. The default does nothing, for trees that
    /// present no derived state.
    fn update(&mut self) {}
}

impl Extend<Event> for InputQueue {
    /// Events past [`InputQueue::CAP`] are counted as dropped, exactly
    /// as [`InputQueue::push`] does — `Extend` cannot report a refusal,
    /// so the count is the only channel it has.
    fn extend<T: IntoIterator<Item = Event>>(&mut self, iter: T) {
        for ev in iter {
            self.push(ev);
        }
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;

    fn ev(n: u8) -> Event {
        Event::Text { ch: n as char }
    }

    #[test]
    fn fifo_and_wraparound() {
        let mut q = InputQueue::new();
        // Push and drain past CAP so head wraps the backing array — the
        // case a VecDeque handled for free and this ring must not get
        // wrong.
        for round in 0..5u32 {
            for i in 0..40u8 {
                assert!(q.push(ev(i)), "round {round} had room");
            }
            for i in 0..40u8 {
                assert_eq!(q.pop(), Some(ev(i)), "FIFO order, round {round}");
            }
            assert!(q.is_empty());
        }
        assert_eq!(q.dropped(), 0, "never over capacity, so never dropped");
    }

    #[test]
    fn overflow_rejects_the_newest_and_counts_it() {
        let mut q = InputQueue::new();
        for i in 0..InputQueue::CAP {
            assert!(q.push(ev(i as u8)));
        }
        assert!(!q.push(ev(200)), "full queue refuses");
        assert!(!q.push(ev(201)));
        assert_eq!(q.dropped(), 2, "and says how much it lost");
        assert_eq!(q.len(), InputQueue::CAP);

        // The queued prefix is intact: rejecting the newest is what
        // keeps it a coherent history rather than one with a hole.
        assert_eq!(q.pop(), Some(ev(0)));
        assert_eq!(q.iter().count(), InputQueue::CAP - 1);
    }

    #[test]
    fn iter_is_oldest_first_across_a_wrap() {
        let mut q = InputQueue::new();
        for i in 0..50u8 {
            q.push(ev(i));
        }
        for _ in 0..40 {
            q.pop();
        }
        for i in 50..70u8 {
            q.push(ev(i));
        }
        let seen: Vec<u8> = q.iter().map(|e| match e {
            Event::Text { ch } => *ch as u8,
            _ => unreachable!(),
        }).collect();
        assert_eq!(seen, (40..70u8).collect::<Vec<_>>(), "oldest first, wrap included");
    }

    #[test]
    fn extend_counts_what_it_could_not_take() {
        let mut q = InputQueue::new();
        q.extend((0..70u8).map(ev));
        assert_eq!(q.len(), InputQueue::CAP);
        assert_eq!(q.dropped(), 70 - InputQueue::CAP as u32);
    }
}
