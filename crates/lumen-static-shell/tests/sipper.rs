//! **M88/T88.0: a streaming job loses what it streams** —
//! `docs/plan-platform-requests.md`.
//!
//! A pin, green on today's code: it asserts the gap rather than the
//! fix. One job under `Immediate` sends 10,000 `u64`s through
//! `Sender::send` into a one-node host, which is then pumped until the
//! ledger is empty. Today the bounded inbox (`Inbox::CAPACITY`) refuses
//! everything past its 256th message, the drain applies at most
//! `Inbox::PER_FRAME` a pump, and the last value — the one a progress
//! bar most needs — is among the refused. T88.1 adds
//! `Sender::progress` and flips this file.
//!
//! Printed: refused sends, the largest number applied in one pump, and
//! whether 9,999 was applied. The accept direction runs the same harness
//! at 200 sends and sees every one delivered, so the harness can see a
//! delivery when there is one.

use lumen_static::event::App;
use lumen_static::inbox::Inbox;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, Immediate, Job};
use lumen_static::Rebuild;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Default)]
struct Stream {
    ui: Ui,
    landed: Vec<u64>,
}

impl App for Stream {
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

/// Sends `0..payload` through `send`, ignoring refusals — which is what
/// a streaming body has to do today, since the alternative is blocking.
fn stream(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    for i in 0..d.payload {
        let _ = tx.send(i);
    }
}

/// What one run measured.
struct Run {
    refused: usize,
    per_pump_max: usize,
    last_applied: usize,
    applied: usize,
}

fn run(n: u64) -> Run {
    let mut h: Host<Stream, Rebuild> = Host::new(Stream::default(), 100, 50);
    h.pump();
    h.set_spawner(Box::new(Immediate));
    let refused0 = h.inbox.refused();
    assert!(h.spawn(Job { desc: Descriptor { kind: 1, step: 0, payload: n }, job: stream, on_fail: None }));
    // Setup landed: the job is listed — its body has returned under
    // `Immediate`, but what it sent is still waiting to be drained.
    let listed = h.in_flight();
    println!("  setup: in_flight before the first pump = {listed} (want 1)");
    assert_eq!(listed, 1, "setup: the job must be in flight before the pump");

    let mut per_pump_max = 0;
    let mut pumps = 0;
    while h.in_flight() > 0 {
        let before = h.app.landed.len();
        h.pump();
        per_pump_max = per_pump_max.max(h.app.landed.len() - before);
        pumps += 1;
        assert!(pumps < 10_000, "the ledger never emptied");
    }
    let last = n - 1;
    Run {
        refused: h.inbox.refused() - refused0,
        per_pump_max,
        last_applied: h.app.landed.iter().filter(|&&v| v == last).count(),
        applied: h.app.landed.len(),
    }
}

/// **The gap, pinned.** T88.1 flips every assertion here: refused 0, at
/// most one applied per pump, and 9,999 applied exactly once.
#[test]
fn a_streaming_job_loses_most_of_its_values_and_its_last() {
    let r = run(10_000);
    println!(
        "sipper (T88.0 pin): 10000 sends; refused {} (pinned >= {}), applied {}, \
         per-pump max {} (pinned = PER_FRAME {}), value 9999 applied {} time(s) (pinned 0: lost)",
        r.refused,
        10_000usize.saturating_sub(Inbox::<u64>::CAPACITY),
        r.applied,
        r.per_pump_max,
        Inbox::<u64>::PER_FRAME,
        r.last_applied
    );
    assert_eq!(
        r.last_applied, 0,
        "pinned (T88.0): the final value is lost today -- it was applied {} time(s)",
        r.last_applied
    );
    assert!(
        r.refused >= 10_000usize.saturating_sub(Inbox::<u64>::CAPACITY),
        "pinned (T88.0): refused {} < {}",
        r.refused,
        10_000usize.saturating_sub(Inbox::<u64>::CAPACITY)
    );
    assert_eq!(
        r.per_pump_max,
        Inbox::<u64>::PER_FRAME,
        "pinned (T88.0): a pump applies up to PER_FRAME"
    );
}

/// **Accept direction**: the same harness at 200 sends — under the
/// inbox's capacity — refuses nothing and delivers the final value.
#[test]
fn a_short_stream_is_delivered_whole() {
    let r = run(200);
    println!(
        "sipper accept: 200 sends; refused {} (bound 0), applied {}, value 199 applied {} time(s) (want 1)",
        r.refused, r.applied, r.last_applied
    );
    assert_eq!(r.refused, 0, "a stream under capacity must not be refused");
    assert_eq!(r.last_applied, 1, "the final value must be delivered");
    assert_eq!(r.applied, 200);
}
