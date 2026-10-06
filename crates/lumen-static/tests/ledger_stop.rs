//! **`Ledger::unsettled` leaves a stopped job out** — M84/T84.7, on a
//! ledger directly since M88/T88.1.
//!
//! A stopped job counts as finished once what it sent and its progress
//! slot are drained (`JobTrack::settled`, M88/T88.1), so a host's pump —
//! whose drain follows the diff that stops — takes it off the ledger in
//! the same frame, and `tests/reissue.rs` in the shell can no longer
//! stage a stopped job still listed. What remains is a result sent
//! between a drain and the prune after it: listed, stopped, unsettled.
//! A save reads `unsettled` then, and must not carry the job to the new
//! build. This builds that state by hand.

use lumen_static::inbox::Inbox;
use lumen_static::spawn::{Descriptor, Ledger};

#[test]
fn a_stopped_job_with_an_undrained_result_is_listed_but_not_unsettled() {
    let inbox: Inbox<u64> = Inbox::new();
    let mut ledger: Ledger<u64> = Ledger::new();
    let (stopped_d, other_d) = (Descriptor::new(3, 8), Descriptor::new(1, 14));
    let (tx, track) = inbox.tracked_sender();
    ledger.record(stopped_d, track);
    let (tx2, track2) = inbox.tracked_sender();
    ledger.record(other_d, track2);
    tx.send(8).expect("setup: queued");
    tx2.send(14).expect("setup: queued");
    drop((tx, tx2)); // both bodies returned; both results wait

    let told = ledger.stop(stopped_d);
    let listed: Vec<u64> = ledger.descriptors().map(|d| d.payload).collect();
    let carried: Vec<u64> = ledger.unsettled().map(|d| d.payload).collect();
    println!("told {told}; listed {listed:?} (want [8, 14]); unsettled {carried:?} (want [14])");
    assert_eq!(told, 1, "setup: the job was told to stop");
    assert_eq!(listed, vec![8, 14], "setup: the stopped job is listed, its result undrained");
    assert_eq!(carried, vec![14], "a stopped job was offered for re-issue: {carried:?}");

    // Accept direction, and the drain's half: everything drains, the
    // stopped job's result is dropped, and both settle.
    let mut landed = Vec::new();
    inbox.drain(&mut landed, |l, m| l.push(m));
    let pruned = ledger.prune();
    println!("drained: landed {landed:?}, stopped_dropped {}, pruned {pruned}", inbox.stopped_dropped());
    assert_eq!(landed, vec![14]);
    assert_eq!(inbox.stopped_dropped(), 1);
    assert_eq!(pruned, 2, "both jobs settled once drained");
}
