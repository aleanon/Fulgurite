//! **M47/T47.3's kill rule**: a no-op job from spawn to `apply`, under
//! the pool that ships (`Threads`) and under the thread-per-job spawner
//! it replaced (`PerJob`, below, the pre-T47.3 body verbatim).
//!
//! Written before the pool was judged (`docs/plan-timers-wake.md` §5): if
//! the pool's median is slower than the per-job median by more than the
//! per-job arm's own spread across eight runs, T47.3 ships only the
//! correction of the claim. The thread count is the gate either way.
//!
//! One iteration: `Host::spawn` a job whose body sends one `u64`, then
//! pump until it has been applied. So each row includes a pump of a
//! one-node tree and the ledger's insert and prune — the same on both
//! arms.
//!
//! Run: `cargo run --release -p benches --bin spawn -- pool|perjob`
//! (one arm per process, so each run starts a fresh pool).

use std::hint::black_box;

use benches::bench;
use lumen_static::event::App;
use lumen_static::inbox::Sender;
use lumen_static::prelude::*;
use lumen_static::spawn::{Descriptor, Job, Spawner, Threads};
use lumen_static::Rebuild;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    a: Fixed,
}

#[derive(Default)]
struct Work {
    ui: Ui,
    landed: u64,
}

impl App for Work {
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
        this.landed += m;
        true
    }
}

fn noop(d: Descriptor, tx: &Sender<u64>) {
    let _ = tx.send(d.payload);
}

/// The spawner `Threads` was until M47/T47.3: one `std::thread` per job.
/// Its body, minus the failure report a no-op job never reaches; the
/// fallback clone is kept because the old path paid for it on every job.
struct PerJob;

impl Spawner<u64> for PerJob {
    fn spawn(&mut self, job: Job<u64>, tx: Sender<u64>) {
        let fallback = tx.clone();
        let _ = std::thread::Builder::new().spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (job.job)(job.desc, &tx)
            }));
        });
        drop(fallback);
    }
}

fn arm(label: &str, spawner: Box<dyn Spawner<u64>>) {
    let mut h: Host<Work, Rebuild> = Host::new(Work::default(), 100, 50);
    h.set_spawner(spawner);
    h.pump();
    const JOB: Job<u64> = Job { desc: Descriptor { kind: 1, step: 0, payload: 1 }, job: noop, on_fail: None };
    bench(label, 20_000, || {
        let want = h.app.landed + 1;
        h.spawn(JOB);
        while h.app.landed < want {
            h.pump();
        }
        black_box(h.in_flight() as u64)
    });
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("pool") => arm("spawn->apply, pool (Threads)", Box::new(Threads::new())),
        Some("perjob") => arm("spawn->apply, thread per job", Box::new(PerJob)),
        _ => {
            eprintln!("usage: spawn pool|perjob");
            std::process::exit(2);
        }
    }
}
