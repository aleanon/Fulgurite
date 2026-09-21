//! **M28/T28.2: a result posted between frames is applied by the next
//! pump, in order, exactly once.**
//!
//! On `Boundaries<16>`, because a result that changes state must *mark*
//! it. Under the default `Everything` the tree is walked regardless, so
//! an `apply` that wrote state and marked nothing would look correct —
//! the shape M25/T25.0 and M26/T26.1 both were.
//!
//! # The fixture's applied field carries no `#[bind]`, deliberately
//!
//! `pump` ends with `if self.app.update() { mark_all() }`. A bound field
//! would therefore mark the whole tree whatever `apply` did, and the
//! injection below — *apply without marking* — would stay green. The
//! plan named this trap before the test was written, and it is the
//! "setup silently did not happen" shape: both sides would agree about a
//! frame in which the marking question never arose.
//!
//! So the label here is written by `apply` **directly**, and nothing
//! refreshes it.

use lumen_static::event::App;
use lumen_static::inbox::SendError;
use lumen_static::invalidate::Boundaries;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::{Everything, Rebuild};
use lumen_static_shell::Host;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("out")]
    out: Label<String>,
}

#[derive(Default)]
struct Deferring {
    ui: Ui,
    /// Every message that landed, in order. The assertion reads this
    /// rather than the label, so "applied once, in order" is checked
    /// separately from "the tree saw it".
    log: Vec<u32>,
}

impl App for Deferring {
    type Tree = Ui;
    type Msg = u32;

    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    /// Writes the label **directly**. No `#[bind]` refreshes it — see
    /// the module docs.
    fn apply(&mut self, m: u32) -> bool {
        self.log.push(m);
        self.ui.out.set(format!("got {m}"));
        true
    }
}

type Marked = Host<Deferring, Rebuild, Boundaries<16>>;

fn settled() -> Marked {
    let mut h: Marked = Host::new(Deferring::default(), 200, 100);
    for _ in 0..3 {
        h.pump();
    }
    h
}

#[test]
fn a_result_posted_between_frames_lands_on_the_next_pump() {
    let mut h = settled();
    assert!(h.app.log.is_empty(), "setup: nothing applied before anything was posted");

    h.inbox.post(7);
    assert!(h.app.log.is_empty(), "posting alone must not apply — the frame thread does that");

    h.pump();
    assert_eq!(h.app.log, vec![7], "the next pump applied it");

    // Exactly once: a second pump with nothing posted must not re-apply.
    h.pump();
    assert_eq!(h.app.log, vec![7], "a drained message is gone; draining twice yields nothing");
}

#[test]
fn results_are_applied_in_the_order_they_were_sent() {
    let mut h = settled();
    let tx = h.sender();
    for i in 0..5u32 {
        tx.send(i).expect("capacity");
    }
    h.pump();
    assert_eq!(h.app.log, vec![0, 1, 2, 3, 4], "in order, all of them, once each");
}

/// The `Boundaries` half: applying must **mark**, or the frame keeps the
/// previous geometry. This is the assertion the injection fires.
#[test]
fn applying_marks_the_tree_under_boundaries() {
    let mut h = settled();
    let before = h.states[1].size.w;

    h.inbox.post(123456);
    h.pump();

    let after = h.states[1].size.w;
    println!("label width {before} -> {after} after applying (Boundaries)");
    assert_eq!(h.app.log, vec![123456], "setup: the message did land");
    assert_ne!(
        before, after,
        "the label's box did not change under Boundaries, so the subtree was never \
         marked: apply wrote state and the frame kept last frame's geometry"
    );
}

/// The same case under `Everything`, which is what makes the one above
/// meaningful: `Everything` walks the tree regardless, so it stays green
/// under the injection and the pair localises the defect to the marking.
#[test]
fn everything_sees_the_change_with_or_without_marking() {
    let mut h: Host<Deferring, Rebuild, Everything> =
        Host::new(Deferring::default(), 200, 100);
    for _ in 0..3 {
        h.pump();
    }
    let before = h.states[1].size.w;
    h.inbox.post(123456);
    h.pump();
    assert_ne!(before, h.states[1].size.w, "Everything relays out regardless");
}

/// The queue is bounded and **refuses rather than blocks**, counting
/// what it refused.
///
/// A worker that blocked here would be one the frame loop can stall —
/// backpressure on the wrong side of the seam, and a deadlock on a
/// single-threaded spawner.
#[test]
fn an_overrun_is_refused_and_counted_not_blocked() {
    use lumen_static::inbox::Inbox;

    let h = settled();
    let tx = h.sender();
    let cap = Inbox::<u32>::CAPACITY;

    // Fill it exactly, then overrun by a known amount. The test would
    // hang here rather than fail if the channel blocked, which is the
    // failure this bound exists to remove.
    for i in 0..cap {
        tx.send(i as u32).expect("within capacity");
    }
    let mut refusals = 0;
    for i in 0..10u32 {
        match tx.send(i) {
            Err(SendError::Full(back)) => {
                assert_eq!(back, i, "a refused send hands the message back");
                refusals += 1;
            }
            Ok(()) => panic!("send {i} was accepted past a capacity of {cap}"),
            Err(SendError::Disconnected(_)) => panic!("the host is alive"),
        }
    }
    println!("capacity {cap}: accepted {cap}, refused {refusals}, counter {}", h.inbox.refused());
    assert_eq!(refusals, 10, "every overrun was refused");
    assert_eq!(h.inbox.refused(), 10, "and counted where a test can read it");
}

/// The drain is capped per frame, so a backlog is spread over frames
/// rather than becoming one long frame — and the host says there is more.
#[test]
fn a_backlog_is_spread_over_frames_and_the_host_says_so() {
    use lumen_static::inbox::Inbox;

    let mut h = settled();
    let per = Inbox::<u32>::PER_FRAME;
    let tx = h.sender();
    for i in 0..(per + 5) {
        tx.send(i as u32).expect("within capacity");
    }

    h.pump();
    assert_eq!(h.app.log.len(), per, "one frame applied at most PER_FRAME = {per}");
    assert!(h.more_deferred(), "and the host reports that more is waiting");

    h.pump();
    assert_eq!(h.app.log.len(), per + 5, "the rest landed on the next frame");
    assert!(!h.more_deferred(), "and the host says the backlog is gone");

    // Still in order across the frame boundary, which is the property a
    // cap could plausibly break.
    assert!(
        h.app.log.windows(2).all(|w| w[0] < w[1]),
        "the cap must not reorder: {:?}",
        &h.app.log[..8.min(h.app.log.len())]
    );
}
