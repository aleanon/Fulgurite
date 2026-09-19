//! Deferred work re-enters the app as data — plan E3, extended by
//! M28/T28.1.
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
//!
//! # Waking the loop — M28/T28.1
//!
//! A sent message that nothing looks at is not deferred work, it is a
//! leak. A windowed host sleeps in `ControlFlow::Wait` until input
//! arrives, so a completion posted from a worker at 3 a.m. would sit in
//! the queue until the user moved the mouse.
//!
//! [`Waker`] is the seam that prevents it, and it is deliberately a
//! trait rather than a winit proxy: the core cannot name winit, a
//! headless `Host` has no loop to wake, and a test needs to assert that
//! the wake was *asked for* without opening a window.

use std::sync::mpsc;
use std::sync::Arc;

/// Something that can rouse a sleeping frame loop — M28/T28.1.
///
/// Implemented by the shell over its event loop's proxy. The core never
/// names the loop, and `wake` is called **from the worker's thread**,
/// which is why the bound is `Send + Sync`.
///
/// It must be safe to call when the loop has already exited: a worker
/// that outlives the window is ordinary, not exceptional, and the winit
/// implementation answers a closed loop by dropping the request.
pub trait Waker: Send + Sync + 'static {
    /// Ask for one frame. Coalescing is the implementation's business —
    /// winit's proxy already coalesces, and nothing here depends on the
    /// number of frames a burst produces, only that at least one runs
    /// after the last message.
    fn wake(&self);
}

/// Host-owned receiver for results produced off the frame loop.
pub struct Inbox<M> {
    rx: mpsc::Receiver<M>,
    tx: mpsc::Sender<M>,
    waker: Option<Arc<dyn Waker>>,
}

/// Handed to deferred work; clone freely, move into tasks/threads.
///
/// **A struct since M28/T28.1, not an alias for `mpsc::Sender`.** It
/// carries the [`Waker`] alongside the channel so that *sending is what
/// wakes the loop* — a worker cannot send and forget to ask for a frame,
/// because it has no way to do the first without the second. The
/// alternative, a `wake()` the caller must remember, is the shape that
/// produces a message delivered on the next unrelated mouse move.
pub struct Sender<M> {
    tx: mpsc::Sender<M>,
    waker: Option<Arc<dyn Waker>>,
}

impl<M> Clone for Sender<M> {
    /// Hand-written: `#[derive(Clone)]` would demand `M: Clone`, and the
    /// message type has no reason to be cloneable.
    fn clone(&self) -> Self {
        Sender { tx: self.tx.clone(), waker: self.waker.clone() }
    }
}

impl<M> Sender<M> {
    /// Queue a message and ask for a frame.
    ///
    /// The error is the channel's: a host that has been dropped cannot
    /// receive, and a worker outliving its window is ordinary. The wake
    /// is skipped in that case, because there is nothing to wake.
    pub fn send(&self, m: M) -> Result<(), mpsc::SendError<M>> {
        self.tx.send(m)?;
        if let Some(w) = &self.waker {
            w.wake();
        }
        Ok(())
    }
}

impl<M> Default for Inbox<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Inbox<M> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Inbox { rx, tx, waker: None }
    }

    /// Route every later [`Sender::send`] through `waker` — M28/T28.1.
    ///
    /// The shell calls this once, after its event loop exists. Senders
    /// handed out **before** this call keep the waker they were given,
    /// which is `None`: a sender minted before the loop exists cannot
    /// wake it, and pretending otherwise would be worse than saying so.
    /// In practice the host sets this in its constructor, before any
    /// application code can ask for a sender.
    pub fn set_waker(&mut self, waker: Arc<dyn Waker>) {
        self.waker = Some(waker);
    }

    /// Whether a waker is installed — the deterministic half of T28.1's
    /// gate asks this, and a host that forgot to install one is the
    /// defect that gate exists to catch.
    pub fn has_waker(&self) -> bool {
        self.waker.is_some()
    }

    /// A sender for deferred work. Nothing about it references code that
    /// a reload could unload.
    pub fn sender(&self) -> Sender<M> {
        Sender { tx: self.tx.clone(), waker: self.waker.clone() }
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
    ///
    /// **Wakes, like [`Sender::send`]**: a synchronous producer posting
    /// from a handler is already inside a frame, so the wake is
    /// redundant there and harmless; a producer posting from anywhere
    /// else needs it for the same reason a worker does.
    pub fn post(&self, m: M) {
        if self.tx.send(m).is_ok() {
            if let Some(w) = &self.waker {
                w.wake();
            }
        }
    }
}
