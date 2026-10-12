//! M27/T27.9 — `OptionList`, `PickList`, `Combobox`, `Menu`.
//!
//! One type with three presentations, and the difference between them is
//! exactly two things: whether there is a trigger, and whether typing in
//! it filters.

// on_event only: these test the option logic and filtering, one call deep.
// What the pass delivers -- a choice closing the list, Escape, the keyboard
// -- is crates/fulgurite-shell/tests/chooser_closes.rs, and
// scripts/drive/widgets/choose.py drives it live. (M57/D2)

use fulgurite::event::{Event, EventCtx, Key};
use fulgurite::widget::{Node, Widget};
use fulgurite::{Constraints, NodeState, Point};
use fulgurite_widgets::{Combobox, Menu, OptionList, PickList, MAX_OPTIONS};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };
const OPTS: [&str; 4] = ["Apple", "Banana", "Cherry", "apricot"];

fn lay<W: Widget + Node>(w: &mut W) -> Vec<NodeState> {
    let styles = fulgurite::pass::default_styles(w);
    let mut st = vec![NodeState::default(); <W as Widget>::NODE_COUNT];
    let mut m = fulgurite::cx::BlockMetrics;
    {
        let mut cx = fulgurite::LayoutCx::new(&mut m);
        Node::layout(w, &mut cx, VIEW, &styles, &mut st, 0);
    }
    Node::place(w, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    st
}

fn release_at<W: Widget + Node>(w: &mut W, y: f32) -> bool {
    let ev = Event::PointerUp { x: 0.0, y: 0.0, button: fulgurite::PointerButton::Left };
    let mut cx = EventCtx::for_event(0, ev);
    cx.local = Point { x: 10.0, y };
    w.on_event(&ev, &cx)
}

// ---- OptionList ------------------------------------------------------

/// **A release chooses, a press does not.** Every menu in every toolkit
/// commits on release; choosing on the press also closes the list under
/// a finger that is still down.
#[test]
fn a_release_chooses_and_a_press_does_not() {
    let mut l = OptionList::new(&OPTS);
    lay(&mut l);
    let ev = Event::PointerDown {
        x: 0.0,
        y: 0.0,
        button: fulgurite::PointerButton::Left,
        clicks: 1,
    };
    let mut cx = EventCtx::for_event(0, ev);
    cx.local = Point { x: 10.0, y: 40.0 };
    l.on_event(&ev, &cx);
    assert_eq!(l.take_picked(), None, "a press chose a row");
    release_at(&mut l, 40.0);
    assert_eq!(l.take_picked(), Some(1), "a release did not choose the second row");
}

/// Taken once: a choice served twice is two selections from one press.
#[test]
fn a_choice_is_taken_once() {
    let mut l = OptionList::new(&OPTS);
    lay(&mut l);
    release_at(&mut l, 10.0);
    assert_eq!(l.take_picked(), Some(0));
    assert_eq!(l.take_picked(), None, "the same choice was served twice");
}

/// The filter is ASCII-case-insensitive, and it says so rather than
/// pretending to be Unicode-correct.
#[test]
fn the_filter_ignores_ascii_case_and_matches_anywhere() {
    let mut l = OptionList::new(&OPTS);
    l.filter("ap");
    assert_eq!(l.shown(), 2, "`ap` should match Apple and apricot, got {}", l.shown());
    l.filter("RR");
    assert_eq!(l.shown(), 1, "`RR` should match Cherry, got {}", l.shown());
}

/// **An empty filter shows everything.** One that hid every row when the
/// field was cleared would make a combobox unusable the moment anyone
/// deleted their query.
#[test]
fn an_empty_filter_shows_everything() {
    let mut l = OptionList::new(&OPTS);
    l.filter("zzz");
    assert_eq!(l.shown(), 0, "setup: nothing should match `zzz`");
    l.filter("");
    assert_eq!(l.shown(), OPTS.len(), "clearing the filter hid {} rows", OPTS.len() - l.shown());
    l.filter("   ");
    assert_eq!(l.shown(), OPTS.len(), "whitespace is not a query either");
}

/// **A filtered list chooses the row you can see**, not the one at that
/// index in the unfiltered set. This is the arithmetic a filter is most
/// likely to get wrong.
#[test]
fn a_filtered_list_chooses_by_visible_position() {
    let mut l = OptionList::new(&OPTS);
    l.filter("ap");
    lay(&mut l);
    // The second *visible* row is `apricot`, which is index 3.
    release_at(&mut l, 40.0);
    assert_eq!(
        l.take_picked(), Some(3),
        "the second visible row of a filtered list is `apricot` (index 3)"
    );
}

#[test]
fn it_truncates_rather_than_refusing() {
    let many = ["x"; 40];
    let l = OptionList::new(&many);
    assert_eq!(l.count(), MAX_OPTIONS, "it did not truncate to {MAX_OPTIONS}");
}

// ---- the choosers ----------------------------------------------------

#[test]
fn a_picklist_shows_its_selection_in_the_trigger() {
    let p = PickList::new(&OPTS);
    assert_eq!(p.selected(), Some(0), "a picklist opens with nothing chosen");
    assert_eq!(p.text(), "Apple", "the trigger reads {:?}", p.text());
}

#[test]
fn choosing_a_row_closes_the_list_and_updates_the_trigger() {
    let mut p = PickList::new(&OPTS);
    p.open();
    let mut st = lay(&mut p);
    assert!(p.is_open(), "setup: the list is not open");
    // Choose the third row through the list itself, then let `layout`
    // take it — which is where a chooser reads its own list.
    let list = &mut p.list.content;
    let ev = Event::PointerUp { x: 0.0, y: 0.0, button: fulgurite::PointerButton::Left };
    let mut cx = EventCtx::for_event(0, ev);
    cx.local = Point { x: 10.0, y: 70.0 };
    list.on_event(&ev, &cx);
    let mut m = fulgurite::cx::BlockMetrics;
    let styles = fulgurite::pass::default_styles(&p);
    {
        let mut lc = fulgurite::LayoutCx::new(&mut m);
        Node::layout(&mut p, &mut lc, VIEW, &styles, &mut st, 0);
    }
    assert_eq!(p.selected(), Some(2), "the choice did not reach the picklist");
    assert_eq!(p.text(), "Cherry", "the trigger reads {:?}", p.text());
    assert!(!p.is_open(), "choosing a row left the list open");
}

/// Clear the trigger, then type — which is what a person does and what
/// the drive does. **Typing appends at the caret**, like any field, so a
/// combobox opened on "Apple" and typed into reads "Appleban" and
/// matches nothing. `NumberInput` recorded the same lesson in T27.5.
fn query(c: &mut Combobox, text: &str) {
    for _ in 0..32 {
        let ev = Event::Key { key: Key::Backspace, down: true };
        c.on_event(&ev, &EventCtx::for_event(0, ev));
    }
    for ch in text.chars() {
        let ev = Event::Text { ch };
        c.on_event(&ev, &EventCtx::for_event(0, ev));
    }
}

/// A combobox filters as its trigger is **typed in**.
///
/// Typed through `on_event`, not by writing the text — which is the
/// difference the live drive found. Setting the text directly leaves the
/// widget in the "typed" state by construction, so the first version of
/// this test could not have seen a combobox that filtered on its own
/// *selection*.
#[test]
fn a_combobox_filters_on_what_is_typed_into_it() {
    let mut c = Combobox::new(&OPTS);
    lay(&mut c);
    query(&mut c, "ban");
    lay(&mut c);
    assert!(c.is_query(), "typing did not make the trigger a query");
    assert_eq!(
        c.list.content.shown(), 1,
        "typing `ban` left {} rows visible of {:?}",
        c.list.content.shown(), OPTS
    );
}

/// **And it does not filter on its own selection.** Choosing "Apple"
/// puts "Apple" in the trigger; filtering on that would leave one row,
/// so re-opening the list would show only what is already chosen and
/// every other option would be unreachable.
#[test]
fn a_combobox_does_not_filter_on_its_selection() {
    let mut c = Combobox::new(&OPTS);
    c.select(0);
    lay(&mut c);
    assert_eq!(c.text(), "Apple", "setup: the trigger does not hold the selection");
    assert!(!c.is_query(), "a committed selection is not a query");
    assert_eq!(
        c.list.content.shown(), OPTS.len(),
        "a combobox showing `Apple` filtered itself down to {} of {} rows",
        c.list.content.shown(), OPTS.len()
    );
}

/// And choosing again clears a query that was in flight, or the next
/// opening shows the old filter.
#[test]
fn choosing_clears_a_query() {
    let mut c = Combobox::new(&OPTS);
    lay(&mut c);
    query(&mut c, "ap");
    lay(&mut c);
    assert_eq!(c.list.content.shown(), 2, "setup: `ap` should leave two rows");
    c.select(2);
    lay(&mut c);
    assert!(!c.is_query());
    assert_eq!(
        c.list.content.shown(), OPTS.len(),
        "choosing left the query's filter in place: {} rows",
        c.list.content.shown()
    );
}

/// A `PickList` does **not** filter — its trigger shows the choice and is
/// read only (M57/B20), not a query. Without this the combobox test passes on an implementation
/// that filters everything.
#[test]
fn a_picklist_does_not_filter() {
    let mut p = PickList::new(&OPTS);
    p.trigger.set_str("ban");
    lay(&mut p);
    assert_eq!(
        p.list.content.shown(), OPTS.len(),
        "a picklist filtered on its trigger: {} of {} rows",
        p.list.content.shown(), OPTS.len()
    );
}

/// `on_event` only: this checks the widget's own Escape arm, and **no
/// layout pass runs**, so it cannot see a list that closes and reopens in
/// the same frame — which is what shipped (`docs/widget-audit.md` X5).
/// The pumped check is `fulgurite-shell/tests/chooser_closes.rs`.
#[test]
fn escape_closes_an_open_chooser() {
    let mut p = PickList::new(&OPTS);
    p.open();
    let ev = Event::Key { key: Key::Escape, down: true };
    assert!(p.on_event(&ev, &EventCtx::for_event(0, ev)));
    assert!(!p.is_open(), "Escape did not close it");
}

/// A `Menu` has no trigger to show — it is opened by the application.
#[test]
fn a_menu_lays_out_no_trigger() {
    let mut m = Menu::new(&OPTS);
    m.open();
    let st = lay(&mut m);
    assert_eq!(st[1].size.h, 0.0, "a menu laid out a trigger {} tall", st[1].size.h);
    assert!(m.is_open());
}

/// The three kinds answer three names, so a sheet can style them apart.
#[test]
fn the_three_kinds_have_three_names() {
    assert_eq!(<PickList as Widget>::KIND, "picklist");
    assert_eq!(<Combobox as Widget>::KIND, "combobox");
    assert_eq!(<Menu as Widget>::KIND, "menu");
}
