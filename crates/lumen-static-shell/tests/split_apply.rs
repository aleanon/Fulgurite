//! **A deferred message gets what a handler gets** — M33/T33.2
//! (`docs/plan-state-split.md` §2.3).
//!
//! A split application: the tree and a `Work` state are siblings, `apply`
//! receives `&mut Work`, and the outbox lives in the state, forwarded by
//! `App::outbox`. A three-step chain must complete through it. The other
//! direction — an `apply` that writes the tree does not compile — is
//! `crates/lumen-static-macros/tests/ui/fail_split_apply_reaches_tree.rs`.

use lumen_static::event::App;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, JobFn, Outbox, Queued};
use lumen_static::Rebuild;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

const KIND: u64 = 7;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

/// What `apply` may touch: no tree in here.
#[derive(Default)]
struct Work {
    outbox: Outbox,
    seen: Vec<(u64, u64)>,
}

#[derive(Default)]
struct Split {
    ui: Ui,
    state: Work,
}

fn step0(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send(d.payload + 1);
}
fn step1(d: Descriptor, tx: &lumen_static::inbox::Sender<u64>) {
    let _ = tx.send((1 << 32) | (d.payload * 10));
}

impl App for Split {
    type State = Work;
    fn state_mut(&mut self) -> &mut Work {
        &mut self.state
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
    fn apply(s: &mut Work, m: u64) -> bool {
        let (step, out) = (m >> 32, m & 0xffff_ffff);
        s.seen.push((step, out));
        if step == 0 {
            s.outbox.push(Descriptor { kind: KIND, step: 1, payload: out });
        }
        true
    }
    fn outbox(&mut self) -> Option<&mut Outbox> {
        Some(&mut self.state.outbox)
    }
    fn job_for(d: Descriptor) -> Option<JobFn<u64>> {
        match (d.kind, d.step) {
            (KIND, 0) => Some(step0),
            (KIND, 1) => Some(step1),
            _ => None,
        }
    }
}

#[test]
fn a_chain_completes_through_a_split_applications_apply() {
    let mut h: Host<Split, Rebuild> = Host::new(Split::default(), 100, 50);
    h.set_spawner(Box::new(Queued::<u64>::default()));
    h.app.state.outbox.push(Descriptor::new(KIND, 4));
    for _ in 0..6 {
        h.pump();
        h.run_pending_jobs();
    }
    println!("seen {:?}", h.app.state.seen);
    assert_eq!(
        h.app.state.seen,
        vec![(0, 5), (1, 50)],
        "both steps must land through `apply(&mut Work, _)` — the state, not the application"
    );
}
