//! **A split's divider stays under the pointer, stops at a pixel
//! minimum, and can run either way** — M57/B46
//! (`docs/plan-widget-fixes.md`, `docs/widget-audit.md` SplitPane).
//!
//! The bar's edge was placed at `split · (w − bar)` and a drag set
//! `split = x / w`: the two disagreed, so the divider drifted away from
//! the pointer as it moved.

use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Spacer;
use fulgurite_widgets::{SplitPane, VSplitPane};

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("h")]
    h: SplitPane<Spacer, Spacer>,
    #[widget_id("v")]
    v: VSplitPane<Spacer, Spacer>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host() -> Host<App> {
    // 1000 wide, not 600: at 600 the old 8% stop is 47.5 px, within half
    // a pixel of `min_px`'s 48, and an injection restoring it stayed green.
    let mut h = Host::new(App::default(), 1000, 800);
    h.pump();
    h
}

fn at(id: &str) -> usize {
    fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, id).unwrap()
}

/// The bar's centre along the axis, in window coordinates: where the
/// second pane starts, less half the bar.
fn bar_centre(h: &Host<App>, id: &str, vertical: bool) -> f32 {
    let s = h.states[at(id) + 2];
    (if vertical { s.pos.y } else { s.pos.x }) - 3.0
}

fn drag(h: &mut Host<App>, from: (f32, f32), to: (f32, f32)) {
    h.post(Event::PointerDown { x: from.0, y: from.1, button: fulgurite::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerMove { x: to.0, y: to.1 });
    h.pump();
    h.post(Event::PointerUp { x: to.0, y: to.1, button: fulgurite::PointerButton::Left });
    h.pump();
}

#[test]
fn the_divider_follows_the_pointer_at_the_grab() {
    let mut h = host();
    let p = h.states[at("h")];
    let y = p.pos.y + p.size.h / 2.0;
    let c0 = bar_centre(&h, "h", false);
    // Grabbed 5 px right of centre, moved 150 px right. At 2 px the old
    // `x / w` mapping missed by half a pixel, the tolerance itself.
    drag(&mut h, (c0 + 5.0, y), (c0 + 155.0, y));
    let c1 = bar_centre(&h, "h", false);
    println!("bar centre {c0} -> {c1} for a 150 px drag grabbed 5 px off centre");
    assert!((c1 - (c0 + 150.0)).abs() < 0.5, "the divider drifted: moved {} for 150", c1 - c0);
}

#[test]
fn a_pane_stops_at_min_px() {
    let mut h = host();
    let p = h.states[at("h")];
    let y = p.pos.y + p.size.h / 2.0;
    let c0 = bar_centre(&h, "h", false);
    drag(&mut h, (c0, y), (-500.0, y));
    let first = h.states[at("h") + 1].size.w;
    let c0 = bar_centre(&h, "h", false);
    drag(&mut h, (c0, y), (5000.0, y));
    let second = h.states[at("h") + 2].size.w;
    println!("dragged to the ends: first pane {first} px, second {second} px (min_px 48)");
    assert!((first - 48.0).abs() < 0.5 && (second - 48.0).abs() < 0.5, "a pane is not held at min_px");
}

#[test]
fn a_vertical_split_moves_along_y() {
    let mut h = host();
    let p = h.states[at("v")];
    let x = p.pos.x + p.size.w / 2.0;
    let c0 = bar_centre(&h, "v", true);
    drag(&mut h, (x, c0), (x, c0 + 60.0));
    let c1 = bar_centre(&h, "v", true);
    let first = h.states[at("v") + 1].size;
    println!("vertical bar {c0} -> {c1}; first pane {first:?}");
    assert!((c1 - (c0 + 60.0)).abs() < 0.5, "a vertical split did not follow y");
    assert_eq!(first.w, p.size.w, "a vertical split's pane is not full width");
}
