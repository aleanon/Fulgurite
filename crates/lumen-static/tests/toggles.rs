//! M11/T11.1: checkbox, radio and switch.

use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::toggle::{CheckBox, Radio, Switch};
use lumen_static::widget::Node;
use lumen_static::{pass, Cmd, Constraints, DisplayList, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 200.0 };

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct Ui {
    #[widget_id("check")]
    check: CheckBox<App>,
    #[widget_id("radio")]
    radio: Radio<App>,
    #[widget_id("switch")]
    switch: Switch<App>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    flips: u32,
}

impl lumen_static::event::App for App {
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

fn on_flip(a: &mut App, _: &EventCtx) {
    a.flips += 1;
    // The application owns the truth; the binding would write `on` back.
    a.ui.check.on = !a.ui.check.on;
}

struct F {
    app: App,
    styles: Vec<lumen_static::Style>,
    st: [NodeState; Ui::NODE_COUNT],
    focus: Option<u32>,
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.check.on_toggle = on_flip;
        app.ui.radio.on_toggle = on_flip;
        app.ui.switch.on_toggle = on_flip;
        let styles = pass::default_styles(&app.ui);
        let mut f = F { app, styles, st: [NodeState::default(); Ui::NODE_COUNT], focus: None };
        f.layout();
        f
    }
    fn layout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.st, VIEW);
    }
    fn send(&mut self, ev: Event) -> bool {
        let r = pass::dispatch(&mut self.app, &self.styles, &mut self.st, &mut self.focus, &ev);
        self.layout();
        r
    }
    fn centre(&self, i: usize) -> (f32, f32) {
        let s = &self.st[i];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    }
    fn rects(&self) -> usize {
        let mut l = DisplayList::new();
        pass::paint(&self.app.ui, &self.st, &mut l);
        l.cmds.iter().filter(|c| matches!(c, Cmd::RoundedRect { .. })).count()
    }
}

#[test]
fn every_toggle_meets_the_pointer_target_floor_by_default() {
    // A control that ships below its own accessibility floor has the
    // wrong default. 24 logical px is WCAG 2.5.8, the same number
    // `scripts/ui_check.py targets` checks.
    let f = F::new();
    for i in 1..Ui::NODE_COUNT {
        assert!(
            f.st[i].size.w >= 24.0 && f.st[i].size.h >= 24.0,
            "node {i} is {:?}, under the 24 px target floor",
            f.st[i].size
        );
    }
}

#[test]
fn the_shape_is_a_type_not_a_field() {
    // Why it matters: `kind_name` feeds the cascade and the agent's
    // selectors, and `defaults` is snapshotted once when a stylesheet is
    // set. A shape in a field would make both state-dependent — the
    // hazard `Option::defaults` was fixed for (F3.2).
    let f = F::new();
    assert_eq!(f.app.ui.check.kind_name(), "checkbox");
    assert_eq!(f.app.ui.radio.kind_name(), "radio");
    assert_eq!(f.app.ui.switch.kind_name(), "switch");
    // And a radio is round by default while a checkbox is not.
    assert!(f.styles[2].radius > f.styles[1].radius, "radio is round");
}

#[test]
fn clicking_runs_the_apps_handler_and_the_widget_does_not_flip_itself() {
    // The C3 contract: a widget that flipped itself would consume the
    // event and the app's handler would never run, leaving two copies
    // of one boolean free to disagree.
    let mut f = F::new();
    assert!(!f.app.ui.check.on);
    let (x, y) = f.centre(1);
    f.send(Event::PointerDown { x, y });
    assert!(!f.app.ui.check.on, "nothing happens on press");
    f.send(Event::PointerUp { x, y });
    assert_eq!(f.app.flips, 1, "the app's handler ran");
    assert!(f.app.ui.check.on, "and it is the app that set the copy");
}

#[test]
fn space_activates_the_focused_toggle() {
    // The convention, and the reason `Key::Space` was added in T11.1.
    let mut f = F::new();
    let (x, y) = f.centre(1);
    f.send(Event::PointerDown { x, y });
    f.send(Event::PointerUp { x, y });
    assert_eq!(f.focus, Some(1), "pointer-down focused it");
    let before = f.app.flips;

    assert!(f.send(Event::Key { key: Key::Space, down: true }), "Space activates");
    assert_eq!(f.app.flips, before + 1);
    assert!(!f.send(Event::Key { key: Key::Space, down: false }), "release does not");
    assert_eq!(f.app.flips, before + 1);
}

#[test]
fn an_on_toggle_paints_more_than_an_off_one() {
    // The mark is what changes; the body is always drawn.
    let mut f = F::new();
    let off = f.rects();
    f.app.ui.check.on = true;
    f.app.ui.radio.on = true;
    f.app.ui.switch.on = true;
    let on = f.rects();
    assert!(on > off, "an on toggle draws its mark: {off} -> {on}");
}

#[test]
fn a_switch_knob_moves_from_one_end_to_the_other() {
    let mut f = F::new();
    let knob = |f: &F| {
        let mut l = DisplayList::new();
        pass::paint(&f.app.ui, &f.st, &mut l);
        // The switch is the last node; its knob is the last rect.
        l.cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::RoundedRect { rect, .. } => Some(rect[0]),
                _ => None,
            })
            .next_back()
            .unwrap()
    };
    let left = knob(&f);
    f.app.ui.switch.on = true;
    let right = knob(&f);
    assert!(right > left, "the knob travels: {left} -> {right}");
}

#[test]
fn a_stylesheet_reaches_them_by_kind_and_by_id() {
    let mut app = App::default();
    let mut styles = pass::default_styles(&app.ui);
    lumen_static::style::cascade_deep(
        &mut app.ui,
        &[
            Rule::new(Selector::Kind("switch"), Patch::new().radius(3.0)),
            Rule::new(Selector::Id("check"), Patch::new().radius(9.0)),
        ],
        &mut styles,
    );
    assert_eq!(styles[1].radius, 9.0, "#check by id");
    assert_eq!(styles[3].radius, 3.0, "switch by kind");
    assert_ne!(styles[2].radius, 3.0, "and the radio is untouched");
}

#[test]
fn both_states_are_visible_with_no_stylesheet() {
    // The defect this pins: `Style::background` is transparent by
    // default and `rounded_rect` drops a transparent command, so the
    // first version of `Toggle::paint` — which inverted fg/bg — drew an
    // "on" switch with no knob. Found by the knob-travel test below,
    // which measured the body and called it the knob.
    let mut f = F::new();
    let count = |f: &F| {
        let mut l = DisplayList::new();
        pass::paint(&f.app.ui, &f.st, &mut l);
        l.cmds.len()
    };
    let off = count(&f);
    f.app.ui.check.on = true;
    f.app.ui.radio.on = true;
    f.app.ui.switch.on = true;
    let on = count(&f);
    // Off: three bodies plus the switch's knob, which is always drawn
    // and only moves. On: those four, plus the checkbox's two-stroke
    // tick and the radio's dot. The exact numbers matter less than the
    // property — nothing vanishes in either state.
    assert_eq!(off, 4, "three bodies and the switch knob");
    assert!(on > off + 2, "plus a stepped tick and a dot: {off} -> {on}");
}
