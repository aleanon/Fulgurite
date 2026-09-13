//! M23: a switch's knob is light in **both** states.
//!
//! `Toggle` swaps `(body, mark)` on the on/off boundary, and for a
//! checkbox and a radio that is exactly right — the mark exists only
//! when the control is on, so two colours really do cover four states.
//! A switch is the odd one out: its knob is drawn in both states, so
//! the swap put the *accent* colour on the knob of an **off** switch,
//! which reads as on.

use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static::{Constraints, LayoutCx, Node, NodeState, Point, Switch};

/// The rounded rects a switch paints, in order: the track, then the
/// knob.
fn painted(on: bool) -> Vec<(Color, f32)> {
    let mut w = Switch::<()> { on, ..Default::default() };
    let style = [w.defaults(); 4];
    let mut st = [NodeState::default(); 4];
    let limits = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 80.0 };
    let mut metrics = lumen_static::cx::BlockMetrics;
    let mut cx = LayoutCx::new(&mut metrics);
    w.layout(&mut cx, limits, &style, &mut st, 0);
    w.place(Point { x: 0.0, y: 0.0 }, &style, &mut st, 0);
    let mut l = DisplayList::new();
    w.paint(&st, 0, &mut l);
    l.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::RoundedRect { rect, color, .. } => Some((*color, rect[2] - rect[0])),
            _ => None,
        })
        .collect()
}

fn luma(c: Color) -> f32 {
    0.299 * c.0 as f32 + 0.587 * c.1 as f32 + 0.114 * c.2 as f32
}

/// **The knob is lighter than its track, on or off.**
///
/// Both directions in one test, because the off state alone was what
/// broke and the on state alone would have passed throughout.
#[test]
fn the_knob_is_light_in_both_states() {
    for on in [false, true] {
        let parts = painted(on);
        assert_eq!(parts.len(), 2, "a switch should paint a track and a knob (on = {on})");
        let (track, knob) = (parts[0].0, parts[1].0);
        println!(
            "on = {on:<5} track {track:?} luma {:.0}, knob {knob:?} luma {:.0}",
            luma(track),
            luma(knob)
        );
        // **Ordering, not a magnitude.** How much lighter depends on
        // the theme: measured on the defaults the off state is 255
        // against 231 — only 24 levels, because the resting track is
        // already nearly white, while the on state is 255 against 99. A
        // fixed threshold would encode one theme's palette. The
        // inversion this guards against reverses the *order*, which is
        // what makes an off switch read as on.
        assert!(
            luma(knob) > luma(track) + 10.0,
            "on = {on}: the knob (luma {:.0}) is not lighter than its track (luma {:.0})",
            luma(knob),
            luma(track)
        );
    }
}

/// **And the knob is the same colour in both states**, which is the
/// property the inversion broke: a knob that changes colour with the
/// state is a second signal competing with the one that matters, the
/// track.
#[test]
fn the_knob_does_not_change_colour_with_the_state() {
    let off = painted(false)[1].0;
    let on = painted(true)[1].0;
    println!("knob off {off:?}, on {on:?}");
    assert_eq!(off, on, "the knob changes colour between states");
}

/// The track is the thing that carries the state, and it must actually
/// change — or the two tests above are satisfied by a switch that looks
/// identical on and off.
#[test]
fn the_track_carries_the_state() {
    let off = painted(false)[0].0;
    let on = painted(true)[0].0;
    println!("track off {off:?}, on {on:?}");
    assert_ne!(off, on, "the track is the same colour on and off");
}
