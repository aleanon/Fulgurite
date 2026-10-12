//! **A hidden node loses capture** — M44/T44.3
//! (`docs/plan-input-semantics.md` §1.3), closing R3-F4.
//!
//! C5 drops focus from a node that is invisible or has no area; capture
//! had no such check. A slider pressed and then hidden mid-drag — its
//! `Either` switched to the other branch — kept receiving moves and
//! changing value until the release, invisibly.

use fulgurite::event::Event;
use fulgurite::hatch::{Either, Side};
use fulgurite::style::{Length, Patch, Rule, Selector};
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
use fulgurite_widgets::ops::Label;
use fulgurite_widgets::slider::Slider;

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 60.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    sw: Either<Slider, Label>,
}

#[derive(Default)]
struct App {
    ui: Ui,
}

impl fulgurite::event::App for App {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
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

/// The slider's table index: root 0, the `Either` 1, branch A's root 2.
const SLIDER: usize = 2;

struct F {
    app: App,
    styles: Vec<fulgurite::Style>,
    st: [NodeState; Ui::NODE_COUNT],
    input: fulgurite::event::InputState,
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.sw.a = Slider::new(0.1);
        app.ui.sw.b.set("hidden");
        let mut styles = pass::default_styles(&app.ui);
        fulgurite::style::cascade_deep(
            &mut app.ui,
            &[Rule::new(Selector::Kind("slider"), Patch::new().width(Length::Fixed(100.0)))],
            &mut styles,
        );
        let mut f = F { app, styles, st: [NodeState::default(); Ui::NODE_COUNT], input: Default::default() };
        f.layout();
        f
    }
    fn layout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.st, VIEW);
    }
    fn send(&mut self, ev: Event) -> bool {
        let r = pass::dispatch(&mut self.app, &self.styles, &mut self.st, &mut self.input, &ev);
        self.layout();
        r
    }
}

#[test]
fn a_slider_hidden_mid_drag_stops_tracking() {
    let mut f = F::new();
    assert_eq!(<Ui as Widget>::SUB_TABLE.len(), 4, "setup: root, either, slider, label");
    let s = f.st[SLIDER];
    assert!(s.size.w > 0.0, "setup: the slider has area before the switch ({:?})", s.size);
    // Press at the slider's current value, so the press itself leaves it at 0.1.
    let (x, y) = (s.pos.x + s.size.w * 0.1, s.pos.y + s.size.h / 2.0);
    assert!(f.send(Event::down(x, y)), "setup: the press was not handled");
    assert_eq!(f.input.capture, Some(SLIDER as u32), "setup: the press did not capture the slider");
    let start = f.app.ui.sw.a.value();

    // The switch hides the slider mid-drag.
    f.app.ui.sw.show(Side::B);
    f.layout();
    let hidden = f.st[SLIDER].size;
    assert!(hidden.w <= 0.0 || hidden.h <= 0.0, "setup: the switch did not hide the slider ({hidden:?})");

    f.send(Event::PointerMove { x: x + 80.0, y });
    let after = f.app.ui.sw.a.value();
    println!(
        "hidden slider, move +80: value {start} -> {after} (expected {start}); capture {:?} (expected None)",
        f.input.capture
    );
    assert_eq!(after, start, "a hidden slider kept tracking the drag");
    assert_eq!(f.input.capture, None, "capture outlived the node it captured");
}
