//! **A handled event narrows, for a split application** — M29/T29.6
//! (`docs/plan-damage-precision.md`).
//!
//! The invalidation oracle over a **click-driven** session: every frame
//! byte-identical under `Everything` and `Boundaries`. The session covers
//! every way an event can change the tree without a binding: a
//! `TextInput` taking characters, a `Slider` moving on a track press, two
//! overlays closed by one press outside, and a button whose handler
//! changes a bound label *outside* the button's subtree. Each step prints
//! what the `Boundaries` frame marked, so a narrowed frame is told apart
//! from one that fell back to everything.

use lumen_static::event::EventCtx;
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::Event;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::select::{Row, Select};
use lumen_static_widgets::slider::Slider;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Top {
    #[widget_id("formula")]
    formula: Label<String>,
    #[widget_id("status")]
    status: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Picks {
    #[widget_id("s1")]
    s1: Select<Store>,
    #[widget_id("s2")]
    s2: Select<Store>,
}

/// **The field sizes to its content, with a label after it.** The first
/// version gave the field a fixed width, and an injection that stopped
/// marking the consumed field left every frame identical: a fixed box
/// does not move when its text grows, so a skipped layout was invisible.
/// Here a stale field box leaves `after` where a shorter text put it.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Entry {
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("after")]
    after: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct Ui {
    top: Top,
    entry: Entry,
    #[widget_id("slider")]
    slider: Slider,
    picks: Picks,
    #[widget_id("go")]
    go: Button<Store>,
}

#[derive(Default)]
struct Store {
    n: i64,
    a_open: bool,
    b_open: bool,
}

#[derive(DeriveApp, Default)]
struct Split {
    #[tree]
    ui: Ui,
    #[state]
    #[bind(ui.top.formula, n, "= {}")]
    #[bind(ui.picks.s1.menu.open, a_open)]
    #[bind(ui.picks.s2.menu.open, b_open)]
    store: Store,
}

fn row(r: &mut Row<Store>, i: usize) {
    r.label = format!("option {i}");
}

fn build() -> Split {
    let mut s = Split::default();
    s.ui.top.status.set("Sheet1".into());
    s.ui.entry.after.set("after".into());
    s.ui.picks.s1 = Select::new(3, 20.0, row);
    s.ui.picks.s2 = Select::new(3, 20.0, row);
    s.ui.picks.s1.on_dismiss = |s: &mut Store, _: &EventCtx| s.a_open = false;
    s.ui.picks.s2.on_dismiss = |s: &mut Store, _: &EventCtx| s.b_open = false;
    s.ui.go.on_click = |s: &mut Store, _: &EventCtx| s.n = 1_234_567;
    s
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("field"), Patch::new().width(Length::Shrink).height(Length::Fixed(24.0))),
        Rule::new(Selector::Id("slider"), Patch::new().width(Length::Fixed(200.0)).height(Length::Fixed(20.0))),
        Rule::new(Selector::Kind("select"), Patch::new().width(Length::Fixed(90.0))),
        Rule::new(Selector::Kind("repeat"), Patch::new().width(Length::Fixed(90.0)).height(Length::Fixed(60.0))),
        Rule::new(Selector::Id("go"), Patch::new().width(Length::Fixed(60.0)).height(Length::Fixed(24.0))),
    ]
}

fn host<A: lumen_static::event::App, I: Invalidation>(app: A) -> Host<A, Incremental, I> {
    let mut h: Host<A, Incremental, I> = Host::new(app, 480, 360);
    h.set_stylesheet(&sheet());
    h.pump();
    h.pump();
    h
}

fn centre<A: lumen_static::event::App, I: Invalidation>(h: &Host<A, Incremental, I>, wid: &str) -> (f32, f32) {
    let i = lumen_static::style::index_of_wid(<A::Tree as Widget>::SUB_TABLE, wid).expect(wid);
    let s = h.states[i];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

struct Pair {
    e: Host<Split, Incremental, Everything>,
    b: Host<Split, Incremental, Boundaries<16>>,
}

impl Pair {
    fn step(&mut self, what: &str, f: impl Fn(&mut dyn FnMut(Event))) {
        f(&mut |ev| self.e.post(ev));
        f(&mut |ev| self.b.post(ev));
        self.e.pump();
        self.b.pump();
        let (he, hb) = (self.e.list.hash_from(0), self.b.list.hash_from(0));
        println!("{what:<34} Everything {he:#018x}  Boundaries {hb:#018x}  marks {:?}", self.b.last_marks());
        assert_eq!(he, hb, "{what}: the narrowed frame differs from the whole-tree frame");
    }
}

#[test]
fn a_click_driven_session_narrows_and_draws_the_same_frames() {
    let mut p = Pair { e: host(build()), b: host(build()) };

    // 1. Focus the field and type — the field writes itself.
    let (fx, fy) = centre(&p.b, "field");
    p.step("focus the field", |post| {
        post(Event::down(fx, fy));
        post(Event::up(fx, fy));
    });
    for (k, ch) in "1234567 wide".chars().enumerate() {
        p.step(&format!("type {ch:?} ({k})"), |post| post(Event::Text { ch }));
        assert!(!p.b.last_marks().all, "typing into a split application marked everything");
    }
    assert_eq!(p.b.app.ui.entry.field.as_str(), "1234567 wide", "setup: the typing did not land");

    // 2. Press the slider's track at three quarters — the slider moves itself.
    let si = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "slider").unwrap();
    let s = p.b.states[si];
    let (sx, sy) = (s.pos.x + s.size.w * 0.75, s.pos.y + s.size.h / 2.0);
    let before = p.b.app.ui.slider.value();
    p.step("press the slider's track", |post| {
        post(Event::down(sx, sy));
        post(Event::up(sx, sy));
    });
    assert_ne!(p.b.app.ui.slider.value(), before, "setup: the slider did not move");

    // 3. Open both selects (host-driven), then close both with one press outside.
    p.e.app.store.a_open = true;
    p.e.app.store.b_open = true;
    p.b.app.store.a_open = true;
    p.b.app.store.b_open = true;
    p.step("open both selects", |_| {});
    p.step("(settle)", |_| {});
    assert!(p.b.app.ui.picks.s1.is_open() && p.b.app.ui.picks.s2.is_open(), "setup: not both open");
    p.step("press outside both", |post| {
        post(Event::down(470.0, 350.0));
        post(Event::up(470.0, 350.0));
    });
    p.step("(the bindings close them)", |_| {});
    assert!(
        !p.b.app.ui.picks.s1.is_open() && !p.b.app.ui.picks.s2.is_open(),
        "setup: one press outside did not close both"
    );

    // 4. A button whose handler changes a bound label outside its subtree.
    let (gx, gy) = centre(&p.b, "go");
    p.step("click go (writes #formula)", |post| {
        post(Event::down(gx, gy));
        post(Event::up(gx, gy));
    });
    assert_eq!(p.b.app.ui.top.formula.as_str(), "= 1234567", "setup: the handler's write did not render");
    assert!(!p.b.last_marks().all, "a split application's click marked everything");
}

// ---- the unsplit twin -------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct UUi {
    top: Top,
    #[widget_id("go")]
    go: Button<Unsplit>,
}

/// No `#[state]`: its handler writes a tree label directly, which only
/// `mark_all` can account for.
#[derive(DeriveApp, Default)]
struct Unsplit {
    #[tree]
    ui: UUi,
}

#[test]
fn an_unsplit_handler_that_writes_the_tree_still_draws_the_right_frame() {
    let mut app = Unsplit::default();
    app.ui.go.on_click = |u: &mut Unsplit, _: &EventCtx| u.ui.top.formula.set("written by a handler".into());
    let mut e: Host<Unsplit, Incremental, Everything> = host(app);
    let mut app = Unsplit::default();
    app.ui.go.on_click = |u: &mut Unsplit, _: &EventCtx| u.ui.top.formula.set("written by a handler".into());
    let mut b: Host<Unsplit, Incremental, Boundaries<16>> = host(app);
    let (gx, gy) = centre(&b, "go");
    e.click(gx, gy);
    b.click(gx, gy);
    e.pump();
    b.pump();
    assert_eq!(b.app.ui.top.formula.as_str(), "written by a handler", "setup: the click did not land");
    assert!(b.last_marks().all, "an unsplit handler's click must mark everything");
    assert_eq!(e.list.hash_from(0), b.list.hash_from(0), "the unsplit twin's frame is stale");
}
