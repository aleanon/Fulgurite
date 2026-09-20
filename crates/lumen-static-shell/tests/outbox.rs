//! **M28/T28.4: a handler asks for work, and the host spawns it on the
//! frame the click happened.**
//!
//! The review's first blocking finding against this plan's first draft
//! was that nothing could spawn a task at all: every application code
//! path is `fn(&mut S, &EventCtx)`, which reaches the app and nothing
//! else. It cannot be handed a spawner, because a spawner handle in the
//! app would be a code reference in saved state — which
//! `#[reload(keep)]` already refuses to compile.
//!
//! So the request is data. Input has a queue into the frame; the outbox
//! is the queue out of it.

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, Immediate, Outbox, Queued};
use lumen_static::{Event, PointerButton, Rebuild};
use lumen_static_shell::Host;

const KIND_FETCH: u64 = 7;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("go")]
    go: Button<Work>,
}

#[derive(Default)]
struct Work {
    ui: Ui,
    outbox: Outbox,
    landed: Vec<u64>,
}

fn fetch(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(d.payload + 100);
}

impl App for Work {
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn apply(&mut self, m: u64) -> bool {
        self.landed.push(m);
        true
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.outbox)
    }
    fn job_for(d: Descriptor) -> Option<lumen_static::spawn::JobFn<u64>> {
        match (d.kind, d.step) {
            (KIND_FETCH, 0) => Some(fetch),
            _ => None,
        }
    }
}

/// The handler. A value goes into the outbox; no code, no host.
fn on_click(s: &mut Work, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_FETCH, 5));
}

fn host() -> Host<Work, Rebuild> {
    let mut app = Work::default();
    app.ui.go.on_click = on_click;
    let mut h: Host<Work, Rebuild> = Host::new(app, 200, 100);
    h.pump();
    h
}

/// Click the button by posting a press and a release at its centre.
fn click(h: &mut Host<Work, Rebuild>) {
    let r = &h.states[1];
    let (x, y) = (r.pos.x + r.size.w / 2.0, r.pos.y + r.size.h / 2.0);
    assert!(r.size.w > 0.0 && r.size.h > 0.0, "setup: the button has a box to click");
    h.post(Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 });
    h.post(Event::PointerUp { x, y, button: PointerButton::Left });
}

#[test]
fn a_handler_that_pushes_causes_exactly_one_spawn_on_that_frame() {
    let mut h = host();
    h.set_spawner(Box::new(Immediate));

    click(&mut h);
    h.pump();
    assert!(h.app.outbox.is_empty(), "the host drained the outbox");

    // **One pump, end to end, under `Immediate`** — and that is a
    // consequence of the two drains' order rather than a coincidence.
    // `pump_with` dispatches, then drains the outbox (T28.4), and only
    // later drains the inbox before `App::update` (T28.2). An immediate
    // spawner runs the job inside the first of those, so its result is
    // waiting by the time the second runs.
    //
    // This assertion was written the other way round first — "spawning
    // is not applying", expecting a second pump — and the test was wrong
    // rather than the code. Worth recording: the round trip closing
    // inside one frame is a property of the pump's order, and a later
    // task that reorders the two drains will fail here and should.
    assert_eq!(h.app.landed, vec![105], "exactly one result, from one spawn, in one pump");

    // And a second frame with no click spawns nothing more.
    h.pump();
    assert_eq!(h.app.landed, vec![105], "no click, no spawn");
}

/// The ordering assertion the injection fires: **drained after dispatch,
/// not before.**
///
/// Under the queued spawner the question is deterministic — a descriptor
/// pushed by this frame's handler must be *pending* by the end of this
/// frame. Drained before dispatch it would still be sitting in the
/// outbox, waiting for the next event to arrive.
#[test]
fn the_descriptor_is_spawned_on_the_frame_its_handler_ran() {
    let mut h = host();
    h.set_spawner(Box::new(Queued::<u64>::default()));

    click(&mut h);
    h.pump();

    let pending = h.run_pending_jobs();
    println!("one click: {pending} job(s) pending at the end of that frame");
    assert_eq!(
        pending, 1,
        "the descriptor this frame's handler pushed was not spawned on this frame: \
         the outbox was drained before dispatch, so the request waits for the next event"
    );
}

/// A full outbox refuses and counts, and the count is readable.
#[test]
fn a_full_outbox_refuses_and_counts() {
    let mut app = Work::default();
    for i in 0..Outbox::CAPACITY {
        assert!(app.outbox.push(Descriptor::new(KIND_FETCH, i as u64)), "within capacity");
    }
    let mut refused = 0;
    for i in 0..4 {
        if !app.outbox.push(Descriptor::new(KIND_FETCH, i)) {
            refused += 1;
        }
    }
    println!(
        "capacity {}: queued {}, refused {}, counter {}",
        Outbox::CAPACITY,
        app.outbox.len(),
        refused,
        app.outbox.refused()
    );
    assert_eq!(refused, 4, "every overrun refused");
    assert_eq!(app.outbox.refused(), 4, "and counted");
    assert_eq!(app.outbox.len(), Outbox::CAPACITY, "a refusal must not overwrite");
}

/// An unrecognised `kind` is dropped and **counted**, never guessed at —
/// `docs/plan-tasks.md` §2.1's stability rule. Guessing is what makes a
/// renumbering silently run the wrong job.
#[test]
fn an_unrecognised_kind_is_dropped_and_counted() {
    let mut h = host();
    h.set_spawner(Box::new(Immediate));

    h.app.outbox.push(Descriptor::new(KIND_FETCH, 1));
    h.app.outbox.push(Descriptor::new(999, 1));
    h.pump();
    h.pump();

    println!("unknown kinds dropped: {}", h.unknown_kind());
    assert_eq!(h.app.landed, vec![101], "the known kind ran; the unknown one did not");
    assert_eq!(h.unknown_kind(), 1, "and the drop is counted rather than silent");
}
