//! **M28/T28.12 — use the thing.**
//!
//! `AGENT.md`, after M24: an end-to-end assembly is the only thing that
//! can find a capability which is present, correct in isolation, and
//! unreachable. Every one of M28's pieces has a passing test; none of
//! them had ever been used together, by an application, through a real
//! window.
//!
//! This application does all five: **spawn**, **batch**, **chain**,
//! **cancel** and **subscribe** — and, since M88/T88.1, a sixth: a
//! **sipper**, one job streaming 10,000 values through
//! `Sender::progress`. Every one is driven by a click, and
//! every one puts its consequence on screen where
//! `scripts/tasks_drive.sh` can read it back over the agent endpoint.
//!
//! Run it: `cargo run -p tasks-demo`, or `just tasks-drive` to have the
//! script click every button and check what happened.

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static::spawn::{Descriptor, Failure, JobFn, Outbox, Subs, Timers};
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::Color;
use lumen_static_winit::{application, Program, Run};

// ---- the job table. Stable, append-only ids (plan-tasks.md §2.1) -----

const KIND_ONE: u64 = 1;
const KIND_BATCH: u64 = 2;
const KIND_CHAIN: u64 = 3;
const KIND_SLOW: u64 = 4;
const KIND_SUB: u64 = 5;
const KIND_SIP: u64 = 6;

/// A message is data. Nothing here is a callback, and nothing crossing
/// the frame boundary holds a code reference.
#[derive(Clone, Copy, Debug)]
enum Msg {
    One(u64),
    Batch(u64),
    Chain { step: u64, value: u64 },
    Slow(u64),
    Tick(u64),
    /// One value of the sipper's stream (M88/T88.1), reported through
    /// `Sender::progress`.
    Sip(u64),
    Failed(u64, Failure),
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("title")]
    title: Label<String>,
    #[widget_id("spawn-one")]
    spawn_one: Button<Tasks>,
    #[widget_id("spawn-batch")]
    spawn_batch: Button<Tasks>,
    #[widget_id("start-chain")]
    start_chain: Button<Tasks>,
    #[widget_id("start-slow")]
    start_slow: Button<Tasks>,
    #[widget_id("cancel")]
    cancel: Button<Tasks>,
    #[widget_id("toggle-sub")]
    toggle_sub: Button<Tasks>,
    #[widget_id("sip")]
    sip: Button<Tasks>,
    #[widget_id("sip-value")]
    sip_value: Label<String>,
    #[widget_id("status")]
    status: Label<String>,
    #[widget_id("counts")]
    counts: Label<String>,
}

/// The tree, and the state its handlers may touch — split in
/// M33/T33.4. Hand-written rather than derived because the derive
/// cannot express a real `Msg`, which is why `STATE_IS_SPLIT` is set by
/// hand below: a promise nothing checks, which M33 §2.2 names.
#[derive(Default)]
struct Demo {
    ui: Ui,
    state: Tasks,
}

/// What a handler and `apply` may touch. The tree is not in here.
#[derive(Default)]
struct Tasks {
    outbox: Outbox,
    subs: Subs,
    timers: Timers,

    /// What the last completed thing was — read by the drive script.
    last: String,
    /// A cancel asked for by a handler, acted on by the next frame —
    /// a handler reaches the app and nothing else, so this is how the
    /// request travels, exactly like a descriptor in the outbox.
    cancel_requested: bool,
    /// How many results of each kind have landed. The drive script
    /// asserts on these, because "something changed" is not a claim.
    one: u32,
    batch: u32,
    chain_steps: u32,
    slow: u32,
    ticks: u32,
    failures: u32,
    sub_on: bool,
    /// The sipper's latest value shown, `None` before the first.
    sip: Option<u64>,
}

// ---- job bodies. Non-capturing `fn` items, by type ------------------

fn one(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::One(d.payload));
}
fn batch(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::Batch(d.payload));
}
fn chain(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::Chain { step: d.step, value: d.payload * 2 });
}
/// Long enough that a cancel can land while it is still running — which
/// is the only way to demonstrate what cancel actually does.
fn slow(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    std::thread::sleep(std::time::Duration::from_millis(400));
    let _ = tx.send(Msg::Slow(d.payload));
}
fn tick(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    let _ = tx.send(Msg::Tick(d.payload));
}

// ANCHOR: sip
/// **A sipper** — M88/T88.1. Ten thousand values, each reported with
/// `progress`, which replaces the job's one-value slot instead of
/// queueing: it never blocks and is never refused, the window applies
/// the newest value at most once a frame, and the last one is always
/// applied. Through `send` this stream lost 9,744 values to the bounded
/// inbox, its last one among them (`docs/results-tasks.md`).
fn sip(d: Descriptor, tx: &lumen_static::inbox::Sender<Msg>) {
    for i in 0..d.payload {
        tx.progress(Msg::Sip(i));
    }
}
// ANCHOR_END: sip

impl App for Demo {
    type State = Tasks;
    fn state_mut(&mut self) -> &mut Tasks {
        &mut self.state
    }
    const STATE_IS_SPLIT: bool = true;
    type Tree = Ui;
    type Msg = Msg;

    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.state.outbox)
    }
    fn subscriptions(&mut self) -> Option<&mut Subs> {
        Some(&mut self.state.subs)
    }
    fn timers(&mut self) -> Option<&mut Timers> {
        Some(&mut self.state.timers)
    }

    fn job_for(d: Descriptor) -> Option<JobFn<Msg>> {
        match (d.kind, d.step) {
            (KIND_ONE, 0) => Some(one),
            (KIND_BATCH, 0) => Some(batch),
            // The chain's three steps share a body and differ by index —
            // that is the point of §2.1: the step is data.
            (KIND_CHAIN, 0..=2) => Some(chain),
            (KIND_SLOW, 0) => Some(slow),
            (KIND_SUB, 0) => Some(tick),
            (KIND_SIP, 0) => Some(sip),
            _ => None,
        }
    }

    fn take_cancel(&mut self) -> bool {
        core::mem::take(&mut self.state.cancel_requested)
    }

    fn failed(d: Descriptor, why: Failure) -> Option<Msg> {
        Some(Msg::Failed(d.kind, why))
    }

    /// The step function, in the currently loaded build.
    ///
    /// Receives the state (M33/T33.2), so it cannot write the labels: the
    /// binding step below does, from what this changed.
    fn apply(this: &mut Tasks, m: Msg) -> bool {
        match m {
            Msg::One(v) => {
                this.one += 1;
                this.last = format!("one={v}");
            }
            Msg::Batch(v) => {
                this.batch += 1;
                this.last = format!("batch={v}");
            }
            Msg::Chain { step, value } => {
                this.chain_steps += 1;
                this.last = format!("chain step={step} value={value}");
                // What happens next is data: another descriptor, or
                // nothing. "Nothing" is the absence of a push.
                if step < 2 {
                    this.outbox.push(Descriptor {
                        kind: KIND_CHAIN,
                        step: step + 1,
                        payload: value,
                    });
                }
            }
            Msg::Slow(v) => {
                this.slow += 1;
                this.last = format!("slow={v}");
            }
            Msg::Tick(k) => {
                this.ticks += 1;
                this.last = format!("tick={k}");
            }
            Msg::Sip(v) => this.sip = Some(v),
            Msg::Failed(kind, why) => {
                this.failures += 1;
                this.last = format!("failed kind={kind} why={why:?}");
            }
        }
        true
    }

    /// **The binding step, hand-written** because this impl is.
    ///
    /// The labels used to be written by `apply` and two handlers calling
    /// a `refresh()`, so that the drive script's "the consequence landed"
    /// assertion was about `apply`. After the split neither can reach the
    /// tree: `apply` changes the state and this renders it, so the drive
    /// checks the same consequence, reached through the path that can be
    /// narrowed. `format_into` compares before it writes, so this reports
    /// only what changed.
    fn update(&mut self) -> bool {
        let s = &self.state;
        let a = lumen_static::bind::format_into(&mut self.ui.status, format_args!("last: {}", s.last));
        let b = lumen_static::bind::format_into(
            &mut self.ui.counts,
            format_args!(
                "one={} batch={} chain={} slow={} ticks={} failed={} sub={}",
                s.one,
                s.batch,
                s.chain_steps,
                s.slow,
                s.ticks,
                s.failures,
                if s.sub_on { "on" } else { "off" },
            ),
        );
        let c = match s.sip {
            Some(v) => lumen_static::bind::format_into(&mut self.ui.sip_value, format_args!("sip {v}")),
            None => lumen_static::bind::format_into(&mut self.ui.sip_value, format_args!("sip -")),
        };
        a | b | c
    }
}

fn on_one(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_ONE, 7));
}

/// A batch: several descriptors pushed together. There is no batch type.
fn on_batch(s: &mut Tasks, _: &lumen_static::EventCtx) {
    for i in 1..=3 {
        s.outbox.push(Descriptor::new(KIND_BATCH, i));
    }
}

fn on_chain(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_CHAIN, 1));
}

fn on_slow(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_SLOW, 99));
}

/// Cancel is a *request*, pushed like any other — the handler cannot
/// reach the host, and `EventCtx` carries no inbox. The host's
/// `pump` sees the flag on the next frame.
fn on_cancel(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.cancel_requested = true;
    s.last = "cancel requested".into();
}

fn on_sip(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.outbox.push(Descriptor::new(KIND_SIP, 10_000));
}

fn on_toggle_sub(s: &mut Tasks, _: &lumen_static::EventCtx) {
    s.sub_on = !s.sub_on;
    s.subs.clear();
    s.timers.cancel(KIND_SUB, 1);
    if s.sub_on {
        s.subs.declare(KIND_SUB, 1);
        // A subscription that keeps producing is a timer plus a stream:
        // the declare starts it once, the timer keeps it going.
        s.timers.every(KIND_SUB, 1, 120.0);
    }
    s.last = format!("subscription {}", if s.sub_on { "on" } else { "off" });
}

/// The application, built once — the boot of the program below.
fn build() -> Demo {
    let mut app = Demo::default();
    app.ui.title.set("M28: spawn, batch, chain, cancel, subscribe".into());
    app.ui.spawn_one.label = "spawn one";
    app.ui.spawn_one.on_click = on_one;
    app.ui.spawn_batch.label = "spawn batch";
    app.ui.spawn_batch.on_click = on_batch;
    app.ui.start_chain.label = "start chain";
    app.ui.start_chain.on_click = on_chain;
    app.ui.start_slow.label = "start slow";
    app.ui.start_slow.on_click = on_slow;
    app.ui.cancel.label = "cancel";
    app.ui.cancel.on_click = on_cancel;
    app.ui.toggle_sub.label = "toggle subscription";
    app.ui.toggle_sub.on_click = on_toggle_sub;
    app.ui.sip.label = "start sipper";
    app.ui.sip.on_click = on_sip;
    app
}

/// The sheet. **Kinds are lowercase** (`button`, `label`): this said
/// `"Button"` and `"Label"` until M32/T32.1's migration found that
/// neither rule had ever matched anything — the demo had been unstyled
/// since its sheet was written. The test below asserts the sheet lint
/// finds nothing, which would have caught it on the day.
fn stylesheet() -> Vec<Rule> {
    vec![
        Rule::new(
            Selector::Kind("button"),
            Patch::new()
                .background(Color(226, 232, 240, 255))
                .padding(Edges::all(8.0))
                .radius(6.0)
                .height(Length::Fixed(32.0)),
        ),
        Rule::new(Selector::Kind("label"), Patch::new().font_size(14.0)),
    ]
}

/// **The program, described once** — M32 (`docs/plan-program.md`). The
/// window and `--shot` are both built from it.
fn program() -> Program<Demo> {
    application(build).title("tasks-demo").size(520, 420).style(|_: &Demo| stylesheet())
}

fn main() {
    // The endpoint's address is `LUMEN_AGENT_ADDR`, read by the window
    // (M48/T48.11); unset, this window serves none.
    // M55/T55.4: the strategy is this application's choice, made here.
    #[cfg(feature = "everything")]
    program().invalidation::<lumen_static::Everything>().run();
    #[cfg(not(feature = "everything"))]
    program().run();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M25/T25.2's lint over this program's own sheet: every rule matches
    /// a node and changes something.
    #[test]
    fn every_stylesheet_rule_changes_something() {
        let host = program().host::<lumen_static::retain::Incremental, lumen_static::Everything>();
        let sheet = host.stylesheet();
        let found: Vec<String> = host
            .sheet_findings()
            .iter()
            .map(|f| format!("rule {} {:?}: {:?}", f.rule, sheet[f.rule].selector, f.issue))
            .collect();
        println!("{} rules, {} findings", sheet.len(), found.len());
        assert!(!sheet.is_empty(), "setup: the program installed its sheet");
        assert!(found.is_empty(), "stylesheet rules that change nothing:\n{}", found.join("\n"));
    }
}
