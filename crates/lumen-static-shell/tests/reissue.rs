//! **M49/T49.10 (R4-F3): what a hot swap hands the new build, and what
//! the new build does with it.**
//!
//! `Host::reissuable` is read by the old build's session as it saves;
//! `Host::reissue` by the new build's boot after the restore
//! (`lumen_static_reload::hot`). The live half — a chain in flight at a
//! real dylib swap completing under the new build's step function — is
//! `just window-reload-demo`'s `chain_across_a_swap`. These are the
//! selection rules that gate cannot reach: its ledger holds one job.
//!
//! `Queued` holds a job's body apart from its drain, so each state is a
//! line and nothing sleeps.

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, JobFn, Queued, Subs};
use lumen_static::Rebuild;
use lumen_static_shell::{Host, Reissued};
use lumen_static_widgets::prelude::*;

const ECHO: u64 = 1;
const SILENT: u64 = 2;
const SUB: u64 = 3;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Default)]
struct Work {
    ui: Ui,
    subs: Subs,
    landed: Vec<u64>,
}

fn echo(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(d.payload);
}

fn silent(_d: Descriptor, _tx: &lumen_static::inbox::Sender<u64>) {}

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
    fn subscriptions(&mut self) -> Option<&mut Subs> {
        Some(&mut self.subs)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        match (d.kind, d.step) {
            // A subscription's job answers too, so once its body has run
            // it stays listed until a pump drains it.
            (ECHO, 0) | (SUB, 0) => Some(echo),
            (SILENT, 0) => Some(silent),
            _ => None,
        }
    }
}

fn host() -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.set_spawner(Box::new(Queued::<u64>::default()));
    h.pump();
    h
}

fn spawn(h: &mut Host<Work, Rebuild>, kind: u64, payload: u64) {
    let d = Descriptor::new(kind, payload);
    let job = Work::job_for(d).expect("the table has it");
    assert!(h.spawn(lumen_static::spawn::Job { desc: d, job, on_fail: None }), "setup: the spawn was refused");
}

/// **In flight is the ledger's definition, less subscriptions**, as of the
/// save — not as of the last prune.
///
/// | job | state at the save | re-issued |
/// |---|---|---|
/// | echo 10 | body ran, result applied by a pump | no |
/// | echo 11 | body ran, result still in the inbox | **yes** — the inbox goes with the old session |
/// | echo 12 | not started | **yes** |
/// | silent 13 | body returned since the last pump, sent nothing | no — settled, though still listed |
/// | the subscription `(SUB, 7)` | running | no — the new build starts what its application declares |
#[test]
fn a_swap_carries_what_is_in_flight_and_nothing_that_landed() {
    let mut h = host();
    spawn(&mut h, ECHO, 10);
    h.run_pending_jobs();
    h.pump();
    assert_eq!(h.app.landed, vec![10], "setup: echo 10 did not land");

    h.app.subs.declare(SUB, 7);
    h.pump(); // starts the subscription's job, which `Queued` holds
    spawn(&mut h, ECHO, 11);
    spawn(&mut h, SILENT, 13);
    // 7's and 11's results wait in the inbox; 13 settles, unpruned.
    h.run_pending_jobs();
    spawn(&mut h, ECHO, 12);

    let listed: Vec<u64> = h.ledger().descriptors().map(|d| d.payload).collect();
    let carried: Vec<u64> = h.reissuable().iter().map(|d| d.payload).collect();
    println!("listed (as of the last prune) {listed:?}");
    println!("re-issued at the save         {carried:?} (want [11, 12])");
    assert!(listed.contains(&13), "setup: the silent job was pruned before the save, so this cannot test it");
    assert!(listed.contains(&7), "setup: the subscription's job is not listed, so this cannot test it");
    assert!(!carried.contains(&13), "a job that settled since the last prune was re-issued: {carried:?}");
    assert!(!carried.contains(&7), "a running subscription's job was re-issued: {carried:?}");
    assert_eq!(carried, vec![11, 12], "the work in flight, oldest first");

    // Accepting direction: once everything has landed, nothing is carried.
    h.app.subs.clear();
    h.pump();
    h.run_pending_jobs();
    h.pump();
    h.pump();
    println!("after everything landed: re-issued {:?}, in flight {}", h.reissuable(), h.in_flight());
    assert!(h.reissuable().is_empty(), "a settled host still carries {:?}", h.reissuable());
}

/// **The new build spawns what it has a job for, once.**
///
/// The new host already has a boot job in flight, `(ECHO, 0, 5)`. The old
/// build carried that job, a second request for the same thing, another
/// job, and one this build's table no longer has.
#[test]
fn the_new_build_reissues_through_its_own_table_once() {
    let mut h = host();
    spawn(&mut h, ECHO, 5); // the new build's own boot job
    let carried = [
        Descriptor::new(ECHO, 5),
        Descriptor::new(ECHO, 5),
        Descriptor::new(ECHO, 6),
        Descriptor::new(99, 1),
    ];
    let r = h.reissue(&carried);
    let in_flight = h.in_flight();
    println!("reissue {r:?}; in flight {in_flight} (want 3); unknown_kind {}", h.unknown_kind());
    assert_eq!(r, Reissued { spawned: 2, already: 1, unknown: 1, refused: 0 });
    assert_eq!(in_flight, 3, "the boot job and the two spawned");
    assert_eq!(h.unknown_kind(), 1, "a descriptor this build has no job for is counted where drain_outbox counts it");

    h.run_pending_jobs();
    h.pump();
    let mut landed = h.app.landed.clone();
    landed.sort_unstable();
    println!("landed {landed:?} (want [5, 5, 6])");
    assert_eq!(landed, vec![5, 5, 6], "each job ran once through this build's table");
    assert_eq!(h.in_flight(), 0);
}

/// **A stopped subscription's job is not carried** — M84/T84.7.
///
/// Stopping a subscription takes it off `running`, so before T84.7 its
/// job, still unsettled, was an ordinary job in flight to
/// `Host::reissuable` — and a swap restarted a stream the application had
/// stopped. `Ledger::unsettled` now leaves a stopped job out. The ordinary
/// job beside it is the accept direction: it is still carried.
#[test]
fn a_stopped_subscription_is_not_carried_across_a_swap() {
    let mut h = host();
    h.app.subs.declare(SUB, 8);
    h.pump(); // starts the subscription's job, which `Queued` holds
    spawn(&mut h, ECHO, 14);
    h.app.subs.clear();
    h.pump(); // the diff stops (SUB, 8)
    assert_eq!(h.subscription_counts(), (1, 1), "setup: the subscription was not stopped");
    let listed: Vec<u64> = h.ledger().descriptors().map(|d| d.payload).collect();
    let carried: Vec<u64> = h.reissuable().iter().map(|d| d.payload).collect();
    println!("listed {listed:?}; re-issued at the save {carried:?} (want [14])");
    assert!(listed.contains(&8), "setup: the stopped subscription's job is not listed, so this cannot test it");
    assert!(!carried.contains(&8), "a stopped subscription's job was re-issued: {carried:?}");
    assert_eq!(carried, vec![14], "the ordinary job in flight is still carried");

    // Its body runs late and sends; the drain drops it, counted.
    h.run_pending_jobs();
    h.pump();
    println!("landed {:?}; dropped as stopped {}", h.app.landed, h.stopped_dropped());
    assert_eq!(h.app.landed, vec![14], "the stopped job's late result was applied");
    assert_eq!(h.stopped_dropped(), 1);
}
