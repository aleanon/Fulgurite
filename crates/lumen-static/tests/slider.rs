//! M11/T11.2: the slider, and the pointer capture it needed.
//!
//! The property worth pinning is not that dragging works — it is that a
//! drag keeps reaching the control **after the pointer has left it**,
//! which is the half a plain hit test cannot do and the reason capture
//! exists.

use lumen_static::event::{Event, Key};
use lumen_static_widgets::slider::Slider;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 100.0, min_h: 0.0, max_h: 60.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("vol")]
    vol: Slider,
}

#[derive(Default)]
struct App {
    ui: Ui,
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

struct F {
    app: App,
    styles: Vec<lumen_static::Style>,
    st: [NodeState; Ui::NODE_COUNT],
    input: lumen_static::event::InputState,
}

impl F {
    fn new() -> Self {
        let app = App::default();
        let mut styles = pass::default_styles(&app.ui);
        lumen_static::style::cascade_deep(
            &mut App::default().ui,
            &[Rule::new(Selector::Id("vol"), Patch::new().width(Length::Fixed(100.0)))],
            &mut styles,
        );
        let mut f =
            F { app, styles, st: [NodeState::default(); Ui::NODE_COUNT], input: Default::default() };
        f.layout();
        f
    }
    fn layout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.st, VIEW);
    }
    fn send(&mut self, ev: Event) -> bool {
        let r = pass::dispatch(
            &mut self.app,
            &self.styles,
            &mut self.st,
            &mut self.input,
            &ev,
        );
        self.layout();
        r
    }
    fn v(&self) -> f32 {
        self.app.ui.vol.value()
    }
    /// Absolute point at a fraction along the track.
    fn at(&self, f: f32) -> (f32, f32) {
        let s = &self.st[1];
        (s.pos.x + s.size.w * f, s.pos.y + s.size.h / 2.0)
    }
}

#[test]
fn pressing_the_track_jumps_to_that_value() {
    let mut f = F::new();
    assert_eq!(f.v(), 0.0);
    let (x, y) = f.at(0.5);
    assert!(f.send(Event::down(x, y)));
    assert!((f.v() - 0.5).abs() < 0.02, "got {}", f.v());
}

#[test]
fn a_drag_keeps_tracking_after_the_pointer_leaves_the_control() {
    // The whole reason capture exists. Without it the move hit-tests to
    // nothing and the slider stops following the finger — the bug every
    // hand-rolled slider has.
    let mut f = F::new();
    let (x, y) = f.at(0.5);
    f.send(Event::down(x, y));
    assert_eq!(f.input.capture, Some(1), "the press claimed the gesture");

    // Far below the control, and far to the right of it.
    f.send(Event::PointerMove { x: x + 5000.0, y: y + 5000.0 });
    assert_eq!(f.v(), 1.0, "still tracking off-control");

    f.send(Event::PointerMove { x: -5000.0, y: y + 5000.0 });
    assert_eq!(f.v(), 0.0, "and in the other direction");

    f.send(Event::up(-5000.0, y + 5000.0));
    assert_eq!(f.input.capture, None, "the release gives the gesture back");

    // A move after the release changes nothing.
    let before = f.v();
    f.send(Event::PointerMove { x, y });
    assert_eq!(f.v(), before, "no capture, no tracking");
}

#[test]
fn a_move_with_nothing_captured_is_not_routed() {
    // M9's objection preserved: moves do not walk the tree looking for
    // a handler. With no capture the slider never sees one.
    let mut f = F::new();
    let (x, y) = f.at(0.9);
    assert!(!f.send(Event::PointerMove { x, y }), "unrouted");
    assert_eq!(f.v(), 0.0, "and the slider did not move");
}

#[test]
fn arrow_keys_step_it_so_it_is_reachable_without_a_pointer() {
    let mut f = F::new();
    let (x, y) = f.at(0.0);
    f.send(Event::down(x, y));
    f.send(Event::up(x, y));
    assert_eq!(f.input.focus, Some(1));

    assert!(f.send(Event::Key { key: Key::Right, down: true }));
    assert!((f.v() - 0.05).abs() < 1e-6, "one step: {}", f.v());
    f.send(Event::Key { key: Key::End, down: true });
    assert_eq!(f.v(), 1.0);
    assert!(!f.send(Event::Key { key: Key::Right, down: true }), "at the end, unconsumed");
    f.send(Event::Key { key: Key::Home, down: true });
    assert_eq!(f.v(), 0.0);
}

#[test]
fn the_value_is_clamped_and_never_non_finite() {
    // A non-finite value would reach `NodeState` through the knob's rect
    // and make every `contains` against it false.
    let mut s = Slider::new(f32::NAN);
    assert_eq!(s.value(), 0.0);
    s.set_value(f32::INFINITY);
    assert_eq!(s.value(), 1.0, "inf clamps rather than propagating");
    s.set_value(-3.0);
    assert_eq!(s.value(), 0.0);
    assert!(!s.set_value(0.0), "an unchanged value is not a change");
}

#[test]
fn it_meets_the_pointer_target_floor() {
    let f = F::new();
    assert!(f.st[1].size.h >= 24.0, "{:?}", f.st[1].size);
}
