//! The frame loop, headless: input in, pixels out. This is the shape a
//! winit front end wraps — it adds window events and a blit, nothing
//! else — so the loop's behaviour is pinned without a display.

use lumen_static::ops::{Button, Label, Spacer};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{Event, EventCtx, StaticTree, Widget};
use lumen_static_shell::software::Canvas;
use lumen_static_shell::{Host, CLEAR};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("label")]
    label: Label<String>,
    gap: Spacer,
    #[widget_id("go")]
    go: Button<App>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    clicks: u32,
}

impl lumen_static::event::App for App {
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

fn on_go(a: &mut App, _: &EventCtx) {
    a.clicks += 1;
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.label.set("ready".into());
    app.ui.go.label = "go";
    app.ui.go.on_click = on_go;
    let mut h = Host::new(app, 200, 120);
    h.pump();
    h
}

#[test]
fn a_frame_lays_out_and_paints() {
    let h = host();
    assert_eq!(h.frame_number(), 1);
    assert!(!h.list.is_empty(), "the frame produced drawing commands");
    let (layout_ns, paint_ns) = h.timings();
    assert!(layout_ns > 0 && paint_ns > 0, "both phases were timed");
}

#[test]
fn input_reaches_handlers_through_the_queue() {
    let mut h = host();
    // Ask the laid-out tree where the button is rather than guessing.
    let st = h.states[<Ui as Widget>::SUB_TABLE.len() - 1];
    let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);

    h.click(x, y);
    h.pump();
    assert_eq!(h.app.clicks, 1);

    // Everything went through the queue, so a miss simply misses.
    h.click(-10.0, -10.0);
    h.pump();
    assert_eq!(h.app.clicks, 1);
}

#[test]
fn the_update_step_reaches_the_screen() {
    let mut h = host();
    let before = h.list.hash_from(0);

    h.app.ui.label.set("changed".into());
    h.pump();
    assert_ne!(h.list.hash_from(0), before, "new state paints differently");
}

#[test]
fn a_stylesheet_changes_layout_without_touching_the_tree() {
    let mut h = host();
    let natural = h.states[1].size.w;

    h.set_stylesheet(&[Rule::new(
        Selector::Id("label"),
        Patch::new().width(Length::Fixed(150.0)),
    )]);
    h.pump();
    assert_eq!(h.states[1].size.w, 150.0);
    assert_ne!(h.states[1].size.w, natural);
}

#[test]
fn rendering_produces_pixels() {
    let h = host();
    let mut c = Canvas::new(200, 120);
    c.clear(CLEAR);
    h.render(&mut c);

    assert!(c.ink_fraction(CLEAR) > 0.0, "something was drawn");
    let ppm = c.to_ppm();
    assert!(ppm.starts_with(b"P6\n200 120\n255\n"));
    assert_eq!(ppm.len(), 15 + 200 * 120 * 3);
}

#[test]
fn resizing_relayouts() {
    let mut h = host();
    let wide = h.states[0].size.w;
    h.resize(80, 120);
    h.pump();
    assert!(h.states[0].size.w < wide, "the tree follows the surface");
}

#[test]
fn keyboard_events_route_to_focus() {
    let mut h = host();
    let st = h.states[<Ui as Widget>::SUB_TABLE.len() - 1];
    h.post(Event::PointerDown { x: st.pos.x + 1.0, y: st.pos.y + 1.0 });
    h.pump();
    assert!(h.focus.is_some(), "pointer-down focused the button");

    // No handler consumes keys yet, but routing does not panic and the
    // focus survives the frame.
    h.post(Event::Key { key: lumen_static::Key::Enter, down: true });
    h.pump();
    assert!(h.focus.is_some());
}
