//! **A held Enter or Space activates once** — M57/A10
//! (`docs/plan-widget-fixes.md`).
//!
//! Both keys activate on the key-down, and a platform sends a down per
//! auto-repeat with one up at the end, so holding Enter pressed a button
//! once per repeat (`docs/widget-audit.md`, Button: "three Enter-downs gave
//! three presses").

use fulgurite::event::Key;
use fulgurite::{App as DeriveApp, Event, EventCtx, StaticTree};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Button;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Ui {
    #[widget_id("go")]
    go: Button<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    presses: u32,
}

fn focused_host() -> Host<App> {
    let mut app = App::default();
    app.ui.go.label = "Go";
    app.ui.go.on_click = |s: &mut App, _: &EventCtx| s.presses += 1;
    let mut h = Host::new(app, 200, 100);
    h.pump();
    h.post(Event::Key { key: Key::Tab, down: true });
    h.post(Event::Key { key: Key::Tab, down: false });
    h.pump();
    assert!(h.input.focus.is_some(), "setup: Tab did not focus the button");
    h
}

fn key(h: &mut Host<App>, key: Key, down: bool) {
    h.post(Event::Key { key, down });
    h.pump();
}

#[test]
fn a_held_key_activates_once_per_press() {
    for k in [Key::Enter, Key::Space] {
        let mut h = focused_host();
        // One press, auto-repeating twice.
        key(&mut h, k, true);
        key(&mut h, k, true);
        key(&mut h, k, true);
        key(&mut h, k, false);
        println!("{k:?}: three downs and an up -> {} press(es)", h.app.presses);
        assert_eq!(h.app.presses, 1, "{k:?} held down activated once per repeat");
        // The other direction: a second, real press activates again.
        key(&mut h, k, true);
        key(&mut h, k, false);
        assert_eq!(h.app.presses, 2, "{k:?}: a second press after the release did not activate");
    }
}
