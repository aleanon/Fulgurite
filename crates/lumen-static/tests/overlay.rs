//! T15.0: the overlay phase.
//!
//! The property the whole design exists for is the first test: **a
//! popup inside a `ScrollView` paints outside the scroller's clip**. A
//! widget cannot do that — it paints between the scroller's `push_clip`
//! and `pop_clip` — which is why overlays are a phase.

use lumen_static::ops::{Fixed, Label};
use lumen_static::overlay::OverlaySink;
use lumen_static::{
    pass, Cmd, Constraints, DisplayList, NoOverlays, NodeState, Overlays, Popup, ScrollView,
    StaticTree, Widget,
};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Menu {
    a: Label,
    b: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Inner {
    #[widget_id("row")]
    row: Fixed,
    #[widget_id("pop")]
    pop: Popup<Menu>,
    #[widget_id("after")]
    after: Fixed,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    sc: ScrollView<Inner>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 80.0 };

fn build(open: bool) -> (Ui, Vec<lumen_static::Style>, [NodeState; Ui::NODE_COUNT]) {
    let mut ui = Ui::default();
    ui.sc.content.row.w = 100.0;
    ui.sc.content.row.h = 40.0;
    ui.sc.content.after.w = 100.0;
    ui.sc.content.after.h = 40.0;
    ui.sc.content.pop.open = open;
    ui.sc.content.pop.content.a.set("one");
    ui.sc.content.pop.content.b.set("two");
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    (ui, sty, st)
}

/// Commands, as `(kind, rect)`, so a test can say where things landed.
fn cmds(list: &DisplayList) -> Vec<(&'static str, [f32; 4])> {
    list.cmds
        .iter()
        .map(|c| match c {
            Cmd::Rect { rect, .. } => ("rect", *rect),
            Cmd::RoundedRect { rect, .. } => ("rrect", *rect),
            Cmd::Text { rect, .. } => ("text", *rect),
            Cmd::Shadow { rect, .. } => ("shadow", *rect),
            Cmd::PushClip { rect } => ("push", *rect),
            Cmd::PopClip => ("pop", [0.0; 4]),
        })
        .collect()
}

/// **The property the phase exists for.**
#[test]
fn a_popup_inside_a_scroller_paints_after_the_clip_is_popped() {
    let (ui, _, st) = build(true);
    let mut list = DisplayList::new();
    let mut ov: Overlays<4> = Overlays::default();
    pass::paint_overlaid(&ui, &st, &mut list, &mut ov);

    assert_eq!(ov.len(), 1, "the popup deferred itself");
    let c = cmds(&list);
    let last_pop = c.iter().rposition(|(k, _)| *k == "pop").expect("the scroller clipped");
    let popup_text =
        c.iter().rposition(|(k, _)| *k == "text").expect("the popup's labels painted");
    assert!(
        popup_text > last_pop,
        "the popup must paint AFTER the scroller's clip is popped, or it is clipped by it: {c:?}"
    );
}

#[test]
fn without_a_sink_it_paints_inline_rather_than_vanishing() {
    // Degraded, not broken: a host that has not opted into an overlay
    // sink still gets a working application, with the popup clipped the
    // way it would have been before overlays existed.
    let (ui, _, st) = build(true);
    let mut list = DisplayList::new();
    let mut ov = NoOverlays;
    pass::paint_overlaid(&ui, &st, &mut list, &mut ov);

    let c = cmds(&list);
    assert_eq!(ov.len(), 0, "nothing was deferred");
    let last_pop = c.iter().rposition(|(k, _)| *k == "pop").unwrap();
    let popup_text = c.iter().rposition(|(k, _)| *k == "text").expect("still painted");
    assert!(popup_text < last_pop, "inline, so inside the clip");
}

#[test]
fn a_closed_popup_paints_nothing_and_costs_no_space() {
    let (closed, _, st_closed) = build(false);
    let (_, _, st_open) = build(true);

    // Out of flow: the sibling after the popup sits in the same place
    // whether it is open or shut. This is what stops a dropdown pushing
    // the rest of a form down the screen.
    let after = 3 + Popup::<Menu>::NODE_COUNT; // sc, inner, row, popup..., after
    let _ = after;
    let after_closed = st_closed.iter().rposition(|s| s.size.h == 40.0).unwrap();
    let after_open = st_open.iter().rposition(|s| s.size.h == 40.0).unwrap();
    assert_eq!(
        st_closed[after_closed].pos.y, st_open[after_open].pos.y,
        "opening the popup moved its sibling"
    );

    let mut list = DisplayList::new();
    let mut ov: Overlays<4> = Overlays::default();
    pass::paint_overlaid(&closed, &st_closed, &mut list, &mut ov);
    assert_eq!(ov.len(), 0);
    assert!(
        !cmds(&list).iter().any(|(k, _)| *k == "text"),
        "a closed popup paints no content"
    );
}

/// **The case the reverse-order rule exists for.**
///
/// A popup declared *after* what it covers. The ordinary walk visits
/// children in declaration order and returns the first hit, so it finds
/// the thing *underneath*; only the overlay phase, which tests deferred
/// subtrees first, returns the popup. With the popup declared *before*
/// its neighbour the two agree — which is why this fixture puts it
/// last, and why the earlier one did not catch anything.
#[test]
fn an_overlay_is_hit_before_the_content_under_it() {
    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Covered {
        #[widget_id("under")]
        under: Fixed,
        #[widget_id("pop")]
        pop: Popup<Menu>,
    }

    let mut ui = Covered::default();
    ui.under.w = 100.0;
    ui.under.h = 40.0;
    ui.pop.open = true;
    // Above: the content is drawn back over `under`, which sits before
    // it in the flow and before it in the table.
    ui.pop.anchor = lumen_static::Anchor::Above;
    ui.pop.content.a.set("one");
    ui.pop.content.b.set("two");

    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Covered::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let mut list = DisplayList::new();
    let mut ov: Overlays<4> = Overlays::default();
    pass::paint_overlaid(&ui, &st, &mut list, &mut ov);
    assert_eq!(ov.len(), 1, "the popup deferred");

    // A point inside the popup's first label, which lies over `under`.
    let label = st[ov.at(0) as usize + 1];
    let (x, y) = (label.pos.x + 1.0, label.pos.y + 1.0);
    assert!(
        lumen_static::hit::contains(&st[1], x, y),
        "the fixture must actually overlap, or it proves nothing"
    );

    let under = pass::hit_test(&ui, &st, x, y).map(|h| h.node);
    let over = pass::hit_test_overlaid(&ui, &st, &ov, x, y).map(|h| h.node);
    assert_eq!(under, Some(1), "the ordinary walk finds what is underneath");
    assert_ne!(over, under, "the overlay phase must win, or a click goes under the popup");
}

#[test]
fn a_full_sink_refuses_and_counts_rather_than_dropping_silently() {
    let (ui, _, st) = build(true);
    let mut list = DisplayList::new();
    let mut ov: Overlays<0> = Overlays::default();
    pass::paint_overlaid(&ui, &st, &mut list, &mut ov);
    assert_eq!(ov.len(), 0);
    assert_eq!(ov.refused(), 1, "the refusal is counted, not silent");
    // And the content still painted, inline.
    assert!(cmds(&list).iter().any(|(k, _)| *k == "text"));
}

#[test]
fn no_overlays_is_a_zst() {
    // The gating policy's second mechanism: a tree that declares no
    // overlay pays nothing, and the type says so.
    assert_eq!(core::mem::size_of::<NoOverlays>(), 0);
    assert_eq!(core::mem::size_of::<Overlays<8>>(), 8 * 4 + 2 * core::mem::size_of::<usize>());
}

// ---- T15.1: dismissal ------------------------------------------------

mod dismissal {
    use super::*;
    use lumen_static::event::InputState;
    use lumen_static::{Event, EventCtx, Key, Mods};

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Ui {
        #[widget_id("btn")]
        btn: lumen_static::ops::Button<App>,
        #[widget_id("pop")]
        pop: Popup<Menu>,
    }

    #[derive(lumen_static::App, Default)]
    struct App {
        #[tree]
        ui: Ui,
        pressed: u32,
    }

    fn app() -> (App, Vec<lumen_static::Style>, [NodeState; Ui::NODE_COUNT], Overlays<4>) {
        let mut a = App::default();
        a.ui.btn.label = "open";
        a.ui.btn.on_click = |s: &mut App, _: &EventCtx| s.pressed += 1;
        a.ui.pop.open = true;
        a.ui.pop.anchor = lumen_static::Anchor::Over;
        a.ui.pop.content.a.set("one");
        a.ui.pop.content.b.set("two");
        let sty = pass::default_styles(&a.ui);
        let mut st = [NodeState::default(); Ui::NODE_COUNT];
        pass::layout(&mut a.ui, &sty, &mut st, VIEW);
        let mut list = DisplayList::new();
        let mut ov: Overlays<4> = Overlays::default();
        pass::paint_overlaid(&a.ui, &st, &mut list, &mut ov);
        (a, sty, st, ov)
    }

    #[test]
    fn a_press_outside_closes_it_and_is_consumed() {
        let (mut a, sty, mut st, ov) = app();
        let mut input = InputState::default();
        // Far from the popup, and on the button — which must NOT fire:
        // the click that closes a menu should not also press what is
        // underneath it.
        let b = st[1];
        let (x, y) = (b.pos.x + 2.0, b.pos.y + 2.0);
        let handled =
            pass::dispatch_overlaid(&mut a, &sty, &mut st, &mut input, &ov, &Event::down(x, y));
        assert!(handled, "the dismissing press is consumed");
        assert!(!a.ui.pop.open, "and it closed");
        assert_eq!(a.pressed, 0, "the button underneath did not fire");
    }

    #[test]
    fn escape_closes_the_topmost() {
        let (mut a, sty, mut st, ov) = app();
        let mut input = InputState::default();
        let handled = pass::dispatch_overlaid(
            &mut a,
            &sty,
            &mut st,
            &mut input,
            &ov,
            &Event::Key { key: Key::Escape, down: true },
        );
        assert!(handled);
        assert!(!a.ui.pop.open);
    }

    #[test]
    fn a_press_inside_does_not_close_it() {
        let (mut a, sty, mut st, ov) = app();
        let mut input = InputState::default();
        let label = st[ov.at(0) as usize + 1];
        let (x, y) = (label.pos.x + 1.0, label.pos.y + 1.0);
        pass::dispatch_overlaid(&mut a, &sty, &mut st, &mut input, &ov, &Event::down(x, y));
        assert!(a.ui.pop.open, "clicking the popup's own content keeps it open");
    }

    #[test]
    fn with_nothing_open_it_is_ordinary_dispatch() {
        let (mut a, sty, mut st, _) = app();
        a.ui.pop.open = false;
        let empty: Overlays<4> = Overlays::default();
        let mut input = InputState::default();
        let b = st[1];
        let (x, y) = (b.pos.x + 2.0, b.pos.y + 2.0);
        pass::dispatch_overlaid(&mut a, &sty, &mut st, &mut input, &empty, &Event::down(x, y));
        pass::dispatch_overlaid(&mut a, &sty, &mut st, &mut input, &empty, &Event::up(x, y));
        assert_eq!(a.pressed, 1, "the button works normally when no overlay is open");
    }

    #[test]
    fn escape_with_nothing_open_falls_through() {
        let (mut a, sty, mut st, _) = app();
        a.ui.pop.open = false;
        let empty: Overlays<4> = Overlays::default();
        let mut input = InputState::default();
        let handled = pass::dispatch_overlaid(
            &mut a,
            &sty,
            &mut st,
            &mut input,
            &empty,
            &Event::Key { key: Key::Escape, down: true },
        );
        assert!(!handled, "Escape is the application's when no overlay wants it");
        let _ = Mods::NONE;
    }
}

// ---- T15.1: the focus trap -------------------------------------------

mod focus_trap {
    use super::*;
    use lumen_static::event::InputState;
    use lumen_static::{Event, Key, TextInput};

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Dialog {
        #[widget_id("in1")]
        in1: TextInput,
        #[widget_id("in2")]
        in2: TextInput,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Ui {
        #[widget_id("outside")]
        outside: TextInput,
        #[widget_id("modal")]
        modal: Popup<Dialog>,
    }

    #[derive(lumen_static::App, Default)]
    struct App {
        #[tree]
        ui: Ui,
    }

    /// **The half of "modal" that is not painting.** Tab must cycle
    /// inside the dialog and never reach the field behind it.
    #[test]
    fn tab_cycles_inside_the_modal_and_never_leaves() {
        let mut a = App::default();
        a.ui.modal.open = true;
        a.ui.modal.anchor = lumen_static::Anchor::Over;
        let sty = pass::default_styles(&a.ui);
        let mut st = [NodeState::default(); Ui::NODE_COUNT];
        pass::layout(&mut a.ui, &sty, &mut st, VIEW);
        let mut list = DisplayList::new();
        let mut ov: Overlays<4> = Overlays::default();
        pass::paint_overlaid(&a.ui, &st, &mut list, &mut ov);
        assert_eq!(ov.len(), 1);

        // The dialog's own nodes: the popup at `top`, its two fields
        // after it.
        let top = ov.at(0) as usize;
        let inside = top..top + Popup::<Dialog>::NODE_COUNT;

        let mut input = InputState::default();
        let tab = Event::Key { key: Key::Tab, down: true };
        let mut seen = Vec::new();
        for _ in 0..6 {
            pass::dispatch_overlaid(&mut a, &sty, &mut st, &mut input, &ov, &tab);
            let f = input.focus.expect("Tab focused something") as usize;
            assert!(inside.contains(&f), "Tab escaped the modal to node {f}");
            seen.push(f);
        }
        assert!(seen.iter().any(|&f| f != seen[0]), "and it actually moved between fields");
    }

    #[test]
    fn with_no_overlay_tab_walks_the_whole_tree_as_before() {
        let mut a = App::default();
        a.ui.modal.open = false;
        let sty = pass::default_styles(&a.ui);
        let mut st = [NodeState::default(); Ui::NODE_COUNT];
        pass::layout(&mut a.ui, &sty, &mut st, VIEW);
        let empty: Overlays<4> = Overlays::default();

        let mut input = InputState::default();
        pass::dispatch_overlaid(
            &mut a,
            &sty,
            &mut st,
            &mut input,
            &empty,
            &Event::Key { key: Key::Tab, down: true },
        );
        assert_eq!(input.focus, Some(1), "the field outside is reachable again");
    }
}
