//! **A tick that writes the tree is marked** — found by M34/T34.0's
//! inventory (`docs/plan-boundaries-default.md`), asked in M33 §2.3.
//!
//! `App::tick` takes `&mut self`, so it may write any widget — showcase's
//! does: it writes the readout and reseeds the stage. `Host::tick` marked
//! nothing of its own, so under `Boundaries` a tick-written label kept
//! the box its previous text was laid out in.

use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Readout {
    #[widget_id("value")]
    value: Label<String>,
    #[widget_id("after")]
    after: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    readout: Readout,
    other: Label<String>,
}

#[derive(DeriveApp, Default)]
#[app(tick = advance)]
struct Anim {
    #[tree]
    ui: Ui,
    t: u32,
}

/// Writes the tree directly, as showcase's `advance` does.
fn advance(a: &mut Anim, dt: f32) -> bool {
    if dt > 0.0 {
        a.t += 1;
        a.ui.readout.value.set("x".repeat(a.t as usize));
    }
    true
}

fn host<I: Invalidation>() -> Host<Anim, Incremental, I> {
    let mut a = Anim::default();
    a.ui.readout.after.set("after".into());
    a.ui.other.set("other".into());
    let mut h: Host<Anim, Incremental, I> = Host::new(a, 300, 100);
    h.pump();
    h.pump();
    h
}

#[test]
fn a_tick_that_writes_a_label_draws_the_same_frame_under_both_strategies() {
    let mut e: Host<Anim, Incremental, Everything> = host();
    let mut b: Host<Anim, Incremental, Boundaries<256>> = host();
    for k in 1..=4 {
        e.tick(16.0);
        b.tick(16.0);
        e.pump();
        b.pump();
        let (he, hb) = (e.list.hash_from(0), b.list.hash_from(0));
        println!("tick {k}: {:?}  Everything {he:#x}  Boundaries {hb:#x}", b.app.ui.readout.value.as_str());
        assert_eq!(he, hb, "tick {k}: a tick-written label kept a stale box under Boundaries");
    }
}
