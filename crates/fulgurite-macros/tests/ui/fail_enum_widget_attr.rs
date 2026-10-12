//! M55/T55.2 (R16-F4): an enum widget takes no `StaticTree` attribute.
//! `#[widget]`, `#[widget_id]` and `#[widget_class]` were accepted on an
//! enum and its variants, and nothing read them.
use fulgurite::prelude::*;
use fulgurite_widgets::prelude::*;

#[derive(StaticTree)]
#[widget(row)]
enum OnTheEnum {
    A(Label),
}

#[derive(StaticTree)]
enum OnAVariant {
    #[widget_id("a")]
    A(Label),
}

fn main() {}
