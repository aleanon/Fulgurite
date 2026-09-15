//! M25/T25.12: a toggle's look is resolved when its inputs change, and
//! read on every other layout.
//!
//! **The first test is the one that makes the others mean something.** A
//! cache that always misses passes every staleness gate — the pressed
//! hue, the palette swap, the hot reload — and is the pilot again. So it
//! counts look calls across layouts whose inputs did not change.

use std::cell::Cell;

use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static::style::{Palette, Status};
use lumen_static::toggle::{default_look, ToggleLook};
use lumen_static::{CheckBox, Constraints, LayoutCx, Node, NodeState, Point};

const RED: Color = Color(200, 30, 30, 255);
const BLUE: Color = Color(30, 60, 220, 255);

thread_local! {
    static CALLS: Cell<u32> = const { Cell::new(0) };
}

fn counting(p: &Palette, s: Status) -> ToggleLook {
    CALLS.with(|c| c.set(c.get() + 1));
    default_look(p, s)
}

fn red_tick(p: &Palette, s: Status) -> ToggleLook {
    ToggleLook { mark: RED, ..default_look(p, s) }
}

fn blue_tick(p: &Palette, s: Status) -> ToggleLook {
    ToggleLook { mark: BLUE, ..default_look(p, s) }
}

/// Lay `w` out once under `palette` at `generation`, with `st` as its
/// state, and return the colours of what it painted.
fn frame(
    w: &mut CheckBox<()>,
    st: &mut [NodeState; 1],
    palette: &Palette,
    generation: u32,
) -> Vec<Color> {
    let style = [w.defaults(); 1];
    let limits = Constraints { min_w: 0.0, max_w: 100.0, min_h: 0.0, max_h: 100.0 };
    let mut metrics = lumen_static::cx::BlockMetrics;
    let mut cx =
        LayoutCx::new(&mut metrics).with_palette(palette).with_palette_generation(generation);
    w.layout(&mut cx, limits, &style, st, 0);
    w.place(Point { x: 0.0, y: 0.0 }, &style, st, 0);
    let mut l = DisplayList::new();
    w.paint(st, 0, &mut l);
    l.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::RoundedRect { color, .. } => Some(*color),
            _ => None,
        })
        .collect()
}

#[test]
fn a_layout_whose_inputs_did_not_change_does_not_call_the_look() {
    let mut w = CheckBox::<()>::default();
    w.set_look(counting);
    let mut st = [NodeState::default(); 1];
    CALLS.with(|c| c.set(0));
    for _ in 0..3 {
        frame(&mut w, &mut st, &Palette::LIGHT, 1);
    }
    let still = CALLS.with(Cell::get);
    println!("look calls over three still layouts: {still} (expected 1: the first resolves)");
    assert_eq!(still, 1, "the look was called on layouts whose inputs had not changed");

    // And each input that should miss, does: the status, then the palette.
    st[0].hovered = true;
    frame(&mut w, &mut st, &Palette::LIGHT, 1);
    frame(&mut w, &mut st, &Palette::DARK, 2);
    let after = CALLS.with(Cell::get);
    println!("after a hover and a palette change: {after} (expected 3)");
    assert_eq!(after, 3, "a changed status or palette generation did not re-resolve the look");
}

#[test]
fn replacing_the_look_repaints_on_the_next_layout() {
    let mut w = CheckBox::<()>::default();
    w.on = true;
    w.set_look(red_tick);
    let mut st = [NodeState::default(); 1];
    frame(&mut w, &mut st, &Palette::LIGHT, 1);
    let red = frame(&mut w, &mut st, &Palette::LIGHT, 1);
    assert!(red.len() > 1 && red[1..].iter().all(|c| *c == RED), "setup: the tick is red: {red:?}");

    w.set_look(blue_tick);
    let blue = frame(&mut w, &mut st, &Palette::LIGHT, 1);
    println!("after set_look: {:?}", &blue[1..]);
    assert!(
        blue[1..].iter().all(|c| *c == BLUE),
        "the replaced look was not resolved: the old look's tick is still painted: {:?}",
        &blue[1..]
    );
}
