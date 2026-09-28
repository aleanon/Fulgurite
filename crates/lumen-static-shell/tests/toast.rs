//! **A toast floats at the window's bottom, times out, and closes** —
//! M57/B34 (`docs/plan-widget-fixes.md`). It drew a pill in the flow
//! and did nothing else (`docs/widget-audit.md`, Toast).

use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::{Anchor, Popup, Toast};

#[derive(StaticTree, Default)]
#[widget(column)]
struct Slot {
    #[widget_id("toast")]
    t: Toast,
}

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("pop")]
    pop: Popup<Slot>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const W: u32 = 600;
const H: u32 = 400;

fn host(closable: bool) -> Host<App> {
    let mut app = App::default();
    let mut t = Toast::new("Saved");
    t.closable = closable;
    let mut p = Popup::new(Slot { t });
    p.anchor = Anchor::WindowBottom;
    p.open = true;
    app.ui.pop = p;
    let mut h: Host<App> = Host::new(app, W, H);
    // A derived struct is FILL, and the popup centres its *content*: the
    // slot must shrink to the toast, or the toast sits at the left of a
    // window-wide column.
    h.set_stylesheet(&[lumen_static::style::Rule::new(
        lumen_static::style::Selector::Kind("Slot"),
        lumen_static::style::Patch::new().width(lumen_static::style::Length::Shrink),
    )]);
    h.pump();
    h
}

fn toast(h: &Host<App>) -> lumen_static::NodeState {
    h.states[lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "toast").unwrap()]
}

#[test]
fn it_sits_at_the_windows_bottom_centre() {
    let h = host(false);
    let s = toast(&h);
    let centre = s.pos.x + s.size.w / 2.0;
    let bottom = s.pos.y + s.size.h;
    println!("toast {:?}+{:?}: centre x {centre}, bottom {bottom} in a {W}x{H} window", s.pos, s.size);
    assert!(s.size.w > 20.0, "setup: the toast has no size");
    assert!((centre - W as f32 / 2.0).abs() < 1.0, "not centred");
    assert!((bottom - (H as f32 - 24.0)).abs() < 1.0, "not 24 px above the bottom");
}

#[test]
fn a_timed_toast_goes_away_on_the_applications_clock() {
    let mut h = host(false);
    h.app.ui.pop.content.t.show(Some(1000.0));
    assert!(!h.app.ui.pop.content.t.tick(600.0), "gone before its time");
    let gone = h.app.ui.pop.content.t.tick(600.0);
    h.invalidate_all();
    h.pump();
    println!("gone after 1200 ms: {gone}; size now {:?}", toast(&h).size);
    assert!(gone && !h.app.ui.pop.content.t.is_shown(), "a 1000 ms toast outlived 1200 ms");
    assert_eq!(toast(&h).size.w, 0.0, "a hidden toast still takes room");
}

#[test]
fn the_x_closes_it_and_a_toast_without_one_takes_no_press() {
    for closable in [true, false] {
        let mut h = host(closable);
        let s = toast(&h);
        // 6 px in from the right: on the × when there is one, and in the
        // same band when there is not. 14 px in was outside the band a
        // ×-less toast would test, so an injection that made every toast
        // closable left the no-× leg green.
        let (x, y) = (s.pos.x + s.size.w - 6.0, s.pos.y + s.size.h / 2.0);
        h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
        let down = h.pump();
        h.post(Event::PointerUp { x, y, button: lumen_static::PointerButton::Left });
        h.pump();
        println!("closable {closable}: press handled {down}, shown after {}", h.app.ui.pop.content.t.is_shown());
        assert_eq!(h.app.ui.pop.content.t.is_shown(), !closable, "closable={closable}");
        if !closable {
            assert!(!down, "a toast without a × took the press");
        }
    }
}

/// A press elsewhere does not dismiss a window-anchored popup: the
/// showcase's Save button dismissed the toast it had just asked for.
#[test]
fn a_press_elsewhere_leaves_a_window_toast_up() {
    let mut h = host(false);
    let (x, y) = (10.0, 10.0);
    h.post(Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 });
    h.pump();
    h.post(Event::PointerUp { x, y, button: lumen_static::PointerButton::Left });
    h.pump();
    println!("popup open after a press at ({x}, {y}): {}", h.app.ui.pop.open);
    assert!(h.app.ui.pop.open, "a press elsewhere dismissed the toast's popup");
}
