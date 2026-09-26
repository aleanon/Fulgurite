//! **A handled event narrows, for a split application** — M29/T29.6
//! (`docs/plan-damage-precision.md`).
//!
//! The invalidation oracle over a **click-driven** session: every frame
//! byte-identical under `Everything` and `Boundaries`. The session covers
//! every way an event can change the tree without a binding: a
//! `TextInput` taking characters and arrows (beside a button, so the
//! arrow-group rule is in play, M44/T44.1), a `Slider` moving on a track press, two
//! overlays closed by one press outside, a drag from one button to
//! another that activates neither (M44/T44.2), a slider hidden mid-drag
//! that stops tracking (M44/T44.3), a tap by touch (M44/T44.4), and a
//! button whose handler
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
    /// A focusable sibling, so the field is in an arrow group of two:
    /// the shape in which the group rule used to take Left from the
    /// field (M44/T44.1).
    #[widget_id("clear")]
    clear: Button<Store>,
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
    s.ui.entry.clear.label = "clear";
    // A handler of its own, so a release on `clear` that activated would
    // be seen: without one, a drag from `go` ending here fired nothing
    // under the unconditional activation T44.2 replaced, and the step
    // could not tell the two apart.
    s.ui.entry.clear.on_click = |s: &mut Store, _: &EventCtx| s.n = -1;
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
        Rule::new(Selector::Id("clear"), Patch::new().width(Length::Fixed(50.0)).height(Length::Fixed(24.0))),
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
    // A flag, typed and then deleted by one Backspace — M45/T45.5: the
    // grapheme step, narrowed, draws the same frame as the whole walk.
    for ch in ['\u{1f1f3}', '\u{1f1f4}'] {
        p.step(&format!("type {ch:?}"), |post| post(Event::Text { ch }));
    }
    p.step("Backspace over the flag", |post| {
        post(Event::Key { key: lumen_static::Key::Backspace, down: true })
    });
    assert!(!p.b.last_marks().all, "a Backspace in a split application marked everything");
    assert_eq!(p.b.app.ui.entry.field.as_str(), "1234567 wide", "one Backspace did not delete the whole flag");
    // A paste — M45/T45.7: one edit outside dispatch, marked as the
    // focused field's subtree, narrowed for a split application.
    let (ne, nb) = (p.e.paste("!?"), p.b.paste("!?"));
    let (he, hb) = (p.e.list.hash_from(0), p.b.list.hash_from(0));
    println!("paste \"!?\" (kept {ne}/{nb})          Everything {he:#018x}  Boundaries {hb:#018x}  marks {:?}", p.b.last_marks());
    assert_eq!((ne, nb), (2, 2), "setup: the paste did not land");
    assert_eq!(he, hb, "paste: the narrowed frame differs from the whole-tree frame");
    assert!(!p.b.last_marks().all, "a paste into a split application marked everything");
    for k in 0..2 {
        p.step(&format!("Backspace ({k})"), |post| post(Event::Key { key: lumen_static::Key::Backspace, down: true }));
    }
    assert_eq!(p.b.app.ui.entry.field.as_str(), "1234567 wide", "setup: the paste was not undone");

    // 1b. Arrows in a field beside a button stay in the field (M44/T44.1),
    // and the field records them as consumed.
    let field = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "field").unwrap() as u32;
    for k in 0..3 {
        let before = p.b.app.ui.entry.field.caret();
        p.step(&format!("Left in the field ({k})"), |post| post(Event::Key { key: lumen_static::Key::Left, down: true }));
        println!("    (focus, caret) = {:?}, expected {:?}", (p.b.input.focus, p.b.app.ui.entry.field.caret()), (Some(field), before - 1));
        assert_eq!(p.b.input.focus, Some(field), "setup: Left left the field for its sibling");
        assert_eq!(p.b.app.ui.entry.field.caret(), before - 1, "setup: Left did not move the caret");
        assert!(!p.b.last_marks().all, "an arrow in a split application marked everything");
    }

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

    // 2b. A slider hidden mid-drag loses the capture (M44/T44.3): the
    // move after it is consumed by nobody and changes nothing, and the
    // frame still matches `Everything`.
    p.step("press the slider to drag", |post| post(Event::down(sx, sy)));
    assert_eq!(p.b.input.capture, Some(si as u32), "setup: the press did not capture the slider");
    let mut hidden = sheet();
    hidden.push(Rule::new(Selector::Id("slider"), Patch::new().visible(false)));
    p.e.set_stylesheet(&hidden);
    p.b.set_stylesheet(&hidden);
    p.step("hide the slider", |_| {});
    let held = p.b.app.ui.slider.value();
    p.step("move while hidden", |post| post(Event::PointerMove { x: sx - 100.0, y: sy }));
    println!(
        "    value {held} -> {} (expected unchanged); capture {:?} (expected None)",
        p.b.app.ui.slider.value(),
        p.b.input.capture
    );
    assert_eq!(p.b.app.ui.slider.value(), held, "a hidden slider kept tracking the pointer");
    assert_eq!(p.b.input.capture, None, "a hidden slider kept the capture");
    p.e.set_stylesheet(&sheet());
    p.b.set_stylesheet(&sheet());
    p.step("show it again, release", |post| post(Event::up(sx - 100.0, sy)));

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

    // 3b. A drag from one button to another activates neither
    // (M44/T44.2), and the press shade clears under `Boundaries` as it
    // does under `Everything` -- T25.0's pointer marks must cover it.
    let (gx, gy) = centre(&p.b, "go");
    let (cx, cy) = centre(&p.b, "clear");
    let clear = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "clear").unwrap() as u32;
    assert_eq!(p.b.probe(cx, cy).map(|h| h.node), Some(clear), "setup: the release point is not on clear");
    p.step("press go", |post| post(Event::down(gx, gy)));
    let go = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "go").unwrap();
    assert!(p.b.states[go].pressed, "setup: the press did not land on go");
    p.step("release on clear", |post| post(Event::up(cx, cy)));
    println!("    n = {} (0: neither ran; 1234567: go's; -1: clear's); go pressed {}", p.b.app.store.n, p.b.states[go].pressed);
    assert_eq!(p.b.app.store.n, 0, "a drag from go released on clear activated a button");
    assert!(!p.b.states[go].pressed, "the release left go's press shade");

    // 3c. A tap by touch clicks (M44/T44.4): the primary touch drives
    // the pointer, and the frame narrows as a mouse click does.
    p.step("tap clear by touch", |post| {
        post(Event::Touch { id: 1, x: cx, y: cy, phase: lumen_static::event::TouchPhase::Begin });
        post(Event::Touch { id: 1, x: cx, y: cy, phase: lumen_static::event::TouchPhase::End });
    });
    println!("    n = {} (expected -1, clear's handler)", p.b.app.store.n);
    assert_eq!(p.b.app.store.n, -1, "setup: a touch tap on clear did not click it");
    assert!(!p.b.last_marks().all, "a touch tap in a split application marked everything");

    // 4. A button whose handler changes a bound label outside its subtree.
    let (gx, gy) = centre(&p.b, "go");
    p.step("click go (writes #formula)", |post| {
        post(Event::down(gx, gy));
        post(Event::up(gx, gy));
    });
    assert_eq!(p.b.app.ui.top.formula.as_str(), "= 1234567", "setup: the handler's write did not render");
    assert!(!p.b.last_marks().all, "a split application's click marked everything");

    // Last, because a field eighty characters wider pushes `clear` out
    // of the window.
    // 5. More characters than the queue holds, in one call (M44/T44.5):
    // a paste chunks against the queue, nothing is dropped, and every
    // frame it pumped narrows to the same picture as `Everything`.
    let (fx, fy) = centre(&p.b, "field");
    p.step("focus the field again", |post| {
        post(Event::down(fx, fy));
        post(Event::up(fx, fy));
    });
    let long: String = (0..80).map(|k| (b'a' + (k % 26) as u8) as char).collect();
    let before = p.b.app.ui.entry.field.as_str().len();
    let (ne, nb) = (p.e.paste(&long), p.b.paste(&long));
    let (he, hb) = (p.e.list.hash_from(0), p.b.list.hash_from(0));
    println!(
        "paste 80 chars (queue CAP {}): delivered {ne} / {nb}; field +{} chars; dropped {} / {}; hashes {he:#018x} / {hb:#018x}",
        lumen_static::InputQueue::<64>::CAP,
        p.b.app.ui.entry.field.as_str().len() - before,
        p.e.dropped_inputs(),
        p.b.dropped_inputs()
    );
    assert_eq!(p.b.app.ui.entry.field.as_str().len() - before, 80, "setup: the paste did not land whole");
    assert_eq!((p.e.dropped_inputs(), p.b.dropped_inputs()), (0, 0), "a paste lost characters to the queue");
    assert!(!p.b.last_marks().all, "a paste into a split application marked everything");
    assert_eq!(he, hb, "after the paste the narrowed frame differs from the whole-tree frame");
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
