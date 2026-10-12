//! **M28/T28.1's live half: a completion wakes an idle window.**
//!
//! The deterministic half (`crates/fulgurite/tests/inbox_wake.rs`)
//! asserts that sending *asks* for a wake, against a recording proxy and
//! no window. This asserts the other end of the same wire: that the ask
//! reaches a real winit loop sleeping in `ControlFlow::Wait`, and that a
//! frame follows.
//!
//! # Why this is a binary and not a `cargo test`
//!
//! Three reasons, and they are the ones every other live gate in this
//! repository has:
//!
//! * `cargo test` runs on every commit with **no display**, and the
//!   workspace's every-commit path is deliberately windowless.
//! * `cargo test` has no per-test timeout, so the failure mode this gate
//!   is designed to catch — *no wake ever arrives* — would hang CI
//!   rather than fail it.
//! * winit wants the event loop on the main thread.
//!
//! So the fixture **asserts itself**: it exits `0` on success, non-zero
//! with a message on failure, and prints the elapsed time beside its
//! bound either way. `scripts/wake_gate.sh` runs it and skips with exit
//! 0 where there is no display, exactly as `scripts/agent_drive.sh` and
//! `scripts/live_gate.sh` do.
//!
//! # What it proves, and the injection it was shown to fail under
//!
//! A worker thread sleeps past the point where the window has certainly
//! gone idle, then sends. Nothing else touches the window: no input, no
//! animation, no timer. If a frame runs after that, the only thing that
//! could have caused it is [`UserEvent::Wake`] — keyed since M89/T89.1,
//! so the wake must also name the window its waker was built for.
//!
//! Injection — `ProxyWaker::wake` made a no-op, which is the plan's own
//! ("drop the wake"). The fixture then reaches its deadline and exits
//! non-zero, printing the bound it missed. Before M28/T28.1 the same
//! fixture could not be written at all: a build without the `agent`
//! feature used `EventLoop::new()`, which has no proxy to send to.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fulgurite::inbox::{Inbox, Waker};
use fulgurite_winit::{ProxyWaker, UserEvent};

/// The one window's key, as a `Program`'s primary is (M89/T89.1).
const KEY: u32 = 1;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// How long the worker waits before sending. Long enough that the window
/// has certainly stopped drawing on its own: a fresh window emits a
/// burst of resize and redraw events, and a wake measured through that
/// burst would be measuring the burst.
const IDLE_SETTLE: Duration = Duration::from_millis(600);

/// The bound. Generous on purpose — this gate answers "does a wake
/// arrive at all", and a latency *budget* is a different measurement
/// that would need a quiet machine to mean anything. The elapsed time is
/// printed beside it so the number is visible even when it passes, which
/// is how a bound that has drifted far from its quantity becomes
/// obvious.
const DEADLINE: Duration = Duration::from_secs(5);

struct Fixture {
    window: Option<Arc<Window>>,
    inbox: Inbox<u32>,
    /// Set once the worker has sent, so a frame can be attributed.
    sent_at: Arc<Mutex<Option<Instant>>>,
    /// Frames seen since the worker sent.
    frames_after_send: Arc<AtomicU32>,
    started: Instant,
    worker_started: bool,
}

impl ApplicationHandler<UserEvent> for Fixture {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title("wake-gate")
            .with_inner_size(winit::dpi::LogicalSize::new(240.0, 120.0));
        // The live drives' placement requests, read as fulgurite-winit
        // reads them when it opens a window: this fixture opens its own.
        if let Some((x, y)) = std::env::var("FULGURITE_WINDOW_POS").ok().and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some((x.trim().parse::<i32>().ok()?, y.trim().parse::<i32>().ok()?))
        }) {
            attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(x, y));
        }
        if std::env::var("FULGURITE_WINDOW_INACTIVE").as_deref() == Ok("1") {
            attrs = attrs.with_active(false);
        }
        let w = Arc::new(e.create_window(attrs).expect("window"));
        w.request_redraw();
        self.window = Some(w);
    }

    fn window_event(&mut self, e: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => e.exit(),
            WindowEvent::RedrawRequested => {
                // A frame. If the worker has already sent, this is the
                // frame the wake produced — which is the whole claim.
                if self.sent_at.lock().unwrap().is_some() {
                    self.frames_after_send.fetch_add(1, Ordering::SeqCst);
                }
                // **Start the worker from inside the first frame**, not
                // from `main`. A worker started before the loop runs
                // could send while winit is still creating the window,
                // and the wake would be indistinguishable from the
                // startup burst — the "setup silently did not happen"
                // shape, in the direction that makes the gate pass for
                // the wrong reason.
                if !self.worker_started {
                    self.worker_started = true;
                    let tx = self.inbox.sender();
                    std::thread::spawn(move || {
                        std::thread::sleep(IDLE_SETTLE);
                        tx.send(7).expect("the fixture is alive");
                    });
                }
            }
            _ => {}
        }
    }

    // Irrefutable in this build: the fixture enables none of the features
    // that add `UserEvent`'s other variants.
    #[allow(irrefutable_let_patterns)]
    fn user_event(&mut self, _: &ActiveEventLoop, ev: UserEvent) {
        // The waker is keyed (M89/T89.1): the wake must name this
        // fixture's one window, the key it was built with.
        if let UserEvent::Wake(key) = ev {
            if key != KEY {
                eprintln!("wake-gate: FAILED — a wake named window {key}, the waker was built for {KEY}");
                std::process::exit(1);
            }
            // Record *when* the wake landed, then ask for a frame. The
            // frame is what the claim is about; the wake alone only
            // proves the proxy works.
            *self.sent_at.lock().unwrap() = Some(Instant::now());
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, e: &ActiveEventLoop) {
        // The verdict, checked wherever the loop is about to sleep.
        let woke = self.frames_after_send.load(Ordering::SeqCst) > 0;
        if woke {
            let elapsed = self.started.elapsed();
            println!(
                "wake-gate: a frame followed the worker's send in {:.0} ms \
                 (settle {:.0} ms, deadline {:.0} ms)",
                elapsed.as_secs_f64() * 1000.0,
                IDLE_SETTLE.as_secs_f64() * 1000.0,
                DEADLINE.as_secs_f64() * 1000.0
            );
            e.exit();
            return;
        }
        if self.started.elapsed() > DEADLINE {
            eprintln!(
                "wake-gate: FAILED — no frame followed the worker's send within {:.0} ms \
                 (settle {:.0} ms). The loop slept through a completion, which is the \
                 defect M28/T28.1 exists to remove.",
                DEADLINE.as_secs_f64() * 1000.0,
                IDLE_SETTLE.as_secs_f64() * 1000.0
            );
            std::process::exit(1);
        }
        // `WaitUntil`, not `Wait`: the deadline check above only runs
        // when the loop is about to sleep, and under a bare `Wait` with
        // no wake arriving it would sleep forever and never reach it.
        // That would turn a failing gate into a hanging one — the exact
        // thing this fixture is a binary in order to avoid.
        e.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(250)));
    }
}

fn main() {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut inbox: Inbox<u32> = Inbox::new();
    let waker: Arc<dyn Waker> = Arc::new(ProxyWaker::new(event_loop.create_proxy(), KEY));
    inbox.set_waker(waker);
    assert!(inbox.has_waker(), "setup: the fixture installed a waker");

    let mut fixture = Fixture {
        window: None,
        inbox,
        sent_at: Arc::new(Mutex::new(None)),
        frames_after_send: Arc::new(AtomicU32::new(0)),
        started: Instant::now(),
        worker_started: false,
    };
    event_loop.run_app(&mut fixture).expect("run");

    // Reached only on a clean exit from `about_to_wait`'s success path.
    let mut sum = 0u64;
    let n = fixture.inbox.drain(&mut sum, |s, m| *s += m as u64);
    assert_eq!(
        (n, sum),
        (1, 7),
        "the message itself must have arrived, not just the wake: a wake with an empty \
         queue is a spurious frame, not deferred work"
    );
}
