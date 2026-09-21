//! **M28/T28.9: what happens when a job fails.**
//!
//! The plan had no failure story and the review was right that one is
//! needed *before* anything is built on top. Under the thread default a
//! panicking job unwinds, its sender drops, and **a chain waiting on
//! that completion stalls forever with nothing observing it** — no
//! crash, no log line anyone correlates, just work that never finishes.
//!
//! A job that panics, and a spawn the OS refuses, both produce a failure
//! result the application applies like any other. Data, not a callback:
//! a callback would be a code reference, which is what this whole
//! milestone is arranged to avoid.

use std::time::{Duration, Instant};

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::spawn::{Descriptor, Failure, Immediate, JobFn, Job, Outbox, Threads};
use lumen_static::Rebuild;
use lumen_static_shell::Host;

const KIND: u64 = 1;
/// Step 0 panics; step 1 is the continuation that would stall.
const STEP_BOOM: u64 = 0;
const STEP_NEXT: u64 = 1;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Msg {
    Ok(u64),
    Failed(u64, Failure),
}

#[derive(Default)]
struct Work {
    ui: Ui,
    outbox: Outbox,
    seen: Vec<Msg>,
}

fn boom(_d: Descriptor, _tx: &lumen_static::inbox::Sender<Msg>) {
    panic!("the job body panicked on purpose (M28/T28.9)");
}

fn fine(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::Ok(d.payload));
}

impl App for Work {
    type Tree = Ui;
    type Msg = Msg;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    fn apply(&mut self, m: Msg) -> bool {
        self.seen.push(m);
        // **The chain continues past a failure**, which is the point: a
        // failure is a result, so the step function has something to
        // decide about. Here it gives up and reports rather than
        // retrying, which is a policy the application owns.
        if let Msg::Ok(v) = m {
            if v == 1 {
                self.outbox.push(Descriptor { kind: KIND, step: STEP_NEXT, payload: v });
            }
        }
        true
    }

    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }

    fn job_for(d: Descriptor) -> Option<JobFn<Msg>> {
        match (d.kind, d.step) {
            (KIND, STEP_BOOM) => Some(boom),
            (KIND, STEP_NEXT) => Some(fine),
            _ => None,
        }
    }

    fn failed(d: Descriptor, why: Failure) -> Option<Msg> {
        Some(Msg::Failed(d.step, why))
    }
}

fn host() -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.pump();
    h
}

/// A panicking job delivers a **failure** rather than killing the
/// process or stalling the chain.
///
/// Under `Threads`, which is the default and the one where the stall is
/// real. The bound is printed beside the count.
#[test]
fn a_panicking_job_delivers_a_failure_and_the_chain_does_not_stall() {
    let mut h = host();
    h.set_spawner(Box::new(Threads));

    // Silence the default hook's backtrace for the deliberate panic —
    // the panic is expected, and its message would read like a real
    // failure in the test log. Restored immediately after, so an
    // *unexpected* panic elsewhere still prints.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    h.app.outbox.push(Descriptor { kind: KIND, step: STEP_BOOM, payload: 1 });

    const BOUND: usize = 500;
    let started = Instant::now();
    let mut pumps = 0;
    while h.app.seen.is_empty() && pumps < BOUND {
        h.pump();
        pumps += 1;
        if h.app.seen.is_empty() {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    std::panic::set_hook(prev);

    println!(
        "panicking job: saw {:?} after {pumps} pumps / {:.0} ms (bound {BOUND} pumps)",
        h.app.seen,
        started.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(
        h.app.seen,
        vec![Msg::Failed(STEP_BOOM, Failure::Panicked)],
        "a panicking job must deliver a failure; an empty `seen` here is the chain \
         stalling forever, which is the defect this task exists to remove"
    );
}

/// The same under `Immediate`, so the deterministic arm agrees with the
/// shipping one about the thing under test.
///
/// A spawner that let the panic through would make a test written
/// against it pass while the real one stalls.
#[test]
fn the_immediate_spawner_reports_the_same_failure() {
    let mut h = host();
    h.set_spawner(Box::new(Immediate));

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    h.app.outbox.push(Descriptor { kind: KIND, step: STEP_BOOM, payload: 1 });
    h.pump();
    h.pump();
    std::panic::set_hook(prev);

    assert_eq!(h.app.seen, vec![Msg::Failed(STEP_BOOM, Failure::Panicked)]);
}

/// A spawn that cannot start does the same.
///
/// Modelled by a host with **no spawner installed**, which is the
/// reachable form of "the job never started": the OS refusing a thread
/// is the other, and both take the same path.
#[test]
fn a_spawn_that_never_started_delivers_a_failure_too() {
    let mut h = host();
    // No `set_spawner`.
    let ok = h.spawn(Job {
        desc: Descriptor { kind: KIND, step: STEP_BOOM, payload: 1 },
        job: boom,
        on_fail: Some(Work::failed),
    });
    assert!(!ok, "setup: the spawn was refused");
    h.pump();

    println!("refused spawn: saw {:?}, unspawned {}", h.app.seen, h.unspawned());
    assert_eq!(
        h.app.seen,
        vec![Msg::Failed(STEP_BOOM, Failure::NotStarted)],
        "a refused spawn must be reported, not only counted: a chain whose next step \
         never started stalls exactly as one whose job panicked does"
    );
    assert_eq!(h.unspawned(), 1, "and it is still counted");
}

/// The accepting direction: a job that does not fail produces no
/// failure, and the chain runs on.
#[test]
fn a_job_that_succeeds_reports_no_failure() {
    let mut h = host();
    h.set_spawner(Box::new(Immediate));

    h.app.outbox.push(Descriptor { kind: KIND, step: STEP_NEXT, payload: 9 });
    h.pump();
    h.pump();

    assert_eq!(h.app.seen, vec![Msg::Ok(9)], "no failure where there was none");
}
