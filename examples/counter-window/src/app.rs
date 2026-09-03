//! The tally: the tree, its stylesheet, and its handlers.
//!
//! Deliberately shaped after Lumen's `examples/counter` so the two are
//! comparable — a caption, a hero number that recolours with the sign, a
//! status pill, four step buttons and a reset. What differs is
//! structural, not cosmetic: the tree is a `struct`, so its shape is
//! fixed at compile time and `NODE_COUNT` is a constant.

use lumen_static::display::Color;
use lumen_static::prelude::*;

// ---- palette: Lumen's counter tokens ---------------------------------

pub const PAGE: Color = Color(0x0e, 0x11, 0x18, 0xff);
pub const SURFACE: Color = Color(0x16, 0x1c, 0x28, 0xff);
pub const INK: Color = Color(0xf1, 0xf5, 0xfb, 0xff);
pub const MUTED: Color = Color(0x8b, 0x94, 0xa7, 0xff);
pub const ACCENT: Color = Color(0x4d, 0x7c, 0xfe, 0xff);
pub const GHOST: Color = Color(0x23, 0x2c, 0x3e, 0xff);
pub const POS: Color = Color(0x34, 0xd3, 0x99, 0xff);
pub const NEG: Color = Color(0xfb, 0x71, 0x85, 0xff);
pub const ZERO: Color = Color(0x9a, 0xa3, 0xb6, 0xff);
pub const WHITE: Color = Color(0xff, 0xff, 0xff, 0xff);

// ---- the tree --------------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 8.0)]
pub struct Steps {
    #[widget_id("dec10")]
    pub dec10: Button<Tally, String>,
    #[widget_id("dec")]
    pub dec: Button<Tally, String>,
    #[widget_id("inc")]
    pub inc: Button<Tally, String>,
    #[widget_id("inc10")]
    pub inc10: Button<Tally, String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 16.0)]
pub struct Card {
    #[widget_id("caption")]
    pub caption: Label,
    #[widget_id("value")]
    pub value: Label<String>,
    #[widget_id("pill")]
    pub pill: Label<String>,
    #[widget_id("steps")]
    pub steps: Steps,
    #[widget_id("reset")]
    pub reset: Button<Tally, String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Page {
    /// The framework has no `justify-content`: a container places
    /// children sequentially and `Fill` divides the remainder. Two Fill
    /// spacers sharing it is main-axis centring, and it is the same
    /// trick the control row uses to push its buttons apart.
    pub top: Spacer,
    #[widget_id("card")]
    pub card: Card,
    pub bottom: Spacer,
}

// ---- state, and how it reaches the screen ----------------------------

fn sign_word(v: &i64) -> &'static str {
    match v.signum() {
        1 => "POSITIVE",
        -1 => "NEGATIVE",
        _ => "AT ZERO",
    }
}

/// The pill's text is a word, not the number — a `with =` projection
/// rather than a format string.
fn set_sign(dst: &mut Label<String>, src: &i64) {
    lumen_static::bind::set_if_changed(dst, sign_word(src));
}

#[derive(DeriveApp, Default)]
pub struct Tally {
    #[tree]
    pub ui: Page,

    /// The one piece of state. Everything on screen derives from it.
    #[bind(ui.card.value, "{}")]
    #[bind(ui.card.pill, with = set_sign)]
    pub count: i64,
}

pub fn build() -> Tally {
    let mut t = Tally::default();
    t.ui.card.caption.set("TALLY");
    t.ui.card.steps.dec10.label = "\u{2212}10".into();
    t.ui.card.steps.dec10.on_click = |t, _| t.count -= 10;
    t.ui.card.steps.dec.label = "\u{2212}1".into();
    t.ui.card.steps.dec.on_click = |t, _| t.count -= 1;
    t.ui.card.steps.inc.label = "+1".into();
    t.ui.card.steps.inc.on_click = |t, _| t.count += 1;
    t.ui.card.steps.inc10.label = "+10".into();
    t.ui.card.steps.inc10.on_click = |t, _| t.count += 10;
    t.ui.card.reset.label = "Reset".into();
    t.ui.card.reset.on_click = |t, _| t.count = 0;
    t
}

/// The stylesheet, as rules rather than `.lss` — the parser is Phase C.
/// Selector for selector, this is Lumen's `app.lss`.
/// The stylesheet for a given sign.
///
/// Lumen's counter toggles a `.lss` class (`.value.pos` / `.value.neg`)
/// and re-cascades. There is no class *toggling* here — `#[widget_class]`
/// is static, fixed in the type — so the sign-dependent rules are
/// swapped instead and the host re-cascades. Same mechanism underneath:
/// the cascade runs when the sheet changes, not per frame.
pub fn stylesheet_for(sign: i64) -> Vec<Rule> {
    let (value_fg, pill_bg) = match sign {
        1 => (POS, POS),
        -1 => (NEG, NEG),
        _ => (INK, ZERO),
    };
    let mut rules = stylesheet();
    rules.push(Rule::new(
        Selector::Id("value"),
        Patch::new().color(value_fg).font_size(76.0).font_weight(800.0),
    ));
    rules.push(Rule::new(
        Selector::Id("pill"),
        Patch::new()
            .background(pill_bg)
            .color(PAGE)
            .radius(999.0)
            .font_size(12.0)
            .font_weight(700.0)
            .padding(Edges { left: 14.0, right: 14.0, top: 5.0, bottom: 5.0 }),
    ));
    rules
}

/// Lumen's `.btn`: 16px, weight 600, 16h/11v padding, 13px radius.
const fn btn(bg: Color, fg: Color) -> Patch {
    Patch::new()
        .background(bg)
        .color(fg)
        .radius(13.0)
        .font_size(16.0)
        .font_weight(600.0)
        .padding(Edges { left: 16.0, right: 16.0, top: 11.0, bottom: 11.0 })
}

fn stylesheet() -> Vec<Rule> {
    vec![
        Rule::new(
            Selector::Id("card"),
            Patch::new()
                .background(SURFACE)
                .radius(26.0)
                .padding(Edges::all(32.0))
                .width(Length::Fixed(360.0))
                .shadow(18.0)
                .align(Align::Center),
        ),
        Rule::new(
            Selector::Id("caption"),
            Patch::new().color(MUTED).font_size(13.0).font_weight(700.0),
        ),
        Rule::new(Selector::Id("steps"), Patch::new().align(Align::Center)),
        Rule::new(
            Selector::Kind("Page"),
            Patch::new().background(PAGE).padding(Edges::all(24.0)).align(Align::Center),
        ),
        Rule::new(Selector::Id("reset"), btn(Color(0x1b, 0x23, 0x33, 0xff), MUTED)),
        // The four step buttons: two ghost, two accent.
        Rule::new(Selector::Id("dec10"), btn(GHOST, INK)),
        Rule::new(Selector::Id("dec"), btn(GHOST, INK)),
        Rule::new(Selector::Id("inc"), btn(ACCENT, WHITE)),
        Rule::new(Selector::Id("inc10"), btn(ACCENT, WHITE)),
    ]
}
