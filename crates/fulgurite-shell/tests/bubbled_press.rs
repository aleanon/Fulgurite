//! **A press a child declines is not a press on its container** —
//! M57/A4 (`docs/plan-widget-fixes.md`).
//!
//! `dispatch` offers an event to the node hit, then to each ancestor. It
//! kept `ctx.local` relative to the node first hit, and four containers
//! took any press that reached them for a press on their own chrome
//! (`docs/widget-audit.md` X2):
//! - a press on a modal's panel asked to dismiss it;
//! - a press inside a split pane moved its divider;
//! - a press on an open accordion section's text collapsed it;
//! - a press inside a pane-grid pane moved the vertical divider.
//!
//! Each case presses a `Label` inside the container through a live
//! `Host`, asserts the press **landed on the label** (so the setup cannot
//! be a press on nothing), and asserts nothing changed. Then it presses
//! the container's own chrome and asserts that still works.

use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Label;
use fulgurite_widgets::{Collapsible, Modal, PaneGrid, SplitPane};

macro_rules! text_box {
    ($name:ident, $id:literal) => {
        #[derive(StaticTree, Default)]
        #[widget(column, spacing = 0.0)]
        struct $name {
            #[widget_id($id)]
            t: Label,
        }
    };
}
text_box!(Left, "left_text");
text_box!(Right, "right_text");
text_box!(Body, "body_text");
text_box!(P1, "p1");
text_box!(P2, "p2");
text_box!(P3, "p3");
text_box!(P4, "p4");
text_box!(Dialog, "dialog_text");

#[derive(StaticTree, Default)]
#[widget(column, spacing = 16.0)]
struct Ui {
    #[widget_id("split")]
    split: SplitPane<Left, Right>,
    #[widget_id("acc")]
    acc: Collapsible<Body>,
    #[widget_id("grid")]
    grid: PaneGrid<P1, P2, P3, P4>,
    #[widget_id("modal")]
    modal: Modal<Dialog>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.split = SplitPane::new(Left::default(), Right::default());
    app.ui.split.first.t.set("left pane text");
    app.ui.split.second.t.set("right pane text");
    app.ui.acc = Collapsible::new("Section", Body::default());
    app.ui.acc.body.t.set("Your name and email");
    app.ui.acc.set_open(true);
    app.ui.grid = PaneGrid::new(P1::default(), P2::default(), P3::default(), P4::default());
    for p in [&mut app.ui.grid.top_left.t, &mut app.ui.grid.top_right.t] {
        p.set("pane");
    }
    app.ui.grid.bottom_left.t.set("pane");
    app.ui.grid.bottom_right.t.set("south-east pane");
    app.ui.modal = Modal::new(Dialog::default());
    app.ui.modal.panel.t.set("Delete the file?");
    let mut h = Host::new(app, 800, 700);
    h.set_stylesheet(&[
        Rule::new(Selector::Id("split"), Patch::new().width(Length::Fixed(360.0)).height(Length::Fixed(80.0))),
        Rule::new(Selector::Id("acc"), Patch::new().width(Length::Fixed(300.0))),
        Rule::new(Selector::Id("grid"), Patch::new().width(Length::Fixed(300.0)).height(Length::Fixed(160.0))),
    ]);
    h.pump();
    h
}

fn index(wid: &str) -> usize {
    fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).unwrap_or_else(|| panic!("no #{wid}"))
}

/// Press and release at the centre of `wid`, asserting the press landed
/// on it. Returns whether the press was handled.
fn press_on(h: &mut Host<App>, wid: &str) -> bool {
    let s = h.states[index(wid)];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    let hit = h.probe(x, y).map(|h| h.node as usize);
    assert_eq!(hit, Some(index(wid)), "setup: a press at the centre of #{wid} does not land on it");
    press_at(h, x, y)
}

fn press_at(h: &mut Host<App>, x: f32, y: f32) -> bool {
    h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    let down = h.pump();
    h.post(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
    h.pump();
    down
}

#[test]
fn a_press_on_a_modal_panel_does_not_dismiss_it() {
    let mut h = host();
    h.app.ui.modal.open();
    h.invalidate_all();
    h.pump();
    press_on(&mut h, "dialog_text");
    assert!(!h.app.ui.modal.take_dismissed(), "a press on the dialog's own text asked to dismiss it");
    // The other direction: the scrim, in the corner of the veil's own box
    // (a veil covers its box, not the window).
    let v = h.states[index("modal")];
    assert_eq!(h.probe(v.pos.x + 3.0, v.pos.y + 3.0).map(|x| x.node as usize), Some(index("modal")), "setup: the corner is not the scrim");
    press_at(&mut h, v.pos.x + 3.0, v.pos.y + 3.0);
    assert!(h.app.ui.modal.take_dismissed(), "a press on the scrim did not ask to dismiss");
}

#[test]
fn a_press_inside_a_split_pane_leaves_the_divider() {
    let mut h = host();
    let before = h.app.ui.split.split();
    let handled = press_on(&mut h, "right_text");
    println!("split {before} -> {} (handled {handled})", h.app.ui.split.split());
    assert!(!handled, "the pane press was taken by something");
    assert_eq!(h.app.ui.split.split(), before, "a press inside the right pane moved the divider");
    // The divider itself still drags.
    let s = h.states[index("split")];
    let bar = s.pos.x + (s.size.w - 6.0) * before + 3.0;
    h.post(Event::PointerDown { x: bar, y: s.pos.y + 10.0, button: fulgurite::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerMove { x: bar + 60.0, y: s.pos.y + 10.0 });
    h.pump();
    h.post(Event::PointerUp { x: bar + 60.0, y: s.pos.y + 10.0, button: fulgurite::PointerButton::Left });
    h.pump();
    assert!(h.app.ui.split.split() > before + 0.1, "the divider no longer drags: {}", h.app.ui.split.split());
}

#[test]
fn a_press_on_an_open_sections_text_leaves_it_open() {
    let mut h = host();
    assert!(h.app.ui.acc.is_open(), "setup: the section starts open");
    let handled = press_on(&mut h, "body_text");
    assert!(!handled, "the body press was taken by something");
    assert!(h.app.ui.acc.is_open(), "a press on the section's text collapsed it");
    let s = h.states[index("acc")];
    press_at(&mut h, s.pos.x + 20.0, s.pos.y + 10.0);
    assert!(!h.app.ui.acc.is_open(), "a press on the header no longer toggles");
}

#[test]
fn a_press_inside_a_pane_leaves_every_divider() {
    let mut h = host();
    let before = (h.app.ui.grid.vertical(), h.app.ui.grid.horizontal(0), h.app.ui.grid.horizontal(1));
    let handled = press_on(&mut h, "p4");
    let after = (h.app.ui.grid.vertical(), h.app.ui.grid.horizontal(0), h.app.ui.grid.horizontal(1));
    println!("dividers {before:?} -> {after:?} (handled {handled})");
    assert!(!handled, "the pane press was taken by something");
    assert_eq!(after, before, "a press inside the south-east pane moved a divider");
}
