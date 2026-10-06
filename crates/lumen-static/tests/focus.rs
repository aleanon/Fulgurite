//! T15.6: keyboard navigation beyond a forward Tab.
//!
//! Keyboard-only operation was Tab and nothing else. This is the item
//! the plan called disproportionate in value to cost, and the tests are
//! about the three things that were missing: going *back*, moving
//! *within* a group, and being able to *see* where focus is.

use lumen_static::event::InputState;
use lumen_static_widgets::ops::{Button, Label};
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::{pass, Cmd, Constraints, DisplayList, Event, Key, Mods, NodeState, StaticTree, Widget};
use lumen_static_widgets::{TextInput};

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

/// What `ops::focus_ring` draws for node `i`. Since M57/A6 the ring is
/// drawn once, by the host, around the focused node — no widget draws
/// its own — so this asks the helper the host calls. The host's side
/// (keyboard-only, one ring, the palette's colour, not over an overlay)
/// is `lumen-static-shell/tests/focus_ring.rs`.
fn ring_cmds(f: &F, i: usize) -> Vec<Cmd> {
    let mut list = DisplayList::new();
    lumen_static::ops::focus_ring(&f.st[i], lumen_static::Color(1, 2, 3, 255), &mut list);
    list.cmds.clone()
}

#[test]
fn the_ring_is_an_outline_outside_the_node_with_a_gap() {
    let mut f = F::new();
    pass::layout(&mut f.app.ui, &f.styles, &mut f.st, VIEW);
    let cmds = ring_cmds(&f, 3);
    assert_eq!(cmds.len(), 1, "one command: {cmds:?}");
    let s = f.st[3];
    let Cmd::StrokedRect { rect, width, .. } = cmds[0] else {
        panic!("the ring is not an outline: {:?}", cmds[0]);
    };
    // The stroke is inset in its rect, so its inner edge is `rect` shrunk
    // by `width` — and that must clear the node's own box by the gap.
    let inner_left = rect[0] + width;
    println!("node x {} ring inner edge {inner_left} gap {}", s.pos.x, s.pos.x - inner_left);
    assert!(
        s.pos.x - inner_left >= lumen_static::ops::FOCUS_GAP - 0.01,
        "the ring touches the control: its inner edge is at {inner_left}, the node at {}",
        s.pos.x
    );
}

#[test]
fn a_stylesheet_can_turn_the_ring_off() {
    let mut f = F::new();
    pass::layout(&mut f.app.ui, &f.styles, &mut f.st, VIEW);
    assert_eq!(ring_cmds(&f, 3).len(), 1, "setup: the default draws a ring");

    let sheet = [Rule::new(Selector::Id("a"), Patch::new().focus_ring(0))];
    lumen_static::style::cascade_deep(&mut f.app.ui, &sheet, &mut f.styles);
    pass::layout(&mut f.app.ui, &f.styles, &mut f.st, VIEW);
    assert!(ring_cmds(&f, 3).is_empty(), "a rule turned it off, as a rule should");
}

#[test]
fn the_ring_costs_no_bytes_per_node() {
    // The M15 policy: a per-node field either fits or is re-budgeted.
    // As an f32 this took Style from 72 to 76; as a u8 it is free, and
    // a ring wider than 255 px is not a ring.
    //
    // **The ring is still free; `Style` is 80 for a different reason.**
    // M27/T27.1 added the border, which found the tail slot exactly full
    // — `align`, `focus_ring`, `cursor` and `visible` in four bytes with
    // nothing spare — and was re-budgeted with a measurement
    // (`docs/results-embedded.md`). The claim this test makes is about
    // the *ring*, so it is kept and its number moved rather than deleted:
    // if `focus_ring` ever became an `f32` it would cost bytes again.
    // **84 since M82/T82.3**, for `background_fill` (`tests/cursor.rs`
    // prints the used bytes, 82 of 84 since M86/T86.1's `transition`).
    // `NodeState` is **88 since M86/T86.1**, for `pointer_mix`, the
    // transition level — a `u16`, not the ring (`docs/results-transitions.md`).
    let (style, state) = (core::mem::size_of::<lumen_static::Style>(), core::mem::size_of::<NodeState>());
    println!(
        "Style {style} B (bound 84), NodeState {state} B (bound 88): {:.1} KB per 100 nodes",
        (style + state) as f32 * 100.0 / 1024.0
    );
    assert_eq!(style, 84);
    assert_eq!(state, 88);
    assert_eq!(core::mem::size_of::<lumen_static::Cmd>(), 60, "Cmd changed");
    assert_eq!(core::mem::size_of::<lumen_static::Event>(), 16, "Event changed");
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
    use lumen_static::{stack_bytes, MAX_DEPTH, STACK_BASE_BYTES, STACK_BYTES_PER_LEVEL};

    // A real screen. This test's own fixture is the evidence: nothing
    // here is anywhere near the cap. Compared through a `let` rather
    // than against a literal, because clippy is right that comparing
    // two consts is not an assertion.
    let depth = <Ui as Widget>::DEPTH;
    let real = stack_bytes(depth);
    assert!(real <= 4096, "a real tree fits the embedded budget: {real} B");
    assert!(depth < MAX_DEPTH / 8, "and is nowhere near the cap: {depth}");

    // The cap, read as a budget, would be wrong in both directions:
    // ~18 KB is nothing on a desktop and twice the embedded device's
    // whole 8 KB stack. A base plus a slope since M52/T52.6.
    assert_eq!(stack_bytes(MAX_DEPTH), STACK_BASE_BYTES + MAX_DEPTH * STACK_BYTES_PER_LEVEL);
    assert!(stack_bytes(MAX_DEPTH) > 4096, "which is why it is not the budget");
}

// ---- R3-F9: a group has no cap ----------------------------------------

/// Twenty focusable siblings. The group used to be collected into a
/// sixteen-slot buffer, so members past the sixteenth were unreachable by
/// arrows and an arrow *on* one of them was a dead key (M44/T44.1).
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Twenty {
    b0: Button<Wide>,
    b1: Button<Wide>,
    b2: Button<Wide>,
    b3: Button<Wide>,
    b4: Button<Wide>,
    b5: Button<Wide>,
    b6: Button<Wide>,
    b7: Button<Wide>,
    b8: Button<Wide>,
    b9: Button<Wide>,
    b10: Button<Wide>,
    b11: Button<Wide>,
    b12: Button<Wide>,
    b13: Button<Wide>,
    b14: Button<Wide>,
    b15: Button<Wide>,
    b16: Button<Wide>,
    b17: Button<Wide>,
    b18: Button<Wide>,
    b19: Button<Wide>,
}

#[derive(lumen_static::App, Default)]
struct Wide {
    #[tree]
    ui: Twenty,
}

fn twenty() -> (Wide, Vec<lumen_static::Style>, [NodeState; Twenty::NODE_COUNT]) {
    let mut app = Wide::default();
    let styles = pass::default_styles(&app.ui);
    let mut st = [NodeState::default(); Twenty::NODE_COUNT];
    let wide = Constraints { min_w: 0.0, max_w: 4000.0, min_h: 0.0, max_h: 100.0 };
    pass::layout(&mut app.ui, &styles, &mut st, wide);
    (app, styles, st)
}

fn right(app: &mut Wide, styles: &[lumen_static::Style], st: &mut [NodeState], input: &mut InputState) -> bool {
    pass::dispatch(app, styles, st, input, &Event::Key { key: Key::Right, down: true })
}

#[test]
fn a_group_of_twenty_is_navigable_end_to_end() {
    let (mut app, styles, mut st) = twenty();
    let members: Vec<u32> = (1..Twenty::NODE_COUNT as u32).collect();
    assert_eq!(members.len(), 20, "setup: twenty focusable members");
    assert!(members.iter().all(|&i| st[i as usize].size.w > 0.0), "setup: every member has area");

    let mut input = InputState { focus: Some(members[0]), ..Default::default() };
    let mut visited = std::collections::BTreeSet::new();
    for _ in 0..20 {
        assert!(right(&mut app, &styles, &mut st, &mut input), "Right was not handled");
        visited.insert(input.focus.expect("focus survived"));
    }
    println!("visited {} / 20, back at {:?} (start {})", visited.len(), input.focus, members[0]);
    assert_eq!(visited.len(), 20, "Right x20 did not visit every member");
    assert_eq!(input.focus, Some(members[0]), "Right x20 did not return to the start");

    // And a member past the sixteenth is not a dead key.
    input.focus = Some(members[18]);
    let handled = right(&mut app, &styles, &mut st, &mut input);
    println!("Right from member 18: handled {handled}, focus {:?} (expected {})", input.focus, members[19]);
    assert_eq!(input.focus, Some(members[19]), "Right from member 18 did not reach 19");
}

/// **Focus is published after the event that moved it** (M44/T44.6).
///
/// The publish ran at the top of dispatch, before Tab moved focus, so the
/// frame after a Tab drew the ring on the node focus had just left and
/// none on the node it reached, until some later event republished it.
#[test]
fn a_focus_move_is_visible_in_the_state_array_at_once() {
    let mut f = F::new();
    f.key(Key::Tab, Mods::NONE);
    let first = f.input.focus.expect("Tab focused something") as usize;
    f.key(Key::Tab, Mods::NONE);
    let second = f.input.focus.unwrap() as usize;
    let lit: Vec<usize> = (0..f.st.len()).filter(|&i| f.st[i].focused).collect();
    println!("after two Tabs: focus {second} (was {first}); nodes with the focused bit: {lit:?}");
    assert_eq!(lit, vec![second], "the focused bit is not on exactly the node focus is on");
}
