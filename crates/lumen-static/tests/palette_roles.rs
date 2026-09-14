//! M25/T25.11: a `Patch` colour may name a palette role, and the cascade
//! resolves it against the palette it is handed.
//!
//! The host-level gate — that `Host::set_palette` re-resolves the kept
//! sheet under `Boundaries` — is `lumen-static-shell/tests/palette_host.rs`.
//! These pin the cascade's own contract: which palette a role reads, that
//! a role reaches an arena's template, and which of a role and a value
//! wins.

use lumen_static::display::Color;
use lumen_static::ops::{Fixed, Label};
use lumen_static::repeat::Repeat;
use lumen_static::style::{
    cascade_deep, cascade_deep_with, Fields, Palette, Patch, Role, Rule, Selector, Style,
};
use lumen_static::widget::Node;
use lumen_static::{StaticTree, Widget};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    rows: Repeat<Fixed>,
}

fn bind(_: &mut Fixed, _: usize) {}

const TITLE: usize = 1;
const INK: Color = Color(90, 10, 10, 255);

/// The main table's styles, and the repeater template's one style row.
fn cascaded(sheet: &[Rule], palette: &Palette) -> (Vec<Style>, Style) {
    let mut ui = Ui { rows: Repeat::new(2, 10.0, bind), ..Default::default() };
    let mut out = vec![Style::default(); <Ui as Widget>::NODE_COUNT];
    cascade_deep_with(&mut ui, sheet, palette, &mut out);
    let row = ui.rows.arena_mut(0).expect("setup: a repeater has an arena").style[0];
    (out, row)
}

#[test]
fn a_role_resolves_against_the_palette_it_is_given() {
    let sheet = [Rule::new(Selector::Id("title"), Patch::new().color_role(Role::Accent))];
    let (light, _) = cascaded(&sheet, &Palette::LIGHT);
    let (dark, _) = cascaded(&sheet, &Palette::DARK);
    println!("light {:?}, dark {:?}", light[TITLE].color, dark[TITLE].color);
    assert_ne!(Palette::LIGHT.accent, Palette::DARK.accent, "setup: the palettes differ here");
    assert_ne!(
        Label::<&str>::type_defaults().color,
        Palette::LIGHT.accent,
        "setup: the label's own colour is not the role's"
    );
    assert_eq!(light[TITLE].color, Palette::LIGHT.accent, "the role did not resolve");
    assert_eq!(dark[TITLE].color, Palette::DARK.accent, "the role ignored the palette given");
}

#[test]
fn every_role_names_its_own_field() {
    let p = Palette::DARK;
    let pairs = [
        (Role::Page, p.page),
        (Role::Surface, p.surface),
        (Role::Control, p.control),
        (Role::Text, p.text),
        (Role::Muted, p.muted),
        (Role::Accent, p.accent),
        (Role::OnAccent, p.on_accent),
        (Role::Focus, p.focus),
    ];
    for (role, colour) in pairs {
        assert_eq!(p.get(role), colour, "{role:?}");
    }
}

#[test]
fn the_palette_free_cascade_resolves_against_light() {
    let sheet = [Rule::new(Selector::Id("title"), Patch::new().background_role(Role::Surface))];
    let mut ui = Ui { rows: Repeat::new(2, 10.0, bind), ..Default::default() };
    let mut out = vec![Style::default(); <Ui as Widget>::NODE_COUNT];
    cascade_deep(&mut ui, &sheet, &mut out);
    assert_eq!(out[TITLE].background, Palette::LIGHT.surface);
}

/// The only selector that reaches a repeater's row is its kind, and a
/// theme is exactly a kind-wide rule — so a role must resolve there too.
#[test]
fn a_role_reaches_an_arena_template() {
    let sheet = [Rule::new(Selector::Kind("fixed"), Patch::new().color_role(Role::Muted))];
    let (_, row) = cascaded(&sheet, &Palette::DARK);
    println!("row colour {:?}", row.color);
    assert_eq!(row.color, Palette::DARK.muted, "the arena's template did not resolve the role");
}

/// The setters clear the other half, so a later call wins as it would for
/// any other field; a struct literal that sets both gets the role.
#[test]
fn a_later_setter_wins_and_a_role_beats_a_value_set_alongside_it() {
    let then_role = Patch::new().color(INK).color_role(Role::Text);
    let then_value = Patch::new().color_role(Role::Text).color(INK);
    assert_eq!((then_role.color, then_role.color_role), (None, Some(Role::Text)));
    assert_eq!((then_value.color, then_value.color_role), (Some(INK), None));
    assert_eq!(then_role.fields(), Fields::COLOR);

    let both = Patch { color: Some(INK), color_role: Some(Role::Text), ..Patch::new() };
    let (out, _) = cascaded(&[Rule::new(Selector::Id("title"), both)], &Palette::DARK);
    assert_eq!(out[TITLE].color, Palette::DARK.text, "a role set beside a value lost to it");
}
