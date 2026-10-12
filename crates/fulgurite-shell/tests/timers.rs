//! **M28/T28.11: timers, and the deadline the loop waits on.**
//!
//! A timer is a deadline, not a thread that sleeps. A thread per timer
//! costs a thread and a context switch per tick, and it puts the timer's
//! identity in a thread handle rather than in data — the coupling the
//! rest of this milestone is arranged to avoid.
//!
//! But **a deadline needs a frame to be checked on, and an idle window
//! produces none**: the loop is `ControlFlow::Wait` and asks for a
//! redraw only while something is animating. So the host publishes its
//! earliest pending deadline and the loop waits *until* that instant.
//!
//! The plan's first draft gated this on "a second of driven frames",
//! which is the one situation where the defect cannot appear — a driven
//! loop produces frames by definition.

use fulgurite::event::App;
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite::spawn::{Descriptor, Job, JobFn, Outbox, Spawner, Timers};
use fulgurite::Rebuild;
use fulgurite_shell::Host;

const TICK: u64 = 9;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Default)]
struct Work {
    ui: Ui,
    outbox: Outbox,
    timers: Timers,
    fired: Vec<u64>,
}

fn on_tick(d: Descriptor, tx: &fulgurite::inbox::Sender<u64>) {
    let _ = tx.send(d.payload);
}

impl App for Work {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(this: &mut Self, m: u64) -> bool {
        this.fired.push(m);
        true
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }
    fn timers(&mut self) -> Option<&mut Timers> {
        Some(&mut self.timers)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        (d.kind == TICK).then_some(on_tick as JobFn<u64>)
    }
}

/// A spawner that counts, so "zero threads for a timer" is measured
/// **through T28.3's seam** — which every thread this milestone creates
/// goes through — rather than by asking the OS how many threads exist.
#[derive(Default)]
struct Counting {
    spawns: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Spawner<u64> for Counting {
    fn spawn(&mut self, job: Job<u64>, tx: fulgurite::inbox::Sender<u64>) {
        self.spawns.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (job.job)(job.desc, &tx);
    }
}

/// **The deterministic half.** Forty frames of a 25 ms tick is exactly
/// one synthetic second; a 50 ms timer must fire exactly twenty times.
///
/// Both quantities print beside their bounds, so a setup that silently
/// did not tick is visible in the output rather than inferred from a
/// passing assertion.
#[test]
fn a_50ms_timer_fires_exactly_20_times_in_a_synthetic_second() {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    let spawns = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    h.set_spawner(Box::new(Counting { spawns: spawns.clone() }));
    h.pump();

    assert!(h.app.timers.every(TICK, 1, 50.0), "setup: the timer was accepted");

    let mut elapsed = 0.0f32;
    for _ in 0..40 {
        h.tick(25.0);
        elapsed += 25.0;
        h.pump();
    }

    println!(
        "40 frames x 25 ms: elapsed {elapsed:.1} ms (of 1000.0), a 50 ms timer fired {} times (of 20)",
        h.app.fired.len()
    );
    assert_eq!(elapsed, 1000.0, "setup: exactly one synthetic second was driven");
    assert_eq!(
        h.app.fired.len(),
        20,
        "a 50 ms timer over one second must fire 20 times, not {}",
        h.app.fired.len()
    );
    assert!(h.app.fired.iter().all(|&k| k == 1), "every firing carried the timer's key");
}

/// **One spawn per firing**, counted through the spawner seam.
///
/// This test's label said "zero threads for a timer" until M47/T47.3
/// (R7-F5), and what it counts is **spawns**: a timer creates no timer
/// thread of its own, and each firing is one job. How many *threads*
/// those jobs cost depends on the spawner — one each under the old
/// `Threads`, at most the pool's cap now — and
/// `twenty_firings_create_at_most_the_pool` below counts that.
#[test]
fn a_timer_firing_is_one_spawn() {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    let spawns = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    h.set_spawner(Box::new(Counting { spawns: spawns.clone() }));
    h.pump();
    h.app.timers.every(TICK, 1, 50.0);

    for _ in 0..40 {
        h.tick(25.0);
        h.pump();
    }

    let n = spawns.load(std::sync::atomic::Ordering::SeqCst);
    println!("20 timer firings went through {n} spawns (one job each, no timer thread)");
    assert_eq!(
        n, 20,
        "a timer must spawn exactly one job per firing and no job of its own; {n} \
         spawns for 20 firings means something is spawning per tick as well"
    );
}

/// **The mechanism.** The host's earliest deadline is what a loop waits
/// on, asserted without a window.
#[test]
fn the_host_publishes_its_earliest_deadline() {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.pump();

    assert_eq!(h.next_deadline_ms(), None, "nothing pending: the loop may wait indefinitely");

    h.app.timers.every(TICK, 1, 200.0);
    h.app.timers.every(TICK, 2, 50.0);
    assert_eq!(h.next_deadline_ms(), Some(50.0), "the soonest of the two");

    h.tick(30.0);
    let d = h.next_deadline_ms().expect("still pending");
    println!("after 30 ms, the earliest deadline is {d:.1} ms (was 50.0)");
    assert!((d - 20.0).abs() < 1e-3, "the deadline counts down: expected 20.0, got {d}");

    h.app.timers.cancel(TICK, 2);
    assert_eq!(h.next_deadline_ms(), Some(170.0), "cancelling the soonest exposes the next");
    h.app.timers.cancel(TICK, 1);
    assert_eq!(h.next_deadline_ms(), None, "and cancelling the last returns to indefinite");
}

/// A long frame **catches up** rather than dropping ticks.
///
/// A timer whose count depends on the frame rate is a timer that cannot
/// be asserted, which is what the deterministic half above needs.
#[test]
fn a_long_frame_catches_up_instead_of_dropping_ticks() {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.set_spawner(Box::new(fulgurite::spawn::Immediate));
    h.pump();
    h.app.timers.every(TICK, 1, 25.0);

    // One 80 ms frame: three 25 ms periods elapsed inside it.
    h.tick(80.0);
    h.pump();
    h.pump();

    println!("one 80 ms frame, 25 ms timer: fired {} times (of 3)", h.app.fired.len());
    assert_eq!(h.app.fired.len(), 3, "a long frame must catch up, not skip");
}

#[test]
fn a_nonsense_period_is_refused_and_counted() {
    let mut t = Timers::new();
    assert!(!t.every(TICK, 1, 0.0), "a zero period is an infinite loop");
    assert!(!t.every(TICK, 2, -5.0));
    assert!(!t.every(TICK, 3, f32::NAN), "a NaN period never fires");
    assert!(t.every(TICK, 4, 16.0), "and a real one is taken");
    println!("nonsense periods refused: {} (of 3), accepted {}", t.refused(), t.len());
    assert_eq!(t.refused(), 3);
    assert_eq!(t.len(), 1);
}

// ---------------------------------------------------------------------------
// M47/T47.0 — the gates for R7-F3, R7-F4 and R7-F5, committed red.
// ---------------------------------------------------------------------------

/// **A stalled `dt` must not hang the catch-up** — R7-F3.
///
/// `advance` caught up one period at a time in `f32`: `while remaining
/// <= 0 { remaining += period }`. Past about 2^24 periods, adding 16 to
/// the remainder no longer changes it and the loop never ends. The
/// re-triage's probe: `dt` 1e9 at 16 ms did not return within 5000 ms.
///
/// Run on its own thread behind a 200 ms watchdog, because the failure
/// is a hang and `cargo test` has no per-test timeout. The spinning
/// thread is abandoned, not joined; the harness exits past it.
#[test]
fn a_huge_dt_returns_within_the_watchdog() {
    const WATCHDOG: std::time::Duration = std::time::Duration::from_millis(200);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut t = Timers::new();
        assert!(t.every(TICK, 1, 16.0), "setup: the timer was accepted");
        let mut out = Outbox::new();
        let started = std::time::Instant::now();
        let fired = t.advance(1e9, &mut out);
        let _ = tx.send((fired, t.earliest(), out.len(), started.elapsed()));
    });
    match rx.recv_timeout(WATCHDOG) {
        Ok((done, remaining, queued, took)) => {
            println!(
                "advance(1e9) at 16 ms: returned in {:.3} ms (bound {} ms), fired {} \
                 (of 62500000: delivered {}, coalesced {}), queued {queued}, remaining {remaining:?} \
                 (in (0, 16])",
                took.as_secs_f64() * 1e3,
                WATCHDOG.as_millis(),
                done.fired(),
                done.delivered,
                done.coalesced
            );
            assert!(took < WATCHDOG);
            assert_eq!(done.fired(), 62_500_000, "1e9 ms of 16 ms periods");
            assert_eq!(done.delivered, Outbox::CAPACITY, "the outbox filled, and no further");
            let r = remaining.expect("the timer is still running");
            assert!(r > 0.0 && r <= 16.0, "remaining {r} is not in (0, 16]");
        }
        Err(_) => panic!(
            "advance(1e9) at a 16 ms period did not return within {} ms: the catch-up \
             loop is spinning (R7-F3)",
            WATCHDOG.as_millis()
        ),
    }
}

/// **Every firing is delivered or counted** — R7-F4.
///
/// `Host::tick` advanced into a stack-local scratch outbox of 32 and
/// dropped its refusal count, so a 10 ms timer handed `tick(1000)` fired
/// 100, delivered 32, and counted the other 68 nowhere a caller can
/// read. `counted` here is the sum of every counter a caller can read.
#[test]
fn a_stalled_tick_delivers_or_counts_every_firing() {
    const FIRED: usize = 100; // 1000 ms / 10 ms
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.set_spawner(Box::new(fulgurite::spawn::Immediate));
    h.pump();
    assert!(h.app.timers.every(TICK, 1, 10.0), "setup: the timer was accepted");

    h.tick(1000.0);
    // Enough pumps to drain the inbox at 64 a frame.
    for _ in 0..8 {
        h.pump();
    }

    let delivered = h.app.fired.len();
    let counted = counted_losses(&mut h);
    println!(
        "10 ms timer, tick(1000): fired {FIRED}, delivered {delivered}, counted {counted} \
         (delivered + counted must be {FIRED})"
    );
    assert!(delivered > 0, "setup: the timer fired at all");
    assert_eq!(
        delivered + counted,
        FIRED,
        "{} firings were neither delivered nor counted anywhere a caller can read",
        FIRED - delivered - counted
    );
}

/// Every drop counter a caller can read on this host.
fn counted_losses(h: &mut Host<Work, Rebuild>) -> usize {
    h.timer_coalesced() as usize
        + h.app.outbox.refused()
        + h.app.timers.refused()
        + h.inbox.refused()
        + h.inbox.stale()
        + h.unknown_kind()
        + h.unspawned()
}

/// Which threads ran a [`on_tick_where`] job. A static because a job is a
/// non-capturing `fn`; only [`twenty_firings_create_at_most_the_pool`]
/// uses this kind.
static RAN_ON: std::sync::Mutex<Vec<std::thread::ThreadId>> = std::sync::Mutex::new(Vec::new());
const TICK_WHERE: u64 = 10;

fn on_tick_where(d: Descriptor, tx: &fulgurite::inbox::Sender<u64>) {
    RAN_ON.lock().unwrap().push(std::thread::current().id());
    let _ = tx.send(d.payload);
}

#[derive(Default)]
struct Where {
    inner: Work,
}

impl App for Where {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.inner.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.inner.ui
    }
    fn apply(this: &mut Self, m: u64) -> bool {
        this.inner.fired.push(m);
        true
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.inner.outbox)
    }
    fn timers(&mut self) -> Option<&mut Timers> {
        Some(&mut self.inner.timers)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        (d.kind == TICK_WHERE).then_some(on_tick_where as JobFn<u64>)
    }
}

/// **Threads created for 20 timer firings** under the spawner a window
/// installs — R7-F5, closed by M47/T47.3.
///
/// `Local::from_host` installs `Threads`, and until T47.3 that was one
/// `std::thread` per job: strace counted 28 clones for 28 ticks of
/// stock `tasks-demo`. Counted here as the **distinct `ThreadId`s** the
/// jobs ran on: an id is never reused within a process, so the count is
/// threads created, not threads alive, and threads other tests in this
/// binary create cannot enter it.
#[test]
fn twenty_firings_create_at_most_the_pool() {
    use fulgurite::spawn::Threads;
    const FIRINGS: usize = 20;

    let pool = Threads::new();
    let cap = pool.cap();
    let mut h: Host<Where, Rebuild> = Host::new(Where::default(), 100, 50);
    h.set_spawner(Box::new(pool));
    h.pump();
    assert!(h.app.inner.timers.every(TICK_WHERE, 1, 50.0), "setup: the timer was accepted");

    for _ in 0..40 {
        h.tick(25.0);
        h.pump();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.app.inner.fired.len() < FIRINGS && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(1));
        h.pump();
    }

    let mut ids = RAN_ON.lock().unwrap().clone();
    let ran = ids.len();
    ids.sort_by_key(|i| format!("{i:?}"));
    ids.dedup();
    let created = ids.len();
    println!("threads created {created} for {FIRINGS} firings (bound {cap}, the pool's cap); jobs ran {ran}");
    assert_eq!(h.app.inner.fired.len(), FIRINGS, "setup: every firing landed");
    assert!(
        created <= cap,
        "{created} threads for {FIRINGS} timer firings: the spawner creates a thread per job"
    );
}

/// The accepting direction for the two gates above: **an hour** at 16 ms
/// returns (the re-triage's probe: 225000 fired), and a stall that fits
/// the outbox delivers every firing with nothing counted.
#[test]
fn an_hour_returns_and_a_stall_that_fits_delivers_everything() {
    let mut t = Timers::new();
    t.every(TICK, 1, 16.0);
    let mut out = Outbox::new();
    let started = std::time::Instant::now();
    let fired = t.advance(3_600_000.0, &mut out).fired();
    println!(
        "advance(1 h) at 16 ms: fired {fired} (of 225000) in {:.1} ms, queued {}, coalesced {:?}",
        started.elapsed().as_secs_f64() * 1e3,
        out.len(),
        t.coalesced(TICK, 1)
    );
    assert_eq!(fired, 225_000, "one hour of 16 ms periods");
    assert_eq!(t.coalesced(TICK, 1), Some(225_000 - 32), "and what did not fit is counted per timer");

    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.set_spawner(Box::new(fulgurite::spawn::Immediate));
    h.pump();
    h.app.timers.every(TICK, 1, 10.0);
    h.tick(300.0);
    h.pump();
    h.pump();
    let counted = counted_losses(&mut h);
    println!("10 ms timer, tick(300): delivered {} (of 30), counted {counted}", h.app.fired.len());
    assert_eq!(h.app.fired.len(), 30, "a stall that fits the outbox delivers every firing");
    assert_eq!(counted, 0, "and counts nothing");
}
