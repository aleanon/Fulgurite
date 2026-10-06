//! **M88/T88.1: `Sender::progress`, the sipper** —
//! `docs/plan-platform-requests.md`.
//!
//! T88.0 pinned the gap here: a job streaming 10,000 values through
//! `Sender::send` had 9,744 refused, a pump applied up to 64, and the
//! last value was lost (`docs/results-tasks.md`, M88 baseline). This file
//! is that pin, flipped: the same job reports through `Sender::progress`,
//! a one-value slot on its `JobTrack` that the pump reads before the
//! channel, under all three spawners —
//!
//! | claim | bound |
//! |---|---|
//! | values applied in one pump | ≤ 1 |
//! | value 9,999 applied | exactly once |
//! | refused | 0 |
//! | applied + `Inbox::coalesced` | 10,000 — every value accounted for |
//!
//! — and four arms beside it: an **unsplit** application under
//! `Boundaries<256>` sees its progress (the slot's change reaches
//! `mark_all`), **no body blocks while nothing drains** (the panic-screen
//! and swap case), a **stopped** job's slot is dropped and counted, and
//! the accept direction, a job that `send`s three values still gets all
//! three.
//!
//! Every run that could hang under an injection (a `progress` that
//! blocks) runs on a watchdog thread, so it fails by name instead.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use lumen_static::display::Cmd;
use lumen_static::event::App;
use lumen_static::inbox::Sender;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, Immediate, Job, JobFn, Outbox, Queued, Spawner, Subs, Threads, Timers};
use lumen_static::Rebuild;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

/// A tick's message: one value no sipper reports.
const TICK: u64 = u64::MAX;
const KIND_TICK: u64 = 2;
const KIND_SUB: u64 = 3;

#[derive(Default)]
struct Stream {
    ui: Ui,
    landed: Vec<u64>,
    ticks: u32,
    outbox: Outbox,
    timers: Timers,
    subs: Subs,
}

fn tick(_: Descriptor, tx: &Sender<u64>) {
    let _ = tx.send(TICK);
}

/// Three progress values, then return — without asking `is_stopped`, so
/// in the stop arm only the drain can hold.
fn three(_: Descriptor, tx: &Sender<u64>) {
    for v in 1..=3 {
        tx.progress(v);
    }
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
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }
    fn timers(&mut self) -> Option<&mut Timers> {
        Some(&mut self.timers)
    }
    fn subscriptions(&mut self) -> Option<&mut Subs> {
        Some(&mut self.subs)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        match d.kind {
            KIND_TICK => Some(tick),
            KIND_SUB => Some(three),
            _ => None,
        }
    }
    fn apply(this: &mut Self, m: u64) -> bool {
        if m == TICK {
            this.ticks += 1;
        } else {
            this.landed.push(m);
        }
        true
    }
}

/// Reports `0..payload` through `progress`: the sipper.
fn sip(d: Descriptor, tx: &Sender<u64>) {
    for i in 0..d.payload {
        tx.progress(i);
    }
}

/// Sends `0..payload` through `send` — the accept arm.
fn stream(d: Descriptor, tx: &Sender<u64>) {
    for i in 0..d.payload {
        let _ = tx.send(i);
    }
}

/// Run `f` on a thread and fail by name if it does not finish in `secs`
/// — a `progress` that blocks hangs `Immediate`, which runs the body on
/// the spawning thread.
fn watchdog<T: Send + 'static>(what: &str, secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(v) => v,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("{what}: did not finish within {secs} s -- a job body blocked")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("{what}: failed (its panic is printed above)"),
    }
}

#[derive(Debug)]
struct Run {
    refused: usize,
    per_pump_max: usize,
    last_applied: usize,
    applied: usize,
    coalesced: usize,
    pumps: usize,
    listed: usize,
}

#[derive(Clone, Copy, Debug)]
enum Spawn {
    Immediate,
    Queued,
    Threads2,
}

fn run(spawn: Spawn, body: JobFn<u64>, n: u64) -> Run {
    let mut h: Host<Stream, Rebuild> = Host::new(Stream::default(), 100, 50);
    h.pump();
    let s: Box<dyn Spawner<u64>> = match spawn {
        Spawn::Immediate => Box::new(Immediate),
        Spawn::Queued => Box::new(Queued::<u64>::default()),
        Spawn::Threads2 => Box::new(Threads::with_cap(2)),
    };
    h.set_spawner(s);
    let (refused0, coalesced0) = (h.inbox.refused(), h.inbox.coalesced());
    assert!(h.spawn(Job { desc: Descriptor { kind: 1, step: 0, payload: n }, job: body, on_fail: None }));
    // Setup landed: listed before the first pump, under every spawner —
    // queued, finished with its slot full, or still running. Asserted
    // after the claims, by the caller: a ledger that drops the job with
    // its slot full answers 0 here, and the claim that names that is
    // "9,999 applied exactly once".
    let listed = h.in_flight();
    if let Spawn::Queued = spawn {
        assert_eq!(h.run_pending_jobs(), 1, "setup: the queued job ran");
    }

    let mut per_pump_max = 0;
    let mut pumps = 0;
    let until = Instant::now() + Duration::from_secs(20);
    // At least one pump, then until the ledger is empty.
    loop {
        let before = h.app.landed.len();
        h.pump();
        per_pump_max = per_pump_max.max(h.app.landed.len() - before);
        pumps += 1;
        if h.in_flight() == 0 {
            break;
        }
        assert!(Instant::now() < until, "({spawn:?}) the ledger never emptied after {pumps} pumps");
        if let Spawn::Threads2 = spawn {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    let last = n - 1;
    Run {
        refused: h.inbox.refused() - refused0,
        per_pump_max,
        last_applied: h.app.landed.iter().filter(|&&v| v == last).count(),
        applied: h.app.landed.len(),
        coalesced: h.inbox.coalesced() - coalesced0,
        pumps,
        listed,
    }
}

fn check_sipper(spawn: Spawn) {
    let r = watchdog(&format!("sipper under {spawn:?}"), 30, move || run(spawn, sip, 10_000));
    println!(
        "sipper ({spawn:?}): in flight before the first pump {} (want 1); 10000 progress calls over {} pumps; per-pump max {} (bound 1), \
         value 9999 applied {} time(s) (bound 1), refused {} (bound 0); applied {} + coalesced {} = {} (want 10000)",
        r.listed,
        r.pumps,
        r.per_pump_max,
        r.last_applied,
        r.refused,
        r.applied,
        r.coalesced,
        r.applied + r.coalesced
    );
    assert!(r.per_pump_max <= 1, "({spawn:?}) a pump applied {} progress values -- the slot holds one", r.per_pump_max);
    assert_eq!(r.last_applied, 1, "({spawn:?}) value 9999 must be applied exactly once -- it was applied {} time(s)", r.last_applied);
    assert_eq!(r.refused, 0, "({spawn:?}) progress was refused {} time(s)", r.refused);
    assert_eq!(r.applied + r.coalesced, 10_000, "({spawn:?}) values neither applied nor coalesced");
    assert_eq!(r.listed, 1, "setup ({spawn:?}): the job must be in flight before the first pump");
}

#[test]
fn a_sipper_under_immediate_delivers_its_last_value_once() {
    check_sipper(Spawn::Immediate);
}

#[test]
fn a_sipper_under_queued_delivers_its_last_value_once() {
    check_sipper(Spawn::Queued);
}

#[test]
fn a_sipper_under_threads_delivers_its_last_value_once() {
    check_sipper(Spawn::Threads2);
}

/// **Accept direction**: the channel is untouched — a job that `send`s
/// three values still gets all three, in order.
#[test]
fn a_job_that_sends_still_gets_every_value() {
    let r = watchdog("send x3", 30, || run(Spawn::Immediate, stream, 3));
    println!("send accept: 3 sends; applied {} (want 3), refused {} (bound 0)", r.applied, r.refused);
    assert_eq!((r.applied, r.refused), (3, 0));
}

/// The size the slot costs, printed for `docs/results-tasks.md` T88.1.
#[test]
fn the_slot_lives_inline_in_the_track() {
    let size = core::mem::size_of::<lumen_static::inbox::JobTrack<u64>>();
    let slot = core::mem::size_of::<std::sync::Mutex<Option<u64>>>();
    println!("size_of::<JobTrack<u64>>() = {size} B (slot Mutex<Option<u64>> {slot} B + fresh 1 + epoch 8)");
    assert!(size >= slot + 9, "setup: the slot and its epoch are in the track");
}

// ---- an unsplit application sees its progress (second review A2) ------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct LabelUi {
    value: Label<String>,
}

/// `STATE_IS_SPLIT = false` (the hand-written default), and `apply`
/// writes the label straight into the tree — so nothing marks it but the
/// host's `mark_all` on an applied change.
#[derive(Default)]
struct Unsplit {
    ui: LabelUi,
}

impl App for Unsplit {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    type Tree = LabelUi;
    type Msg = u64;
    fn tree(&self) -> &LabelUi {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut LabelUi {
        &mut self.ui
    }
    fn apply(this: &mut Self, m: u64) -> bool {
        this.ui.value.set(m.to_string());
        true
    }
}

fn shown(h: &Host<Unsplit>) -> String {
    h.list
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { text, .. } => Some(h.list.str_of(*text).to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[test]
fn an_unsplit_application_sees_its_progress_under_boundaries() {
    let mut app = Unsplit::default();
    app.ui.value.set("0".into());
    // `Host<Unsplit>`: `Incremental` retention, `Boundaries<256>`.
    let mut h: Host<Unsplit> = Host::new(app, 200, 40);
    h.pump();
    let before = shown(&h);
    assert_eq!(before, "0", "setup: the label reads 0 before the job");
    h.set_spawner(Box::new(Immediate));
    let coalesced0 = h.inbox.coalesced();
    let mut pumps = 0;
    assert!(h.spawn(Job { desc: Descriptor::new(1, 0), job: three, on_fail: None }));
    loop {
        h.pump();
        pumps += 1;
        if h.in_flight() == 0 {
            break;
        }
        assert!(pumps < 100, "the ledger never emptied");
    }
    let coalesced = h.inbox.coalesced() - coalesced0;
    let after = shown(&h);
    println!(
        "unsplit Boundaries<256>: label {before:?} -> {after:?} (want \"3\"); \
         pumps {pumps}, applied 1 + coalesced {coalesced} = {} (want 3)",
        1 + coalesced
    );
    // Under `Immediate` the body ran at spawn: two values were replaced
    // unread and the third waits in the slot for the pump.
    assert_eq!(1 + coalesced, 3, "setup: three values reported, one applied");
    assert_eq!(after, "3", "the frame shows a stale label: the slot's change never reached mark_all (the idle skip)");
}

// ---- no block while nothing drains (P15(4)) ---------------------------

static ELAPSED: [AtomicU64; 2] = [AtomicU64::new(u64::MAX), AtomicU64::new(u64::MAX)];
static RUNS: AtomicUsize = AtomicUsize::new(0);

/// A sipper that times itself: `payload` is its slot in `ELAPSED`.
fn timed_sip(d: Descriptor, tx: &Sender<u64>) {
    let t0 = Instant::now();
    for i in 0..10_000 {
        tx.progress(i);
    }
    ELAPSED[d.payload as usize].store(t0.elapsed().as_micros() as u64, Ordering::Relaxed);
    RUNS.fetch_add(1, Ordering::Relaxed);
}

/// Three seconds: two sippers spawned at once on a two-worker pool and a
/// 120 ms timer, **no pump for the first second**, then a tick and a pump
/// every few milliseconds. Returns the ticks applied.
fn no_drain_run(sippers: bool) -> u32 {
    let mut h: Host<Stream, Rebuild> = Host::new(Stream::default(), 100, 50);
    h.pump();
    h.set_spawner(Box::new(Threads::with_cap(2)));
    assert!(h.app.timers.every(KIND_TICK, 1, 120.0), "setup: the timer is armed");
    let t0 = Instant::now();
    if sippers {
        for slot in 0..2u64 {
            assert!(h.spawn(Job { desc: Descriptor::new(9, slot), job: timed_sip, on_fail: None }));
        }
    }
    // Nothing drains: the panic screen, or a swap in progress.
    std::thread::sleep(Duration::from_secs(1));
    let mut last = Instant::now();
    h.tick(last.duration_since(t0).as_secs_f32() * 1000.0);
    h.pump();
    while t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(4));
        let now = Instant::now();
        h.tick(now.duration_since(last).as_secs_f32() * 1000.0);
        last = now;
        h.pump();
    }
    h.app.ticks
}

#[test]
fn no_sipper_blocks_while_nothing_drains() {
    let control = watchdog("control run", 20, || no_drain_run(false));
    let with = watchdog("sipper run", 20, || no_drain_run(true));
    let el: Vec<u64> = ELAPSED.iter().map(|e| e.load(Ordering::Relaxed)).collect();
    let ms = |e: u64| if e == u64::MAX { "never (still blocked at 3 s)".to_string() } else { format!("{:.2} ms", e as f64 / 1000.0) };
    println!(
        "no drain for 1 s: sipper bodies returned in {} and {} (bound 50 ms each; {} of 2 returned); \
         timer ticks {with} with sippers, {control} in the control (bound +-1; control must be >= 20)",
        ms(el[0]),
        ms(el[1]),
        RUNS.load(Ordering::Relaxed),
    );
    assert!(control >= 20, "setup: the control run's timer ticked only {control} times");
    for (i, e) in el.iter().enumerate() {
        assert!(
            *e <= 50_000,
            "sipper {i}'s body returned in {} with nothing draining (bound 50 ms): progress blocked",
            ms(*e)
        );
    }
    assert!(
        with.abs_diff(control) <= 1,
        "the timer ticked {with} times beside two sippers and {control} without (bound +-1): the sippers held the pool"
    );
}

// ---- progress after stop (M84/T84.7 landed first; M11) ----------------

#[test]
fn a_stopped_jobs_last_progress_value_is_not_applied() {
    let mut h: Host<Stream, Rebuild> = Host::new(Stream::default(), 100, 50);
    h.pump();
    h.set_spawner(Box::new(Queued::<u64>::default()));
    assert!(h.app.subs.declare(KIND_SUB, 1));
    h.pump(); // spawns the subscription's job, which `Queued` holds
    assert_eq!(h.in_flight(), 1, "setup: the subscription's job is listed");
    assert_eq!(h.run_pending_jobs(), 1, "setup: its body ran and filled its slot");
    let dropped0 = h.stopped_dropped();
    h.app.subs.clear();
    h.pump(); // the diff stops it; the slot drain must drop its value
    let applied = h.app.landed.iter().filter(|&&v| v == 3).count();
    let dropped = h.stopped_dropped() - dropped0;
    let left = h.in_flight();
    println!(
        "progress after stop: final value applied {applied} time(s) (bound 0), stopped_dropped +{dropped} (want 1), \
         in flight after {left} (want 0)"
    );
    assert_eq!(applied, 0, "a stopped job's final progress value was applied");
    assert_eq!(dropped, 1, "the dropped slot value was not counted in stopped_dropped");
    assert_eq!(left, 0, "a stopped job with its slot dropped is settled");
}
