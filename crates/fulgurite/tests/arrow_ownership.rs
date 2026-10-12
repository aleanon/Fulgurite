//! **An arrow belongs to the widget that uses it** — M44/T44.1
//! (`docs/plan-input-semantics.md` §2.1), closing R3-F1 and R10-F3.
//!
//! The arrow-group rule (T15.6) moves focus within the focused node's
//! parent when that parent has more than one focusable child. It ran
//! *before* the widget, and never asked whether the widget uses arrows —
//! so a text field beside a button could not move its caret, a number
//! input could not be edited in the middle, and a slider could not step.
//! Each case below puts an arrow-driven widget beside a focusable sibling
//! and prints `(focus, caret)` beside the pair it expects.
//!
//! The `Host` legs (an unsplit and a split application, under
//! `Everything` and `Boundaries`) are in
//! `crates/fulgurite-shell/tests/arrow_ownership_host.rs`, because the
//! shell is not a dependency of this crate.

use fulgurite::event::InputState;
use fulgurite::{pass, Constraints, Event, Key, Mods, NodeState, StaticTree, Widget};
use fulgurite_widgets::ops::Button;
use fulgurite_widgets::{NumberInput, Slider, TextArea, TextInput};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct TextRow {
    field: TextInput,
    button: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct NumberRow {
    number: NumberInput,
    button: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
struct SliderRow {
    slider: Slider,
    button: Button<App>,
}

/// R10-F3's shape: three text widgets in one column, so each has two
/// focusable siblings.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Fields {
    first: TextInput,
    area: TextArea,
    last: TextInput,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
struct Ui {
    text: TextRow,
    number: NumberRow,
    slider: SliderRow,
    fields: Fields,
}

#[derive(fulgurite::App, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 600.0, min_h: 0.0, max_h: 600.0 };

struct F {
    app: App,
    styles: Vec<fulgurite::Style>,
    st: [NodeState; Ui::NODE_COUNT],
    input: InputState,
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.text.button.label = "ok";
        app.ui.number.button.label = "ok";
        app.ui.slider.button.label = "ok";
        app.ui.text.field.set_str("abc");
        app.ui.number.number = NumberInput::new(0.0, 0.0, 10.0, 1.0);
        app.ui.slider.slider = Slider::new(0.5);
        app.ui.fields.first.set_str("abc");
        app.ui.fields.area.set_str("one\ntwo");
        app.ui.fields.last.set_str("xyz");
        let styles = pass::default_styles(&app.ui);
        let mut f = F { app, styles, st: [NodeState::default(); Ui::NODE_COUNT], input: InputState::default() };
        f.relayout();
        f
    }

    fn relayout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.st, VIEW);
    }

    fn ev(&mut self, ev: Event) -> bool {
        let h = pass::dispatch(&mut self.app, &self.styles, &mut self.st, &mut self.input, &ev);
        // A layout between events, as a frame does: the text widgets
        // rebuild their clusters there, and a caret move reads them.
        self.relayout();
        h
    }

    fn key(&mut self, key: Key) -> bool {
        self.ev(Event::Key { key, down: true })
    }

    fn focus(&mut self, path: &str) -> u32 {
        let i = index(path);
        self.input.focus = Some(i);
        // Publish the bit the way the pass does, so a widget's layout
        // sees it before the first key.
        self.ev(Event::Modifiers(Mods::NONE));
        i
    }
}

/// The table index of a node by its field path, found by kind and order
/// rather than by a hard-coded number, so a reordered fixture fails here
/// and not in an assertion about the wrong node.
fn index(path: &str) -> u32 {
    let t = <Ui as Widget>::SUB_TABLE;
    // Children of the root, then of the named group.
    let groups: Vec<usize> = (0..t.len()).filter(|&i| t[i].parent == 0).collect();
    let (g, k) = match path {
        "text.field" => (0, 0),
        "text.button" => (0, 1),
        "number.number" => (1, 0),
        "number.button" => (1, 1),
        "slider.slider" => (2, 0),
        "slider.button" => (2, 1),
        "fields.first" => (3, 0),
        "fields.area" => (3, 1),
        "fields.last" => (3, 2),
        _ => panic!("no such path {path}"),
    };
    let parent = groups[g];
    let kids: Vec<usize> = (0..t.len()).filter(|&i| t[i].parent as usize == parent).collect();
    kids[k] as u32
}

#[test]
fn left_in_a_text_field_beside_a_button_moves_the_caret() {
    let mut f = F::new();
    let field = f.focus("text.field");
    f.key(Key::End);
    assert_eq!(f.app.ui.text.field.caret(), 3, "setup: End put the caret at the end");
    let handled = f.key(Key::Left);
    let got = (f.input.focus, f.app.ui.text.field.caret());
    println!("TextInput beside Button, End then one Left: (focus, caret) = {got:?}, expected {:?}", (Some(field), 2));
    assert!(handled, "the Left was not handled by anyone");
    assert_eq!(got, (Some(field), 2), "the arrow left the field instead of moving its caret");
}

#[test]
fn a_number_input_beside_a_button_edits_in_the_middle() {
    // `NumberInput` is a composite: its `node_mut` returns itself for the
    // inner field's index, so it must answer for arrows itself.
    let mut f = F::new();
    let number = f.focus("number.number");
    assert_eq!(f.app.ui.number.number.as_str(), "0.00", "setup: the value's text");
    f.key(Key::End);
    f.key(Key::Left);
    f.key(Key::Left);
    f.ev(Event::Text { ch: '9' });
    println!(
        "NumberInput beside Button, End Left Left '9': text {:?} focus {:?}, expected \"0.900\" {:?}",
        f.app.ui.number.number.as_str(),
        f.input.focus,
        Some(number)
    );
    assert_eq!(f.input.focus, Some(number), "the arrow left the number input");
    assert_eq!(f.app.ui.number.number.as_str(), "0.900", "the digit landed somewhere other than the caret");
}

#[test]
fn right_on_a_slider_beside_a_button_moves_the_value() {
    let mut f = F::new();
    let slider = f.focus("slider.slider");
    let before = f.app.ui.slider.slider.value();
    f.key(Key::Right);
    let after = f.app.ui.slider.slider.value();
    println!("Slider beside Button, Right: value {before} -> {after}, focus {:?} (expected {:?})", f.input.focus, Some(slider));
    assert_eq!(f.input.focus, Some(slider), "the arrow left the slider");
    assert!(after > before, "the slider did not step");
}

#[test]
fn arrows_stay_in_a_column_of_text_widgets() {
    // R10-F3: Left in the first field, and Up on a `TextArea`'s first
    // line. The second is the case "ask the widget first" does not close:
    // the area *declines* Up on line 0, and a declined arrow still
    // navigated.
    let mut f = F::new();
    let first = f.focus("fields.first");
    f.key(Key::End);
    f.key(Key::Left);
    let got = (f.input.focus, f.app.ui.fields.first.caret());
    println!("first TextInput, End then Left: (focus, caret) = {got:?}, expected {:?}", (Some(first), 2));
    assert_eq!(got, (Some(first), 2), "Left left the first field");

    let area = f.focus("fields.area");
    assert_eq!(f.app.ui.fields.area.caret_line(), 0, "setup: the caret is on the area's first line");
    f.key(Key::Up);
    let got = (f.input.focus, f.app.ui.fields.area.caret_line());
    println!("TextArea on line 0, Up: (focus, line) = {got:?}, expected {:?}", (Some(area), 0));
    assert_eq!(got, (Some(area), 0), "Up on the area's first line left the area");
}
