//! **M28/T28.10: subscriptions — start, stop, and identity.**
//!
//! iced hashes a *recipe* to decide which streams keep running. Here
//! identity is `(kind, key)` — values — and the host diffs the
//! application's declared set each frame against what it already
//! started.
//!
//! Hashing code would put a stream's identity in the code that produces
//! it, which is the same coupling `docs/events-decision.md` rule 3
//! removes from chains: two builds that format a URL differently would
//! restart every subscription on reload, and two that format it the same
//! would silently share one.

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::spawn::{Descriptor, Immediate, JobFn, Outbox, Subs};
use lumen_static::Rebuild;
use lumen_static_shell::Host;

const KIND: u64 = 5;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Default)]
struct Work {
    ui: Ui,
    outbox: Outbox,
    subs: Subs,
    landed: Vec<u64>,
}

fn stream(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    // One message per start, carrying the key — so "started twice" is
    // visible in the app's own state as well as in the host's counter.
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
    fn subscriptions(&mut self) -> Option<&mut Subs> {
        Some(&mut self.subs)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        (d.kind == KIND).then_some(stream as JobFn<u64>)
    }
}

fn host() -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.set_spawner(Box::new(Immediate));
    h.pump();
    h
}

#[test]
fn a_subscription_that_appears_starts_once_and_is_not_restarted() {
    let mut h = host();
    h.app.subs.declare(KIND, 1);

    for _ in 0..5 {
        h.pump();
    }

    let (started, stopped) = h.subscription_counts();
    println!("one subscription over five frames: started {started}, stopped {stopped}");
    assert_eq!(started, 1, "a subscription must start once, not once per frame");
    assert_eq!(stopped, 0, "and it was never stopped");
    assert_eq!(h.app.landed, vec![1], "one start, one message");
}

#[test]
fn a_subscription_that_vanishes_stops() {
    let mut h = host();
    h.app.subs.declare(KIND, 1);
    h.pump();
    assert_eq!(h.subscription_counts(), (1, 0), "setup: it started");

    h.app.subs.clear();
    h.pump();

    assert_eq!(h.subscription_counts(), (1, 1), "declaring nothing stops it");
    // And it stays stopped rather than being stopped again every frame.
    h.pump();
    h.pump();
    assert_eq!(h.subscription_counts(), (1, 1), "stopped once, not once per frame");
}

/// **The key is part of the identity.** A subscription whose key changes
/// is a different subscription: stop the old, start the new.
///
/// This is the case the injection fires on, and it is the one that
/// matters in practice — a stream keyed on "which document is open"
/// changes key far more often than it appears or vanishes.
#[test]
fn a_subscription_whose_key_changes_is_stopped_and_restarted() {
    let mut h = host();
    h.app.subs.declare(KIND, 1);
    h.pump();
    assert_eq!(h.subscription_counts(), (1, 0), "setup: key 1 started");

    h.app.subs.clear();
    h.app.subs.declare(KIND, 2);
    h.pump();

    let (started, stopped) = h.subscription_counts();
    println!("after the key changed 1 -> 2: started {started}, stopped {stopped}");
    assert_eq!(
        (started, stopped),
        (2, 1),
        "a changed key must stop the old stream and start a new one; diffing by kind \
         alone leaves the old one running and starts nothing"
    );
    assert_eq!(h.app.landed, vec![1, 2], "and the new key's stream ran");
}

/// Declaring the same `(kind, key)` twice is **one** subscription.
///
/// Without this an application declaring from a loop would start N
/// copies of one stream, and the diff would never settle.
#[test]
fn declaring_the_same_identity_twice_is_one_subscription() {
    let mut h = host();
    h.app.subs.declare(KIND, 7);
    h.app.subs.declare(KIND, 7);
    assert_eq!(h.app.subs.len(), 1, "the set is a set");

    h.pump();
    assert_eq!(h.subscription_counts(), (1, 0));
    assert_eq!(h.app.landed, vec![7]);
}

#[test]
fn the_set_refuses_past_capacity_and_counts() {
    let mut s = Subs::new();
    for k in 0..Subs::CAPACITY as u64 {
        assert!(s.declare(KIND, k), "within capacity");
    }
    let mut refused = 0;
    for k in 100..103u64 {
        if !s.declare(KIND, k) {
            refused += 1;
        }
    }
    println!(
        "capacity {}: declared {}, refused {}, counter {}",
        Subs::CAPACITY,
        s.len(),
        refused,
        s.refused()
    );
    assert_eq!(refused, 3);
    assert_eq!(s.refused(), 3, "counted where a test can read it");
    assert_eq!(s.len(), Subs::CAPACITY, "a refusal must not evict");
}
