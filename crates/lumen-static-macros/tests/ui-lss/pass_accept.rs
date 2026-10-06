//! M61/T61.0 gate 2's accept direction (`docs/plan-lss.md` §2.1): every
//! construct v1 accepts, lowered to exactly the rules a person would
//! write -- compared here field for field, through `Debug` so the case
//! does not depend on T61.2's `PartialEq`.
use lumen_static::style::{Align, Cursor, Edges, Length, Patch, Role, Rule, Selector, TextOverflow};
use lumen_static::{include_lss, Color, StaticTree};
use lumen_static_widgets::ops::{Fixed, Label};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
    #[widget_id("badge")]
    badge: Fixed,
    #[widget_class("note")]
    note: Label,
}

const BASE: &[Rule] = include_lss!(Ui, "pass_accept.lss");
const LOUD: &[Rule] = include_lss!(Ui, "pass_accept.lss", loud);

fn by_hand(loud: bool) -> Vec<Rule> {
    let boxy = Patch::new()
        .width(Length::Fixed(120.0))
        .height(Length::Fixed(40.0))
        .padding(Edges { left: 8.0, right: 8.0, top: 4.0, bottom: 4.0 })
        .align(Align::Center)
        .visible(true)
        .disabled(false)
        .cursor(Cursor::Pointer)
        .focus_ring(2)
        .background(Color(0x16, 0x20, 0x2c, 0xff))
        .border_width(1)
        .border_color(Color(0x33, 0x44, 0x55, 0xff))
        .radius(8.0)
        .shadow(12.0)
        .hover_dim(0.07)
        .pressed_dim(0.15)
        .transition_ms(150);
    let mut v = vec![
        Rule::new(Selector::Any, Patch::new().spacing(6.0)),
        Rule::new(
            Selector::Kind("label"),
            Patch::new().color(Color(0xf1, 0xf5, 0xfb, 0xff)).font_size(15.0).font_weight(600.0).line_height(1.5).overflow(TextOverflow::Ellipsis),
        ),
        Rule::new(
            Selector::Class("note"),
            Patch::new().color_role(Role::Muted).background_role(Role::Surface).border_color_role(Role::Focus).background_fill(1),
        ),
        Rule::new(Selector::Id("title"), boxy),
        Rule::new(Selector::Id("badge"), boxy),
        Rule::new(
            Selector::Id("badge"),
            Patch::new()
                .width(Length::FILL)
                .height(Length::Shrink)
                .padding(Edges { left: 4.0, right: 2.0, top: 1.0, bottom: 3.0 }),
        ),
    ];
    if loud {
        v.push(Rule::new(
            Selector::Id("title"),
            Patch::new().font_size(24.0).color_role(Role::Accent).width(Length::FillPortion(2)),
        ));
    }
    v
}

fn main() {
    let (base, loud) = (format!("{BASE:?}"), format!("{LOUD:?}"));
    println!("base: {} rules, loud: {} rules", BASE.len(), LOUD.len());
    assert_eq!(base, format!("{:?}", by_hand(false)), "the base sheet");
    assert_eq!(loud, format!("{:?}", by_hand(true)), "base + @sheet loud");
}
