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
}

impl Event {
    /// Pointer position, for the events that carry one.
    pub fn position(&self) -> Option<Point> {
        match *self {
            Event::PointerDown { x, y }
            | Event::PointerUp { x, y }
            | Event::PointerMove { x, y }
            | Event::Scroll { x, y, .. } => Some(Point { x, y }),
            Event::Key { .. } | Event::Text { .. } => None,
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
        }
    }
}

/// The single input queue (see the module note).
#[derive(Default, Debug)]
pub struct InputQueue {
    q: std::collections::VecDeque<Event>,
}

impl InputQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, ev: Event) {
        self.q.push_back(ev);
    }

    pub fn pop(&mut self) -> Option<Event> {
        self.q.pop_front()
    }

    pub fn len(&self) -> usize {
        self.q.len()
    }

    pub fn is_empty(&self) -> bool {
        self.q.is_empty()
    }

    /// Everything queued, oldest first — the recording seed.
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        self.q.iter()
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
    fn extend<T: IntoIterator<Item = Event>>(&mut self, iter: T) {
        self.q.extend(iter);
    }
}
