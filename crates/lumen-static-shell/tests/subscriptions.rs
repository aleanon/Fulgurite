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
//!
//! **M84/T84.7: a stopped subscription stops** — its job is told through
//! `JobTrack::stop`, returns on `Sender::is_stopped`, and the drain drops
//! what it sent (`Host::stopped_dropped`). T84.5 pinned the gap green;
//! T84.7 flipped it (`docs/plan-images-in-motion.md`).

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::spawn::{Descriptor, Immediate, JobFn, Outbox, Subs, Threads};
use lumen_static::Rebuild;
use lumen_static_shell::Host;
use std::sync::atomic::{AtomicUsize, Ordering};

const KIND: u64 = 5;
/// T84.5/T84.7's subscription: a job that keeps sending until it is
/// told to stop.
const TICKER: u64 = 6;

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
    /// T84.5: the application un-declares its subscriptions once the
    /// first message has landed.
    stop_after_first: bool,
    /// T84.7's re-key: once the first message has landed, the
    /// application declares the ticker under this key instead.
    rekey_after_first: Option<u64>,
}

fn stream(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    // One message per start, carrying the key — so "started twice" is
    // visible in the app's own state as well as in the host's counter.
    let _ = tx.send(d.payload);
}

/// The ticker's iterations, counted on the worker **per key**: each test
/// that starts a `TICKER` uses its own keys, so tests running in parallel
/// do not share a counter.
static TICKS: [AtomicUsize; 8] = [const { AtomicUsize::new(0) }; 8];
/// The most iterations a ticker runs, so a worker nobody stops still
/// ends: 500 ms at 10 ms, and the 50 the accept direction counts.
const TICKER_LIMIT: u64 = 50;

fn ticks(key: u64) -> usize {
    TICKS[key as usize].load(Ordering::SeqCst)
}

fn ticker(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    for n in 0..TICKER_LIMIT {
        // **The cooperative half** (T84.7): asked before each unit of
        // work, so a stop costs at most the iteration already under way.
        if tx.is_stopped() {
            return;
        }
        TICKS[d.payload as usize].fetch_add(1, Ordering::SeqCst);
        if let Err(lumen_static::inbox::SendError::Disconnected(_)) = tx.send(d.payload * 1000 + n) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
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
        if this.stop_after_first && this.landed.len() == 1 {
            this.subs.clear();
        }
        if let (Some(k), 1) = (this.rekey_after_first, this.landed.len()) {
            this.subs.clear();
            this.subs.declare(TICKER, k);
        }
        true
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }
    fn subscriptions(&mut self) -> Option<&mut Subs> {
        Some(&mut self.subs)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        match d.kind {
            KIND => Some(stream),
            TICKER => Some(ticker),
            _ => None,
        }
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

/// A host on real workers, with the ticker declared under `key`, pumped
/// until its first message lands. Whatever the application does in that
/// first `apply` (stop, re-key) has been decided, and the diff that acts
/// on it is the next pump's.
fn ticking(app: Work, key: u64) -> Host<Work, Rebuild> {
    let mut h: Host<Work, Rebuild> = Host::new(app, 100, 50);
    h.set_spawner(Box::new(Threads::new()));
    h.pump();
    h.app.subs.declare(TICKER, key);
    let begun = std::time::Instant::now();
    while h.app.landed.is_empty() {
        assert!(begun.elapsed().as_secs() < 5, "setup: the ticker's first message never landed");
        h.pump();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    h
}

/// **M84/T84.7: a stopped subscription stops.** A ticker sends every
/// 10 ms; the application un-declares it when the first message lands,
/// and the next frame's diff stops it. T84.5 pinned the opposite green
/// (a_stopped_subscription_keeps_delivering_today: 20 messages applied
/// and 20 iterations after the stop); T84.7 flipped it.
///
/// "After the stop" is from the frame whose diff stops it: that frame's
/// drain runs after its diff (`Host::pump_with`), so a message it applies
/// was applied after the stop, and the drain sees the flag the diff just
/// set. **The test waits 35 ms before that frame**, so the ticker has
/// sent into the inbox meanwhile and the drain's drop has something to
/// drop — without the wait the stopping frame finds the queue empty and
/// a drain that ignored the flag would pass. That something was waiting
/// (iterations before the stop exceed what landed) is asserted as setup
/// for that reason, and `stopped_dropped` must account for it.
#[test]
fn a_stopped_subscription_stops() {
    const KEY: u64 = 1;
    let mut h = ticking(Work { stop_after_first: true, ..Work::default() }, KEY);
    assert!(h.app.subs.as_slice().is_empty(), "setup: the application did not un-declare its subscription");
    assert_eq!(h.subscription_counts(), (1, 0), "setup: stopped before the next frame's diff");

    std::thread::sleep(std::time::Duration::from_millis(35));
    // The frame that stops it, and everything from it on.
    let landed_at_stop = h.app.landed.len();
    let ticks_at_stop = ticks(KEY);
    let dropped_at_stop = h.stopped_dropped();
    h.pump();
    assert_eq!(h.subscription_counts(), (1, 1), "setup: the diff did not stop the subscription");
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        h.pump();
    }
    let after = h.app.landed.len() - landed_at_stop;
    let ticks_after = ticks(KEY) - ticks_at_stop;
    let dropped = h.stopped_dropped() - dropped_at_stop;
    // Sent before the stop and not yet applied: what the stopping frame's
    // drain has to drop. One message per iteration, so it is the
    // iterations less what landed, read without the mechanism under test.
    let pending = ticks_at_stop - landed_at_stop;
    println!(
        "first message landed {:?}; {} iterations before the stop; after it, over 200 ms: \
         {after} messages applied (bound 0), {ticks_after} job iterations (bound <= 1), \
         {dropped} dropped at the drain ({pending} were waiting)",
        &h.app.landed[..landed_at_stop],
        ticks_at_stop,
    );
    assert!(
        pending > 0,
        "setup: nothing was waiting in the inbox at the stop, so the drain's drop could not be seen \
         ({ticks_at_stop} iterations, {landed_at_stop} landed)"
    );
    // The job first, so each half of the stop fails under its own name:
    // a host that never sets the flag leaves the job iterating (and its
    // messages applied), a drain that ignores it leaves only the
    // messages.
    assert!(
        ticks_after <= 1,
        "a stopped subscription's job must return within one iteration -- it ran {ticks_after}"
    );
    assert_eq!(
        after, 0,
        "a stopped subscription's messages must not be applied after the stop -- {after} were"
    );
    assert!(
        dropped >= pending,
        "the drain dropped {dropped} of the {pending} messages waiting at the stop"
    );
}

/// **Accept direction** (T84.7): a subscription still declared keeps
/// delivering — every message its job sends is applied, 50 of 50, and
/// nothing is dropped as stopped.
#[test]
fn a_declared_subscription_keeps_delivering() {
    const KEY: u64 = 2;
    let mut h = ticking(Work::default(), KEY);
    let begun = std::time::Instant::now();
    while h.app.landed.len() < TICKER_LIMIT as usize {
        assert!(
            begun.elapsed().as_secs() < 5,
            "a declared subscription stopped delivering at {} of {TICKER_LIMIT}",
            h.app.landed.len()
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
        h.pump();
    }
    println!(
        "declared throughout: {} of {TICKER_LIMIT} messages applied, {} iterations, {} dropped as stopped",
        h.app.landed.len(),
        ticks(KEY),
        h.stopped_dropped()
    );
    assert_eq!(h.app.landed.len(), TICKER_LIMIT as usize);
    assert_eq!(ticks(KEY), TICKER_LIMIT as usize, "the job ran every iteration");
    assert_eq!(h.stopped_dropped(), 0, "nothing of a declared subscription is dropped");
    assert_eq!(h.subscription_counts(), (1, 0), "and it was never stopped");
}

/// **Re-key** (T84.7): the same kind under a new key is a new
/// subscription. After the change only the new key's messages are
/// applied; the old key's job is told to stop like any other.
#[test]
fn a_rekeyed_subscription_delivers_only_the_new_key() {
    const OLD: u64 = 3;
    const NEW: u64 = 4;
    let mut h = ticking(Work { rekey_after_first: Some(NEW), ..Work::default() }, OLD);
    assert_eq!(h.app.subs.as_slice(), &[(TICKER, NEW)], "setup: the application did not re-key");

    std::thread::sleep(std::time::Duration::from_millis(35));
    let landed_at_change = h.app.landed.len();
    let old_ticks_at_change = ticks(OLD);
    h.pump();
    assert_eq!(h.subscription_counts(), (2, 1), "setup: the diff did not stop the old key and start the new");
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        h.pump();
    }
    let after = &h.app.landed[landed_at_change..];
    let old_after = after.iter().filter(|m| **m / 1000 == OLD).count();
    let new_after = after.iter().filter(|m| **m / 1000 == NEW).count();
    let old_ticks_after = ticks(OLD) - old_ticks_at_change;
    println!(
        "after the change {OLD} -> {NEW}: key {OLD} applied {old_after} (bound 0), key {NEW} applied \
         {new_after} (want > 0); key {OLD}'s job iterated {old_ticks_after} more (bound <= 1), \
         {} dropped as stopped",
        h.stopped_dropped()
    );
    assert_eq!(old_after, 0, "the old key's messages were applied after the change");
    assert!(new_after > 0, "the new key delivered nothing after the change");
    assert!(old_ticks_after <= 1, "the old key's job ran on: {old_ticks_after} iterations");
}
