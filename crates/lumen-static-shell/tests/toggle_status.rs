//! M25/T25.5: a toggle's pressed look can be a different hue, not only a
//! darker shade — the thing `pressed_dim` could not express.
//!
//! On `Boundaries<16>`, so the press reaches the look only through
//! T25.0's mark: under `Everything` layout runs every frame and the look
//! would resolve whether or not anything reported the press.

use lumen_static::display::{Cmd, Color};
use lumen_static::invalidate::Boundaries;
use lumen_static::style::{Palette, Status};
use lumen_static::toggle::{default_look, ToggleLook};
use lumen_static::{App as DeriveApp, CheckBox, Event, StaticTree, Widget};
use lumen_static_shell::Host;

const BLUE: Color = Color(30, 60, 220, 255);
const GREEN: Color = Color(20, 180, 60, 255);

fn green_when_pressed(p: &Palette, s: Status) -> ToggleLook {
    let base = default_look(p, s);
    match s {
        Status::Pressed => ToggleLook { on_body: GREEN, ..base },
        _ => ToggleLook { on_body: BLUE, ..base },
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("check")]
    check: CheckBox<Pressable>,
}

#[derive(DeriveApp, Default)]
struct Pressable {
    #[tree]
    ui: Ui,
}

/// The body colour the checkbox painted last frame: the first rounded
/// rect inside its box.
fn body(h: &Host<Pressable, lumen_static::Incremental, Boundaries<16>>, i: usize) -> Color {
    let s = h.states[i];
    let inside = |r: &[f32; 4]| {
        r[0] >= s.pos.x - 0.5 && r[2] <= s.pos.x + s.size.w + 0.5 && r[1] >= s.pos.y - 0.5
    };
    h.list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::RoundedRect { rect, color, .. } if inside(rect) => Some(*color),
            _ => None,
        })
        .expect("the checkbox painted no body")
}

#[test]
fn a_pressed_look_can_change_hue() {
    let mut h: Host<Pressable, lumen_static::Incremental, Boundaries<16>> =
        Host::new(Pressable::default(), 200, 100);
    h.app.ui.check.on = true;
    h.app.ui.check.set_look(green_when_pressed);
    h.invalidate_all();
    h.pump();
    h.pump();
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "check").unwrap();

    let resting = body(&h, i);
    println!("resting body {resting:?}");
    assert_eq!(resting, BLUE, "setup: the resting on-body is the look's blue");

    let s = h.states[i];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::down(x, y));
    assert!(!h.pump(), "setup: a press activates on release, so it is unhandled here");
    assert!(h.states[i].pressed, "setup: the press latched on the checkbox");

    let pressed = body(&h, i);
    println!("pressed body {pressed:?}");
    assert_eq!(pressed, GREEN, "the pressed look's hue did not reach the frame");
}

/// M25/T25.12: a palette swap reaches a cached look.
///
/// The swap marks the tree, so layout runs; what this guards is the cache
/// underneath it. A stamp keyed on anything but the palette's generation
/// would match the one from before the swap, and the toggle would keep
/// painting the old palette on a frame that was laid out.
#[test]
fn a_palette_swap_reaches_a_cached_look() {
    let mut h: Host<Pressable, lumen_static::Incremental, Boundaries<16>> =
        Host::new(Pressable::default(), 200, 100);
    h.app.ui.check.on = true;
    h.invalidate_all();
    h.pump();
    h.pump();
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "check").unwrap();

    let light = body(&h, i);
    assert_ne!(Palette::LIGHT.accent, Palette::DARK.accent, "setup: the palettes differ here");
    assert_eq!(light, Palette::LIGHT.accent, "setup: an on checkbox's body is the light accent");

    h.set_palette(Palette::DARK);
    h.pump();
    let dark = body(&h, i);
    println!("body under light {light:?}, after set_palette(DARK) {dark:?}");
    assert_eq!(
        dark,
        Palette::DARK.accent,
        "the palette swap did not reach the cached look: the body kept the old palette's accent"
    );
}
