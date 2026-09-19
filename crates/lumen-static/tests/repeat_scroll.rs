//! M27/T27.4 — **a repeater scrolls itself.**
//!
//! Nothing did. Every repeater in the tree until M24 sat inside a
//! `ScrollView`, which handled the wheel and drove `Repeat::scroll` as a
//! plain field. `Select`'s menu is a bare `Popup<Repeat<Row>>`, so a
//! dropdown with more options than fit on screen **could not be scrolled
//! at all** — not by a wheel, not by a drag, not by anything. Nothing
//! caught it because no test and no example had ever built a list longer
//! than its box outside a scroller.
//!
//! Found by `just widget-drive` at the moment the showcase's catalogue
//! outgrew its own menu: twenty-one entries in a menu that shows
//! eighteen, and the last three widgets unreachable.
//!
//! There were two halves and the second is the older one. After the
//! wheel arrived, it still did nothing: `visible_range` and the scroll
//! clamp measured against `limits.max_h`, which for a `Popup`'s content
//! is the whole window — so a 630 px list in a 560 px panel computed
//! `max_scroll = 630 - 720 = 0`. M24/T24.2 fixed exactly this asymmetry
//! for the *width* ("a row must be laid out against the box the repeater
//! settled on, not the one it was offered") and left the height.

use lumen_static::prelude::*;
use lumen_static::repeat::Repeat;
use lumen_static::style::{cascade_deep, Length, Patch, Rule, Selector};
use lumen_static::widget::Node;
use lumen_static::{Constraints, EventCtx, NodeState, Point};

const ROW_H: f32 = 30.0;
const ROWS: usize = 21;
const PANEL_H: f32 = 300.0;
/// Deliberately taller than the panel: a popup hands its content the
/// window, and that is the number the repeater used to believe.
const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 720.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Row {
    #[widget_id("cell")]
    l: Label<String>,
}

fn bind(r: &mut Row, i: usize) {
    r.l.set(format!("row {i}"));
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("list")]
    list: Repeat<Row>,
}

fn laid_out() -> (Ui, Vec<Style>, Vec<NodeState>) {
    let mut ui = Ui { list: Repeat::new(ROWS, ROW_H, bind) };
    let sheet = [Rule::new(
        Selector::Id("list"),
        Patch::new().width(Length::Fixed(300.0)).height(Length::Fixed(PANEL_H)),
    )];
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    relayout(&mut ui, &styles, &mut st);
    (ui, styles, st)
}

fn relayout(ui: &mut Ui, styles: &[Style], st: &mut [NodeState]) {
    let mut m = lumen_static::cx::BlockMetrics;
    let mut cx = lumen_static::LayoutCx::new(&mut m);
    Node::layout(ui, &mut cx, VIEW, styles, st, 0);
    Node::place(ui, Point { x: 0.0, y: 0.0 }, styles, st, 0);
}

fn wheel(ui: &mut Ui, dy: f32) -> bool {
    let ev = lumen_static::Event::Scroll { x: 10.0, y: 10.0, dy };
    Node::on_event(&mut ui.list, &ev, &EventCtx::for_event(1, ev))
}

/// **The setup, asserted.** 21 rows of 30 px is 630, in a 300 px panel —
/// so there is 330 px to scroll. If the panel silently resolved to its
/// content height there would be nothing to scroll and every assertion
/// below would agree about a list that cannot move.
#[test]
fn the_list_is_longer_than_its_panel() {
    let (ui, _, st) = laid_out();
    let content = ROWS as f32 * ROW_H;
    assert_eq!(st[1].size.h, PANEL_H, "the panel is not the height the sheet asked for");
    assert!(
        content > PANEL_H,
        "setup: {content} px of rows in a {PANEL_H} px panel — nothing to scroll"
    );
    assert_eq!(
        ui.list.max_scroll(st[1].size.h),
        content - PANEL_H,
        "max_scroll is measured against the wrong height; the offered limit was \
         {} px and the panel is {PANEL_H} px",
        VIEW.max_h
    );
}

/// The wheel moves it, and the rows that are materialized move with it.
#[test]
fn a_wheel_scrolls_a_bare_repeater() {
    let (mut ui, styles, mut st) = laid_out();
    assert_eq!(ui.list.first_row(), 0, "setup: it does not start at the top");
    assert!(wheel(&mut ui, -120.0), "the repeater declined a wheel it had room for");
    relayout(&mut ui, &styles, &mut st);
    assert_eq!(
        ui.list.scroll, 120.0,
        "offset is {} after one 120 px wheel",
        ui.list.scroll
    );
    assert_eq!(
        ui.list.first_row(),
        4,
        "first materialized row is {} after 120 px at {ROW_H} px a row",
        ui.list.first_row()
    );
}

/// And it stops at the end rather than running off — and *says* it
/// stopped, so a parent scroller gets the rest of the gesture.
#[test]
fn it_clamps_at_the_end_and_declines_there() {
    let (mut ui, styles, mut st) = laid_out();
    for _ in 0..20 {
        wheel(&mut ui, -120.0);
        relayout(&mut ui, &styles, &mut st);
    }
    let max = ROWS as f32 * ROW_H - PANEL_H;
    assert_eq!(ui.list.scroll, max, "offset ran past the end: {} over {max}", ui.list.scroll);
    assert!(
        !wheel(&mut ui, -120.0),
        "it consumed a wheel at the end — a parent scroller would never see the gesture"
    );
    // And back up, which is the half a clamp in the wrong place breaks:
    // an unclamped offset has to be undone before anything moves.
    assert!(wheel(&mut ui, 120.0), "it declined a wheel back from the end");
    relayout(&mut ui, &styles, &mut st);
    assert_eq!(ui.list.scroll, max - 120.0, "scrolling back did not move it");
}

/// The other direction, from the top.
#[test]
fn it_declines_a_wheel_up_from_the_top() {
    let (mut ui, ..) = laid_out();
    assert!(!wheel(&mut ui, 120.0), "it consumed a wheel up while already at the top");
    assert_eq!(ui.list.scroll, 0.0);
}

/// A list that fits has nothing to scroll and must say so.
#[test]
fn a_list_that_fits_declines_everything() {
    let mut ui = Ui { list: Repeat::new(3, ROW_H, bind) };
    let sheet = [Rule::new(
        Selector::Id("list"),
        Patch::new().width(Length::Fixed(300.0)).height(Length::Fixed(PANEL_H)),
    )];
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    relayout(&mut ui, &styles, &mut st);
    assert_eq!(ui.list.max_scroll(st[1].size.h), 0.0, "3 rows in {PANEL_H} px should not scroll");
    assert!(!wheel(&mut ui, -120.0), "a list that fits consumed a wheel");
}

/// **A measurement must not change what it measures** (M27/T27.10).
///
/// The agent's ambient audit measures every leaf twice per frame, under
/// a 1e6 and a 2e6 px box, to tell a node sized by its content from one
/// sized by its container (`crates/lumen-static-agent/src/audit.rs`).
/// Against a 2 000 000 px viewport there is nothing to scroll — so a
/// `layout` that clamped the *stored* offset against the box it was
/// handed reset the list to the top on every frame the audit ran, and a
/// `DataGrid` in the showcase could not be scrolled at all while this
/// file's other tests, a widget unit test and a headless `Host` all
/// agreed it could.
///
/// The probe is spelled out here rather than imported: this crate does
/// not depend on the agent, and the property is about *any* speculative
/// pass — a shrink-wrapping parent that measures before it commits does
/// the same thing.
#[test]
fn a_measuring_pass_does_not_move_the_offset() {
    // **A repeater whose height is its container's**, which is the case
    // the probes can reach: `laid_out()` pins the list at `Fixed`, so
    // its resolved height — and with it `max_scroll` — is the same
    // under a 2e6 px box as under the real one, and both injections
    // below left this test green when it used that fixture. The
    // showcase's grid hands its rows `FILL` of what is left under the
    // header, and a filling node is exactly one that answers the
    // question the audit is asking.
    let mut ui = Ui { list: Repeat::new(ROWS, ROW_H, bind) };
    let sheet = [Rule::new(Selector::Id("list"), Patch::new().height(Length::FILL))];
    let mut styles = vec![Style::default(); Ui::NODE_COUNT];
    cascade_deep(&mut ui, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    let panel = Constraints { max_h: PANEL_H, ..VIEW };
    let relay = |ui: &mut Ui, st: &mut Vec<NodeState>| {
        let mut m = lumen_static::cx::BlockMetrics;
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        Node::layout(ui, &mut cx, panel, &styles, st, 0);
        Node::place(ui, Point { x: 0.0, y: 0.0 }, &styles, st, 0);
    };
    relay(&mut ui, &mut st);
    let viewport = st[1].size.h;
    assert_eq!(viewport, PANEL_H, "setup: a filling list in a {PANEL_H} px panel");
    assert!(wheel(&mut ui, -120.0), "setup: the list declined the first wheel");
    relay(&mut ui, &mut st);
    let scrolled = ui.list.scroll;
    assert_eq!(scrolled, 120.0, "setup: the wheel did not move the list");

    // The audit's two probes, in its order, on the live widget.
    let mut m = lumen_static::cx::BlockMetrics;
    let mut cx = lumen_static::LayoutCx::new(&mut m);
    let mut scratch = vec![NodeState::default(); Ui::NODE_COUNT];
    for max in [1.0e6f32, 2.0e6] {
        let probe = Constraints { min_w: 0.0, max_w: max, min_h: 0.0, max_h: max };
        let h = Node::layout(&mut ui.list, &mut cx, probe, &styles, &mut scratch, 1).h;
        assert!(
            h > PANEL_H,
            "setup: the probe at {max} px resolved the list to {h} px, so it \
             cannot tell a filling node from a fixed one"
        );
    }
    assert_eq!(
        ui.list.scroll, scrolled,
        "two probes at 1e6 and 2e6 px moved the offset from {scrolled} to {}",
        ui.list.scroll
    );
    // And the wheel still works afterwards — the offset surviving is no
    // use if the viewport it is clamped against did not.
    assert!(
        wheel(&mut ui, -120.0),
        "after the probes the list declined a wheel; it reports {} px of \
         scroll against a viewport of {}",
        ui.list.max_scroll(ui.list.viewport()),
        ui.list.viewport()
    );
    assert_eq!(ui.list.scroll, scrolled + 120.0, "the wheel after the probes moved nothing");
}
