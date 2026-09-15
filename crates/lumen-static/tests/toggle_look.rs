//! M25/T25.5: a toggle is coloured by its look, and its look by the
//! palette.
//!
//! The first test is the one the milestone exists for: a tick in a colour
//! `Style` could not give it. Before M25 the mark was a hard-coded white,
//! because a third colour on every node was too dear for one part of one
//! widget.

use lumen_static::display::{Cmd, Color, DisplayList};
use lumen_static::style::{Palette, Status};
use lumen_static::toggle::{default_look, ToggleLook};
use lumen_static::{CheckBox, Constraints, LayoutCx, Node, NodeState, Point, Style};

const RED: Color = Color(200, 30, 30, 255);
const WHITE: Color = Color(255, 255, 255, 255);

/// The colours of every rounded rect a checkbox paints, in order: the
/// body, then the tick's dots when it is on.
fn painted(w: &mut CheckBox<()>, style: Style, palette: &Palette) -> Vec<Color> {
    let style = [style; 1];
    let mut st = [NodeState::default(); 1];
    let limits = Constraints { min_w: 0.0, max_w: 100.0, min_h: 0.0, max_h: 100.0 };
    let mut metrics = lumen_static::cx::BlockMetrics;
    let mut cx = LayoutCx::new(&mut metrics).with_palette(palette);
    w.layout(&mut cx, limits, &style, &mut st, 0);
    w.place(Point { x: 0.0, y: 0.0 }, &style, &mut st, 0);
    let mut l = DisplayList::new();
    w.paint(&st, 0, &mut l);
    l.cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::RoundedRect { color, .. } => Some(*color),
            _ => None,
        })
        .collect()
}

/// [`painted`] under the widget's own defaults.
fn painted_default(w: &mut CheckBox<()>, palette: &Palette) -> Vec<Color> {
    let d = w.defaults();
    painted(w, d, palette)
}

fn red_tick(p: &Palette, s: Status) -> ToggleLook {
    ToggleLook { mark: RED, ..default_look(p, s) }
}

#[test]
fn a_look_colours_the_tick() {
    let default_mark = default_look(&Palette::LIGHT, Status::Active).mark;
    println!("look mark {RED:?}, default mark {default_mark:?}");
    // Setup: the colour under test is neither the old hard-coded white nor
    // the default look's, or a paint that ignored the look would pass.
    assert_ne!(RED, WHITE);
    assert_ne!(RED, default_mark);

    let mut w = CheckBox::<()>::default();
    w.on = true;
    w.set_look(red_tick);
    let colours = painted_default(&mut w, &Palette::LIGHT);
    println!("painted {colours:?}");
    assert!(colours.len() > 1, "setup: an on checkbox paints a body and a tick");
    assert!(
        colours[1..].iter().all(|c| *c == RED),
        "the tick was not painted in the look's mark colour: {:?}",
        &colours[1..]
    );
}

#[test]
fn the_palette_colours_the_default_look() {
    for palette in [Palette::LIGHT, Palette::DARK] {
        let mut on = CheckBox::<()>::default();
        on.on = true;
        let mut off = CheckBox::<()>::default();
        let (on_c, off_c) =
            (painted_default(&mut on, &palette), painted_default(&mut off, &palette));
        println!("on {:?} off {:?}", on_c[0], off_c[0]);
        assert_eq!(on_c[0], palette.accent, "an on box's body is the palette's accent");
        assert_eq!(off_c[0], palette.control, "an off box's body is the palette's control grey");
        assert_eq!(on_c[1], palette.on_accent, "the tick is the palette's on_accent");
    }
    assert_ne!(Palette::LIGHT.accent, Palette::DARK.accent, "setup: the two palettes differ");
}

#[test]
fn a_stylesheet_background_no_longer_colours_a_toggle() {
    let magenta = Color(255, 0, 255, 255);
    let mut w = CheckBox::<()>::default();
    let style =
        Style { background: magenta, color: magenta, ..CheckBox::<()>::default().defaults() };
    let colours = painted(&mut w, style, &Palette::LIGHT);
    println!("body {:?}", colours[0]);
    assert_eq!(
        colours[0],
        Palette::LIGHT.control,
        "a Style background reached a toggle's body; its mask says it does not read one"
    );
}
