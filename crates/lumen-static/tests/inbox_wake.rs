//! M28/T28.1, the deterministic half: **posting asks the loop for a
//! wake.**
//!
//! A windowed host sleeps in `ControlFlow::Wait` until input arrives. A
//! completion posted from a worker at 3 a.m. therefore sits in the queue
//! until the user moves the mouse — and the symptom is not a crash but a
//! frame that arrives minutes late, attributed to anything but this.
//!
//! The live half of the gate needs a window and lives in
//! `scripts/wake_gate.sh`; this half needs none, because the claim
//! "sending asks for a wake" is about the seam and not about winit. That
//! split is the one this repository already uses for the agent's gates,
//! and the reason is that `cargo test` runs on every commit with no
//! display and no per-test timeout.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use lumen_static::inbox::{Inbox, Waker};

/// A proxy that records instead of waking. The whole point of the
/// [`Waker`] trait being a trait.
#[derive(Default)]
struct Counting {
    wakes: AtomicUsize,
}

impl Counting {
    fn count(&self) -> usize {
        self.wakes.load(Ordering::SeqCst)
    }
}

impl Waker for Counting {
    fn wake(&self) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn a_send_from_a_worker_asks_for_a_wake() {
    let mut inbox: Inbox<u32> = Inbox::new();
    let proxy = Arc::new(Counting::default());
    inbox.set_waker(proxy.clone());
    assert!(inbox.has_waker(), "setup: the waker is installed");

    let tx = inbox.sender();
    assert_eq!(proxy.count(), 0, "setup: minting a sender wakes nothing");

    let worker = std::thread::spawn(move || {
        for i in 0..4u32 {
            tx.send(i).expect("the host is alive");
        }
    });
    worker.join().expect("the worker finished");

    assert_eq!(
        proxy.count(),
        4,
        "four sends asked for {} wakes, not 4 — a completion would sit in the queue \
         until unrelated input arrived",
        proxy.count()
    );

    // And the messages are actually there: a wake that arrives with an
    // empty queue is a spurious frame, and a message that arrives with
    // no wake is the bug. Assert both halves of the pair.
    let mut sum = 0u64;
    let n = inbox.drain(&mut sum, |s, m| *s += m as u64);
    assert_eq!((n, sum), (4, 6), "the four messages arrived and applied");
}

#[test]
fn post_wakes_too() {
    let mut inbox: Inbox<u32> = Inbox::new();
    let proxy = Arc::new(Counting::default());
    inbox.set_waker(proxy.clone());

    inbox.post(7);
    assert_eq!(proxy.count(), 1, "a synchronous post must wake as well");
}

/// The accepting direction, and the one that keeps this from becoming a
/// host that wakes on every frame: **a headless host installs no waker
/// and must still work.**
///
/// `Host::new` in a test, a bench, or an embedded target has no event
/// loop. Sending has to stay infallible-shaped there, not panic and not
/// require a stub.
#[test]
fn an_inbox_with_no_waker_still_delivers() {
    let inbox: Inbox<u32> = Inbox::new();
    assert!(!inbox.has_waker(), "setup: no waker, the headless shape");

    let tx = inbox.sender();
    tx.send(11).expect("delivery does not depend on a waker");
    inbox.post(31);

    let mut sum = 0u64;
    let n = inbox.drain(&mut sum, |s, m| *s += m as u64);
    assert_eq!((n, sum), (2, 42), "both messages arrived without a loop to wake");
}

/// A sender minted **before** the waker was installed carries no waker,
/// and the rustdoc says so rather than pretending otherwise.
///
/// Asserted because it is a real ordering hazard: a host that hands out
/// a sender in its constructor and installs the waker afterwards would
/// silently produce exactly the late-frame symptom this task exists to
/// remove, and nothing else would notice.
#[test]
fn a_sender_minted_before_the_waker_says_so() {
    let mut inbox: Inbox<u32> = Inbox::new();
    let early = inbox.sender();

    let proxy = Arc::new(Counting::default());
    inbox.set_waker(proxy.clone());
    let late = inbox.sender();

    early.send(1).unwrap();
    assert_eq!(proxy.count(), 0, "the early sender cannot wake a loop that did not exist");
    late.send(2).unwrap();
    assert_eq!(proxy.count(), 1, "the late one can");
}
