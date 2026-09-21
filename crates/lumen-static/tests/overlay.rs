//! T15.0: the overlay phase.
//!
//! The property the whole design exists for is the first test: **a
//! popup inside a `ScrollView` paints outside the scroller's clip**. A
//! widget cannot do that — it paints between the scroller's `push_clip`
//! and `pop_clip` — which is why overlays are a phase.

use lumen_static_widgets::ops::{Fixed, Label};
use lumen_static::overlay::OverlaySink;
use lumen_static::{pass, Cmd, Constraints, DisplayList, NoOverlays, NodeState, Overlays, StaticTree, Widget};
use lumen_static_widgets::{Popup, ScrollView};

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
            Cmd::StrokedRect { rect, .. } => ("stroke", *rect),
            Cmd::Text { rect, .. } => ("text", *rect),
            Cmd::Shadow { rect, .. } => ("shadow", *rect),
            Cmd::Image { rect, .. } => ("image", *rect),
            Cmd::Gradient { rect, .. } => ("grad", *rect),
            Cmd::Line { a, b, .. } => ("line", [a.x, a.y, b.x, b.y]),
            Cmd::PushClip { rect } => ("push", *rect),
            Cmd::Glass { rect, .. } => ("glass", *rect),
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
    ui.pop.anchor = lumen_static_widgets::Anchor::Above;
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
        btn: lumen_static_widgets::ops::Button<App>,
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
        a.ui.pop.anchor = lumen_static_widgets::Anchor::Over;
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
    use lumen_static::{Event, Key};
use lumen_static_widgets::{TextInput};

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
        a.ui.modal.anchor = lumen_static_widgets::Anchor::Over;
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

/// **`Deferring` must forward every `Painter` method, including the
/// provided ones** — found 2026-09-10 by the M18 spec review.
///
/// `Painter`'s provided bodies exist for *leaf* painters that cannot
/// draw a thing: `line` and `image` are no-ops, `gradient` is a flat
/// fill of its first stop, `clip_bounds` is "cull nothing". For a
/// *wrapper* every one of those is wrong, because the default acts on
/// the wrapper instead of forwarding — so a wrapper that inherits them
/// silently rewrites the frame.
///
/// `Deferring` inherited four. Measured before the fix, on the list
/// below: **four commands in, two out** — `Line` and `Image` dropped,
/// `Gradient` flattened to `Rect`. `Scaled` and `Recording` override all
/// eleven; this one overrode the seven that existed when it was written
/// and never gained T15.2's image, T15.3's line and gradient, or
/// T16.12's `clip_bounds`.
///
/// Compares against the *unwrapped* stream rather than a hard-coded
/// list, so a future command is covered the day it is added — which is
/// the failure mode this test is about.
#[test]
fn deferring_forwards_every_command() {
    use lumen_static::display::{Color, ImageId, Painter, Stops};
    use lumen_static::Point;
    use lumen_static::overlay::Deferring;

    fn paint(p: &mut dyn Painter) {
        p.rect([0.0, 0.0, 10.0, 10.0], Color(1, 2, 3, 255));
        p.line(Point { x: 0.0, y: 0.0 }, Point { x: 9.0, y: 9.0 }, 1.0, Color(9, 9, 9, 255));
        p.gradient(
            [0.0, 0.0, 10.0, 10.0],
            Stops::two(Color(10, 0, 0, 255), Color(0, 0, 10, 255)),
            0.0,
        );
        p.image([0.0, 0.0, 10.0, 10.0], ImageId(1), Color(255, 255, 255, 255));
        p.glass(
            [0.0, 0.0, 10.0, 10.0],
            4.0,
            lumen_static::display::Material::FROSTED,
        );
    }

    let mut direct = DisplayList::new();
    paint(&mut direct);

    let mut wrapped = DisplayList::new();
    {
        let mut sink = NoOverlays;
        let mut d = Deferring::new(&mut wrapped, &mut sink);
        paint(&mut d);
    }

    // Assert the setup happened before comparing: two empty lists are
    // equal, and that is the vacuous shape `AGENT.md` names.
    assert_eq!(direct.cmds.len(), 5, "the unwrapped painter dropped something itself");
    assert_eq!(
        direct.cmds, wrapped.cmds,
        "Deferring changed the command stream: {} in, {} out",
        direct.cmds.len(),
        wrapped.cmds.len()
    );

    // `clip_bounds` is invisible to the comparison above -- a
    // `DisplayList` has no surface and answers `None` either way, so
    // wrapping one could not show the difference. It needs a painter
    // that answers `Some`, or the fourth missing method stays untested
    // while the test looks thorough.
    struct Surfaced(Option<lumen_static::display::Rect>);
    impl Painter for Surfaced {
        fn rect(&mut self, _: lumen_static::display::Rect, _: Color) {}
        fn text_weighted(
            &mut self,
            _: lumen_static::display::Rect,
            _: &str,
            _: f32,
            _: f32,
            _: Color,
        ) {
        }
        fn rounded_rect(&mut self, _: lumen_static::display::Rect, _: Color, _: f32) {}
        fn shadow(&mut self, _: lumen_static::display::Rect, _: f32, _: f32) {}
        fn stroked_rect(
            &mut self,
            _: lumen_static::display::Rect,
            _: Color,
            _: f32,
            _: f32,
        ) {
        }
        fn push_clip(&mut self, _: lumen_static::display::Rect) {}
        fn pop_clip(&mut self) {}
        fn clip_bounds(&self) -> Option<lumen_static::display::Rect> {
            self.0
        }
    }

    let want = Some([0.0, 0.0, 800.0, 600.0]);
    let mut surf = Surfaced(want);
    let mut sink = NoOverlays;
    let d = Deferring::new(&mut surf, &mut sink);
    assert_eq!(
        d.clip_bounds(),
        want,
        "Deferring hid the surface bounds, so culling turns off under an overlay pass"
    );
}

// ---- M24/T24.2: activation inside an overlay -------------------------

/// A click on a widget *inside* an open overlay must reach the
/// application.
///
/// It could not until 2026-09-14, and the dismissal tests above did not
/// see it because they asserted the popup **stayed open**, never that
/// anything inside it fired. `dispatch_overlaid` hit-tested with the
/// overlays on top, offered the result to `on_event`, and then fell
/// through to `dispatch` — which hit-tests *plainly*, and the ordinary
/// walk cannot reach an out-of-flow child: a container rejects a point
/// outside its own rect long before it gets there. So the hit that had
/// just been found was thrown away and recomputed as `None`.
///
/// The comment at that fall-through read "fall through to activation on
/// the overlay's node", which is what it was for and not what it did.
mod overlay_activation {
    use super::*;
    use lumen_static::event::InputState;
    use lumen_static::{Event, EventCtx};

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Panel {
        #[widget_id("row")]
        row: lumen_static_widgets::ops::Button<App>,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Ui {
        #[widget_id("head")]
        head: lumen_static_widgets::ops::Fixed,
        #[widget_id("pop")]
        pop: Popup<Panel>,
    }

    #[derive(lumen_static::App, Default)]
    struct App {
        #[tree]
        ui: Ui,
        popped: u32,
    }

    #[test]
    fn a_click_inside_an_open_overlay_reaches_the_handler() {
        let mut a = App::default();
        a.ui.pop.open = true;
        a.ui.pop.anchor = lumen_static_widgets::Anchor::Below;
        a.ui.pop.content.row.label = "pick";
        a.ui.pop.content.row.on_click = |s: &mut App, _: &EventCtx| s.popped += 1;

        let sheet = [
            lumen_static::style::Rule::new(
                lumen_static::style::Selector::Id("head"),
                lumen_static::style::Patch::new()
                    .width(lumen_static::style::Length::Fixed(80.0))
                    .height(lumen_static::style::Length::Fixed(20.0)),
            ),
            lumen_static::style::Rule::new(
                lumen_static::style::Selector::Id("row"),
                lumen_static::style::Patch::new()
                    .width(lumen_static::style::Length::Fixed(80.0))
                    .height(lumen_static::style::Length::Fixed(20.0)),
            ),
        ];
        let mut sty = vec![lumen_static::Style::default(); Ui::NODE_COUNT];
        lumen_static::style::cascade_deep(&mut a.ui, &sheet, &mut sty);
        let mut st = [NodeState::default(); Ui::NODE_COUNT];
        pass::layout(&mut a.ui, &sty, &mut st, VIEW);
        let mut list = DisplayList::new();
        let mut ov: Overlays<4> = Overlays::default();
        pass::paint_overlaid(&a.ui, &st, &mut list, &mut ov);

        assert!(!ov.is_empty(), "setup: paint registered no overlay");
        // The *button* inside the panel inside the popup — the node a
        // person clicks, two levels below the overlay's root.
        let row = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "row")
            .expect("the row node");
        let c = st[row];
        assert!(c.size.w > 0.0 && c.size.h > 0.0, "setup: the overlay content has no area");
        let (x, y) = (c.pos.x + c.size.w / 2.0, c.pos.y + c.size.h / 2.0);
        // **The premise.** If the ordinary walk can already find this
        // point then the overlay path is not being exercised and a pass
        // means nothing — the shape `docs/plan-select.md` and AGENT.md
        // both call "the setup silently did not happen".
        assert!(
            pass::hit_test(&a.ui, &st, x, y).is_none(),
            "setup: the plain walk finds ({x}, {y}) on its own, so this test \
             would pass with the overlay routing removed"
        );
        assert!(
            pass::hit_test_overlaid(&a.ui, &st, &ov, x, y).is_some(),
            "setup: not even the overlay walk finds the content"
        );

        let mut input = InputState::default();
        pass::dispatch_overlaid(&mut a, &sty, &mut st, &mut input, &ov, &Event::down(x, y));
        let handled =
            pass::dispatch_overlaid(&mut a, &sty, &mut st, &mut input, &ov, &Event::up(x, y));
        assert!(handled, "the release was not handled");
        assert_eq!(a.popped, 1, "the overlay content's handler never ran");
    }
}
