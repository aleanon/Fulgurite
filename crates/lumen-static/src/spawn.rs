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
/// | [`Threads`] | the default: one `std::thread` per job, no dependency |
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

/// One `std::thread` per job — the default, and no dependency.
///
/// `docs/plan-tasks.md` §0.3: no figure in this repository says what an
/// async runtime would cost, because nobody has measured one. A thread
/// per job is not the fastest arrangement imaginable; it is the one
/// whose cost is already known and whose crate count is zero.
#[derive(Default)]
pub struct Threads;

impl<M: Send + 'static> Spawner<M> for Threads {
    fn spawn(&mut self, job: Job<M>, tx: Sender<M>) {
        // **The panic is caught here, at the seam** — M28/T28.9.
        //
        // Left to unwind, the thread dies, its `Sender` drops, and a
        // chain waiting on that completion stalls forever with nothing
        // observing it. `catch_unwind` turns that into a value the
        // application applies like any other result.
        //
        // The default hook still prints the panic to stderr, which is
        // wanted: this reports the failure, it does not hide it.
        // A second handle, so the refusal path below still has one: the
        // closure moves the first into the thread.
        let fallback = tx.clone();
        let started = std::thread::Builder::new().spawn(move || {
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (job.job)(job.desc, &tx)
            }));
            if out.is_err() {
                report(&job, &tx, Failure::Panicked);
            }
        });
        // `Builder::spawn` rather than `thread::spawn`, because the
        // latter panics when the OS refuses and this is the one place
        // that refusal has somewhere to go.
        if started.is_err() {
            report(&job, &fallback, Failure::NotStarted);
        }
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
