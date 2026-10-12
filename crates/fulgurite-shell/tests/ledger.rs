//! **M47/T47.3: the in-flight ledger, and the pool it rides on.**
//!
//! `Host::in_flight` is the length of the host's ledger: a job is listed
//! from the moment the host hands it to a spawner until its body has
//! returned **and** every result it sent has been drained. These tests
//! pin each half of that conjunction with the spawner that can hold the
//! two apart deterministically (`Queued`), and then check the shipping
//! pool (`Threads`) against the same definition.
//!
//! The ledger is the one source M48/T48.2 ("is anything pending") and
//! M49/T49.10 (re-issue what was in flight across a swap) read, so a
//! wrong answer here is a wrong answer in both.

use std::time::{Duration, Instant};

use fulgurite::event::App;
use fulgurite::prelude::*;
use fulgurite::spawn::{Descriptor, Job, Queued, Threads};
use fulgurite::Rebuild;
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Default)]
struct Work {
    ui: Ui,
    landed: Vec<u64>,
}

impl App for Work {
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
        this.landed.push(m);
        true
    }
}

fn echo(d: Descriptor, tx: &fulgurite::inbox::Sender<u64>) {
    let _ = tx.send(d.payload);
}

fn silent(_d: Descriptor, _tx: &fulgurite::inbox::Sender<u64>) {}

/// Sleeps for `payload` ms, then answers with it.
fn slow(d: Descriptor, tx: &fulgurite::inbox::Sender<u64>) {
    std::thread::sleep(Duration::from_millis(d.payload));
    let _ = tx.send(d.payload);
}

fn job(f: fulgurite::spawn::JobFn<u64>, payload: u64) -> Job<u64> {
    Job { desc: Descriptor { kind: 1, step: 0, payload }, job: f, on_fail: None }
}

fn host() -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.pump();
    h
}

/// **Each half of the conjunction, held apart.** Under `Queued` the body
/// runs only when asked, so the three states are three lines:
///
/// | after | body returned | result drained | in flight |
/// |---|---|---|---|
/// | spawn | no | no | 1 |
/// | `run_pending` | yes | no — it is in the inbox | 1 |
/// | pump | yes | yes | 0 |
///
/// The middle row is the one a count kept by the spawner would get
/// wrong, and the one a reload re-issuing work most needs: a result in
/// the old inbox is dropped by a swap, so its job must still be listed.
#[test]
fn a_job_is_in_flight_until_its_result_is_applied() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));
    assert_eq!(h.in_flight(), 0, "setup: nothing spawned");

    assert!(h.spawn(job(echo, 7)));
    let spawned = h.in_flight();
    h.run_pending_jobs();
    let ran = h.in_flight();
    h.pump();
    let applied = h.in_flight();

    println!(
        "in flight: after spawn {spawned} (want 1), after the body ran {ran} (want 1), \
         after the pump applied {:?} {applied} (want 0)",
        h.app.landed
    );
    assert_eq!(h.app.landed, vec![7], "setup: the result landed");
    assert_eq!(spawned, 1, "a spawned job is in flight");
    assert_eq!(ran, 1, "a job whose result is still in the inbox is still in flight");
    assert_eq!(applied, 0, "a job whose result was applied is not");
    assert_eq!(h.ledger().descriptors().count(), 0, "and the ledger lists nothing");
}

/// A job that sends nothing ends when its body returns — no frame needed,
/// because `in_flight` prunes before it answers.
#[test]
fn a_job_that_sends_nothing_ends_with_its_body() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));
    h.spawn(job(silent, 0));
    let before = h.in_flight();
    h.run_pending_jobs();
    let after = h.in_flight();
    println!("silent job: in flight {before} before its body ran, {after} after (want 1, 0)");
    assert_eq!((before, after), (1, 0));
}

/// A cancelled result is consumed as stale, and its job ends with it.
#[test]
fn a_cancelled_result_still_ends_its_job() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));
    h.spawn(job(echo, 3));
    h.run_pending_jobs();
    h.inbox.cancel();
    h.pump();
    let n = h.in_flight();
    println!("cancelled: landed {:?}, stale {}, in flight {n} (want 0)", h.app.landed, h.inbox.stale());
    assert_eq!(h.inbox.stale(), 1, "setup: the result was dropped as stale");
    assert_eq!(n, 0, "a stale result is as landed as it will ever be");
}

/// **The shipping pool, against the same definition**, and the ledger
/// listing what it should while work runs.
#[test]
fn threads_jobs_leave_the_ledger_when_they_land() {
    let mut h = host();
    h.set_spawner(Box::new(Threads::new()));
    for p in [60, 1, 2] {
        h.spawn(job(slow, p));
    }
    let listed: Vec<u64> = h.ledger().descriptors().map(|d| d.payload).collect();
    let at_spawn = h.in_flight();

    let deadline = Instant::now() + Duration::from_secs(5);
    while h.in_flight() > 0 && Instant::now() < deadline {
        h.pump();
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut landed = h.app.landed.clone();
    landed.sort();
    println!(
        "Threads: listed {listed:?} at spawn ({at_spawn} in flight), landed {landed:?}, \
         in flight {} at the end (want 0)",
        h.in_flight()
    );
    assert_eq!(at_spawn, 3, "three spawned, three in flight");
    assert_eq!(listed, vec![60, 1, 2], "listed oldest first, as spawned");
    assert_eq!(landed, vec![1, 2, 60], "setup: every job landed");
    assert_eq!(h.in_flight(), 0, "and none is left in flight");
}

/// **A busy worker does not hold a second job back** — the risk §5 of
/// `docs/plan-timers-wake.md` names for a pool. A 400 ms job is started,
/// then a 1 ms one; the second must land long before the first, which a
/// pool of one worker cannot do.
#[test]
fn a_slow_job_does_not_hold_a_quick_one_back() {
    let mut h = host();
    let pool = Threads::new();
    let cap = pool.cap();
    h.set_spawner(Box::new(pool));
    let started = Instant::now();
    h.spawn(job(slow, 400));
    std::thread::sleep(Duration::from_millis(20)); // the slow one is running
    h.spawn(job(slow, 1));

    let deadline = Instant::now() + Duration::from_secs(5);
    while !h.app.landed.contains(&1) && Instant::now() < deadline {
        h.pump();
        std::thread::sleep(Duration::from_millis(1));
    }
    let quick = started.elapsed();
    const BOUND: Duration = Duration::from_millis(300);
    println!(
        "quick job landed after {:.0} ms beside a 400 ms one (bound {} ms, pool cap {cap}); \
         landed {:?}",
        quick.as_secs_f64() * 1e3,
        BOUND.as_millis(),
        h.app.landed
    );
    assert_eq!(h.app.landed, vec![1], "the quick job landed first, and alone");
    assert!(quick < BOUND, "a 400 ms job held a 1 ms one back for {quick:?}");
}
