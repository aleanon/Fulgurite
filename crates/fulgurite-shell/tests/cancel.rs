//! **M28/T28.8: cancellation by epoch.**
//!
//! `docs/plan-tasks.md` §2.3. Every sender is minted at an epoch; a
//! result carrying any other epoch is dropped at the drain and counted.
//!
//! # What "cancel" honestly means
//!
//! The result is dropped; the work is not stopped. A thread already
//! running runs to completion and its output is discarded. Stopping work
//! needs cooperation from the job body, which this milestone does not
//! design — and saying so is better than a `cancel` that implies more
//! than it does.
//!
//! # A reload does not bump the epoch
//!
//! The plan's first draft had it do so, and that made T28.7's chain
//! **unpassable by construction**: the pre-swap result would be
//! discarded before the new build's step could interpret it. It also
//! voided §2.1's own argument, which is precisely that a result is data
//! and therefore safe for the new build to read. So the epoch is
//! explicit cancellation, driven by the application, and nothing else
//! touches it — asserted below.

use fulgurite::event::App;
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;
use fulgurite::spawn::{Descriptor, JobFn, Immediate, Outbox, Queued};
use fulgurite::Rebuild;
use fulgurite_shell::Host;

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

fn echo(d: Descriptor, tx: &fulgurite::inbox::Sender<u64>) {
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
        this.landed.push(m);
        true
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        (d.kind == KIND && d.step == 0).then_some(echo as JobFn<u64>)
    }
}

fn host() -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.pump();
    h
}

/// The cancelled direction: a result already in the queue never reaches
/// the app.
#[test]
fn a_cancelled_tasks_result_never_reaches_the_app() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));

    h.app.outbox.push(Descriptor::new(KIND, 7));
    h.pump();
    // The job runs and its result is now sitting in the queue, carrying
    // the epoch it was spawned at.
    assert_eq!(h.run_pending_jobs(), 1, "setup: the job ran and sent");

    let e = h.inbox.cancel();
    println!("cancelled: epoch now {e}");
    h.pump();

    assert!(h.app.landed.is_empty(), "a cancelled result reached the app: {:?}", h.app.landed);
    assert_eq!(h.inbox.stale(), 1, "and the drop was counted, not silent");
}

/// The accepting direction, which is what keeps this from being a
/// cancel that drops everything.
#[test]
fn an_uncancelled_result_still_arrives() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));

    h.app.outbox.push(Descriptor::new(KIND, 7));
    h.pump();
    h.run_pending_jobs();
    h.pump();

    assert_eq!(h.app.landed, vec![7], "nothing was cancelled, so it arrived");
    assert_eq!(h.inbox.stale(), 0, "and nothing was dropped");
}

/// Cancel is not permanent: work spawned **after** it carries the new
/// epoch and arrives.
///
/// Without this the type could pass the two tests above by dropping
/// everything after the first cancel, which is the "gate that refuses
/// everything" shape.
#[test]
fn work_spawned_after_a_cancel_arrives() {
    let mut h = host();
    h.set_spawner(Box::new(Immediate));

    h.app.outbox.push(Descriptor::new(KIND, 1));
    h.pump();
    assert_eq!(h.app.landed, vec![1], "setup: the first result arrived");

    h.inbox.cancel();
    h.app.outbox.push(Descriptor::new(KIND, 2));
    h.pump();

    assert_eq!(h.app.landed, vec![1, 2], "work spawned after the cancel must still arrive");
    assert_eq!(h.inbox.stale(), 0, "nothing was in flight when the cancel happened");
}

/// **A sender minted before the cancel cannot un-cancel itself.**
///
/// The epoch is fixed at mint time rather than read at send time. Read
/// at send time, a long-running job would quietly rejoin the current
/// epoch and deliver a result the application asked to be rid of —
/// which is the failure the whole mechanism is for.
#[test]
fn a_sender_minted_before_the_cancel_stays_cancelled() {
    let mut h = host();
    let old = h.sender();

    h.inbox.cancel();
    let fresh = h.sender();

    old.send(10).expect("capacity");
    fresh.send(20).expect("capacity");
    h.pump();

    println!("after cancel: landed {:?}, stale {}", h.app.landed, h.inbox.stale());
    assert_eq!(h.app.landed, vec![20], "only the sender minted after the cancel delivers");
    assert_eq!(h.inbox.stale(), 1, "the old sender's result was dropped and counted");
}

/// **A reload does not cancel.** T28.7's assertion, seen from this side.
///
/// `fresh()` replaces the application object with one built by the new
/// code. If that bumped the epoch, every result in flight would be
/// discarded before the new build could interpret it — which is exactly
/// what the plan's first draft specified, and what made T28.7's chain
/// unpassable by construction.
///
/// Modelled here by replacing `host.app` wholesale, which is what a
/// reload does to it, and asserting the in-flight result still lands.
/// The end-to-end version is `just hot-demo`, across a real dylib swap.
#[test]
fn replacing_the_app_does_not_cancel_work_in_flight() {
    let mut h = host();
    let tx = h.sender();
    let before = h.inbox.epoch();

    tx.send(99).expect("capacity");

    // The reload's shape: a brand-new application object, built by
    // "the new code", grafted in under the same host.
    h.app = Work::default();

    assert_eq!(h.inbox.epoch(), before, "a reload must not touch the epoch");
    h.pump();
    assert_eq!(
        h.app.landed,
        vec![99],
        "a result in flight across a reload must be applied by the new build — \
         discarding it is what the plan's first draft specified, and it made the \
         cross-swap chain unpassable"
    );
    assert_eq!(h.inbox.stale(), 0);
}
