//! M25/T25.0: pointer and focus state reach layout under `Boundaries`.
//!
//! `dispatch` writes `hovered`, `pressed`, `focused` and
//! `focus_visible` without a handler running, and until T25.0 the host
//! marked its dirty set only when a handler ran or a binding wrote. The
//! derive skips a clean subtree before `record_painted` resolves the
//! shade, so under `Boundaries` a hovered button kept its resting colour
//! — correct only under `Everything` (the default until M34), which lays out every
//! node every frame whatever was marked.
//!
//! # Why this tree, and not the oracle's
//!
//! The skip happens at the top of a *derived container's* `layout`; a
//! leaf is laid out whenever its parent is. So "the mark was scoped to
//! the hovered subtree" is only observable in a **sibling container**,
//! and the oracle's tree has one container beside the root. Here the
//! button lives in `Left` and a label in `Right`, and a mutation of the
//! right label that nobody reports stays stale exactly when the hover's
//! mark did not reach it.
//!
//! Every mutation used as a probe is shown to be real — invalidated,
//! it does change the frame — so a stale size means a skip and not a
//! setter that did nothing.

use fulgurite::event::Key;
use fulgurite::invalidate::{Boundaries, Everything, Invalidation};
use fulgurite_widgets::ops::{Button, Label};
use fulgurite::{App as DeriveApp, Color, Event, StaticTree};
use fulgurite_shell::Host;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Left {
    #[widget_id("btn")]
    btn: Button<Pointer>,
    #[widget_id("left_note")]
    note: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Right {
    #[widget_id("right_note")]
    note: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
struct Ui {
    #[widget_id("left")]
    left: Left,
    #[widget_id("right")]
    right: Right,
}

#[derive(DeriveApp, Default)]
struct Pointer {
    #[tree]
    ui: Ui,
}

type Bounded = Host<Pointer, fulgurite::Incremental, Boundaries<16>>;

fn host<I: Invalidation>() -> Host<Pointer, fulgurite::Incremental, I> {
    let mut app = Pointer::default();
    app.ui.left.note.set("left".to_owned());
    app.ui.right.note.set("right".to_owned());
    let mut h = Host::new(app, 400, 200);
    h.set_stylesheet(&[]);
    h.pump();
    // A still frame, so the set is empty and the next frame can skip.
    h.pump();
    h
}

fn index(wid: &str) -> usize {
    let table = <Ui as fulgurite::Widget>::SUB_TABLE;
    fulgurite::style::index_of_wid(table, wid).expect("id exists")
}

fn centre<I: Invalidation>(h: &Host<Pointer, fulgurite::Incremental, I>, wid: &str) -> (f32, f32) {
    let s = &h.states[index(wid)];
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

/// `ops::dim`, restated: the shade the cascade's `hover_dim` and
/// `pressed_dim` produce.
fn dim(c: Color, f: f32) -> Color {
    let k = 1.0 - f.clamp(0.0, 1.0);
    let ch = |v: u8| (v as f32 * k) as u8;
    Color(ch(c.0), ch(c.1), ch(c.2), c.3)
}

const LONG: &str = "a considerably longer note than the one laid out before";

#[test]
fn a_hover_reaches_the_shade_and_marks_only_its_own_subtree() {
    let mut h: Bounded = host();
    let btn = index("btn");
    let rest = h.states[btn].bg;
    let hover_dim = h.styles[btn].hover_dim;
    assert!(hover_dim > 0.0, "the button must have a hover shade, or there is nothing to see");

    // A mutation nobody reports, in the *other* container.
    let right_before = h.states[index("right_note")].size;
    h.app.ui.right.note.set(LONG.to_owned());

    let (x, y) = centre(&h, "btn");
    h.post(Event::PointerMove { x, y });
    assert!(!h.pump(), "a move onto a button is unrouted; handled would mean mark_all");
    assert!(h.states[btn].hovered, "the move did not land on the button");

    let painted = h.states[btn].bg;
    let expected = dim(rest, hover_dim);
    println!("hover: rest={rest:?} painted={painted:?} expected={expected:?}");
    assert_eq!(painted, expected, "the hovered button kept a stale background");

    let right_after = h.states[index("right_note")].size;
    println!("right note: before={right_before:?} after hover={right_after:?}");
    assert_eq!(
        right_after, right_before,
        "the hover's mark reached a sibling container: it was not scoped to the hovered subtree"
    );

    // The probe was real: reported, the same mutation does change the frame.
    assert!(h.invalidate("right"));
    h.pump();
    let reported = h.states[index("right_note")].size;
    assert!(reported.w > right_before.w, "the mutation never changed anything: {right_before:?} -> {reported:?}");

    // And the oracle agrees about the shade.
    let mut oracle: Host<Pointer, fulgurite::Incremental, Everything> = host();
    oracle.post(Event::PointerMove { x, y });
    oracle.pump();
    assert_eq!(oracle.states[btn].bg, painted, "Everything and Boundaries disagree about the hover shade");
}

#[test]
fn a_press_reaches_the_shade_when_nothing_else_changed() {
    let mut h: Bounded = host();
    let btn = index("btn");
    let (x, y) = centre(&h, "btn");

    // Hover and focus are already on the button before the press under
    // test, so the only thing that changes is `pressed` — otherwise the
    // hover or focus comparison would mark the node and this would pass
    // with the press comparison deleted.
    h.post(Event::PointerMove { x, y });
    h.pump();
    h.click(x, y);
    h.pump();
    h.pump();
    assert!(h.states[btn].hovered && h.input.focus == Some(btn as u32), "setup: hover and focus on the button");
    let rest = h.styles[btn].background;
    let hovered = h.states[btn].bg;

    h.post(Event::down(x, y));
    assert!(!h.pump(), "a press is activated on release, so it must be unhandled");
    assert!(h.states[btn].pressed, "the press did not latch");

    let painted = h.states[btn].bg;
    let expected = dim(rest, h.styles[btn].pressed_dim);
    println!("press: rest={rest:?} hovered={hovered:?} painted={painted:?} expected={expected:?}");
    assert_eq!(painted, expected, "the pressed button kept a stale background");
}

#[test]
fn a_move_that_stays_on_the_same_node_marks_nothing() {
    let mut h: Bounded = host();
    let (x, y) = centre(&h, "btn");
    h.post(Event::PointerMove { x, y });
    h.pump();
    h.pump();

    // A mutation nobody reports, in the *same* container as the button.
    let before = h.states[index("left_note")].size;
    h.app.ui.left.note.set(LONG.to_owned());

    h.post(Event::PointerMove { x: x + 1.0, y });
    assert!(!h.pump());
    assert!(h.states[index("btn")].hovered, "setup: still over the button");
    let after = h.states[index("left_note")].size;
    println!("left note: before={before:?} after a move within the button={after:?}");
    assert_eq!(after, before, "a move that changed no pointer state still marked the button's container");

    assert!(h.invalidate("left"));
    h.pump();
    let reported = h.states[index("left_note")].size;
    assert!(reported.w > before.w, "the mutation never changed anything: {before:?} -> {reported:?}");
}

#[test]
fn keyboard_focus_becoming_visible_reaches_the_ring() {
    let mut h: Bounded = host();
    let btn = index("btn");
    let (x, y) = centre(&h, "btn");

    // Pointer focus: no ring. Since M57/A6 the host draws the one ring
    // itself, after paint, from `input.focus` and `focus_visible` — so
    // what this asserts is the *painted* list, which is what a stale
    // frame would get wrong.
    h.click(x, y);
    h.pump();
    h.pump();
    assert_eq!(h.input.focus, Some(btn as u32), "setup: the click focused the button");
    assert!(!h.input.focus_visible);
    assert!(!ring_around(&h, btn), "setup: pointer focus shows no ring");

    // An arrow key makes focus visible **without being handled**: the
    // button is alone in its group, so the arrow does not move focus.
    // Tab would be handled and reach `mark_all`, which is why it is not
    // the key used here.
    h.post(Event::Key { key: Key::Right, down: true });
    assert!(!h.pump(), "the arrow must be unhandled, or mark_all makes this test vacuous");
    assert!(h.input.focus_visible, "setup: the arrow made focus visible");
    assert_eq!(h.input.focus, Some(btn as u32), "setup: focus did not move");

    let styled = h.styles[btn].focus_ring;
    println!("focus ring: styled={styled} painted={}", ring_around(&h, btn));
    assert!(styled > 0, "setup: the button's style draws a ring");
    assert!(ring_around(&h, btn), "keyboard focus became visible and the ring stayed hidden");
}

/// Whether the frame's list holds the host's ring — an outline outside
/// node `i`'s box.
fn ring_around<I: Invalidation>(h: &Host<Pointer, fulgurite::Incremental, I>, i: usize) -> bool {
    let s = h.states[i];
    h.list.cmds.iter().any(|c| match c {
        fulgurite::Cmd::StrokedRect { rect, .. } => rect[0] < s.pos.x && rect[2] > s.pos.x + s.size.w,
        _ => false,
    })
}
