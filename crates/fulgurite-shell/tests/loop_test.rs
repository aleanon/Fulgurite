//! The frame loop, headless: input in, pixels out. This is the shape a
//! winit front end wraps — it adds window events and a blit, nothing
//! else — so the loop's behaviour is pinned without a display.

use fulgurite_widgets::ops::{Button, Label, Spacer};
use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::{Event, EventCtx, StaticTree, Widget};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::{Host, CLEAR};

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

impl fulgurite::event::App for App {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = fulgurite::event::Never;
    fn apply(_: &mut Self, m: Self::Msg) -> bool {
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
    // Exactly what the tree draws: a label, a spacer (nothing), and a
    // button. A count, not `!is_empty()` — which passed for any
    // nonempty list, including a corrupted one.
    assert_eq!(h.list.len(), 3, "the label's text run, and the button's rect and label");
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

    // A direct write to the tree is marked by its writer. Unmarked, it
    // painted the new text into last frame's box under `Boundaries` (the
    // shadow oracle names exactly that, `tests/shadow.rs`), and since
    // M53/T53.3 an unmarked pump is skipped outright.
    h.app.ui.label.set("changed".into());
    assert!(h.invalidate("label"), "setup: #label is on the table");
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

    // A band assertion, not "> 0.0" — which passed for a single pixel.
    // The tree is a 4px-spaced column, so the top of the surface carries
    // the label and the bottom is empty.
    let ink = c.ink_fraction(CLEAR);
    assert!((0.01..0.5).contains(&ink), "plausible coverage for this tree, got {ink:.3}");
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
    assert_eq!(h.states[0].size.w, 80.0, "the root is exactly the new surface width");
    assert!(h.states[0].size.w < wide);
}

#[test]
fn a_stale_focus_index_does_not_panic_and_is_cleared() {
    // `Host.focus` is public and a hot reload can shrink the tree under
    // a focus set against the old one; `dispatch` then used it as a
    // `table[i]` index unchecked. It is checked where the index enters
    // now, which turns out to be *before* the PointerUp gate — so a
    // stale focus is dropped by the first event that consults it,
    // keyboard included, rather than surviving until M9 removes the
    // gate. F1.8.
    let mut h = host();
    h.input.focus = Some(9999);
    h.post(Event::Key { key: fulgurite::Key::Enter, down: true });
    h.pump();
    assert_eq!(h.input.focus, None, "an out-of-range focus is dropped, not indexed");

    // Since C5 (2026-09-04) the check runs before *any* routing, not
    // just the positionless arm, so even a pointer event that hits
    // nothing drops a stale index. Strictly earlier than F1.8 left it.
    h.input.focus = Some(u32::MAX);
    h.post(Event::up(-50.0, -50.0));
    h.pump();
    assert_eq!(h.input.focus, None, "any event validates the focus it might use");

    // And a focus that is in range *and on screen* is left alone. C5
    // added the second half: an index that is in range but names a node
    // with no area — a hidden `Either` branch, an absent `Option` — is
    // dropped too, because it would otherwise take every keystroke
    // invisibly.
    let last = (<Ui as Widget>::SUB_TABLE.len() - 1) as u32;
    h.input.focus = Some(last);
    h.post(Event::Key { key: fulgurite::Key::Enter, down: true });
    h.pump();
    assert_eq!(h.input.focus, Some(last), "a valid index survives");
}

#[test]
fn keyboard_events_route_to_focus() {
    let mut h = host();
    let st = h.states[<Ui as Widget>::SUB_TABLE.len() - 1];
    h.post(Event::down(st.pos.x + 1.0, st.pos.y + 1.0));
    h.pump();
    assert_eq!(
        h.input.focus,
        Some((<Ui as Widget>::SUB_TABLE.len() - 1) as u32),
        "pointer-down focused the button — the button, not merely something"
    );

    // No handler consumes keys yet, but routing does not panic and the
    // focus survives the frame.
    h.post(Event::Key { key: fulgurite::Key::Enter, down: true });
    h.pump();
    assert_eq!(h.input.focus, Some((<Ui as Widget>::SUB_TABLE.len() - 1) as u32), "and keeps it");
}
