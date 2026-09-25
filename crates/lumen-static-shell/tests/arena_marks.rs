//! **A click that touches no arena must not re-run the arena cascade** —
//! M53/T53.0, the gate for T53.2 (R6-F3, `docs/plan-frame-cost.md` §1.2).
//!
//! `pump_with` answers a handled event with `arenas_dirty = true` "either
//! way" — narrowed for a split application or not — so every click runs
//! `cascade_arenas_deep`, which asks every node in the table for its
//! arenas. T29.6 narrowed a split click's *layout*; the cascade sits
//! outside the stage sum every table published, so it was not seen
//! (`docs/results-deep-review.md`, R6-F3).
//!
//! The fixture is a **split** application with a stylesheet, a `Repeat`
//! (an arena owner) and, elsewhere, a `CheckBox` whose handler flips a
//! state field the binding writes back. Counts, not clocks
//! (`Host::pass_counts`): a click on the box must cascade 0 times in 40,
//! and a wheel over the repeater — which moves its window, so its new
//! rows need styling — must still cascade 40 times in 40.

use lumen_static::event::EventCtx;
use lumen_static::invalidate::Boundaries;
use lumen_static::prelude::*;
use lumen_static::repeat::Repeat;
use lumen_static::retain::Incremental;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::Event;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Line {
    #[widget_id("cell")]
    cell: Label,
}

fn bind(_: &mut Line, _: usize) {}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct Ui {
    #[widget_id("check")]
    check: CheckBox<Store>,
    #[widget_id("rows")]
    rows: Repeat<Line>,
}

#[derive(Default)]
struct Store {
    on: bool,
}

#[derive(DeriveApp, Default)]
struct Split {
    #[tree]
    ui: Ui,
    #[state]
    #[bind(ui.check.on, on)]
    store: Store,
}

fn build() -> Split {
    let mut s = Split::default();
    s.ui.check.on_toggle = |s: &mut Store, _: &EventCtx| s.on = !s.on;
    s.ui.rows = Repeat::new(1000, 20.0, bind);
    s
}

fn host() -> Host<Split, Incremental, Boundaries<256>> {
    let mut h: Host<Split, Incremental, Boundaries<256>> = Host::new(build(), 320, 240);
    h.set_stylesheet(&[
        Rule::new(Selector::Id("check"), Patch::new().width(Length::Fixed(24.0)).height(Length::Fixed(24.0))),
        Rule::new(Selector::Id("rows"), Patch::new().width(Length::Fixed(200.0)).height(Length::Fixed(120.0))),
        // An arena rule: `#cell` lives in the repeater's row template,
        // which only the arena cascade reaches.
        Rule::new(Selector::Id("cell"), Patch::new().height(Length::Fixed(20.0))),
    ]);
    h.pump();
    h.pump();
    h
}

fn centre(h: &Host<Split, Incremental, Boundaries<256>>, wid: &str) -> (f32, f32) {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid);
    let s = h.states[i];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

/// **Red until T53.2.** 40 clicks on a box nowhere near an arena.
#[test]
#[ignore = "red until T53.2: a handled click keeps arenas_dirty and re-runs the arena cascade (R6-F3)"]
fn a_click_that_touches_no_arena_does_not_cascade() {
    let mut h = host();
    let (x, y) = centre(&h, "check");
    let before = h.pass_counts();
    let mut landed = 0;
    for _ in 0..40 {
        let was = h.app.store.on;
        h.post(Event::down(x, y));
        h.post(Event::up(x, y));
        if h.pump() && h.app.store.on != was {
            landed += 1;
        }
    }
    let d = h.pass_counts().since(before);
    println!("clicks: landed {landed}/40, cascades {}/40 (bound 0), pumps {}", d.cascades, d.pumps);
    assert_eq!(landed, 40, "setup: every click must toggle the box");
    assert_eq!(d.cascades, 0, "a click on #check re-ran the arena cascade {} times in 40", d.cascades);
}

/// **The other direction, and green today.** A wheel over the repeater
/// moves its window, so the new rows are styled by the arena cascade:
/// 40 of 40.
#[test]
fn a_wheel_over_the_repeater_still_cascades() {
    let mut h = host();
    let (x, y) = centre(&h, "rows");
    let before = h.pass_counts();
    let mut handled = 0;
    for _ in 0..40 {
        h.post(Event::Scroll { x, y, dy: -20.0 });
        if h.pump() {
            handled += 1;
        }
    }
    let d = h.pass_counts().since(before);
    println!(
        "wheels: handled {handled}/40, cascades {}/40 (bound 40), scroll {:.0} px",
        d.cascades,
        h.app.ui.rows.scroll
    );
    assert_eq!(handled, 40, "setup: every wheel must be consumed by the repeater");
    assert!(h.app.ui.rows.scroll > 0.0, "setup: the repeater did not move");
    assert_eq!(d.cascades, 40, "a wheel over the repeater must re-style its new rows every time");
}
