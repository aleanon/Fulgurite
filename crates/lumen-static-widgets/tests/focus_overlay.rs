//! M27/T27.9 — does an open overlay steal the press meant for the
//! control that owns it?
//!
//! `just widget-drive` found a `Combobox` whose trigger would not take
//! focus from a synthesized press, while the showcase's plain
//! `TextInput` in the same window did. The one difference is that the
//! combobox has an **open popup**, and the shell dispatches through
//! `dispatch_overlaid` where the headless test used `dispatch`.

use lumen_static::event::{Event, InputState};
use lumen_static::overlay::{OverlaySink, Overlays};
use lumen_static::widget::Widget;
use lumen_static::{pass, Constraints, NodeState, StaticTree};
use lumen_static_widgets::Combobox;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("cb")]
    cb: Combobox,
}

#[derive(Default)]
struct App {
    ui: Ui,
}

impl lumen_static::event::App for App {
    // M32 prototype: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = lumen_static::event::Never;
    fn apply(&mut self, m: Self::Msg) -> bool {
        match m {}
    }
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

fn trigger_index() -> usize {
    <Ui as Widget>::SUB_TABLE.iter().position(|d| d.wid == "chooser-trigger").unwrap()
}

/// **A press on the trigger of an *open* list dismisses it and is
/// consumed** — and that is `Select`'s documented behaviour, not a
/// defect.
///
/// `dispatch_overlaid` treats any press that misses every overlay's own
/// rect as "outside", dismisses them all, clears focus and consumes the
/// event. A trigger is outside its own popup, so a second press on it
/// closes the list rather than re-opening it: *"the classic dropdown
/// bug, avoided by writing no code for it"* (`docs/plan-select.md`).
///
/// **The tension this records**: the rule cannot tell a press on the
/// overlay's *owner* from a press somewhere else entirely, and a
/// `Combobox` wants both — close the list *and* focus the field. It
/// cannot have both today, which is why the flow below is the one that
/// works and the showcase seeds its choosers closed. Found by
/// `just widget-drive`; a headless test using `dispatch` rather than
/// `dispatch_overlaid` cannot see it at all.
#[test]
fn a_press_on_the_trigger_of_an_open_list_dismisses_and_is_consumed() {
    let mut app = App::default();
    app.ui.cb = Combobox::new(&["Apple", "Banana", "Cherry"]);
    app.ui.cb.open();
    let styles = pass::default_styles(&app.ui);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut app.ui, &styles, &mut st, VIEW);
    let mut input = InputState::default();
    let mut ov: Overlays<8> = Overlays::default();
    {
        let mut list = lumen_static::DisplayList::new();
        pass::paint_overlaid(&app.ui, &st, &mut list, &mut ov);
    }
    assert!(!ov.is_empty(), "setup: the open list did not register as an overlay");

    let ti = trigger_index();
    let s = st[ti];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    let ev = Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 };
    let taken = pass::dispatch_overlaid(&mut app, &styles, &mut st, &mut input, &ov, &ev);
    assert!(taken, "the dismissing press was not consumed");
    assert!(!app.ui.cb.is_open(), "the press did not dismiss the list");
    assert_eq!(input.focus, None, "the dismissing press also focused something");
}

/// **And the flow that does work**: a *closed* chooser takes the press,
/// focuses its field and opens — which is how anyone meets one, and is
/// why the showcase seeds them closed.
#[test]
fn a_press_on_a_closed_chooser_focuses_its_field_and_opens_it() {
    let mut app = App::default();
    app.ui.cb = Combobox::new(&["Apple", "Banana", "Cherry"]);
    let styles = pass::default_styles(&app.ui);
    let mut st = vec![NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut app.ui, &styles, &mut st, VIEW);
    let mut input = InputState::default();
    let ov: Overlays<8> = Overlays::default();
    assert!(!app.ui.cb.is_open(), "setup: it starts open");

    let ti = trigger_index();
    let s = st[ti];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    for ev in [
        Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 },
        Event::PointerUp { x, y, button: lumen_static::PointerButton::Left },
    ] {
        pass::dispatch_overlaid(&mut app, &styles, &mut st, &mut input, &ov, &ev);
    }
    assert_eq!(input.focus, Some(ti as u32), "the press did not focus the field");
    assert!(st[ti].focused, "focus was set but never published into the state array");
    // **Laid out first.** A chooser opens in `layout`, because that is
    // where it can see `focused` — and `Host::pump` dispatches and then
    // lays out, so this is the same order a frame has.
    pass::layout(&mut app.ui, &styles, &mut st, VIEW);
    assert!(app.ui.cb.is_open(), "focusing the field did not open the list");

    // And typing now reaches the field and filters. Cleared first,
    // because typing appends at the caret like any field — the trigger
    // holds "Apple", and appending to it matches nothing.
    for _ in 0..16 {
        let ev = Event::Key { key: lumen_static::event::Key::Backspace, down: true };
        pass::dispatch_overlaid(&mut app, &styles, &mut st, &mut input, &ov, &ev);
    }
    for ch in ['b', 'a', 'n'] {
        let ev = Event::Text { ch };
        pass::dispatch_overlaid(&mut app, &styles, &mut st, &mut input, &ov, &ev);
    }
    pass::layout(&mut app.ui, &styles, &mut st, VIEW);
    assert_eq!(
        app.ui.cb.list.content.shown(), 1,
        "typing `ban` into the focused field left {} rows",
        app.ui.cb.list.content.shown()
    );
}
