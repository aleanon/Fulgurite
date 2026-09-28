//! **Escape asks a freshly opened veil to dismiss** — M57/A13
//! (`docs/plan-widget-fixes.md`).
//!
//! The pass offers Escape to the topmost overlay through
//! `Node::dismiss` before any widget sees the key, and `Veil` did not
//! override it. So on a modal just opened, with nothing inside it
//! focused, Escape answered `handled: false` and did nothing; it worked
//! only after a control inside took focus and the key bubbled up
//! (`docs/widget-audit.md`, Modal). The widget's own test called
//! `on_event` with the key in hand, which is the path that worked.

use lumen_static::event::Key;
use lumen_static::{App as DeriveApp, Event, StaticTree};
use lumen_static_shell::Host;
use lumen_static_widgets::ops::Label;
use lumen_static_widgets::{Drawer, Modal, Sheet};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Dialog {
    #[widget_id("dialog_text")]
    t: Label,
}

#[derive(StaticTree, Default)]
#[widget(stack)]
struct Ui {
    #[widget_id("modal")]
    modal: Modal<Dialog>,
    #[widget_id("sheet")]
    sheet: Sheet<Dialog>,
    #[widget_id("drawer")]
    drawer: Drawer<Dialog>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

#[test]
fn escape_on_a_freshly_opened_veil_asks_to_dismiss() {
    for which in ["modal", "sheet", "drawer"] {
        let mut app = App::default();
        match which {
            "modal" => app.ui.modal.open(),
            "sheet" => app.ui.sheet.open(),
            _ => app.ui.drawer.open(),
        };
        let mut h: Host<App> = Host::new(app, 400, 300);
        h.pump();
        assert_eq!(h.open_overlays(), 1, "setup: the {which} did not open as an overlay");
        assert_eq!(h.input.focus, None, "setup: something is focused, so the key would bubble");
        h.post(Event::Key { key: Key::Escape, down: true });
        let handled = h.pump();
        let asked = match which {
            "modal" => h.app.ui.modal.take_dismissed(),
            "sheet" => h.app.ui.sheet.take_dismissed(),
            _ => h.app.ui.drawer.take_dismissed(),
        };
        println!("{which}: Escape handled={handled}, dismissal asked={asked}");
        assert!(handled && asked, "{which}: Escape with nothing focused did not ask to dismiss");
    }
}
