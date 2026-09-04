//! T14.1: modifier state in the queue, and the budget that shape was
//! chosen to protect.

use lumen_static::event::InputState;
use lumen_static::ops::{Button, Label};
use lumen_static::{pass, Constraints, Event, EventCtx, Mods, NodeState, StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("label")]
    label: Label,
    #[widget_id("btn")]
    btn: Button<App>,
}

#[derive(lumen_static::App, Default)]
struct App {
    #[tree]
    ui: Ui,
    /// What the handler saw, so a test can assert on it.
    seen: Mods,
    clicks: u32,
}

fn on_click(a: &mut App, cx: &EventCtx) {
    a.seen = cx.mods;
    a.clicks += 1;
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

struct F {
    app: App,
    styles: Vec<lumen_static::Style>,
    st: [NodeState; Ui::NODE_COUNT],
    input: InputState,
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.btn.label = "go";
        app.ui.btn.on_click = on_click;
        let styles = pass::default_styles(&app.ui);
        let mut f = F { app, styles, st: [NodeState::default(); Ui::NODE_COUNT], input: InputState::default() };
        pass::layout(&mut f.app.ui, &f.styles, &mut f.st, VIEW);
        f
    }

    fn send(&mut self, ev: Event) -> bool {
        pass::dispatch(&mut self.app, &self.styles, &mut self.st, &mut self.input, &ev)
    }

    /// A click on the button, which is node 2.
    fn click(&mut self) {
        let s = self.st[2];
        let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
        self.send(Event::PointerDown { x, y });
        self.send(Event::PointerUp { x, y });
    }
}

/// **The budget this design exists to protect.**
///
/// `InputQueue` is `CAP * size_of::<Event>()`, so a vocabulary change is
/// a memory change. Adding a modifier *field* to every variant would
/// have grown the enum and the queue with it; a `Modifiers` variant
/// does not. Measured before the design was written, and asserted here
/// so it stays true (`docs/plan-desktop.md` T14.1).
#[test]
fn the_event_vocabulary_still_fits_in_sixteen_bytes() {
    assert_eq!(core::mem::size_of::<Event>(), 16, "Event grew; InputQueue grew with it");
    assert_eq!(core::mem::size_of::<Mods>(), 1, "Mods is a bitflag, not four bools");
    assert_eq!(core::mem::size_of::<lumen_static::InputQueue>(), 1048);
}

#[test]
fn a_handler_sees_the_modifiers_that_were_held() {
    let mut f = F::new();
    f.send(Event::Modifiers(Mods::CTRL.with(Mods::SHIFT)));
    f.click();
    assert_eq!(f.app.clicks, 1);
    assert!(f.app.seen.has(Mods::CTRL), "ctrl reached the handler");
    assert!(f.app.seen.has(Mods::SHIFT));
    assert!(!f.app.seen.has(Mods::ALT));
}

#[test]
fn modifiers_persist_until_changed_and_then_clear() {
    let mut f = F::new();
    f.send(Event::Modifiers(Mods::ALT));
    f.click();
    assert!(f.app.seen.has(Mods::ALT));

    // Released: the next click sees nothing. State, not an edge.
    f.send(Event::Modifiers(Mods::NONE));
    f.click();
    assert!(f.app.seen.is_empty(), "a released modifier stops being reported");
}

#[test]
fn the_modifiers_event_is_consumed_and_never_reaches_a_handler() {
    let mut f = F::new();
    assert!(f.send(Event::Modifiers(Mods::CTRL)), "handled, so nothing chains on it");
    assert_eq!(f.app.clicks, 0, "it is state, not an activation");
}

/// The rule T14.2's text suppression is built on.
#[test]
fn is_shortcut_excludes_shift_alone() {
    assert!(!Mods::NONE.is_shortcut());
    assert!(!Mods::SHIFT.is_shortcut(), "Shift+A is how a capital arrives");
    assert!(Mods::CTRL.is_shortcut());
    assert!(Mods::ALT.is_shortcut());
    assert!(Mods::META.is_shortcut());
    assert!(Mods::CTRL.with(Mods::SHIFT).is_shortcut(), "Ctrl+Shift+A is still a shortcut");
}

#[test]
fn has_none_is_always_true() {
    // A shortcut table asks "does this need no modifiers"; the answer
    // for the empty set must be yes, not "only when nothing is held".
    assert!(Mods::CTRL.has(Mods::NONE));
    assert!(Mods::NONE.has(Mods::NONE));
    assert!(!Mods::NONE.has(Mods::CTRL));
}
