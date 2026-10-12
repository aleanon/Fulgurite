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
//!
//! # Two routes back: the channel, and the progress slot — M88/T88.1
//!
//! [`Sender::send`] queues a message on the bounded channel, and every
//! message sent is applied once, in order — or refused, counted, when
//! the channel is full. That is right for a result and wrong for a
//! stream: a job that reports 10,000 values loses 97% of them and its
//! last one (`docs/results-tasks.md`, M88 baseline).
//!
//! [`Sender::progress`] is the other route, iced's `Sipper` in this
//! architecture's terms (`docs/plan-platform-requests.md`): it replaces
//! the value in a **one-value slot on the job's own [`JobTrack`]** and
//! never touches the channel, so it never blocks and is never refused.
//! The host's pump takes each fresh slot's value before it drains the
//! channel — at most one value per job per pump — and the last value
//! stored is always applied, because a job does not leave the ledger
//! while its slot holds one. What a newer value replaced unread is
//! counted ([`Inbox::coalesced`]): the slot is the back-pressure, and
//! the count is what it cost. `progress` and `send` are not ordered
//! relative to each other; a body that needs an ordered final message
//! sends it with `progress`.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

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
    rx: mpsc::Receiver<Item<M>>,
    tx: mpsc::SyncSender<Item<M>>,
    waker: Option<Arc<dyn Waker>>,
    refused: Arc<AtomicUsize>,
    /// Progress values replaced in their slot before any pump read them
    /// — M88/T88.1. Shared with every [`Sender`] for the reason `refused`
    /// is: the replacing happens on a worker.
    coalesced: Arc<AtomicUsize>,
    /// Failures no application message reported — M47/T47.0, R7-F9.
    /// Shared with every [`Sender`] for the reason `refused` is: the
    /// failure happens on a worker and the question is asked on the frame.
    unmodelled: Arc<AtomicUsize>,
    /// The epoch senders are minted at, and the one a result must carry
    /// to be applied — M28/T28.8.
    epoch: u64,
    /// Results dropped for carrying a stale epoch.
    ///
    /// A `Cell` because `drain` takes `&self` — it has since E3, so that
    /// a host can drain while holding `&mut` to the state being applied
    /// to. The counter is the only thing the drain writes to the inbox.
    stale: core::cell::Cell<usize>,
    /// Results dropped because the job that sent them had been told to
    /// stop — M84/T84.7, read as `Host::stopped_dropped`. A `Cell` for
    /// the reason `stale` is one.
    stopped_dropped: core::cell::Cell<usize>,
}

impl<M> Inbox<M> {
    /// How many messages the queue holds before it starts refusing.
    ///
    /// **Bounded since M28/T28.2.** It was `mpsc::channel()` — unbounded
    /// — and `drain` looped until empty, so a producer that outran the
    /// frame loop grew the queue without limit *and* made the drain
    /// unbounded work inside `pump`. Two failure modes from one default:
    /// memory, and a frame that takes as long as the backlog.
    pub const CAPACITY: usize = 256;

    /// The most messages one frame will apply.
    ///
    /// The drain is inside `pump`, so "until empty" is unbounded work on
    /// the frame thread. A backlog is spread over frames instead, which
    /// keeps a burst from turning into one long frame. Anything left is
    /// still there next frame, and the host asks for another.
    pub const PER_FRAME: usize = 64;
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
    tx: mpsc::SyncSender<Item<M>>,
    waker: Option<Arc<dyn Waker>>,
    refused: Arc<AtomicUsize>,
    coalesced: Arc<AtomicUsize>,
    unmodelled: Arc<AtomicUsize>,
    /// The epoch this sender was minted at — M28/T28.8. Fixed at mint
    /// time, so a worker cannot un-cancel itself.
    epoch: u64,
    /// The job this sender reports for, when a host spawned one with it —
    /// M47/T47.3. Every message it sends carries this, so the drain can
    /// say which job's result it applied.
    track: Option<Arc<JobTrack<M>>>,
    /// Whether dropping this sender ends the job: true only for the one
    /// [`Inbox::tracked_sender`] minted, which the spawner owns and
    /// drops when the job body has returned. A clone the body makes
    /// reports for the same job but does not end it.
    owns: bool,
}

/// What travels through the channel: the epoch, the job it came from (if
/// any), and the message.
type Item<M> = (u64, Option<Arc<JobTrack<M>>>, M);

/// One spawned job's progress, shared by its sender and the host's
/// ledger — M47/T47.3.
///
/// A job is **in flight** from the moment a host hands it to a spawner
/// until its body has returned *and* every message it sent has been
/// consumed by a drain (applied, or dropped as stale). That is the
/// definition `spawn::Ledger` counts, and it is why a result sitting in
/// the inbox still counts: an agent asking "is anything pending" and a
/// reload re-issuing unfinished work both need the job to stay listed
/// until its result has actually landed.
///
/// Three counters rather than a message at the end, because a "done"
/// marker would have to go through the bounded channel, where it could
/// be refused — and a refused marker is a job listed for ever.
///
/// **And one flag the other way, since M84/T84.7: `stop`.** The host
/// sets it ([`JobTrack::stop`]) when a subscription the job serves is
/// no longer declared; the job body reads it through
/// [`Sender::is_stopped`] and returns, and the drain drops whatever the
/// job sent that had not yet been applied (`Host::stopped_dropped`
/// counts it). The stop is **cooperative**: a body that never asks runs
/// on, and only the drain's half holds for it — its results are never
/// applied, but its work is not saved. One byte, in the padding after
/// `finished`.
///
/// **And the progress slot, since M88/T88.1** ([`Sender::progress`]):
/// `latest` holds the newest value the job reported and no pump has
/// taken, `fresh` says whether it holds one (so a pump asks without
/// locking), and `epoch` is the inbox epoch the job's sender was minted
/// at, so a cancelled job's slot is dropped like its channel messages
/// ([`Inbox::stale`]). It is inline in the `Arc` that
/// [`Inbox::tracked_sender`] already allocates — no allocation per spawn
/// and none per value; `docs/results-tasks.md` T88.1 has the size for
/// `M = u64`.
pub struct JobTrack<M> {
    sent: AtomicUsize,
    consumed: AtomicUsize,
    finished: AtomicBool,
    stop: AtomicBool,
    /// Whether `latest` holds a value no pump has taken. Written only
    /// with `latest`'s lock held, so the two never disagree under it.
    fresh: AtomicBool,
    /// The inbox epoch this job was spawned at (M28/T28.8).
    epoch: u64,
    /// The newest progress value, not yet applied.
    latest: Mutex<Option<M>>,
}

impl<M> JobTrack<M> {
    fn new(epoch: u64) -> JobTrack<M> {
        JobTrack {
            sent: AtomicUsize::new(0),
            consumed: AtomicUsize::new(0),
            finished: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            fresh: AtomicBool::new(false),
            epoch,
            latest: Mutex::new(None),
        }
    }

    /// Whether the slot holds a progress value no pump has taken —
    /// M88/T88.1.
    pub fn is_fresh(&self) -> bool {
        self.fresh.load(Ordering::Acquire)
    }

    /// Take the slot's value, if it holds one. The lock is held only for
    /// the swap; a poisoned lock (a body that panicked while storing) is
    /// read through, since the value inside is a whole `Option`.
    fn take_latest(&self) -> Option<M> {
        if !self.fresh.load(Ordering::Acquire) {
            return None;
        }
        let mut slot = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        self.fresh.store(false, Ordering::Release);
        slot.take()
    }
    /// Tell the job to stop — M84/T84.7. Called by the host on the
    /// frame's thread; idempotent.
    ///
    /// `Release`, so a body that reads the flag `true` also sees
    /// everything the frame wrote before it (nothing it needs today; the
    /// ordering costs nothing on the one store).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    /// Whether the host has told this job to stop.
    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    /// Whether the job has finished and its every result has been
    /// drained. Read on the frame's thread.
    ///
    /// `finished` is read first, with `Acquire`: the owning sender's
    /// sends all happen before its drop stores it, so once it reads
    /// true, `sent` is final — unless a clone outlives the body, whose
    /// later sends are then applied untracked, which is the documented
    /// limit.
    ///
    /// **Not while its progress slot is fresh** (M88/T88.1): a job whose
    /// last [`Sender::progress`] value no pump has taken is not settled,
    /// so the ledger cannot drop it with its final value undelivered.
    /// The body stores the value before its sender's drop stores
    /// `finished`, so once `finished` reads true `fresh` is current.
    ///
    /// **A stopped job counts as finished** (M84/T84.7 meeting M88): the
    /// application asked for it to end, so its body running on is not
    /// work in flight — but it still waits for the drain to drop what it
    /// sent and what its slot holds, which is where `Host::stopped_dropped`
    /// counts it.
    pub fn settled(&self) -> bool {
        (self.finished.load(Ordering::Acquire) || self.is_stopped())
            && !self.fresh.load(Ordering::Acquire)
            && self.sent.load(Ordering::Acquire) == self.consumed.load(Ordering::Relaxed)
    }
}

impl<M> core::fmt::Debug for JobTrack<M> {
    /// Hand-written: a derive would demand `M: Debug` and print the
    /// slot's value, which is the application's business.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JobTrack")
            .field("sent", &self.sent)
            .field("consumed", &self.consumed)
            .field("finished", &self.finished)
            .field("stop", &self.stop)
            .field("fresh", &self.fresh)
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

impl<M> Clone for Sender<M> {
    /// Hand-written: `#[derive(Clone)]` would demand `M: Clone`, and the
    /// message type has no reason to be cloneable.
    fn clone(&self) -> Self {
        Sender {
            tx: self.tx.clone(),
            waker: self.waker.clone(),
            refused: self.refused.clone(),
            coalesced: self.coalesced.clone(),
            unmodelled: self.unmodelled.clone(),
            epoch: self.epoch,
            track: self.track.clone(),
            owns: false,
        }
    }
}

/// The owning sender's drop is what ends its job — M47/T47.3.
///
/// A drop rather than a call the spawner must remember, so a spawner
/// written outside this crate ends its jobs by doing what it already
/// does: letting the sender go once the body has returned. `Queued`
/// holds its senders until it runs them, so a queued job stays in flight
/// until then, which is the truth.
impl<M> Drop for Sender<M> {
    fn drop(&mut self) {
        if self.owns {
            if let Some(t) = &self.track {
                t.finished.store(true, Ordering::Release);
            }
        }
    }
}

impl<M> Sender<M> {
    /// Queue a message and ask for a frame.
    ///
    /// **Refuses rather than blocks** when the queue is full
    /// ([`Inbox::CAPACITY`]). A worker that blocked here would be a
    /// worker the frame loop can stall — backpressure applied to the
    /// wrong side of the seam, and on a single-threaded spawner a
    /// deadlock. The refusal is counted where a test can read it
    /// ([`Inbox::refused`]) rather than logged, because "how many did we
    /// drop" is a number, and a silent drop is the failure this gate
    /// exists to make visible.
    ///
    /// A disconnected host — a worker outliving its window — is the
    /// other error, and is ordinary rather than exceptional. The wake is
    /// skipped in both cases: there is nothing to look at.
    ///
    /// **For a stream, use [`Sender::progress`]** (M88/T88.1). Refusing
    /// is right for a result — nothing is lost that the caller is not
    /// told about — and wrong for a job that reports thousands of values,
    /// whose later values and last one would be refused. The decision
    /// here stands: this never blocks.
    pub fn send(&self, m: M) -> Result<(), SendError<M>> {
        // Counted **before** the message is visible, so the drain can
        // never see more consumed than sent; taken back if it is refused.
        if let Some(t) = &self.track {
            t.sent.fetch_add(1, Ordering::Release);
        }
        let unsend = || {
            if let Some(t) = &self.track {
                t.sent.fetch_sub(1, Ordering::Release);
            }
        };
        match self.tx.try_send((self.epoch, self.track.clone(), m)) {
            Ok(()) => {
                if let Some(w) = &self.waker {
                    w.wake();
                }
                Ok(())
            }
            Err(mpsc::TrySendError::Full((_, _, m))) => {
                unsend();
                self.refused.fetch_add(1, Ordering::Relaxed);
                Err(SendError::Full(m))
            }
            Err(mpsc::TrySendError::Disconnected((_, _, m))) => {
                unsend();
                Err(SendError::Disconnected(m))
            }
        }
    }

    /// Report the job's latest value — M88/T88.1, the sipper.
    ///
    /// Replaces the value in the job's one-value slot ([`JobTrack`]);
    /// the host's next pump applies it, **at most one value per job per
    /// pump**, and **the last value stored is always applied**: the job
    /// stays in the ledger until its slot has been taken. A value that a
    /// newer one replaced before any pump read it is counted in
    /// [`Inbox::coalesced`], never applied. Never blocks, never refused,
    /// and never touches the channel, so a full inbox cannot cost a
    /// progress value and a stream cannot crowd out a result.
    ///
    /// Wakes the loop only when the slot goes from empty to full, so a
    /// tight loop of 10,000 calls asks for one frame per value a pump
    /// actually takes, not 10,000.
    ///
    /// **Not ordered against [`Sender::send`].** A body that needs its
    /// final message ordered after its stream sends that message with
    /// `progress`, because the last `progress` value cannot be coalesced
    /// away.
    ///
    /// **A sender with no job** ([`Inbox::sender`], used in tests) has no
    /// slot, so this is a plain [`Sender::send`], refusal included.
    ///
    /// Rule 3 holds: the value is converted to `M` by the job body, on
    /// the worker, in the build that spawned it; the slot holds data.
    pub fn progress(&self, m: M) {
        let Some(t) = &self.track else {
            let _ = self.send(m);
            return;
        };
        let (replaced, was_fresh) = {
            let mut slot = t.latest.lock().unwrap_or_else(|e| e.into_inner());
            let replaced = slot.replace(m).is_some();
            (replaced, t.fresh.swap(true, Ordering::AcqRel))
        };
        if replaced {
            self.coalesced.fetch_add(1, Ordering::Relaxed);
        }
        if !was_fresh {
            if let Some(w) = &self.waker {
                w.wake();
            }
        }
    }

    /// Count a job failure no application message reported — M47/T47.6.
    /// Called by the spawners' failure path, on the worker's thread; the
    /// host reads it as [`Inbox::unmodelled`].
    pub(crate) fn count_unmodelled(&self) {
        self.unmodelled.fetch_add(1, Ordering::Relaxed);
    }

    /// How many sends this inbox has refused — see [`Inbox::refused`].
    pub fn refused(&self) -> usize {
        self.refused.load(Ordering::Relaxed)
    }

    /// Whether the host has told this sender's job to stop — M84/T84.7.
    ///
    /// The cooperative half of a stop: a long job (a subscription's
    /// stream, a decoder) asks between units of work and returns when
    /// this answers `true`. Whatever it sends after the stop is dropped
    /// at the drain anyway, so asking saves the work, not correctness.
    ///
    /// A sender with no job — [`Inbox::sender`], [`Inbox::post`]'s path
    /// — answers `false` always: there is nothing a host could stop.
    pub fn is_stopped(&self) -> bool {
        self.track.as_ref().is_some_and(|t| t.is_stopped())
    }
}

impl<M> Default for Inbox<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Inbox<M> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(Self::CAPACITY);
        Inbox {
            rx,
            tx,
            waker: None,
            refused: Arc::new(AtomicUsize::new(0)),
            coalesced: Arc::new(AtomicUsize::new(0)),
            unmodelled: Arc::new(AtomicUsize::new(0)),
            epoch: 0,
            stale: core::cell::Cell::new(0),
            stopped_dropped: core::cell::Cell::new(0),
        }
    }

    /// Cancel every task in flight — M28/T28.8, `docs/plan-tasks.md`
    /// §2.3.
    ///
    /// Bumps the epoch, so results already queued or still running carry
    /// the old one and are dropped at the drain. Returns the new epoch.
    ///
    /// **What "cancel" honestly means here**: the result is dropped, the
    /// work is not stopped. A thread already running runs to completion
    /// and its output is discarded. Stopping work needs cooperation from
    /// the job body, which this milestone does not design.
    ///
    /// **A reload does *not* call this.** The plan's first draft had a
    /// reload bump the epoch, which made T28.7's chain unpassable by
    /// construction — the pre-swap result would be discarded before the
    /// new build's step could interpret it — and voided §2.1's own
    /// argument, which is precisely that a result is *data* and safe for
    /// the new build to read. So the epoch is **explicit cancellation,
    /// driven by the application**, and nothing else touches it.
    pub fn cancel(&mut self) -> u64 {
        self.epoch += 1;
        self.epoch
    }

    /// The epoch new senders are minted at.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// How many results were dropped for carrying a stale epoch.
    ///
    /// Counted rather than logged, for the reason every other refusal
    /// here is: a silent drop is what a gate exists to make visible.
    pub fn stale(&self) -> usize {
        self.stale.get()
    }

    /// How many results were dropped because their job had been told to
    /// stop ([`JobTrack::stop`]) — M84/T84.7. Monotonic. A result both
    /// stale and stopped is counted as stale: the epoch is checked first.
    pub fn stopped_dropped(&self) -> usize {
        self.stopped_dropped.get()
    }

    /// How many sends have been refused for want of capacity.
    ///
    /// Shared with every [`Sender`], because the refusals happen on the
    /// workers' threads and the question is asked on the frame's.
    /// Monotonic; a test reads it as a difference.
    pub fn refused(&self) -> usize {
        self.refused.load(Ordering::Relaxed)
    }

    /// How many [`Sender::progress`] values were replaced in their slot
    /// before a pump took them — M88/T88.1. Monotonic.
    ///
    /// The cost of a slot instead of a queue, counted where a test can
    /// read it: every value reported is either applied, coalesced here,
    /// or dropped as stale or stopped.
    pub fn coalesced(&self) -> usize {
        self.coalesced.load(Ordering::Relaxed)
    }

    /// How many job failures reached no application message — M47/T47.0,
    /// R7-F9: an application that leaves `App::failed` at its default, or
    /// whose handler answers `None` for this failure. Monotonic.
    ///
    /// The spawners' failure path counts here since M47/T47.6. A custom
    /// spawner outside this crate reports failures however it likes and
    /// is not counted unless it goes through the same path.
    pub fn unmodelled(&self) -> usize {
        self.unmodelled.load(Ordering::Relaxed)
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
        Sender {
            tx: self.tx.clone(),
            waker: self.waker.clone(),
            refused: self.refused.clone(),
            coalesced: self.coalesced.clone(),
            unmodelled: self.unmodelled.clone(),
            epoch: self.epoch,
            track: None,
            owns: false,
        }
    }

    /// A sender for **one job**, and the handle its ledger entry keeps —
    /// M47/T47.3.
    ///
    /// Every message the sender (or a clone of it) sends is counted
    /// against the job, and the drain counts each one it consumes.
    /// Dropping *this* sender, not a clone, says the body has returned.
    /// [`JobTrack::settled`] is the conjunction, and a host drops the
    /// entry from its ledger when it holds.
    pub fn tracked_sender(&self) -> (Sender<M>, Arc<JobTrack<M>>) {
        let track = Arc::new(JobTrack::new(self.epoch));
        let mut tx = self.sender();
        tx.track = Some(track.clone());
        tx.owns = true;
        (tx, track)
    }

    /// Apply what arrived since the last frame, in order, **at most
    /// [`Inbox::PER_FRAME`] of them**.
    ///
    /// `apply` comes from the *currently loaded* code — after a hot
    /// reload that is the new build's interpretation of an old build's
    /// message. Returns how many were applied; a caller that gets
    /// `PER_FRAME` back should ask for another frame, because there may
    /// be more.
    ///
    /// The cap is what keeps a burst from becoming one long frame. It
    /// was "until empty", which is unbounded work inside `pump`.
    pub fn drain<S>(&self, state: &mut S, apply: fn(&mut S, M)) -> usize {
        self.drain_at_most(Self::PER_FRAME, state, apply)
    }

    /// [`Inbox::drain`] with the cap named, for the tests that need to
    /// see the boundary from both sides.
    pub fn drain_at_most<S>(&self, cap: usize, state: &mut S, apply: fn(&mut S, M)) -> usize {
        let mut n = 0;
        while n < cap {
            match self.rx.try_recv() {
                Ok((e, track, m)) => {
                    // **Cancellation, at the drain** — M28/T28.8. A
                    // result carrying any epoch but the current one is
                    // dropped. Equality, not an ordering test: see
                    // `Inbox::cancel`.
                    if e != self.epoch {
                        self.stale.set(self.stale.get() + 1);
                    } else if track.as_ref().is_some_and(|t| t.is_stopped()) {
                        // **A stopped job's result, at the drain** —
                        // M84/T84.7. Whatever it sent and nobody has
                        // applied yet is dropped, so the stop holds from
                        // the frame that set it even for a body that
                        // never asks `Sender::is_stopped`.
                        self.stopped_dropped.set(self.stopped_dropped.get() + 1);
                    } else {
                        apply(state, m);
                        n += 1;
                    }
                    // Consumed either way: a stale result is as landed as
                    // it will ever be (M47/T47.3).
                    if let Some(t) = track {
                        t.consumed.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(_) => break,
            }
        }
        n
    }

    /// Apply every fresh progress slot among `ledger`'s jobs — M88/T88.1.
    ///
    /// **One value per job per call**: the slot holds one, and taking it
    /// empties it. A slot whose job was spawned at another epoch is
    /// dropped and counted in [`Inbox::stale`]; one whose job was told to
    /// stop is dropped and counted in [`Inbox::stopped_dropped`] — the
    /// same two rules, in the same order, as the channel's drain. Returns
    /// how many were applied.
    ///
    /// The host calls this **before** [`Inbox::drain`], and only when its
    /// ledger is not empty, so an application that spawns nothing pays a
    /// length check. It walks what is in flight, which is zero on an idle
    /// frame.
    pub fn drain_progress<S>(
        &self,
        ledger: &crate::spawn::Ledger<M>,
        state: &mut S,
        apply: fn(&mut S, M),
    ) -> usize {
        let mut n = 0;
        for t in ledger.tracks() {
            let Some(m) = t.take_latest() else { continue };
            if t.epoch != self.epoch {
                self.stale.set(self.stale.get() + 1);
            } else if t.is_stopped() {
                self.stopped_dropped.set(self.stopped_dropped.get() + 1);
            } else {
                apply(state, m);
                n += 1;
            }
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
        if self.tx.try_send((self.epoch, None, m)).is_ok() {
            if let Some(w) = &self.waker {
                w.wake();
            }
        } else {
            self.refused.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Why a [`Sender::send`] did not queue — M28/T28.2.
///
/// Both variants hand the message back, so a caller that wants to retry
/// or to report the payload still has it. That is `mpsc`'s convention
/// and there is no reason to depart from it.
#[derive(Debug)]
pub enum SendError<M> {
    /// The queue is at [`Inbox::CAPACITY`]. The frame loop is behind;
    /// this is backpressure, and it is counted.
    Full(M),
    /// The host is gone — an ordinary end for a worker that outlived its
    /// window, not an error anyone needs to handle loudly.
    Disconnected(M),
}
