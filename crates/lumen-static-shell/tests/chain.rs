//! **M28/T28.6: chaining, as a step index.**
//!
//! iced chains with `task.then(|out| next)` — a closure held across an
//! await. Ported literally that puts a code pointer in flight and breaks
//! `docs/events-decision.md`'s rule 3 on the first reload: `fresh()`
//! rebuilds the tree but cannot reach into a pending future and swap the
//! pointer it captured, so the continuation would run **last week's
//! code**.
//!
//! The way out is the move this architecture already made once. Handlers
//! stopped needing captures because identity became *positional*; a
//! chain stops needing a closure the same way — **the next step is an
//! index, not a function**. A completion names a step, and the currently
//! loaded build decides what that step means.
//!
//! What crosses the boundary here is `(kind, step, payload)`: three
//! `u64`s. Nothing else.

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::spawn::{Descriptor, JobFn, Outbox, Queued};
use lumen_static::Rebuild;
use lumen_static_shell::Host;

const KIND: u64 = 3;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Default)]
struct Work {
    ui: Ui,
    outbox: Outbox,
    /// Every (step, payload) the app saw, in arrival order.
    seen: Vec<(u64, u64)>,
}

/// The three steps. Each sends its *own* step index in the high bits and
/// its output in the low ones, so the app can tell which step produced
/// what — and so a chain that ran the wrong step is visible rather than
/// merely wrong in a number.
fn pack(step: u64, out: u64) -> u64 {
    (step << 32) | out
}
fn unpack(m: u64) -> (u64, u64) {
    (m >> 32, m & 0xffff_ffff)
}

fn step0(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(pack(0, d.payload + 1));
}
fn step1(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(pack(1, d.payload * 10));
}
fn step2(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(pack(2, d.payload + 5));
}

impl App for Work {
    // M32 prototype: this app keeps handlers on the whole of itself.
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

    /// **The step function, in the currently loaded build.** A
    /// completion says which step it was and what it produced; this
    /// decides what happens next, and what happens next is data: finish,
    /// or another descriptor.
    fn apply(&mut self, m: u64) -> bool {
        let (step, out) = unpack(m);
        self.seen.push((step, out));
        match step {
            0 => {
                self.outbox.push(Descriptor { kind: KIND, step: 1, payload: out });
            }
            1 => {
                self.outbox.push(Descriptor { kind: KIND, step: 2, payload: out });
            }
            // Step 2 finishes. "Finish" is the absence of a push, which
            // is the same shape as a handler that does nothing.
            _ => {}
        }
        true
    }

    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }

    /// Dispatch on `(kind, step)` — the step **is** the chain.
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        match (d.kind, d.step) {
            (KIND, 0) => Some(step0),
            (KIND, 1) => Some(step1),
            (KIND, 2) => Some(step2),
            _ => None,
        }
    }
}

fn host() -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.pump();
    h
}

#[test]
fn a_three_step_chain_runs_in_order_each_seeing_the_last_step_s_payload() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));

    h.app.outbox.push(Descriptor::new(KIND, 4));

    // Deterministic: pump, run whatever was spawned, repeat. No sleep,
    // because `Queued` is the spawner a chain assertion needs — under
    // `Threads` the step boundaries would be a race.
    for _ in 0..6 {
        h.pump();
        h.run_pending_jobs();
    }

    println!("chain saw {:?}", h.app.seen);
    assert_eq!(
        h.app.seen,
        vec![(0, 5), (1, 50), (2, 55)],
        "the chain must run 0 -> 1 -> 2, each step seeing the previous step's payload: \
         4+1=5, 5*10=50, 50+5=55"
    );
}

/// An unrecognised `(kind, step)` is dropped and **counted**, not
/// misinterpreted — §2.1's stability rule, asserted.
///
/// This is the half that makes ids stable rather than merely claimed to
/// be. A renumbering that silently ran the wrong step would produce a
/// plausible-looking result, which is the worst kind.
#[test]
fn an_unrecognised_step_is_dropped_and_counted() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));

    // Step 9 of a kind that exists: the kind is fine, the step is not.
    // Keyed on the pair, so this must be refused — keyed on the kind
    // alone it would have run step 0's body under step 9's name.
    h.app.outbox.push(Descriptor { kind: KIND, step: 9, payload: 1 });
    h.pump();
    h.run_pending_jobs();
    h.pump();

    println!("unknown (kind, step) dropped: {}", h.unknown_kind());
    assert!(h.app.seen.is_empty(), "nothing ran: {:?}", h.app.seen);
    assert_eq!(h.unknown_kind(), 1, "and the drop was counted, not silent");
}
