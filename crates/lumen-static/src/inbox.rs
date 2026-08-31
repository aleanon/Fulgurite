//! Deferred work re-enters the app as data — plan E3.
//!
//! `fresh()` rebuilds the widget tree, but it cannot reach into a pending
//! future, a timer, or a queued task and swap the code pointer that task
//! captured. So the one rule at this seam
//! (`docs/events-decision.md`): **work in flight may not hold a code
//! reference.** It sends a *value*; whatever code is loaded when the
//! value arrives interprets it.
//!
//! The shape enforces the rule — a [`Sender`] moves into the task, and a
//! sender carries no `fn` pointer. This is the one place §7's
//! message-shaped events genuinely survive, because it is the one place
//! the capture problem is real in this architecture.

use std::sync::mpsc;

/// Host-owned receiver for results produced off the frame loop.
pub struct Inbox<M> {
    rx: mpsc::Receiver<M>,
    tx: mpsc::Sender<M>,
}

/// Handed to deferred work; clone freely, move into tasks/threads.
pub type Sender<M> = mpsc::Sender<M>;

impl<M> Default for Inbox<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Inbox<M> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Inbox { rx, tx }
    }

    /// A sender for deferred work. Nothing about it references code that
    /// a reload could unload.
    pub fn sender(&self) -> Sender<M> {
        self.tx.clone()
    }

    /// Apply everything that arrived since the last frame, in order.
    /// `apply` comes from the *currently loaded* code — after a hot
    /// reload that is the new build's interpretation of an old build's
    /// message. Returns how many were applied.
    pub fn drain<S>(&self, state: &mut S, apply: fn(&mut S, M)) -> usize {
        let mut n = 0;
        while let Ok(m) = self.rx.try_recv() {
            apply(state, m);
            n += 1;
        }
        n
    }

    /// Deliver one message directly (tests, and synchronous producers).
    pub fn post(&self, m: M) {
        let _ = self.tx.send(m);
    }
}
