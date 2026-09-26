//! Where deferred work runs — M28/T28.3, `docs/plan-tasks.md` §2.2.
//!
//! `docs/plan-agent-parity.md`'s appendix settled the location: a spawner
//! is `dyn` at the **shell**, never a widget field and never a new type
//! parameter on `Host`. This module is the seam, not a runtime.
//!
//! # What a job is
//!
//! A job body is code by necessity. The only form that is not *a code
//! reference in flight* is a non-capturing `fn` pointer supplied at spawn
//! from the currently loaded build:
//!
//! ```text
//! fn(Descriptor, &Sender<M>)
//! ```
//!
//! Not a boxed closure. An `fn` parameter refuses a capturing closure at
//! the call site, by type — the same guarantee the handler fields
//! already have, for the same reason: `fresh()` rebuilds the tree but
//! cannot reach into work already in flight and swap the pointer it
//! captured (`docs/events-decision.md` rule 3).
//!
//! The pointer is read out of the loaded build *at the moment of spawn*
//! and is gone by the time the result comes back. What crosses the frame
//! boundary is the [`Descriptor`] and the message: data both ways.

use crate::inbox::Sender;

/// What to do, as data — `docs/plan-tasks.md` §2.1.
///
/// Three fields and no closure. `kind` says which job, `step` says where
/// in a chain it is, `payload` is whatever that pair agrees it means.
///
/// **Kind and step ids are stable identifiers, append-only.** A
/// descriptor whose `(kind, step)` the currently loaded build does not
/// recognise is dropped with a counted fallback rather than guessed at —
/// the same rule the reload model already applies to an enum variant a
/// new build removed. Without it a renumbering silently runs the wrong
/// step.
///
/// **Crossing a dylib swap it crosses as primitives.** Rule 3 forbids a
/// code reference in flight; `docs/step4-hot-reload.md`'s honesty list
/// separately forbids a framework type crossing the seam, because v1 and
/// v2 each link their own copy and no layout agreement is promised.
/// Three `u64`s are not a framework type. Within one process, between
/// threads, this is an ordinary Rust value and neither half applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Descriptor {
    /// Which job. Stable, append-only.
    pub kind: u64,
    /// Which step of a chain — `docs/plan-tasks.md` §2.1's answer to
    /// iced's `.then(|out| next)`, which holds a closure across an await
    /// and cannot be ported literally.
    pub step: u64,
    /// Whatever `(kind, step)` agrees it means.
    pub payload: u64,
}

impl Descriptor {
    /// A first step of `kind`, carrying `payload`.
    pub const fn new(kind: u64, payload: u64) -> Descriptor {
        Descriptor { kind, step: 0, payload }
    }

    /// The same job at another step — the chain move, as data.
    pub const fn at(self, step: u64) -> Descriptor {
        Descriptor { step, ..self }
    }
}

/// The body of a job, as the type an application's table returns.
///
/// A named alias because the bare spelling —
/// `fn(Descriptor, &Sender<M>)` inside an `Option` inside a return type
/// — trips clippy's type-complexity lint, and because naming it is where
/// the one rule about it belongs: **non-capturing, by type**. An `fn`
/// pointer refuses a capturing closure at the call site, which is what
/// keeps a code reference out of flight when the result comes back.
pub type JobFn<M> = fn(Descriptor, &Sender<M>);

/// Why a job produced no result — M28/T28.9.
///
/// The plan had no failure story and the review was right that one is
/// needed **before** anything is built on top: under the thread default
/// a panicking job unwinds, its sender drops, and a chain waiting on
/// that completion stalls forever with nothing observing it.
///
/// Both variants become a **failure result the application applies like
/// any other** — data, not a callback. That is the same answer the rest
/// of this milestone gives, for the same reason: a callback would be a
/// code reference, and a chain that stalls silently is worse than one
/// that reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The job body panicked. It was caught at the spawner, so the
    /// process lives and the chain is told.
    Panicked,
    /// The job never started — the OS refused a thread, or no spawner
    /// was installed.
    NotStarted,
}

/// How an application turns a failure into one of its own messages.
///
/// `None` means "this application does not model that failure", and the
/// host counts the drop rather than inventing a message.
pub type FailFn<M> = fn(Descriptor, Failure) -> Option<M>;

/// One unit of deferred work: a descriptor and the code that runs it.
///
/// `job` is read from the currently loaded build at spawn time, which is
/// what keeps the `fn` pointer out of flight.
pub struct Job<M: 'static> {
    /// What to do.
    pub desc: Descriptor,
    /// How, in the build that spawned it. Non-capturing by type.
    pub job: JobFn<M>,
    /// What to send if it never produces a result — M28/T28.9. Supplied
    /// by `Host::spawn` from `App::failed`, out of the same currently
    /// loaded build as `job`.
    pub on_fail: Option<FailFn<M>>,
}

impl<M: 'static> Clone for Job<M> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<M: 'static> Copy for Job<M> {}

/// Somewhere to run jobs — the `dyn` seam.
///
/// Three implementations ship, and the third exists because a test needs
/// **deterministic *and* deferred** at once:
///
/// | spawner | for |
/// |---|---|
/// | [`Threads`] | the default: a small pool of long-lived workers, no dependency (a thread per job until M47/T47.3) |
/// | [`Immediate`] | runs the job synchronously at spawn — tests that do not care when |
/// | [`Queued`] | records the job and runs nothing until asked. The only one that can assert "*not* the frame that spawned it" without a sleep |
///
/// A real async runtime would be a fourth, opt-in behind a feature, if
/// anything ever needs one. `docs/plan-tasks.md` §0.3's crate budget
/// applies and **no figure exists for it**, which is why the default
/// takes no dependency at all.
pub trait Spawner<M: 'static>: Send {
    /// Start `job`, or arrange for it to start. Results come back
    /// through `tx`.
    fn spawn(&mut self, job: Job<M>, tx: Sender<M>);

    /// Run whatever this spawner is holding, and say how many.
    ///
    /// Only [`Queued`] holds anything; the default answers zero, which
    /// is the truth for a spawner that has already started everything it
    /// was given. It is on the trait rather than on `Queued` because a
    /// spawner lives behind `dyn` at the shell — a test that had to
    /// downcast to reach it would be testing the box, and a harness
    /// driving a queued spawner generically is the shape a deterministic
    /// test needs.
    ///
    /// **Provided, and right for every implementor**, which is the case
    /// `AGENT.md` warns is rare: the warning is about a provided method
    /// on a *wrapper*, where forwarding is always the right answer and
    /// the default acts on the wrapper instead. Nothing here wraps
    /// anything.
    fn run_pending(&mut self) -> usize {
        0
    }
}

/// A small pool of long-lived workers — the default, and no dependency.
///
/// `docs/plan-tasks.md` §0.3: no figure in this repository says what an
/// async runtime would cost, because nobody has measured one, so the
/// default takes no crate at all.
///
/// **A pool since M47/T47.3** (R7-F5, `docs/plan-timers-wake.md` §2.3).
/// It was one `std::thread` per job, and a timer firing is a job, so a
/// window with a 120 ms timer created a thread per tick: strace counted
/// 28 clones for 28 ticks of `tasks-demo`. Now jobs go through a channel
/// to at most [`Threads::cap`] workers, started **lazily** — one when a
/// job arrives and no worker is free — so a window that never spawns
/// creates no thread, and one that spawns only timer ticks creates one.
///
/// **What it costs**: a job that blocks occupies a worker, and with every
/// worker busy the next job waits in the channel — timer jobs included.
/// The cap is at least two so that one slow job cannot hold a tick back
/// (T47.3's accepting check runs a 400 ms job beside a 120 ms timer).
/// `docs/results-tasks.md` T47.3 has the spawn-to-apply figure the kill
/// rule judged it on.
///
/// **What it does not track**: whether a job is in flight. That is the
/// host's ledger (`Host::in_flight`), built on the sender the host hands
/// every job ([`crate::inbox::Inbox::tracked_sender`]), so it holds under
/// any spawner, not only this one.
///
/// Dropping it closes the channel: workers finish what is queued and
/// exit. They are not joined, so a drop never waits on a slow job.
pub struct Threads {
    queue: std::sync::mpsc::Sender<PoolTask>,
    rx: std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<PoolTask>>>,
    /// Workers waiting on the channel right now.
    idle: std::sync::Arc<core::sync::atomic::AtomicUsize>,
    /// Jobs sent and not yet taken by a worker.
    waiting: std::sync::Arc<core::sync::atomic::AtomicUsize>,
    started: usize,
    cap: usize,
}

/// One job as a worker sees it: its body, its sender and its failure
/// report, already paired.
type PoolTask = Box<dyn FnOnce() + Send>;

impl Default for Threads {
    fn default() -> Self {
        Threads::new()
    }
}

impl Threads {
    /// The most workers any pool starts, whatever the machine.
    pub const MAX_WORKERS: usize = 4;

    /// A pool sized from the machine's parallelism, between 2 and
    /// [`Threads::MAX_WORKERS`]. Starts no thread until the first job.
    pub fn new() -> Threads {
        let n = std::thread::available_parallelism().map_or(2, |n| n.get());
        Threads::with_cap(n.clamp(2, Threads::MAX_WORKERS))
    }

    /// A pool of at most `cap` workers (at least one).
    pub fn with_cap(cap: usize) -> Threads {
        let (queue, rx) = std::sync::mpsc::channel();
        Threads {
            queue,
            rx: std::sync::Arc::new(std::sync::Mutex::new(rx)),
            idle: Default::default(),
            waiting: Default::default(),
            started: 0,
            cap: cap.max(1),
        }
    }

    /// The most workers this pool will start.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// How many workers it has started — threads created, since a
    /// worker never exits while the pool lives.
    pub fn started(&self) -> usize {
        self.started
    }

    /// Start one worker. `false` if the OS refused the thread.
    fn start_worker(&mut self) -> bool {
        use core::sync::atomic::Ordering::SeqCst;
        let rx = self.rx.clone();
        let idle = self.idle.clone();
        let waiting = self.waiting.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("lumen-job-{}", self.started))
            .spawn(move || loop {
                idle.fetch_add(1, SeqCst);
                // The lock is held only while waiting for the next task;
                // a panicking job cannot poison it, because the task runs
                // after the guard is gone and is caught inside it anyway.
                let next = match rx.lock() {
                    Ok(guard) => guard.recv(),
                    Err(_) => return,
                };
                idle.fetch_sub(1, SeqCst);
                match next {
                    Ok(task) => {
                        waiting.fetch_sub(1, SeqCst);
                        task();
                    }
                    // The pool was dropped and the queue is empty.
                    Err(_) => return,
                }
            });
        if spawned.is_ok() {
            self.started += 1;
        }
        spawned.is_ok()
    }
}

impl<M: Send + 'static> Spawner<M> for Threads {
    fn spawn(&mut self, job: Job<M>, tx: Sender<M>) {
        use core::sync::atomic::Ordering::SeqCst;
        // **A worker when none is free**, up to the cap. `waiting` counts
        // jobs already queued ahead of this one, so a burst of N starts
        // up to N workers rather than piling onto the one that happened
        // to be idle when the burst began.
        let ahead = self.waiting.load(SeqCst);
        if self.started < self.cap && self.idle.load(SeqCst) <= ahead {
            // `Builder::spawn` rather than `thread::spawn`, because the
            // latter panics when the OS refuses and this is the one
            // place that refusal has somewhere to go. A refusal with
            // workers already running only means the job waits; with
            // none, nothing would ever take it.
            if !self.start_worker() && self.started == 0 {
                report(&job, &tx, Failure::NotStarted);
                return;
            }
        }
        // **The panic is caught here, at the seam** — M28/T28.9.
        //
        // Left to unwind, the worker dies, the `Sender` drops, and a
        // chain waiting on that completion stalls forever with nothing
        // observing it. `catch_unwind` turns that into a value the
        // application applies like any other result, and keeps the
        // worker for the next job.
        //
        // The default hook still prints the panic to stderr, which is
        // wanted: this reports the failure, it does not hide it.
        let task: PoolTask = Box::new(move || {
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (job.job)(job.desc, &tx)
            }));
            if out.is_err() {
                report(&job, &tx, Failure::Panicked);
            }
            // `tx` drops here, after the body and any failure report:
            // that is what ends the job in the host's ledger.
        });
        self.waiting.fetch_add(1, SeqCst);
        // Cannot fail: `self.rx` keeps the receiving end alive for as
        // long as `self` can be called.
        let _ = self.queue.send(task);
    }
}

/// Every job a host has spawned and not yet seen land — M47/T47.3.
///
/// **Its length is the in-flight count.** A descriptor goes in when the
/// host hands its job to a spawner, and comes out once the job's body
/// has returned *and* every result it sent has been drained
/// ([`JobTrack::settled`](crate::inbox::JobTrack::settled)). So a job
/// whose result is still sitting in the inbox is listed, and one whose
/// body is still running is listed, and nothing else is.
///
/// It is one structure for three readers, on purpose, so they cannot
/// disagree: `Host::in_flight` answers from its length (and M48/T48.2's
/// "is anything pending" is that answer); M49/T49.10 re-issues
/// [`Ledger::descriptors`] to a new build across a swap; and a test asks
/// it where it would otherwise have to sleep.
///
/// **What it cannot see**: a clone of a job's sender that outlives the
/// body. Its later results are still applied, untracked — the entry ends
/// with the body, which is the only end a spawner can observe. And a
/// subscription whose job never returns stays listed for as long as it
/// runs, which is the truth about it.
///
/// Costs one `Arc` and one `Vec` slot per spawn, both off the frame
/// path; pruning is a walk of what is listed, which is what is in flight.
#[derive(Default)]
pub struct Ledger {
    entries: Vec<(Descriptor, std::sync::Arc<crate::inbox::JobTrack>)>,
}

impl Ledger {
    pub const fn new() -> Ledger {
        Ledger { entries: Vec::new() }
    }

    /// List a job the host has just handed to a spawner.
    pub fn record(&mut self, desc: Descriptor, track: std::sync::Arc<crate::inbox::JobTrack>) {
        self.entries.push((desc, track));
    }

    /// Drop every entry that has settled, oldest order kept. Returns how
    /// many went.
    pub fn prune(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(_, t)| !t.settled());
        before - self.entries.len()
    }

    /// How many jobs were in flight at the last [`Ledger::prune`].
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// What is listed, oldest first: the data a reload re-issues.
    pub fn descriptors(&self) -> impl Iterator<Item = Descriptor> + '_ {
        self.entries.iter().map(|(d, _)| *d)
    }
}

/// Send a failure result, if the application models this one.
///
/// A free function because three spawners need it and none of them owns
/// it. A failure that cannot be expressed is dropped here rather than
/// invented; the host's `unknown_kind`-style counters are for what the
/// host can see, and a spawner has no host.
fn report<M: Send + 'static>(job: &Job<M>, tx: &Sender<M>, why: Failure) {
    if let Some(f) = job.on_fail {
        if let Some(m) = f(job.desc, why) {
            let _ = tx.send(m);
        }
    }
}

/// Runs the job synchronously, on the spawning thread, before returning.
///
/// For tests that care *that* a result arrives and not *when*. It cannot
/// assert "not this frame", because under it the answer is always "this
/// frame" — which is what [`Queued`] is for.
#[derive(Default)]
pub struct Immediate;

impl<M: Send + 'static> Spawner<M> for Immediate {
    fn spawn(&mut self, job: Job<M>, tx: Sender<M>) {
        // Caught here too, so a test that uses this spawner sees the
        // same failure shape a threaded one would. A spawner that let
        // the panic through would make the deterministic arm disagree
        // with the shipping one about the thing under test.
        let out =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (job.job)(job.desc, &tx)));
        if out.is_err() {
            report(&job, &tx, Failure::Panicked);
        }
    }
}

/// Records jobs and runs nothing until [`Queued::run_pending`] is called.
///
/// **Deterministic and deferred at once**, which is the combination an
/// assertion like "the result did not arrive on the frame that spawned
/// it" needs. Under [`Threads`] that assertion is a race; under
/// [`Immediate`] it is false by construction. A review of this plan's
/// first draft found T28.5 asking for exactly this property with neither
/// spawner able to give it, which is why this type exists.
#[derive(Default)]
pub struct Queued<M: 'static> {
    pending: Vec<(Job<M>, Sender<M>)>,
}

impl<M: Send + 'static> Queued<M> {
    /// How many jobs are waiting.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Run everything recorded so far, oldest first, and return how many.
    ///
    /// Drains before running, so a job that spawns another leaves the
    /// new one for the *next* call rather than extending this one — an
    /// unbounded loop here would be the same defect the inbox's
    /// per-frame cap removes.
    pub fn run_pending(&mut self) -> usize {
        let batch: Vec<_> = self.pending.drain(..).collect();
        let n = batch.len();
        for (job, tx) in batch {
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (job.job)(job.desc, &tx)
            }));
            if out.is_err() {
                report(&job, &tx, Failure::Panicked);
            }
        }
        n
    }
}

impl<M: Send + 'static> Spawner<M> for Queued<M> {
    fn spawn(&mut self, job: Job<M>, tx: Sender<M>) {
        self.pending.push((job, tx));
    }

    fn run_pending(&mut self) -> usize {
        Queued::run_pending(self)
    }
}

/// The queue **out** of a frame — M28/T28.4, `docs/plan-tasks.md` §2.5.
///
/// The review's first blocking finding against this plan's first draft
/// was that **nothing could spawn a task at all**. Every
/// application-authored code path is `fn(&mut S, &EventCtx)`: a handler
/// reaches the app and nothing else. It has no host and no spawner, and
/// it cannot be given one — a spawner handle in the app would be a code
/// reference in saved state, which `#[reload(keep)]` already refuses to
/// compile.
///
/// So the request is **data**, and it travels the way input already
/// does. Input has a queue into the frame; this is the queue out of it:
/// a handler pushes a [`Descriptor`], the host drains after dispatch and
/// pairs each one with the job `fn` from the currently loaded build.
///
/// **Fixed capacity, no allocation.** It lives on the application, which
/// is laid out beside the tree and counted by the same allocation gate
/// as a frame. Overflow refuses and counts, like the inbox's.
pub struct Outbox {
    q: [Descriptor; Outbox::CAPACITY],
    len: usize,
    refused: usize,
}

impl Default for Outbox {
    fn default() -> Self {
        Outbox::new()
    }
}

impl Outbox {
    /// How many requests one frame may hold.
    ///
    /// A frame's worth of handler output, not a backlog: the host drains
    /// it to empty every frame, so this bounds what a *single* dispatch
    /// can ask for. Thirty-two is far past any handler that has been
    /// written, and the refusal counter is what says so if that changes.
    pub const CAPACITY: usize = 32;

    pub const fn new() -> Outbox {
        Outbox {
            q: [Descriptor { kind: 0, step: 0, payload: 0 }; Outbox::CAPACITY],
            len: 0,
            refused: 0,
        }
    }

    /// Ask for work. Returns whether it was queued.
    ///
    /// **Refuses rather than overwrites** when full. Overwriting would
    /// drop a request that a handler believes it made, silently; the
    /// count is what makes the drop visible.
    pub fn push(&mut self, d: Descriptor) -> bool {
        if self.len == Outbox::CAPACITY {
            self.refused += 1;
            return false;
        }
        self.q[self.len] = d;
        self.len += 1;
        true
    }

    /// How many requests are waiting for the host to drain.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How many pushes were refused for want of capacity. Monotonic.
    pub fn refused(&self) -> usize {
        self.refused
    }

    /// Take everything queued, oldest first, leaving it empty.
    ///
    /// The host calls this after dispatch. Returns the count and writes
    /// into `out`, so the drain itself allocates nothing.
    pub fn drain_into(&mut self, out: &mut [Descriptor; Outbox::CAPACITY]) -> usize {
        let n = self.len;
        out[..n].copy_from_slice(&self.q[..n]);
        self.len = 0;
        n
    }
}

/// What should be running — M28/T28.10, `docs/plan-tasks.md` §2.4.
///
/// iced hashes a *recipe* to decide which streams keep running. Here
/// identity is `(kind, key)` — values — and the host diffs this set each
/// frame against what it already started: start what appeared, stop what
/// vanished, leave the rest alone.
///
/// Hashing code would put the identity of a stream in the code that
/// produces it, which is the same coupling `docs/events-decision.md`
/// rule 3 removes from chains. Two builds that format a URL differently
/// would restart every subscription on reload; two that format it the
/// same would silently share one.
///
/// **Fixed capacity whose overflow is visible**, in the first of this
/// workspace's three flavours: refuse the new item and count the
/// refusal, as `Overlays<16>` does. (`InputQueue<64>` rejects the
/// *newest* event; `Boundaries<N>` widens to everything, which is
/// conservative rather than lossy. They are not interchangeable and the
/// plan's first draft flattened them into one.)
pub struct Subs {
    set: [(u64, u64); Subs::CAPACITY],
    len: usize,
    refused: usize,
}

impl Default for Subs {
    fn default() -> Self {
        Subs::new()
    }
}

impl Subs {
    /// How many subscriptions may run at once.
    pub const CAPACITY: usize = 16;

    pub const fn new() -> Subs {
        Subs { set: [(0, 0); Subs::CAPACITY], len: 0, refused: 0 }
    }

    /// Declare one. Returns whether it was taken.
    ///
    /// **Declaring the same `(kind, key)` twice is one subscription**,
    /// not two: the set is a set. Without that, an application that
    /// declares from a loop would start N copies of one stream and the
    /// diff would never settle.
    pub fn declare(&mut self, kind: u64, key: u64) -> bool {
        if self.set[..self.len].contains(&(kind, key)) {
            return true;
        }
        if self.len == Subs::CAPACITY {
            self.refused += 1;
            return false;
        }
        self.set[self.len] = (kind, key);
        self.len += 1;
        true
    }

    /// Forget everything declared. The host calls this before asking the
    /// application to re-declare, so "what should run" is rebuilt from
    /// scratch each frame rather than accumulated.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// What is declared right now.
    pub fn as_slice(&self) -> &[(u64, u64)] {
        &self.set[..self.len]
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How many declarations were refused for want of capacity.
    pub fn refused(&self) -> usize {
        self.refused
    }
}

/// Periodic work, as **deadlines** rather than sleeping threads —
/// M28/T28.11.
///
/// A thread per timer is the obvious implementation and the wrong one:
/// it costs a thread and a context switch per tick, and it puts the
/// timer's identity in a thread handle rather than in data, which is the
/// coupling [`Subs`] exists to avoid.
///
/// A deadline needs a frame to be checked on, and **an idle window
/// produces none** — the loop is `ControlFlow::Wait` and asks for a
/// redraw only while something is animating. So a host publishes its
/// earliest pending deadline ([`Timers::earliest`]) and the loop waits
/// *until* that instant instead of indefinitely. The plan's first draft
/// gated this on "a second of driven frames", which is the one situation
/// where the defect cannot appear.
///
/// **Waking at the instant is not checking the timer** (M47/T47.1,
/// R7-F1): the timers are ticked by a frame, so the loop must also *ask
/// for one* when it wakes there. Until T47.1 the window did not, and an
/// idle window's 120 ms timer ticked once in three seconds while the
/// live gate stayed green, because its every read asked for a frame
/// (`docs/results-tasks.md` T28.12, the correction). While a panic is
/// shown the window ticks nothing and arms no deadline, and a retry
/// starts from `dt` 0.
pub struct Timers {
    /// `(kind, key, period_ms, remaining_ms)`.
    t: [(u64, u64, f32, f32); Timers::CAPACITY],
    len: usize,
    refused: usize,
}

impl Default for Timers {
    fn default() -> Self {
        Timers::new()
    }
}

impl Timers {
    pub const CAPACITY: usize = 8;

    pub const fn new() -> Timers {
        Timers { t: [(0, 0, 0.0, 0.0); Timers::CAPACITY], len: 0, refused: 0 }
    }

    /// Fire `(kind, key)` every `period_ms`. Returns whether it was
    /// taken; a non-positive or non-finite period is refused, because a
    /// zero period is an infinite loop and a `NaN` one never fires.
    ///
    /// **Every other period is accepted, up to `f32::MAX`**, so a shell
    /// turning [`Timers::earliest`] into a wait must convert fallibly: a
    /// period above about 1.8e22 ms is no `Duration` and no `Instant`
    /// (R7-F10; `lumen-static-winit` waits without a deadline then, since
    /// M47/T47.1).
    pub fn every(&mut self, kind: u64, key: u64, period_ms: f32) -> bool {
        if !period_ms.is_finite() || period_ms <= 0.0 || self.len == Timers::CAPACITY {
            self.refused += 1;
            return false;
        }
        self.t[self.len] = (kind, key, period_ms, period_ms);
        self.len += 1;
        true
    }

    /// Stop `(kind, key)`. Returns whether one was removed.
    pub fn cancel(&mut self, kind: u64, key: u64) -> bool {
        for i in 0..self.len {
            if (self.t[i].0, self.t[i].1) == (kind, key) {
                self.t.swap(i, self.len - 1);
                self.len -= 1;
                return true;
            }
        }
        false
    }

    /// Advance every timer by `dt_ms` and push a descriptor for each one
    /// that came due. Returns how many fired.
    ///
    /// **Catches up rather than skipping.** A frame that took 80 ms with
    /// a 25 ms timer fires it three times, not once: a timer that
    /// silently drops ticks under load is a timer whose count depends on
    /// the frame rate, and the deterministic half of this task's gate
    /// exists precisely to pin the count.
    pub fn advance(&mut self, dt_ms: f32, out: &mut Outbox) -> usize {
        if !dt_ms.is_finite() || dt_ms <= 0.0 {
            return 0;
        }
        let mut fired = 0;
        for i in 0..self.len {
            let (kind, key, period) = (self.t[i].0, self.t[i].1, self.t[i].2);
            self.t[i].3 -= dt_ms;
            while self.t[i].3 <= 0.0 {
                self.t[i].3 += period;
                out.push(Descriptor { kind, step: 0, payload: key });
                fired += 1;
            }
        }
        fired
    }

    /// How long until the soonest timer is due, in milliseconds.
    ///
    /// `None` when nothing is pending, which is what tells a loop to
    /// fall back to waiting indefinitely rather than spinning.
    pub fn earliest(&self) -> Option<f32> {
        self.t[..self.len].iter().map(|e| e.3).fold(None, |acc: Option<f32>, v| {
            Some(acc.map_or(v, |a| a.min(v)))
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn refused(&self) -> usize {
        self.refused
    }
}
