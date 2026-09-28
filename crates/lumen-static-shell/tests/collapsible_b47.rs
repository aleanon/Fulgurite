//! **A section opens on Space, and its open body has room under it** —
//! M57/B47 (`docs/plan-widget-fixes.md`, `docs/widget-audit.md`
//! Accordion). Only Enter toggled, and the open body sat flush against
//! the section's bottom edge.

use lumen_static::event::Key;
use lumen_static::{App as DeriveApp, Event, StaticTree, Widget};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;
use lumen_static_widgets::Collapsible;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("sec")]
    sec: Collapsible<Label>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

#[test]
fn space_opens_it_and_the_body_does_not_touch_its_bottom() {
    let mut app = App::default();
    let mut l = Label::default();
    l.set("Body text");
    app.ui.sec = Collapsible::new("Section", l);
    let mut h: Host<App> = Host::new(app, 400, 300);
    h.pump();
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "sec").unwrap();
    h.input.focus = Some(i as u32);
    h.post(Event::Key { key: Key::Space, down: true });
    h.post(Event::Key { key: Key::Space, down: false });
    h.pump();
    assert!(h.app.ui.sec.is_open(), "Space did not open the section");
    let (sec, body) = (h.states[i], h.states[i + 1]);
    let gap = (sec.pos.y + sec.size.h) - (body.pos.y + body.size.h);
    println!("section bottom {}, body bottom {}, gap {gap}", sec.pos.y + sec.size.h, body.pos.y + body.size.h);
    assert!(gap >= 12.0 - 0.5, "the open body is {gap} px from the section's bottom");
}
