//! A window library with a **two-step chain** — M49/T49.10 (R4-F3,
//! `docs/plan-reload-hardening.md` §2.4).
//!
//! The button fills the top of a fixed 240×120 window, as in
//! `fixtures/reload-app`, so a loader with no tree in hand clicks it at a
//! known point. A click pushes `(KIND, 0, 4)`. Step 0's result is applied
//! by pushing step 1, and step 1's result is shown as `chain N`.
//!
//! **The two builds disagree about step 0 and agree about step 1**, so
//! the shown value says which build ran which step:
//!
//! | step | v1 | v2 |
//! |---|---|---|
//! | 0 | `p + 1`, after the loader calls `chain_app_release` | `p + 100` |
//! | 1 | `x * 10` | `x * 10` |
//!
//! From payload 4: v2 throughout is `chain 1040`; v1's step 0 with v2's
//! step 1 — the old build's code run after the swap — is `chain 50`.
//! v1's step 0 **blocks** until released, which is what puts it in flight
//! at the swap without a race.
//!
//! **And a sipper, since M88/T88.1** (`docs/plan-platform-requests.md`):
//! `#sip` spawns one job that reports `0..10000` through
//! `Sender::progress`, shown as `sip N`. v1 reports `p`, and **blocks
//! after reporting `p = 5`** until the loader calls
//! `chain_app_release_sip`; v2 reports `p + 100`. So v2 throughout ends
//! at `sip 10099`, and v1's own stream would end at `sip 9999`. The
//! sipper has its own release, because `reload-host`'s
//! chain gate releases v1's step 0 and the same library stays loaded.
//!
//! **And a platform request, since M88/T88.3**: `#ask` pushes
//! `ClipboardRead { tag: 7, .. }`, and `answered` labels the text with
//! the build that converted it — `v1:x` or `v2:x`. The answer lands on
//! `Work::asked`, which the swap **keeps**, so an answer applied before a
//! swap is still shown after it; before any answer the label reads the
//! loaded build's bare prefix (`v1:` or `v2:`). The three buttons share
//! the top 60 px, a third each: `#start`, `#sip`, `#ask`.

use core::sync::atomic::{AtomicBool, Ordering};

use fulgurite::event::{App, EventCtx};
use fulgurite::inbox::Sender;
use fulgurite::request::{Answer, ClipFormat, ClipKind, Request, Requests};
use fulgurite::spawn::{Descriptor, JobFn, Outbox};
use fulgurite::style::{Edges, Length, Patch, Rule, Selector};
use fulgurite::{Reloadable as DeriveReloadable, StaticTree};
use fulgurite_shell::program::{application, Program};
use fulgurite_widgets::prelude::*;

/// The chain's kind. Stable, append-only (`docs/plan-tasks.md` §2.1).
const KIND: u64 = 9;
/// The sipper's kind (M88/T88.1). Its values carry step 2 in `pack`.
const KIND_SIP: u64 = 10;

/// This build's name, as `answered` labels an answer with it.
#[cfg(not(feature = "v2"))]
const BUILD: &str = "v1";
#[cfg(feature = "v2")]
const BUILD: &str = "v2";

/// The three buttons, side by side across the top 60 px.
#[derive(StaticTree, Default, DeriveReloadable)]
#[widget(row, spacing = 0.0)]
pub struct Buttons {
    #[widget_id("start")]
    pub start: Button<Work>,
    #[widget_id("sip")]
    pub sip: Button<Work>,
    #[widget_id("ask")]
    pub ask: Button<Work>,
}

#[derive(StaticTree, Default, DeriveReloadable)]
#[widget(column, spacing = 4.0)]
pub struct Ui {
    pub buttons: Buttons,
    #[widget_id("result")]
    pub result: Label<String>,
    #[widget_id("sip-value")]
    pub sip_value: Label<String>,
    #[widget_id("asked")]
    pub asked: Label<String>,
}

/// What a handler and `apply` may touch (M33). Fresh but for `asked`:
/// the outbox and the request queue are the host's plumbing, and the
/// chain and the sipper are shown from their own streams (a sipper's
/// value is fresh, so a dropped re-issue reads `sip -` — M88/T88.1).
/// `asked` is **kept** (M88/T88.3): an answer's effect is ordinary state,
/// and this is what shows it survives the swap.
#[derive(Default, DeriveReloadable)]
pub struct Work {
    #[reload(fresh)]
    outbox: Outbox,
    #[reload(fresh)]
    requests: Requests,
    #[reload(fresh)]
    result: Option<u64>,
    #[reload(fresh)]
    sip: Option<u64>,
    #[reload(keep)]
    asked: Option<String>,
}

/// A message: a step's or a sipper's packed value, or an answer
/// converted by the build loaded when it arrived (M88/T88.3).
#[derive(Debug)]
pub enum Msg {
    Step(u64),
    Asked(String),
}

/// Hand-written `App`, because the derive cannot express a real `Msg`
/// (as `examples/tasks-demo`). So there is no `#[tree]` attribute to
/// write: the tree is an untagged field, which the `Reloadable` derive
/// restores as a nested widget, as it restores a `#[tree]`.
#[derive(DeriveReloadable)]
pub struct Chain {
    pub ui: Ui,
    pub work: Work,
}

/// A result carries the step that produced it in the high bits, so
/// `apply` knows what comes next.
fn pack(step: u64, v: u64) -> u64 {
    (step << 32) | v
}

/// Set by the loader through `chain_app_release`; v1's step 0 waits on
/// it. A static of **this** library's copy, so releasing v1 releases no
/// other build.
static RELEASED: AtomicBool = AtomicBool::new(false);
/// Set by the loader through `chain_app_release_sip`; v1's sipper waits
/// on it after reporting 5.
static SIP_RELEASED: AtomicBool = AtomicBool::new(false);

fn step0(d: Descriptor, tx: &Sender<Msg>) {
    #[cfg(not(feature = "v2"))]
    {
        // Bounded, so a gate that never releases does not hang a worker
        // for ever — it fails on its own timeout first.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !RELEASED.load(Ordering::Acquire) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let _ = tx.send(Msg::Step(pack(0, d.payload + 1)));
    }
    #[cfg(feature = "v2")]
    {
        let _ = &RELEASED;
        let _ = tx.send(Msg::Step(pack(0, d.payload + 100)));
    }
}

fn step1(d: Descriptor, tx: &Sender<Msg>) {
    let _ = tx.send(Msg::Step(pack(1, d.payload * 10)));
}

/// The sipper: `0..payload` through `progress`, v1 as `p`, v2 as `p + 100`.
fn sipper(d: Descriptor, tx: &Sender<Msg>) {
    for p in 0..d.payload {
        #[cfg(not(feature = "v2"))]
        {
            tx.progress(Msg::Step(pack(2, p)));
            if p == 5 {
                // Bounded, as step 0's wait is.
                let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
                while !SIP_RELEASED.load(Ordering::Acquire) && std::time::Instant::now() < until {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        }
        #[cfg(feature = "v2")]
        {
            let _ = &SIP_RELEASED;
            tx.progress(Msg::Step(pack(2, p + 100)));
        }
    }
}

impl App for Chain {
    type State = Work;
    fn state_mut(&mut self) -> &mut Work {
        &mut self.work
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
        Some(&mut self.work.outbox)
    }
    fn requests(&mut self) -> Option<&mut Requests> {
        Some(&mut self.work.requests)
    }
    /// **An answer, converted by this build** — M88/T88.3. Labelled with
    /// the build, so the label says which one was loaded when it arrived.
    fn answered(tag: u64, a: Answer<'_>) -> Option<Msg> {
        match (tag, a) {
            (7, Answer::Text(t)) => Some(Msg::Asked(format!("{BUILD}:{t}"))),
            _ => None,
        }
    }
    fn job_for(d: Descriptor) -> Option<JobFn<Msg>> {
        match (d.kind, d.step) {
            (KIND, 0) => Some(step0),
            (KIND, 1) => Some(step1),
            (KIND_SIP, 0) => Some(sipper),
            _ => None,
        }
    }
    /// The step function, in the currently loaded build.
    fn apply(w: &mut Work, m: Msg) -> bool {
        let m = match m {
            Msg::Step(m) => m,
            Msg::Asked(t) => {
                w.asked = Some(t);
                return true;
            }
        };
        let (step, v) = (m >> 32, m & 0xffff_ffff);
        match step {
            0 => {
                w.outbox.push(Descriptor { kind: KIND, step: 1, payload: v });
            }
            2 => w.sip = Some(v),
            _ => w.result = Some(v),
        }
        true
    }
    fn update(&mut self) -> bool {
        let a = match self.work.result {
            Some(v) => fulgurite::bind::format_into(&mut self.ui.result, format_args!("chain {v}")),
            None => fulgurite::bind::format_into(&mut self.ui.result, format_args!("chain -")),
        };
        let b = match self.work.sip {
            Some(v) => fulgurite::bind::format_into(&mut self.ui.sip_value, format_args!("sip {v}")),
            None => fulgurite::bind::format_into(&mut self.ui.sip_value, format_args!("sip -")),
        };
        let c = match &self.work.asked {
            Some(t) => fulgurite::bind::set_if_changed(&mut self.ui.asked, t),
            None => fulgurite::bind::format_into(&mut self.ui.asked, format_args!("{BUILD}:")),
        };
        a | b | c
    }
}

fn on_start(w: &mut Work, _: &EventCtx) {
    w.outbox.push(Descriptor::new(KIND, 4));
}

fn on_sip(w: &mut Work, _: &EventCtx) {
    w.outbox.push(Descriptor::new(KIND_SIP, 10_000));
}

/// M88/T88.3: ask for the clipboard's text. The loader stands in for the
/// window: it takes the request and answers tag 7.
fn on_ask(w: &mut Work, _: &EventCtx) {
    w.requests.push(Request::ClipboardRead { tag: 7, kind: ClipKind::Clipboard, format: ClipFormat::Text });
}

fn boot() -> Chain {
    let mut c = Chain { ui: Ui::default(), work: Work::default() };
    c.ui.buttons.start.label = "start";
    c.ui.buttons.start.on_click = on_start;
    c.ui.buttons.sip.label = "sip";
    c.ui.buttons.sip.on_click = on_sip;
    c.ui.buttons.ask.label = "ask";
    c.ui.buttons.ask.on_click = on_ask;
    c
}

pub fn program() -> Program<Chain> {
    application(boot).title("chain-app").size(240, 120).style(|_: &Chain| {
        vec![
            Rule::new(Selector::Kind("Ui"), Patch::new().padding(Edges::all(0.0))),
            Rule::new(Selector::Kind("Buttons"), Patch::new().padding(Edges::all(0.0))),
            Rule::new(Selector::Id("start"), Patch::new().width(Length::FILL).height(Length::Fixed(60.0))),
            Rule::new(Selector::Id("sip"), Patch::new().width(Length::FILL).height(Length::Fixed(60.0))),
            Rule::new(Selector::Id("ask"), Patch::new().width(Length::FILL).height(Length::Fixed(60.0))),
        ]
    })
}

fulgurite_reload::export_program!(program);

/// Let this build's blocked step 0 finish — the loader calls it after the
/// swap, so v1's worker completes into an inbox nobody drains any more.
#[no_mangle]
pub extern "C" fn chain_app_release() {
    RELEASED.store(true, Ordering::Release);
}

/// Let this build's blocked sipper go on past 5 — M88/T88.1. The loader
/// calls it after the swap, so v1's worker reports the rest of its stream
/// into a slot nobody reads any more.
#[no_mangle]
pub extern "C" fn chain_app_release_sip() {
    SIP_RELEASED.store(true, Ordering::Release);
}
