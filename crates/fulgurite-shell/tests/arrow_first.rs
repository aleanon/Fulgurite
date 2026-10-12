//! **An arrow goes to the focused widget before it moves focus** —
//! M57/A11 (`docs/plan-widget-fixes.md`).
//!
//! Arrow-key grouping ran before any widget saw the key, whenever the
//! focused node's parent had more than one focusable child. So a text
//! field in a row beside a button lost Left and Right to focus movement
//! (`docs/widget-audit.md` X11). The showcase never put a field beside
//! another control, which is why it was latent there.

use fulgurite::event::Key;
use fulgurite::{App as DeriveApp, Event, StaticTree};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Button;
use fulgurite_widgets::TextInput;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Ui {
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("go")]
    go: Button<App>,
    #[widget_id("other")]
    other: Button<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn index(wid: &str) -> u32 {
    fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, wid).unwrap() as u32
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.field = TextInput::new("hello");
    app.ui.go.label = "Go";
    app.ui.other.label = "Other";
    let mut h = Host::new(app, 500, 100);
    h.pump();
    h
}

fn key(h: &mut Host<App>, k: Key) -> bool {
    h.post(Event::Key { key: k, down: true });
    let handled = h.pump();
    h.post(Event::Key { key: k, down: false });
    h.pump();
    handled
}

#[test]
fn left_inside_a_field_beside_a_button_moves_the_caret_not_focus() {
    let mut h = host();
    h.input.focus = Some(index("field"));
    h.pump();
    let before = h.app.ui.field.caret();
    key(&mut h, Key::Left);
    println!("caret {before} -> {}, focus {:?} (field is {})", h.app.ui.field.caret(), h.input.focus, index("field"));
    assert_eq!(h.input.focus, Some(index("field")), "Left inside the field moved focus out of it");
    assert!(h.app.ui.field.caret() < before, "Left did not move the caret");
}

/// The other direction: between two buttons, which decline arrows, the
/// arrow still moves focus through the group.
#[test]
fn right_between_buttons_still_moves_focus() {
    let mut h = host();
    h.input.focus = Some(index("go"));
    h.pump();
    key(&mut h, Key::Right);
    assert_eq!(h.input.focus, Some(index("other")), "Right on a button did not move to the next button");
}
