//! **M28/T28.3: the spawner seam, and all three implementations.**
//!
//! `docs/plan-agent-parity.md`'s appendix settled where a spawner lives
//! — `dyn` at the shell — and this asserts what the three that ship
//! actually promise, which differ in exactly one dimension: *when*.
//!
//! | spawner | the claim asserted here |
//! |---|---|
//! | `Immediate` | the result is applied by the pump that follows the spawn |
//! | `Queued` | nothing runs until the test says so, then the next pump applies it — with **no sleep** |
//! | `Threads` | the same job arrives within a bounded number of pumps, the bound printed beside the count |
//!
//! `Queued` exists because of the middle row. Under `Threads` "not the
//! frame that spawned it" is a race, and under `Immediate` it is false
//! by construction — so neither can assert it. A review of this plan's
//! first draft found T28.5 asking for that property with neither
//! spawner able to give it.

use std::time::{Duration, Instant};

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, Immediate, Job, Queued, Threads};
use lumen_static::Rebuild;
use lumen_static_shell::Host;

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
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(&mut self, m: u64) -> bool {
        self.landed.push(m);
        true
    }
}

/// The job body. **Non-capturing by type** — this is an `fn` item, and
/// the field it is assigned to is an `fn` pointer, so a closure that
/// captured anything would be refused at the call site rather than by a
/// comment.
fn double(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(d.payload * 2);
}

fn host() -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.pump();
    h
}

const JOB: Job<u64> = Job { desc: Descriptor { kind: 1, step: 0, payload: 21 }, job: double, on_fail: None };

#[test]
fn immediate_lands_on_the_pump_that_follows_the_spawn() {
    let mut h = host();
    h.set_spawner(Box::new(Immediate));

    assert!(h.spawn(JOB), "the spawner took it");
    assert!(h.app.landed.is_empty(), "running is not applying — the frame thread applies");

    h.pump();
    assert_eq!(h.app.landed, vec![42], "the very next pump applied it");
}

#[test]
fn queued_runs_nothing_until_asked_and_lands_on_the_next_pump() {
    let mut h = host();
    let q: Queued<u64> = Queued::default();
    assert_eq!(q.pending(), 0, "setup: nothing queued yet");
    h.set_spawner(Box::new(q));

    assert!(h.spawn(JOB));
    // Several frames pass. Nothing runs, and no sleep was needed to
    // establish it — which is the whole reason this spawner exists.
    for _ in 0..3 {
        h.pump();
    }
    assert!(
        h.app.landed.is_empty(),
        "a queued job ran without being asked: it landed {:?} across three frames",
        h.app.landed
    );

    // Now run it. `Queued` is inside the box, so the test reaches it the
    // way a shell would: it kept no handle, and asks the host.
    let ran = h.run_pending_jobs();
    assert_eq!(ran, 1, "exactly the one job ran");
    assert!(h.app.landed.is_empty(), "running still is not applying");

    h.pump();
    assert_eq!(h.app.landed, vec![42], "and the next pump applied it");
}

#[test]
fn threads_arrives_within_a_bounded_number_of_pumps() {
    let mut h = host();
    h.set_spawner(Box::new(Threads));
    assert!(h.spawn(JOB));

    // **A timeout, not a latency budget**, and the difference matters
    // for reading the output: this answers "does a threaded result
    // arrive at all", and it measures 2 pumps against a bound of 200.
    // That 100x gap would be the "bound computed from the wrong
    // quantity" smell if the bound were claiming anything about typical
    // latency — it is not, and tightening it would only make the test
    // flaky on a loaded machine. The count prints beside the bound so
    // the gap is visible rather than inferred, which is the part of the
    // rule that applies here.
    const BOUND: usize = 200;
    let started = Instant::now();
    let mut pumps = 0;
    while h.app.landed.is_empty() && pumps < BOUND {
        h.pump();
        pumps += 1;
        if h.app.landed.is_empty() {
            // A real thread needs wall clock; this is the one place a
            // sleep is honest, because the claim IS about a thread.
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    println!(
        "threads: result arrived after {pumps} pumps ({:.0} ms), bound {BOUND}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(h.app.landed, vec![42], "the job's result arrived within {BOUND} pumps");
}

/// A spawn with no spawner is **refused and counted**, not queued.
///
/// Queueing would mean work that runs whenever a shell happens to
/// install one, and a host that never does would accumulate jobs
/// forever. The accepting direction is every test above.
#[test]
fn a_spawn_with_no_spawner_is_refused_and_counted() {
    let mut h = host();
    assert_eq!(h.unspawned(), 0, "setup: nothing refused yet");
    assert!(!h.spawn(JOB), "a host with no spawner must say no");
    assert!(!h.spawn(JOB));
    h.pump();
    assert_eq!(h.unspawned(), 2, "both refusals counted");
    assert!(h.app.landed.is_empty(), "and nothing ran");
}
