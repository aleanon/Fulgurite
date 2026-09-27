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

use core::sync::atomic::{AtomicBool, Ordering};

use lumen_static::event::{App, EventCtx};
use lumen_static::inbox::Sender;
use lumen_static::spawn::{Descriptor, JobFn, Outbox};
use lumen_static::style::{Edges, Length, Patch, Rule, Selector};
use lumen_static::{Reloadable as DeriveReloadable, StaticTree};
use lumen_static_shell::program::{application, Program};
use lumen_static_widgets::prelude::*;

/// The chain's kind. Stable, append-only (`docs/plan-tasks.md` §2.1).
const KIND: u64 = 9;

#[derive(StaticTree, Default, DeriveReloadable)]
#[widget(column, spacing = 8.0)]
pub struct Ui {
    #[widget_id("start")]
    pub start: Button<Work>,
    #[widget_id("result")]
    pub result: Label<String>,
}

/// What a handler and `apply` may touch (M33). Not kept: the outbox is
/// the host's plumbing and the result is shown from the chain itself.
#[derive(Default)]
pub struct Work {
    outbox: Outbox,
    result: Option<u64>,
}

/// Hand-written `App`, because the derive cannot express a real `Msg`
/// (as `examples/tasks-demo`). So there is no `#[tree]` attribute to
/// write: the tree is an untagged field, which the `Reloadable` derive
/// restores as a nested widget, as it restores a `#[tree]`.
#[derive(DeriveReloadable)]
pub struct Chain {
    pub ui: Ui,
    #[reload(fresh)]
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

fn step0(d: Descriptor, tx: &Sender<u64>) {
    #[cfg(not(feature = "v2"))]
    {
        // Bounded, so a gate that never releases does not hang a worker
        // for ever — it fails on its own timeout first.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !RELEASED.load(Ordering::Acquire) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let _ = tx.send(pack(0, d.payload + 1));
    }
    #[cfg(feature = "v2")]
    {
        let _ = &RELEASED;
        let _ = tx.send(pack(0, d.payload + 100));
    }
}

fn step1(d: Descriptor, tx: &Sender<u64>) {
    let _ = tx.send(pack(1, d.payload * 10));
}

impl App for Chain {
    type State = Work;
    fn state_mut(&mut self) -> &mut Work {
        &mut self.work
    }
    const STATE_IS_SPLIT: bool = true;
    type Tree = Ui;
    type Msg = u64;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.work.outbox)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        match (d.kind, d.step) {
            (KIND, 0) => Some(step0),
            (KIND, 1) => Some(step1),
            _ => None,
        }
    }
    /// The step function, in the currently loaded build.
    fn apply(w: &mut Work, m: u64) -> bool {
        let (step, v) = (m >> 32, m & 0xffff_ffff);
        match step {
            0 => {
                w.outbox.push(Descriptor { kind: KIND, step: 1, payload: v });
            }
            _ => w.result = Some(v),
        }
        true
    }
    fn update(&mut self) -> bool {
        match self.work.result {
            Some(v) => lumen_static::bind::format_into(&mut self.ui.result, format_args!("chain {v}")),
            None => lumen_static::bind::format_into(&mut self.ui.result, format_args!("chain -")),
        }
    }
}

fn on_start(w: &mut Work, _: &EventCtx) {
    w.outbox.push(Descriptor::new(KIND, 4));
}

fn boot() -> Chain {
    let mut c = Chain { ui: Ui::default(), work: Work::default() };
    c.ui.start.label = "start";
    c.ui.start.on_click = on_start;
    c
}

pub fn program() -> Program<Chain> {
    application(boot).title("chain-app").size(240, 120).style(|_: &Chain| {
        vec![
            Rule::new(Selector::Kind("Ui"), Patch::new().padding(Edges::all(0.0))),
            Rule::new(Selector::Id("start"), Patch::new().width(Length::FILL).height(Length::Fixed(60.0))),
        ]
    })
}

lumen_static_reload::export_program!(program);

/// Let this build's blocked step 0 finish — the loader calls it after the
/// swap, so v1's worker completes into an inbox nobody drains any more.
#[no_mangle]
pub extern "C" fn chain_app_release() {
    RELEASED.store(true, Ordering::Release);
}
