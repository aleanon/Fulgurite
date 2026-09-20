//! Plan E0–E3: queue, hit testing, dispatch, focus, and the async seam.

use lumen_static::inbox::Inbox;
use lumen_static::ops::{Button, Fixed, Label};
use lumen_static::repeat::Repeat;
use lumen_static::{pass, Constraints, Event, EventCtx, InputQueue, Key, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

// The app state IS the tree here, plus a counter beside it — the shape
// `docs/plan-events.md` targets.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct RowT {
    icon: Fixed,
    pick: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
    #[widget_id("save")]
    save: Button<App>,
    #[widget_id("list")]
    list: Repeat<RowT>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    clicks: u32,
    picked: Vec<u64>,
}

impl lumen_static::event::App for App {
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

fn on_save(app: &mut App, _: &EventCtx) {
    app.clicks += 1;
}

/// The payload a closure would have captured arrives as an argument.
fn on_pick(app: &mut App, ctx: &EventCtx) {
    app.picked.push(ctx.row.expect("row arrives from hit testing"));
}

fn bind_row(r: &mut RowT, _row: usize) {
    r.icon.w = 10.0;
    r.icon.h = 10.0;
    r.pick.label = "p";
    r.pick.on_click = on_pick;
}

struct Fixture {
    app: App,
    states: [NodeState; Ui::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    input: lumen_static::event::InputState,
}

impl Fixture {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.head.set("head");
        app.ui.save.label = "save";
        app.ui.save.on_click = on_save;
        app.ui.list = Repeat::new(100, 20.0, bind_row);
        let styles = pass::default_styles(&app.ui);
        let mut f = Fixture {
            app,
            states: [NodeState::default(); Ui::NODE_COUNT],
            styles,
            input: Default::default(),
        };
        f.layout();
        f
    }

    fn layout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.states, VIEW);
    }

    fn send(&mut self, ev: Event) -> bool {
        let handled =
            pass::dispatch(&mut self.app, &self.styles, &mut self.states, &mut self.input, &ev);
        self.layout();
        handled
    }

    fn click(&mut self, x: f32, y: f32) -> bool {
        self.send(Event::down(x, y));
        self.send(Event::up(x, y))
    }

    fn center_of(&self, node: usize) -> (f32, f32) {
        let s = &self.states[node];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    }
}

#[test]
fn e0_queue_is_fifo_and_round_trips_for_replay() {
    let mut q: InputQueue = InputQueue::new();
    q.push(Event::down(1.0, 2.0));
    q.push(Event::Key { key: Key::Enter, down: true });
    q.push(Event::Text { ch: 'x' });
    assert_eq!(q.len(), 3);

    // Recording is a dev concern, so the derives ride the observe gate —
    // the vocabulary itself is always present.
    #[cfg(feature = "observe")]
    {
        let recorded: Vec<Event> = q.iter().copied().collect();
        let json = serde_json::to_string(&recorded).unwrap();
        let replayed: Vec<Event> = serde_json::from_str(&json).unwrap();
        assert_eq!(recorded, replayed, "a session serializes and comes back");
    }

    assert_eq!(q.pop(), Some(Event::down(1.0, 2.0)));
    assert_eq!(q.pop(), Some(Event::Key { key: Key::Enter, down: true }));
    assert_eq!(q.pop(), Some(Event::Text { ch: 'x' }));
    assert!(q.is_empty());
}

#[test]
fn e1_hit_test_finds_the_deepest_node_and_agrees_with_layout() {
    let f = Fixture::new();
    let (x, y) = f.center_of(2); // the save button
    let h = pass::hit_test(&f.app.ui, &f.states, x, y).unwrap();
    assert_eq!(h.node, 2);
    assert!(h.inner.is_none());
    // Local coordinates are relative to the node's own origin.
    assert_eq!(h.local.x, x - f.states[2].pos.x);

    assert!(pass::hit_test(&f.app.ui, &f.states, 9e5, 9e5).is_none());
}

#[test]
fn e1_hit_test_descends_into_repeater_rows() {
    let mut f = Fixture::new();
    f.app.ui.list.scroll = 40.0; // rows 2.. are on screen
    f.layout();

    let list = &f.states[3];
    let h = pass::hit_test(&f.app.ui, &f.states, list.pos.x + 4.0, list.pos.y + 4.0).unwrap();
    assert_eq!(h.node, 3, "the repeater owns the point");
    let inner = h.inner.expect("resolved into a row");
    assert_eq!(inner.row, 2, "scrolled two rows down");
}

#[test]
fn e2_click_mutates_state_through_the_tree() {
    let mut f = Fixture::new();
    let (x, y) = f.center_of(2);
    assert!(f.click(x, y));
    assert_eq!(f.app.clicks, 1);
    assert!(f.click(x, y));
    assert_eq!(f.app.clicks, 2);

    // A click on a non-interactive node is not consumed.
    let (hx, hy) = f.center_of(1);
    assert!(!f.click(hx, hy));
    assert_eq!(f.app.clicks, 2);
}

#[test]
fn e2_row_click_receives_the_right_row_after_scrolling() {
    let mut f = Fixture::new();

    // The button is template node 2 (row, icon, pick) of window slot 0.
    // Ask the repeater where it actually is rather than assuming.
    let btn = f.app.ui.list.instance_state(0, 2);
    let (x, y) = (btn.pos.x + btn.size.w / 2.0, btn.pos.y + btn.size.h / 2.0);

    assert!(f.click(x, y));
    assert_eq!(f.app.picked, vec![0]);

    f.app.ui.list.scroll = 100.0; // first visible row is now 5
    f.layout();
    let btn = f.app.ui.list.instance_state(0, 2);
    let (x, y) = (btn.pos.x + btn.size.w / 2.0, btn.pos.y + btn.size.h / 2.0);
    assert!(f.click(x, y));
    assert_eq!(f.app.picked, vec![0, 5], "the row is recomputed, never captured");
}

#[test]
fn e2_focus_follows_pointer_down_and_routes_keys() {
    let mut f = Fixture::new();
    assert!(f.input.focus.is_none());

    let (x, y) = f.center_of(2);
    f.send(Event::down(x, y));
    assert_eq!(f.input.focus, Some(2), "the button is focusable");

    // Enter on the focused node activates it (C1, 2026-09-04): a button
    // is reachable without a pointer at all, which is what makes the
    // agent's `input.key` and a screen reader's activation work. Before
    // C1 `dispatch` dropped every event that was not `PointerUp`.
    let before = f.app.clicks;
    assert!(f.send(Event::Key { key: Key::Enter, down: true }), "Enter activates the focus");
    assert_eq!(f.app.clicks, before + 1, "and runs the click handler exactly once");

    // A key *release* is not an activation.
    assert!(!f.send(Event::Key { key: Key::Enter, down: false }));
    assert_eq!(f.app.clicks, before + 1);

    // Tab is consumed by focus movement, never routed (C5).
    assert!(f.send(Event::Key { key: Key::Tab, down: true }), "Tab moves focus");
    assert_eq!(f.app.clicks, before + 1, "and does not activate anything");

    // Clicking a non-focusable area clears focus.
    let (hx, hy) = f.center_of(1);
    f.send(Event::down(hx, hy));
    assert_eq!(f.input.focus, None);
    assert!(!f.send(Event::Text { ch: 'a' }), "no focus, nothing to route to");
}

#[test]
fn e3_inbox_carries_values_not_code_across_the_seam() {
    // The async seam: a worker sends data; the *currently loaded* apply
    // fn interprets it. Nothing code-shaped crosses.
    let inbox: Inbox<u32> = Inbox::new();
    let tx = inbox.sender();
    let worker = std::thread::spawn(move || {
        for i in 1..=3 {
            tx.send(i).unwrap();
        }
    });
    worker.join().unwrap();

    let mut app = App::default();
    fn apply(app: &mut App, m: u32) {
        app.clicks += m;
    }
    assert_eq!(inbox.drain(&mut app, apply), 3);
    assert_eq!(app.clicks, 6);
    assert_eq!(inbox.drain(&mut app, apply), 0, "drained");
}

// ---- C6: pressed and hovered ----------------------------------------

#[test]
fn a_button_is_pressed_while_the_pointer_is_down_on_it() {
    let mut f = Fixture::new();
    let (x, y) = f.center_of(2);
    assert!(!f.states[2].pressed, "resting");

    f.send(Event::down(x, y));
    assert!(f.states[2].pressed, "held");
    assert!(f.states[2].hovered, "and hovered");

    f.send(Event::up(x, y));
    assert!(!f.states[2].pressed, "released");
}

#[test]
fn releasing_away_from_a_control_still_un_presses_it() {
    // The reason `PointerUp` clears every node rather than the hit one:
    // press a button, drag off it, let go. A control that stays lit
    // after that is the classic stuck-button bug.
    let mut f = Fixture::new();
    let (x, y) = f.center_of(2);
    f.send(Event::down(x, y));
    assert!(f.states[2].pressed);

    f.send(Event::up(5000.0, 5000.0));
    assert!(!f.states[2].pressed, "released off the control");
}

#[test]
fn hover_follows_the_pointer_and_only_one_node_holds_it() {
    let mut f = Fixture::new();
    let (bx, by) = f.center_of(2);
    f.send(Event::PointerMove { x: bx, y: by });
    assert!(f.states[2].hovered);
    assert_eq!(f.states.iter().filter(|s| s.hovered).count(), 1, "exactly one");

    f.send(Event::PointerMove { x: 5000.0, y: 5000.0 });
    assert_eq!(f.states.iter().filter(|s| s.hovered).count(), 0, "off the tree, nothing hovered");
}

#[test]
fn the_pressed_shade_reaches_the_paint_attributes() {
    // C6's point: `paint` has no `&[Style]`, so the affordance has to be
    // resolved into `NodeState` during layout — the same A2-revised
    // mechanism the colours use.
    let mut f = Fixture::new();
    f.layout();
    let resting = f.states[2].bg;

    let (x, y) = f.center_of(2);
    f.send(Event::down(x, y));
    f.layout();
    let pressed = f.states[2].bg;
    assert_ne!(pressed, resting, "a held button paints differently");

    // Releasing *on* the control leaves it hovered, which is a third
    // shade — not the resting one. Pressed wins over hovered while held.
    f.send(Event::up(x, y));
    f.layout();
    let hovered = f.states[2].bg;
    assert_ne!(hovered, resting, "still under the pointer");
    assert_ne!(hovered, pressed, "but no longer held");

    // Off the control entirely: back to rest.
    f.send(Event::PointerMove { x: 5000.0, y: 5000.0 });
    f.layout();
    assert_eq!(f.states[2].bg, resting, "and goes back");
}


// ---- C5: focus that is not on screen, and Tab order ------------------

#[test]
fn focus_is_dropped_when_the_focused_node_leaves_the_screen() {
    // The scenario C5 names: focus a control, hide it, keep typing. The
    // hidden node is still in the table and still focusable by index, so
    // without the check it takes every keystroke invisibly.
    let mut f = Fixture::new();
    let (x, y) = f.center_of(2);
    f.send(Event::down(x, y));
    f.send(Event::up(x, y));
    assert_eq!(f.input.focus, Some(2));

    // Hide it the way `Option`, `Either` and a hidden subtree all do:
    // zero geometry, with `Style::visible` untouched.
    f.states[2].size = lumen_static::Size::default();
    let before = f.app.clicks;
    assert!(!f.send(Event::Key { key: Key::Enter, down: true }), "nothing focused now");
    assert_eq!(f.input.focus, None, "focus was dropped");
    assert_eq!(f.app.clicks, before, "and the hidden control was not activated");
}

#[test]
fn tab_walks_the_focusables_in_declaration_order_and_wraps() {
    let mut f = Fixture::new();
    f.layout();
    assert_eq!(f.input.focus, None);

    // Collect where Tab goes, until it comes back.
    let mut seen = Vec::new();
    for _ in 0..8 {
        assert!(f.send(Event::Key { key: Key::Tab, down: true }));
        let Some(n) = f.input.focus else { break };
        if seen.contains(&n) {
            break;
        }
        seen.push(n);
    }
    assert!(!seen.is_empty(), "something is focusable");
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "declaration order: {seen:?}");
    // Wrapped: another Tab from the last returns to the first.
    assert!(f.send(Event::Key { key: Key::Tab, down: true }));
    assert_eq!(f.input.focus, Some(seen[0]), "wraps to the first");
}

#[test]
fn tab_skips_a_focusable_with_no_area() {
    let mut f = Fixture::new();
    f.layout();
    f.send(Event::Key { key: Key::Tab, down: true });
    let first = f.input.focus.expect("something focusable");

    // Collapse it; Tab from nothing must now land elsewhere.
    f.input.focus = None;
    f.states[first as usize].size = lumen_static::Size::default();
    f.send(Event::Key { key: Key::Tab, down: true });
    assert_ne!(f.input.focus, Some(first), "a Tab stop you cannot see is not a Tab stop");
}
