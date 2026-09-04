//! M9 Cluster B: the text field.
//!
//! The properties that matter are UTF-8 correctness (a caret that lands
//! mid-character panics the next slice), that edits go through methods,
//! and that the field edits *itself* through `Node::on_event` — the C3
//! seam, without which routing keystrokes correctly would still leave
//! nothing to receive them.

use lumen_static::event::{Event, EventCtx, Key};
use lumen_static::input::TextInput;
use lumen_static::widget::Node;
use lumen_static::{pass, Cmd, Constraints, DisplayList, NodeState, StaticTree, Widget};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 100.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("field")]
    field: TextInput,
}

fn send(f: &mut TextInput, ev: Event) -> bool {
    f.on_event(&ev, &EventCtx::for_event(0, ev))
}

fn typed(f: &mut TextInput, s: &str) {
    for ch in s.chars() {
        send(f, Event::Text { ch });
    }
}

#[test]
fn typing_inserts_at_the_caret_and_advances_it() {
    let mut f: TextInput = TextInput::default();
    typed(&mut f, "abc");
    assert_eq!(f.as_str(), "abc");
    assert_eq!(f.caret(), 3);

    // Move left twice and type in the middle.
    send(&mut f, Event::Key { key: Key::Left, down: true });
    send(&mut f, Event::Key { key: Key::Left, down: true });
    typed(&mut f, "X");
    assert_eq!(f.as_str(), "aXbc");
    assert_eq!(f.caret(), 2);
}

#[test]
fn backspace_and_delete_cross_whole_characters() {
    // The property that makes this worth a test: byte arithmetic. A
    // caret stepping by one byte through "é" lands mid-character and the
    // next slice panics.
    let mut f: TextInput = TextInput::new("aéb");
    assert_eq!(f.caret(), "aéb".len(), "new() puts the caret at the end");

    assert!(f.backspace());
    assert_eq!(f.as_str(), "aé");
    assert!(f.backspace());
    assert_eq!(f.as_str(), "a", "the two-byte é was removed whole");
    assert!(f.backspace());
    assert_eq!(f.as_str(), "");
    assert!(!f.backspace(), "nothing left to delete");

    let mut g: TextInput = TextInput::new("aéb");
    send(&mut g, Event::Key { key: Key::Home, down: true });
    assert_eq!(g.caret(), 0);
    assert!(g.delete());
    assert_eq!(g.as_str(), "éb");
    assert!(g.delete());
    assert_eq!(g.as_str(), "b", "delete crosses the multi-byte char too");
}

#[test]
fn the_caret_never_lands_off_a_char_boundary() {
    let mut f: TextInput = TextInput::new("aé漢🎉z");
    send(&mut f, Event::Key { key: Key::Home, down: true });
    // Walk right through the whole string, slicing at every stop. A
    // caret off a boundary panics here rather than somewhere subtle.
    loop {
        let c = f.caret();
        assert!(f.as_str().is_char_boundary(c), "caret {c} is mid-character");
        let _ = &f.as_str()[..c];
        if !send(&mut f, Event::Key { key: Key::Right, down: true }) {
            break;
        }
    }
    assert_eq!(f.caret(), f.as_str().len(), "walked to the end");

    while send(&mut f, Event::Key { key: Key::Left, down: true }) {
        assert!(f.as_str().is_char_boundary(f.caret()));
    }
    assert_eq!(f.caret(), 0, "and back to the start");
}

#[test]
fn control_characters_are_not_inserted() {
    // A shell may deliver both `Text { '\n' }` and `Key { Enter }` for
    // one keypress. A single-line field (B7) must not grow a newline.
    let mut f: TextInput = TextInput::default();
    typed(&mut f, "ab");
    send(&mut f, Event::Text { ch: '\n' });
    send(&mut f, Event::Text { ch: '\t' });
    assert_eq!(f.as_str(), "ab");
}

#[test]
fn an_unhandled_key_is_declined_so_an_ancestor_can_have_it() {
    // The C3/C4 contract: `on_event` returns true only when something
    // changed, so Enter reaches the form's submit handler instead of
    // being swallowed by the field it was typed in.
    let mut f: TextInput = TextInput::new("x");
    assert!(!send(&mut f, Event::Key { key: Key::Enter, down: true }), "Enter is not the field's");
    assert!(!send(&mut f, Event::Key { key: Key::Tab, down: true }));
    // At the end already: a Right that cannot move is not consumed.
    assert!(!send(&mut f, Event::Key { key: Key::Right, down: true }));
    // A key *release* edits nothing.
    assert!(!send(&mut f, Event::Key { key: Key::Backspace, down: false }));
    assert_eq!(f.as_str(), "x");
}

#[test]
fn a_caret_is_painted_only_when_the_field_is_focused() {
    let mut ui = Ui::default();
    ui.field.set_str("hello");
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];
    pass::layout(&mut ui, &sty, &mut st, VIEW);

    let rects = |ui: &Ui, st: &[NodeState]| {
        let mut list = DisplayList::new();
        pass::paint(ui, st, &mut list);
        list.cmds.iter().filter(|c| matches!(c, Cmd::Rect { .. })).count()
    };
    let unfocused = rects(&ui, &st);

    st[1].focused = true;
    let focused = rects(&ui, &st);
    assert_eq!(focused, unfocused + 1, "the caret is one more rect (B4), and only when focused");
}

#[test]
fn the_field_is_focusable_and_announces_its_value() {
    assert!(Ui::SUB_TABLE[1].is_focusable(), "a field can be tabbed to");
    let mut ui = Ui::default();
    ui.field.set_str("secret");
    assert_eq!(ui.field.as_str(), "secret", "B8: the app reads it directly");
}

#[test]
fn a_field_wider_than_its_box_scrolls_to_keep_the_caret_visible() {
    // B5, in `layout`, after shaping — the first moment the caret's x is
    // known.
    let mut ui = Ui::default();
    let sty = pass::default_styles(&ui);
    let mut st = [NodeState::default(); Ui::NODE_COUNT];

    ui.field.set_str(&"w".repeat(200));
    pass::layout(&mut ui, &sty, &mut st, VIEW);
    // The node cannot be wider than the viewport, whatever the content.
    assert!(st[1].size.w <= VIEW.max_w + 0.001, "the field fits its box: {}", st[1].size.w);

    // Painting stays inside it: the run is clipped, exactly as a
    // scroller's content is.
    let mut list = DisplayList::new();
    pass::paint(&ui, &st, &mut list);
    assert!(
        list.cmds.iter().any(|c| matches!(c, Cmd::PushClip { .. })),
        "the field opens a clip scope for its overflowing run"
    );
}
