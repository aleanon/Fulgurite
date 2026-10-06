//! M85/T85.4 — `Pop<S, W>`, and the arena refusal.
//!
//! Two claims, each printed beside its bound:
//!
//! 1. **A `Pop` in a tree is told when it scrolls into view.** Its own
//!    table entry carries `NodeDesc::WATCHES_VIEW`, the host lists it,
//!    and wheeling it into a `ScrollView`'s viewport runs `on_view` with
//!    `visible: true` exactly once, and `Pop::visible` answers. A click
//!    on its content reaches the content's handler and not `on_view`.
//!    (T85.3's pass is gated with a test-only leaf in
//!    `crates/lumen-static-shell/tests/pop_visibility.rs`; this is the
//!    widget on top of it.)
//! 2. **A `Pop` inside an arena is refused at construction** —
//!    `Repeat::new`, `List::new` (and `Default`) and `Dyn::new` panic with
//!    a sentence naming the alternative, because an arena's instances are
//!    not table nodes and the host's pass would never see them. The same
//!    row without the `Pop` constructs (the accept direction).

use lumen_static::event::{Event, EventCtx};
use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
use lumen_static::retain::Incremental;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::prelude::Repeat;
use lumen_static::{App as DeriveApp, Dyn, List, NodeDesc, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::{Button, Label, Spacer};
use lumen_static_widgets::{Pop, ScrollView};

// ---- 1. the widget, in a tree ---------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Content {
    #[widget_id("filler")]
    filler: Spacer,
    #[widget_id("pop")]
    pop: Pop<App, Button<App, &'static str>>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("scroll")]
    scroll: ScrollView<Content>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    loads: u32,
    hides: u32,
    presses: u32,
}

fn on_view(a: &mut App, ctx: &EventCtx) {
    match ctx.event {
        Event::Visibility { visible: true } => {
            a.loads += 1;
            // What loading looks like: the placeholder's own text. As
            // many characters as before, so its box does not move.
            a.ui.scroll.content.pop.content.label = "loaded!";
        }
        Event::Visibility { visible: false } => a.hides += 1,
        _ => {}
    }
}

fn press(a: &mut App, _: &EventCtx) {
    a.presses += 1;
}

const WINDOW: (u32, u32) = (300, 300);

fn sheet() -> Vec<Rule> {
    let fixed = |id: &'static str, w: f32, h: f32| {
        Rule::new(Selector::Id(id), Patch::new().width(Length::Fixed(w)).height(Length::Fixed(h)))
    };
    // A 100 px viewport in a 300 px window, the item 340 px down: below
    // the fold, and past the window's bottom too.
    vec![fixed("scroll", 200.0, 100.0), fixed("filler", 200.0, 340.0)]
}

fn app() -> App {
    let mut a = App::default();
    a.ui.scroll.content.pop = Pop::new(Button::default(), on_view);
    a.ui.scroll.content.pop.content.label = "waiting";
    a.ui.scroll.content.pop.content.on_click = press;
    a
}

fn rect<I: Invalidation>(h: &Host<App, Incremental, I>, wid: &str) -> [f32; 4] {
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid);
    let s = h.states[i];
    [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
}

fn meets(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0].max(b[0]) < a[2].min(b[2]) && a[1].max(b[1]) < a[3].min(b[3])
}

fn walk<I: Invalidation>(name: &str) {
    let pop = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "pop").expect("pop");
    let flagged = <Ui as Widget>::SUB_TABLE[pop].flags & NodeDesc::WATCHES_VIEW != 0;
    let mut h: Host<App, Incremental, I> = Host::new(app(), WINDOW.0, WINDOW.1);
    h.set_stylesheet(&sheet());
    h.pump();
    println!(
        "{name}: the pop is node {pop}, flagged WATCHES_VIEW {flagged}; host watches {} (bound 1)",
        h.watched_views()
    );
    assert_eq!(h.watched_views(), 1, "the host does not watch the Pop (its entry lacks WATCHES_VIEW)");
    let viewport = rect(&h, "scroll");
    let (x, y) = ((viewport[0] + viewport[2]) / 2.0, (viewport[1] + viewport[3]) / 2.0);
    assert!(!meets(rect(&h, "pop"), viewport), "setup: the pop starts in the viewport");
    assert_eq!(h.app.loads, 0, "a pop below the fold was told it is in view");

    // Wheel it in: `true` once, on the pump after the step that placed it.
    let mut entered = None;
    for step in 1..=40 {
        let before = h.app.ui.scroll.scroll;
        h.post(Event::Scroll { x, y, dy: -20.0 });
        let handled = h.pump();
        assert!(handled && h.app.ui.scroll.scroll > before, "{name} step {step}: the wheel did not scroll");
        if meets(rect(&h, "pop"), viewport) {
            entered = Some(step);
            // The delivery is the next pump, which the deadline asks for.
            let due = h.next_deadline_ms();
            let before_delivery = h.app.loads;
            h.pump();
            // The delivering frame's damage over the view is the
            // placeholder's box: the load repaints the pop and nothing else
            // there (the scroll was the frame before).
            let item = rect(&h, "pop");
            let over: Vec<[f32; 4]> = match h.damage() {
                lumen_static::retain::Damage::Regions(r) => {
                    r.as_slice().iter().copied().filter(|d| meets(*d, viewport)).collect()
                }
                lumen_static::retain::Damage::All => vec![[0.0, 0.0, WINDOW.0 as f32, WINDOW.1 as f32]],
                lumen_static::retain::Damage::None => Vec::new(),
            };
            let area = |r: [f32; 4]| (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0);
            let worst = over.iter().map(|r| area(*r)).fold(0.0, f32::max);
            let ratio = worst / area(item).max(1.0);
            println!(
                "  step {step}: loads before the delivering pump {before_delivery} (bound 0); its damage over the view {over:?}, the largest {worst} px² against the pop's {} px² = {ratio:.2}x (bound 1.00)",
                area(item)
            );
            assert!(!over.is_empty() && ratio <= 1.0 + 1e-3, "{name}: the load's frame damaged {ratio:.2}x the pop's box over the view");
            let inside = |d: &[f32; 4]| d[0] >= item[0] && d[1] >= item[1] && d[2] <= item[2] && d[3] <= item[3];
            assert!(over.iter().all(inside), "{name}: the load's frame damaged the view outside the pop {item:?}: {over:?}");
            assert_eq!(before_delivery, 0, "{name}: on_view ran in the frame that placed the pop, not the next");
            println!(
                "  step {step}: offset {} pop y {}..{} in the viewport; deadline {due:?}; loads {} visible {}",
                h.app.ui.scroll.scroll,
                rect(&h, "pop")[1],
                rect(&h, "pop")[3],
                h.app.loads,
                h.app.ui.scroll.content.pop.visible()
            );
            assert_eq!(due, Some(0.0), "{name}: the host did not ask for the delivering frame");
            break;
        }
    }
    assert!(entered.is_some(), "setup: the pop never reached the viewport");
    println!("  loads {} (bound 1), visible() {}", h.app.loads, h.app.ui.scroll.content.pop.visible());
    assert_eq!(h.app.loads, 1, "{name}: scrolling the pop into view did not run on_view once");
    assert!(h.app.ui.scroll.content.pop.visible(), "{name}: Pop::visible does not say it is in view");

    // A click on its content is the content's, never `on_view`'s.
    let b = rect(&h, "pop");
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    h.post(Event::PointerDown { x: cx, y: cy, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x: cx, y: cy, button: lumen_static::PointerButton::Left });
    let handled = h.pump();
    println!("  click at ({cx}, {cy}): handled {handled}, presses {} (bound 1), loads {} (bound 1)", h.app.presses, h.app.loads);
    assert!(handled && h.app.presses == 1, "{name}: the click did not reach the pop's content");
    assert_eq!(h.app.loads, 1, "{name}: a click ran the pop's on_view");

    // Wheel it out: `false` once.
    for _ in 0..40 {
        h.post(Event::Scroll { x, y, dy: 20.0 });
        h.pump();
        if h.app.hides > 0 || h.app.ui.scroll.scroll == 0.0 {
            h.pump();
            break;
        }
    }
    println!("  scrolled back to {}: hides {} (bound 1), visible() {}", h.app.ui.scroll.scroll, h.app.hides, h.app.ui.scroll.content.pop.visible());
    assert_eq!(h.app.hides, 1, "{name}: scrolling the pop out of view did not run on_view once");
    assert!(!h.app.ui.scroll.content.pop.visible(), "{name}: Pop::visible still says it is in view");
}

#[test]
fn a_pop_scrolled_into_view_runs_on_view_once() {
    walk::<Everything>("Everything");
    walk::<Boundaries<256>>("Boundaries<256>");
}

// ---- 2. the arena refusal -------------------------------------------

/// A row holding a `Pop` somewhere below its root.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct PopRow {
    name: Label<&'static str>,
    pop: Pop<(), Label<&'static str>>,
}

/// The same row without it: the accept direction.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct PlainRow {
    name: Label<&'static str>,
    image: Label<&'static str>,
}

#[test]
#[should_panic(expected = "a Repeat row cannot hold a node that watches its visibility")]
fn a_repeat_of_a_row_holding_a_pop_is_refused() {
    let _ = Repeat::<PopRow>::new(10, 20.0, lumen_static::repeat::noop_bind::<PopRow>);
}

#[test]
#[should_panic(expected = "a List item cannot hold a node that watches its visibility")]
fn a_list_of_rows_holding_a_pop_is_refused() {
    let _ = List::new(vec![PopRow::default()]);
}

#[test]
#[should_panic(expected = "a List item cannot hold a node that watches its visibility")]
fn a_default_list_of_rows_holding_a_pop_is_refused() {
    let _ = List::<PopRow>::default();
}

#[test]
#[should_panic(expected = "a Dyn island cannot hold a node that watches its visibility")]
fn a_dyn_island_holding_a_pop_is_refused() {
    let _ = Dyn::<()>::new(PopRow::default());
}

#[test]
fn the_same_row_without_a_pop_constructs_in_every_arena() {
    let r = Repeat::<PlainRow>::new(10, 20.0, lumen_static::repeat::noop_bind::<PlainRow>);
    let l = List::new(vec![PlainRow::default(), PlainRow::default()]);
    let d = Dyn::<()>::new(PlainRow::default());
    println!("Repeat count {}, List len {}, Dyn nodes {}", r.count, l.len(), d.node_count());
    assert_eq!((r.count, l.len(), d.node_count()), (10, 2, <PlainRow as Widget>::NODE_COUNT));
    // The refusal reads the row's table, and a `Pop` in the tree (not in
    // an arena) is the shape the catalogue and the showcase use.
    assert!(lumen_static::widget::table_has_flag(<PopRow as Widget>::SUB_TABLE, NodeDesc::WATCHES_VIEW));
    assert!(!lumen_static::widget::table_has_flag(<PlainRow as Widget>::SUB_TABLE, NodeDesc::WATCHES_VIEW));
}
