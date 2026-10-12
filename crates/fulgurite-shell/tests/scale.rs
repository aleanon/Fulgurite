//! M10b: logical units above the renderer, physical below it.
//!
//! The defect this closes: the framework laid out against
//! `window.inner_size()`, which is physical, so a window rendered at
//! half its intended size on a 2× display — and `scripts/ui_check.py`'s
//! 24-logical-pixel accessibility floor was compared against numbers
//! twice too large, and passed.

use fulgurite_widgets::ops::{Button, Label};
use fulgurite::style::{Edges, Length, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Cmd, StaticTree, Widget};
use fulgurite_shell::Host;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("head")]
    head: Label,
    #[widget_id("go")]
    go: Button<App>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("go"), Patch::new().width(Length::Fixed(44.0)).padding(Edges::all(0.0))),
        Rule::new(Selector::Kind("Ui"), Patch::new().padding(Edges::all(10.0))),
    ]
}

fn host(scale: f32) -> Host<App> {
    // 880x1040 physical is 440x520 logical at 2x — the tally's size.
    let (w, h) = ((440.0 * scale) as u32, (520.0 * scale) as u32);
    let mut h_ = Host::new(App::default(), w, h);
    h_.set_scale(scale);
    h_.set_stylesheet(&sheet());
    h_.app.ui.head.set("head");
    h_.app.ui.go.label = "go";
    h_.pump();
    h_
}

#[test]
fn layout_is_identical_in_logical_units_at_every_scale() {
    // The property that makes a logical accessibility budget meaningful:
    // a control is the same number of logical pixels on every display.
    let one = host(1.0);
    let two = host(2.0);
    for i in 0..Ui::NODE_COUNT {
        assert_eq!(
            one.states[i].size, two.states[i].size,
            "node {i} differs between 1x and 2x: {:?} vs {:?}",
            one.states[i].size, two.states[i].size
        );
        assert_eq!(one.states[i].pos, two.states[i].pos, "node {i} position differs");
    }
    // And it is the size the stylesheet asked for, not twice it.
    assert_eq!(two.states[2].size.w, 44.0, "a 44 px button is 44 logical px at 2x");
}

#[test]
fn the_display_list_is_in_physical_pixels() {
    // Below the renderer, everything doubles — that is what makes the
    // frame fill a 2x surface instead of a quarter of it.
    let one = host(1.0);
    let two = host(2.0);
    let rect_of = |h: &Host<App>| {
        h.list
            .cmds
            .iter()
            .find_map(|c| match c {
                Cmd::RoundedRect { rect, .. } | Cmd::Rect { rect, .. } => Some(*rect),
                _ => None,
            })
            .expect("something was painted")
    };
    let (a, b) = (rect_of(&one), rect_of(&two));
    for k in 0..4 {
        assert!((b[k] - a[k] * 2.0).abs() < 0.001, "command coord {k}: {} vs {}", b[k], a[k] * 2.0);
    }
}

#[test]
fn text_is_emitted_at_the_physical_size() {
    let two = host(2.0);
    let size = two
        .list
        .cmds
        .iter()
        .find_map(|c| match c {
            Cmd::Text { size, .. } => Some(*size),
            _ => None,
        })
        .expect("text was painted");
    // Default font size is 16 logical.
    assert_eq!(size, 32.0, "16 logical px reaches the rasteriser as 32 physical");
}

#[test]
fn a_physical_pointer_point_converts_to_logical() {
    // What a shell must do before `post`ing: winit reports physical.
    let two = host(2.0);
    assert_eq!(two.to_logical(200.0, 100.0), (100.0, 50.0));
    let one = host(1.0);
    assert_eq!(one.to_logical(200.0, 100.0), (200.0, 100.0));
}

#[test]
fn hit_testing_speaks_logical_so_a_converted_click_lands() {
    let two = host(2.0);
    let st = two.states[2];
    let (lx, ly) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
    // The same point as the OS would report it.
    let (px, py) = (lx * 2.0, ly * 2.0);
    let (cx, cy) = two.to_logical(px, py);
    assert!(two.probe(cx, cy).is_some(), "the converted point hits");
    assert!(
        two.probe(px, py).is_none() || two.probe(px, py).unwrap().node != 2,
        "and the unconverted one does not — which is the bug this closes"
    );
}

#[test]
fn a_degenerate_scale_is_ignored_rather_than_propagated() {
    let mut h = host(1.0);
    for bad in [0.0, -2.0, f32::NAN, f32::INFINITY] {
        h.set_scale(bad);
        assert_eq!(h.scale(), 1.0, "{bad} must not become a scale");
    }
}

#[test]
fn changing_scale_forces_a_full_redraw() {
    // Every laid-out position changes, so nothing retained describes the
    // next frame.
    let mut h = host(1.0);
    h.pump();
    h.take_damage_for_age(1);
    h.set_scale(2.0);
    h.pump();
    assert_eq!(h.damage(), fulgurite::Damage::All);
}
