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

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::spawn::{Descriptor, Job, JobFn, Outbox, Spawner, Timers};
use lumen_static::Rebuild;
use lumen_static_shell::Host;

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

fn on_tick(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(d.payload);
}

impl App for Work {
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(&mut self, m: u64) -> bool {
        self.fired.push(m);
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
    fn spawn(&mut self, job: Job<u64>, tx: lumen_static::inbox::Sender<u64>) {
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

/// **Zero threads for a timer**, counted through the spawner seam.
///
/// Every thread this milestone creates goes through `Spawner::spawn`, so
/// counting spawns is counting threads — and it is a measurement the
/// test can make, unlike asking the OS.
#[test]
fn a_timer_spawns_no_thread_of_its_own() {
    use lumen_static::spawn::Threads;

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
        "a timer must spawn exactly one job per firing and no thread of its own; {n} \
         spawns for 20 firings means something is spawning per tick as well"
    );
    // `Threads` is named so this test's claim is about the seam and not
    // about the counting spawner being unusual.
    let _: Threads = Threads;
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
    h.set_spawner(Box::new(lumen_static::spawn::Immediate));
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
