//! **A click that touches no arena must not re-run the arena cascade** —
//! M53/T53.0, the gate for T53.2 (R6-F3, `docs/plan-frame-cost.md` §1.2).
//!
//! Until T53.2 `pump_with` answered a handled event with `arenas_dirty =
//! true` "either way" — narrowed for a split application or not — and
//! every click ran `cascade_arenas_deep`, which asks every node in the
//! table for its arenas. Since T53.2 the pump asks only the marked nodes
//! (`style::cascade_arenas_marked`). T29.6 narrowed a split click's *layout*; the cascade sits
//! outside the stage sum every table published, so it was not seen
//! (`docs/results-deep-review.md`, R6-F3).
//!
//! The fixture is a **split** application with a stylesheet, a `Repeat`
//! (an arena owner) and, elsewhere, a `CheckBox` whose handler flips a
//! state field the binding writes back. Counts, not clocks
//! (`Host::pass_counts`): a click on the box must cascade 0 times in 40,
//! and a wheel over the repeater — which marks the repeater, so the
//! marked walk asks it — must still cascade 40 times in 40. (The rows do
//! not strictly need it: an arena's style row is per template. The wheel
//! is the accept direction: an owner inside the marks is reached.)

use fulgurite::event::EventCtx;
use fulgurite::bind::BindValue;
use fulgurite::invalidate::{Boundaries, Everything, Invalidation};
use fulgurite::prelude::*;
use fulgurite::repeat::Repeat;
use fulgurite::retain::Incremental;
use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::Event;
use fulgurite_shell::Host;
use fulgurite_widgets::prelude::*;

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
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid);
    let s = h.states[i];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

/// 40 clicks on a box nowhere near an arena. Red until T53.2 at 40/40.
#[test]
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

/// **The other direction.** A wheel over the repeater marks it, so the
/// marked walk reaches its arena: 40 of 40.
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

// ---------------------------------------------------------------------
// An arena that appears inside a narrowed range.
// ---------------------------------------------------------------------

/// What the sheet gives a row's cell. A `Label` is `Shrink`, and these
/// rows are never bound, so an unstyled cell is 0 px wide.
const CELL_W: f32 = 120.0;

/// Whether the repeater is shown — a newtype, so a value binding can
/// target a foreign type from it.
#[derive(Clone, Copy, Default, PartialEq)]
struct Shown(bool);

#[derive(Default)]
struct Toggled {
    on: bool,
    shown: Shown,
}

fn toggle(s: &mut Toggled, _: &EventCtx) {
    s.on = !s.on;
    s.shown = Shown(s.on);
}

impl BindValue<Shown> for Option<Repeat<Line>> {
    fn bind_value(&mut self, v: &Shown) -> bool {
        if self.is_some() == v.0 {
            return false;
        }
        *self = v.0.then(|| Repeat::new(20, 20.0, bind));
        true
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct OptUi {
    #[widget_id("check")]
    check: CheckBox<Toggled>,
    #[widget_id("maybe")]
    maybe: Option<Repeat<Line>>,
}

#[derive(DeriveApp, Default)]
struct OptApp {
    #[tree]
    ui: OptUi,
    #[state]
    #[bind(ui.check.on, on)]
    #[bind(ui.maybe, shown)]
    store: Toggled,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Empty {
    #[widget_id("empty")]
    text: Label,
}

#[derive(StaticTree)]
enum Body {
    Nothing(Empty),
    Rows(Repeat<Line>),
}

impl Default for Body {
    fn default() -> Self {
        Body::Nothing(Empty::default())
    }
}

impl BindValue<Shown> for Body {
    fn bind_value(&mut self, v: &Shown) -> bool {
        if matches!(self, Body::Rows(_)) == v.0 {
            return false;
        }
        *self = if v.0 { Body::Rows(Repeat::new(20, 20.0, bind)) } else { Body::default() };
        true
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct EnumUi {
    #[widget_id("check")]
    check: CheckBox<Toggled>,
    #[widget_id("body")]
    body: Body,
}

#[derive(DeriveApp, Default)]
struct EnumApp {
    #[tree]
    ui: EnumUi,
    #[state]
    #[bind(ui.check.on, on)]
    #[bind(ui.body, shown)]
    store: Toggled,
}

fn appearing_sheet() -> [Rule; 2] {
    [
        Rule::new(Selector::Id("check"), Patch::new().width(Length::Fixed(24.0)).height(Length::Fixed(24.0))),
        Rule::new(Selector::Id("cell"), Patch::new().width(Length::Fixed(CELL_W)).height(Length::Fixed(20.0))),
    ]
}

/// Click the box once on a fresh host of `A` under strategy `I`; return
/// the frame's marks and the first row's cell width, read by `cell`.
fn click_to_show<A, I>(mut app: A, cell: impl Fn(&A) -> Option<f32>) -> (fulgurite::invalidate::Marks, u64, Option<f32>)
where
    A: fulgurite::event::App,
    A::Tree: fulgurite::Widget,
    I: Invalidation,
    A: HasCheck,
{
    app.wire();
    let mut h: Host<A, Incremental, I> = Host::new(app, 320, 400);
    h.set_stylesheet(&appearing_sheet());
    h.pump();
    h.pump();
    assert!(cell(&h.app).is_none(), "setup: the repeater must start absent");
    let i = fulgurite::style::index_of_wid(<A::Tree as Widget>::SUB_TABLE, "check").expect("check");
    let s = h.states[i];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    let before = h.pass_counts();
    h.click(x, y);
    assert!(h.pump(), "setup: the click was not handled");
    let marks = h.last_marks();
    let cascades = h.pass_counts().since(before).cascades;
    h.pump();
    (marks, cascades, cell(&h.app))
}

trait HasCheck {
    fn wire(&mut self);
}
impl HasCheck for OptApp {
    fn wire(&mut self) {
        self.ui.check.on_toggle = toggle;
    }
}
impl HasCheck for EnumApp {
    fn wire(&mut self) {
        self.ui.check.on_toggle = toggle;
    }
}

fn opt_cell(a: &OptApp) -> Option<f32> {
    a.ui.maybe.as_ref().map(|r| r.instance_state(0, 1).size.w)
}

fn enum_cell(a: &EnumApp) -> Option<f32> {
    match &a.ui.body {
        Body::Rows(r) => Some(r.instance_state(0, 1).size.w),
        Body::Nothing(_) => None,
    }
}

fn check_appearing(what: &str, b: (fulgurite::invalidate::Marks, u64, Option<f32>), e: Option<f32>) {
    let (marks, cascades, w) = b;
    println!(
        "{what}: Boundaries marks {marks:?}, cascades {cascades}, first row's cell {w:?} px; \
         Everything {e:?} px (want {CELL_W})"
    );
    assert!(!marks.all, "setup: {what}'s click must be narrowed, or it tests the full walk: {marks:?}");
    assert_eq!(e, Some(CELL_W), "the oracle: under Everything the new rows are styled");
    assert_eq!(w, Some(CELL_W), "{what}: an arena that appeared inside a narrowed range was never cascaded");
}

/// An `Option<Repeat>` becoming `Some` through a split binding.
#[test]
fn an_option_that_becomes_a_repeater_styles_its_rows() {
    let b = click_to_show::<OptApp, Boundaries<256>>(OptApp::default(), opt_cell);
    let e = click_to_show::<OptApp, Everything>(OptApp::default(), opt_cell).2;
    check_appearing("Option -> Some(Repeat)", b, e);
}

/// An enum switched to the variant that holds a `Repeat`.
#[test]
fn an_enum_switched_to_a_repeater_styles_its_rows() {
    let b = click_to_show::<EnumApp, Boundaries<256>>(EnumApp::default(), enum_cell);
    let e = click_to_show::<EnumApp, Everything>(EnumApp::default(), enum_cell).2;
    check_appearing("enum -> Rows(Repeat)", b, e);
}
