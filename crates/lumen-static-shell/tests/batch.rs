//! **M28/T28.5: one task, and a batch.**
//!
//! iced's `Task::batch([a, b])` is several descriptors pushed together.
//! There is no batch *type* here — a batch is what it looks like from
//! the outbox, which is the whole of `docs/plan-tasks.md` §1's "what
//! changes: every one of those is described by values rather than by
//! closures".
//!
//! # What a batch does and does not promise
//!
//! It promises that **all of them run**. It promises nothing about the
//! order their results arrive in, and this file says so in the only way
//! that is honest: the deterministic assertion runs under the *queued*
//! spawner, and the same three run again under *threads* accepting any
//! permutation while still asserting all three arrived.
//!
//! Asserting an order under `Threads` would be asserting the scheduler.

use std::time::Duration;

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::spawn::{Descriptor, JobFn, Outbox, Queued, Threads};
use lumen_static::Rebuild;
use lumen_static_shell::Host;

const KIND: u64 = 1;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Default)]
struct Work {
    ui: Ui,
    outbox: Outbox,
    landed: Vec<u64>,
}

fn echo(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
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
        self.landed.push(m);
        true
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        (d.kind == KIND).then_some(echo as JobFn<u64>)
    }
}

fn host() -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.pump();
    h
}

#[test]
fn one_tasks_result_reaches_the_app() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));

    h.app.outbox.push(Descriptor::new(KIND, 11));
    h.pump(); // drains the outbox -> one job pending
    assert_eq!(h.run_pending_jobs(), 1, "setup: exactly one job was spawned");
    h.pump();

    assert_eq!(h.app.landed, vec![11], "the result reached the app");
}

#[test]
fn three_spawned_together_all_reach_the_app() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));

    // A batch: several descriptors pushed together, in one handler's
    // worth of outbox.
    for p in [1u64, 2, 3] {
        assert!(h.app.outbox.push(Descriptor::new(KIND, p)));
    }
    assert_eq!(h.app.outbox.len(), 3, "setup: three queued before any frame");

    h.pump();
    let ran = h.run_pending_jobs();
    assert_eq!(ran, 3, "all three were spawned, not just the first");
    h.pump();

    assert_eq!(h.app.landed, vec![1, 2, 3], "all three arrived; queued order is spawn order");
}

/// The same three under the **thread** spawner: all of them arrive, in
/// no promised order.
///
/// This is the honest half. A batch is concurrent, so asserting an order
/// here would be asserting the scheduler — and it would pass on this
/// machine and fail on a loaded one, which is worse than not asserting
/// it at all.
#[test]
fn a_batch_promises_arrival_not_order() {
    let mut h = host();
    h.set_spawner(Box::new(Threads));

    for p in [1u64, 2, 3] {
        assert!(h.app.outbox.push(Descriptor::new(KIND, p)));
    }

    const BOUND: usize = 500;
    let mut pumps = 0;
    while h.app.landed.len() < 3 && pumps < BOUND {
        h.pump();
        pumps += 1;
        if h.app.landed.len() < 3 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    let mut got = h.app.landed.clone();
    got.sort_unstable();
    println!("threads batch: arrived {:?} after {pumps} pumps (bound {BOUND})", h.app.landed);
    assert_eq!(got, vec![1, 2, 3], "all three arrived, in whatever order the scheduler chose");
}
