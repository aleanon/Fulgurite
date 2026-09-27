//! Dragging within an application — `docs/plan-capabilities.md` T15.7.
//!
//! # Two halves that should not be conflated
//!
//! **Within an application** is this module: a payload, drop targets
//! and hover feedback. The payload is the *application's* type, so it
//! costs nothing when nothing drags and needs no allocator.
//!
//! **Across the platform boundary** — a file dropped on the window, text
//! dragged to another program — is `lumen-static-winit`'s, behind a
//! default-off feature, because it is a platform stack and this crate
//! has no platform.
//!
//! # Why the payload is a type parameter
//!
//! A `Box<dyn Any>` payload would need an allocator and a downcast at
//! every drop target. A drag is the application's own enum — "a row
//! index", "a file", "a colour" — and naming it in the type means the
//! drop handler receives it already typed. A `Drag<()>` is **24 bytes**
//! — the phase, the press and current points, and the threshold, with no
//! payload — and no tree carries one unless the application adds it
//! (`tests/drag.rs` asserts the size; it said "one byte" until M54/T54.9).
//!
//! It builds on pointer capture (T11.2), which is what routes
//! `PointerMove` to the node that started the gesture; without that a
//! drag would stop the moment the pointer left the source.

use crate::desc::Point;
use crate::event::{Event, EventCtx};

/// Where a drag is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DragPhase {
    /// Nothing is being dragged.
    #[default]
    Idle,
    /// A press has happened but the pointer has not moved far enough to
    /// be a drag rather than a click.
    Pending,
    /// In flight.
    Active,
}

/// A drag in progress, carrying `T`.
///
/// `T` is the application's payload type. `Drag<()>` is 24 B — the
/// phase, two points and the threshold — and is carried only by a tree
/// that adds one (`tests/drag.rs`).
#[derive(Clone, Copy, Debug)]
pub struct Drag<T> {
    /// How far the pointer must move before a press becomes a drag, in
    /// logical px.
    ///
    /// Without a threshold every click is a one-pixel drag, and a list
    /// whose rows both click and reorder cannot tell them apart. 4 px
    /// is the platform-conventional value and is smaller than
    /// `MULTI_CLICK_SLOP`, which answers a different question.
    pub threshold: f32,
    phase: DragPhase,
    from: Point,
    at: Point,
    payload: Option<T>,
}

impl<T> Default for Drag<T> {
    fn default() -> Self {
        Drag {
            threshold: 4.0,
            phase: DragPhase::Idle,
            from: Point::default(),
            at: Point::default(),
            payload: None,
        }
    }
}

impl<T> Drag<T> {
    /// Offer a press as the start of a drag carrying `payload`.
    ///
    /// Nothing is dragging yet: it becomes a drag only once the pointer
    /// has moved past [`threshold`](Self::threshold), which is what
    /// keeps a click a click.
    pub fn press(&mut self, at: Point, payload: T) {
        self.phase = DragPhase::Pending;
        self.from = at;
        self.at = at;
        self.payload = Some(payload);
    }

    /// Feed a pointer event. Returns whether the drag became active or
    /// moved, so a caller knows whether to repaint.
    ///
    /// A release **ends** the drag without dropping — the *drop* is the
    /// caller's decision, made from [`take`](Self::take), because only
    /// the caller knows what is under the pointer.
    pub fn feed(&mut self, ev: &Event) -> bool {
        match ev {
            Event::PointerMove { x, y } if self.phase != DragPhase::Idle => {
                self.at = Point { x: *x, y: *y };
                if self.phase == DragPhase::Pending {
                    let (dx, dy) = (self.at.x - self.from.x, self.at.y - self.from.y);
                    if dx.abs() > self.threshold || dy.abs() > self.threshold {
                        self.phase = DragPhase::Active;
                        return true;
                    }
                    return false;
                }
                true
            }
            Event::PointerUp { .. } => {
                let was = self.phase == DragPhase::Active;
                self.phase = DragPhase::Idle;
                was
            }
            _ => false,
        }
    }

    /// The payload, if a drag is actually in flight.
    ///
    /// `None` while merely `Pending`, so a caller cannot drop something
    /// the user only clicked on.
    pub fn payload(&self) -> Option<&T> {
        (self.phase == DragPhase::Active).then_some(self.payload.as_ref()).flatten()
    }

    /// Take the payload and reset. What a drop handler calls.
    pub fn take(&mut self) -> Option<T> {
        if self.phase != DragPhase::Active {
            return None;
        }
        self.phase = DragPhase::Idle;
        self.payload.take()
    }

    /// Give up: a press that turned out to be something else, or an
    /// Escape while dragging.
    pub fn cancel(&mut self) -> bool {
        let was = self.phase != DragPhase::Idle;
        self.phase = DragPhase::Idle;
        self.payload = None;
        was
    }

    pub fn phase(&self) -> DragPhase {
        self.phase
    }

    /// Where the pointer is, for drawing the thing being dragged.
    pub fn at(&self) -> Point {
        self.at
    }

    /// How far it has moved, for a drag ghost's offset.
    pub fn delta(&self) -> Point {
        Point { x: self.at.x - self.from.x, y: self.at.y - self.from.y }
    }

    /// Whether `ctx`'s node is a plausible drop target: something is in
    /// flight and the pointer is over it.
    pub fn over(&self, ctx: &EventCtx) -> bool {
        self.phase == DragPhase::Active && matches!(ctx.event, Event::PointerMove { .. })
    }
}
