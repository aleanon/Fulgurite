//! T15.6: keyboard navigation beyond a forward Tab.
//!
//! Keyboard-only operation was Tab and nothing else. This is the item
//! the plan called disproportionate in value to cost, and the tests are
//! about the three things that were missing: going *back*, moving
//! *within* a group, and being able to *see* where focus is.

use lumen_static::event::InputState;
use lumen_static::ops::{Button, Label};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{
    pass, Cmd, Constraints, DisplayList, Event, Key, Mods, NodeState, StaticTree, TextInput,
    Widget,
};

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Group {
    #[widget_id("a")]
    a: Button<App>,
    #[widget_id("b")]
    b: Button<App>,
    #[widget_id("c")]
    c: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("field")]
    field: TextInput,
    group: Group,
    caption: Label,
}

#[derive(lumen_static::App, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 200.0 };

struct F {
    app: App,
    styles: Vec<lumen_static::Style>,
    st: [NodeState; Ui::NODE_COUNT],
    input: InputState,
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        for b in [&mut app.ui.group.a, &mut app.ui.group.b, &mut app.ui.group.c] {
            b.label = "x";
        }
        let styles = pass::default_styles(&app.ui);
        let mut f = F {
            app,
            styles,
            st: [NodeState::default(); Ui::NODE_COUNT],
            input: InputState::default(),
        };
        pass::layout(&mut f.app.ui, &f.styles, &mut f.st, VIEW);
        f
    }

    fn key(&mut self, key: Key, mods: Mods) {
        self.input.mods = mods;
        pass::dispatch(
            &mut self.app,
            &self.styles,
            &mut self.st,
            &mut self.input,
            &Event::Modifiers(mods),
        );
        pass::dispatch(
            &mut self.app,
            &self.styles,
            &mut self.st,
            &mut self.input,
            &Event::Key { key, down: true },
        );
    }
}

#[test]
fn shift_tab_goes_back() {
    // A form you can only walk forwards is a form you cannot correct.
    let mut f = F::new();
    f.key(Key::Tab, Mods::NONE);
    let first = f.input.focus.expect("Tab focused something");
    f.key(Key::Tab, Mods::NONE);
    let second = f.input.focus.unwrap();
    assert_ne!(first, second, "Tab moved");

    f.key(Key::Tab, Mods::SHIFT);
    assert_eq!(f.input.focus, Some(first), "Shift+Tab came back");
}

#[test]
fn tab_and_shift_tab_both_wrap() {
    let mut f = F::new();
    f.key(Key::Tab, Mods::SHIFT);
    let last = f.input.focus.expect("Shift+Tab from nothing lands on the last");
    f.key(Key::Tab, Mods::NONE);
    f.key(Key::Tab, Mods::SHIFT);
    assert_eq!(f.input.focus, Some(last), "and wraps back to it");
}

/// **Arrows move within the group, and the group is the parent.**
#[test]
fn arrows_move_within_a_group_and_wrap_inside_it() {
    let mut f = F::new();
    // Focus the first button: Tab past the text field.
    f.key(Key::Tab, Mods::NONE);
    while f.app.ui.group.a.label != "x" || f.input.focus != Some(3) {
        f.key(Key::Tab, Mods::NONE);
        if f.input.focus == Some(3) {
            break;
        }
    }
    assert_eq!(f.input.focus, Some(3), "on the first button");

    f.key(Key::Right, Mods::NONE);
    assert_eq!(f.input.focus, Some(4), "right moved within the group");
    f.key(Key::Right, Mods::NONE);
    assert_eq!(f.input.focus, Some(5));
    f.key(Key::Right, Mods::NONE);
    assert_eq!(f.input.focus, Some(3), "and wrapped inside the group, not out of it");

    f.key(Key::Left, Mods::NONE);
    assert_eq!(f.input.focus, Some(5), "left goes the other way");
}

/// **The rule that keeps arrows usable inside a text field.**
#[test]
fn an_arrow_in_a_lone_focusable_still_belongs_to_the_widget() {
    let mut f = F::new();
    f.app.ui.field.set_str("abc");
    f.input.focus = Some(1);
    let before = f.app.ui.field.caret();
    f.key(Key::Left, Mods::NONE);
    assert_eq!(f.input.focus, Some(1), "focus did not jump away");
    assert_ne!(f.app.ui.field.caret(), before, "the field moved its caret instead");
}

/// Rounded rects in the frame. The ring is one of these — drawn
/// *behind* the control at its own radius plus the ring width — rather
/// than four lines on top, which covered the antialiased corner and
/// which `ui_check.py` caught within a frame.
fn ring_rects(f: &F) -> usize {
    let mut list = DisplayList::new();
    pass::paint(&f.app.ui, &f.st, &mut list);
    list.cmds.iter().filter(|c| matches!(c, Cmd::RoundedRect { .. })).count()
}

#[test]
fn a_focus_ring_is_drawn_only_on_the_focused_node() {
    let mut f = F::new();
    f.input.focus = Some(3);
    // Focus is applied by dispatch; set the bit the way the pass does.
    f.st[3].focused = true;
    pass::layout(&mut f.app.ui, &f.styles, &mut f.st, VIEW);
    f.st[3].focused = true;

    let with = ring_rects(&f);
    f.st[3].focused = false;
    let without = ring_rects(&f);
    assert_eq!(with, without + 1, "focus adds exactly one rounded rect: the ring");
}

#[test]
fn a_stylesheet_can_turn_the_ring_off() {
    let mut f = F::new();
    f.st[3].focused = true;
    let with_ring = ring_rects(&f);

    let sheet = [Rule::new(Selector::Id("a"), Patch::new().focus_ring(0))];
    lumen_static::style::cascade_deep(&mut f.app.ui, &sheet, &mut f.styles);
    pass::layout(&mut f.app.ui, &f.styles, &mut f.st, VIEW);
    f.st[3].focused = true;
    assert_eq!(ring_rects(&f), with_ring - 1, "a rule turned it off, as a rule should");
}

#[test]
fn the_ring_costs_no_bytes_per_node() {
    // The M15 policy: a per-node field either fits or is re-budgeted.
    // As an f32 this took Style from 72 to 76; as a u8 it is free, and
    // a ring wider than 255 px is not a ring.
    assert_eq!(core::mem::size_of::<lumen_static::Style>(), 72);
    assert_eq!(core::mem::size_of::<NodeState>(), 76);
}

/// **A ring for the keyboard, not for the mouse.**
///
/// A ring after a click is noise; a ring after Tab is the difference
/// between a usable application and an unusable one. CSS calls the
/// distinction `:focus-visible`, and the live gate found it the hard
/// way: the first version drew a ring on every pointer-focused button
/// and `ui_check.py` reported the corner had stopped being
/// anti-aliased.
#[test]
fn focus_visible_follows_the_kind_of_input() {
    let mut f = F::new();
    assert!(!f.input.focus_visible, "nothing has focused anything yet");

    f.key(Key::Tab, Mods::NONE);
    assert!(f.input.focus_visible, "a keystroke that moves focus shows the ring");

    let b = f.st[3];
    pass::dispatch(
        &mut f.app,
        &f.styles,
        &mut f.st,
        &mut f.input,
        &Event::down(b.pos.x + 2.0, b.pos.y + 2.0),
    );
    assert!(!f.input.focus_visible, "a press hides it again");

    f.key(Key::Down, Mods::NONE);
    assert!(f.input.focus_visible, "and an arrow brings it back");
}


// ---- the stack budget, as a fact about a tree ------------------------

/// **`MAX_DEPTH` is a runaway-recursion backstop, not a stack budget**,
/// and the two were conflated until 2026-09-07.
///
/// `docs/results-embedded.md` has budgeted 4 KB of stack since T13.9
/// and nothing checked it: the threshold was written down, the depth
/// was measured once by hand, and the two were never connected.
/// `stack_bytes` connects them, and `fixtures/embedded` asserts it at
/// compile time for the tree that actually has to fit.
#[test]
fn stack_cost_follows_a_trees_own_depth_not_the_cap() {
    use lumen_static::{stack_bytes, MAX_DEPTH, STACK_BYTES_PER_LEVEL};

    // A real screen. This test's own fixture is the evidence: nothing
    // here is anywhere near the cap. Compared through a `let` rather
    // than against a literal, because clippy is right that comparing
    // two consts is not an assertion.
    let depth = <Ui as Widget>::DEPTH;
    let real = stack_bytes(depth);
    assert!(real <= 4096, "a real tree fits the embedded budget: {real} B");
    assert!(depth < MAX_DEPTH / 8, "and is nowhere near the cap: {depth}");

    // The cap, read as a budget, would be wrong in both directions:
    // ~17 KB is nothing on a desktop and twice the embedded device's
    // whole 8 KB stack.
    assert_eq!(stack_bytes(MAX_DEPTH), MAX_DEPTH * STACK_BYTES_PER_LEVEL);
    assert!(stack_bytes(MAX_DEPTH) > 4096, "which is why it is not the budget");
}
