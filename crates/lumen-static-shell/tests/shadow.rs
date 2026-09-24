//! **The shadow oracle** — M34/T34.1 (`docs/plan-boundaries-default.md`).
//!
//! A bounded host re-lays out every frame under an all-dirty view into
//! scratch state and compares. This checks the checker: it must count
//! the frames it compared (a zero means it never ran, and "no divergence"
//! then proves nothing), stay quiet on a correct session, and name the
//! node when a mark is wrong — on a frame that *can* show it. A consumed
//! press marks everything, so the injection targets a **hover**, which
//! only T25.0's precise mark covers.

use lumen_static::invalidate::Boundaries;
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::Event;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("title")]
    title: Label<String>,
    #[widget_id("go")]
    go: Button<Tally>,
}

#[derive(Default)]
struct Tally {
    n: u32,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    #[state]
    #[bind(ui.title, n, "{} presses")]
    tally: Tally,
}

fn host() -> Host<App, Incremental, Boundaries<16>> {
    let mut a = App::default();
    a.ui.go.label = "go";
    a.ui.go.on_click = |t: &mut Tally, _: &lumen_static::EventCtx| t.n += 1;
    let mut h: Host<App, Incremental, Boundaries<16>> = Host::new(a, 300, 120);
    // A hover shade that differs from rest, or a stale hover is invisible.
    h.set_stylesheet(&[Rule::new(Selector::Kind("button"), Patch::new().hover_dim(0.3))]);
    h.pump();
    h.pump();
    h
}

#[test]
fn a_correct_session_is_compared_and_quiet() {
    let mut h = host();
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "go").unwrap();
    let s = h.states[i];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::PointerMove { x, y });
    h.pump();
    assert!(h.states[i].hovered, "setup: the move did not hover the button");
    h.click(x, y);
    h.pump();
    h.post(Event::PointerMove { x: 290.0, y: 110.0 });
    h.pump();
    let r = h.shadow_report();
    println!("compared {} frames, first divergence {:?}, presses {}", r.compared, r.first, h.app.tally.n);
    assert!(r.compared > 0, "the shadow oracle never ran — 'no divergence' would prove nothing");
    assert_eq!(h.app.tally.n, 1, "setup: the click did not land");
    assert!(r.first.is_none(), "a correct session diverged: {:?}", r.first);
}
