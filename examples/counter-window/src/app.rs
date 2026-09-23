//! The tally: the tree, its stylesheet, and its handlers.
//!
//! Deliberately shaped after Lumen's `examples/counter` so the two are
//! comparable — a caption, a hero number that recolours with the sign, a
//! status pill, four step buttons and a reset. What differs is
//! structural, not cosmetic: the tree is a `struct`, so its shape is
//! fixed at compile time and `NODE_COUNT` is a constant.

use lumen_static::display::Color;
use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;

// ---- palette: Lumen's counter tokens ---------------------------------

pub const PAGE: Color = Color(0x0e, 0x11, 0x18, 0xff);
pub const SURFACE: Color = Color(0x16, 0x1c, 0x28, 0xff);
pub const INK: Color = Color(0xf1, 0xf5, 0xfb, 0xff);
pub const MUTED: Color = Color(0x8b, 0x94, 0xa7, 0xff);
/// Lumen's accent is `#4d7cfe`; white on it is 3.7:1, under the 4.5:1
/// WCAG AA asks of 16 px text. This is the same hue two steps darker,
/// 5.0:1 (`scripts/ui_check.py contrast`).
pub const ACCENT: Color = Color(0x35, 0x63, 0xef, 0xff);
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
    pub dec10: Button<Count, String>,
    #[widget_id("dec")]
    pub dec: Button<Count, String>,
    #[widget_id("inc")]
    pub inc: Button<Count, String>,
    #[widget_id("inc10")]
    pub inc10: Button<Count, String>,
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
    pub reset: Button<Count, String>,
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

#[derive(DeriveApp, Default)]
pub struct Tally {
    #[tree]
    pub ui: Page,

    /// **What a handler may touch** — M33/T33.4. The one piece of state;
    /// everything on screen derives from it, through two bindings that
    /// both compare before they write.
    #[state]
    #[bind(ui.card.value, n, "{}")]
    #[bind(ui.card.pill, sign_word(), "{}")]
    pub count: Count,
}

/// The tally, apart from the tree it is shown in.
#[derive(Default)]
pub struct Count {
    pub n: i64,
}

impl Count {
    /// The pill's text is a word, not the number. Named in a binding's
    /// **source path** — a source path is an expression within the field,
    /// so a method call is one — which keeps the pill on the comparing
    /// form. It was a `with =` projection until M33/T33.4, and `with =`
    /// reports a write unconditionally.
    pub fn sign_word(&self) -> &'static str {
        match self.n.signum() {
            1 => "POSITIVE",
            -1 => "NEGATIVE",
            _ => "AT ZERO",
        }
    }
}

/// The one construction path — M31/T31.0.
///
/// A free `build()` became `Application::build`, so `Reloadable::fresh`
/// reaches the eleven assignments below. Six of them are handler `fn`
/// pointers into the build that is loaded now, which is exactly what a
/// reload must re-supply.
impl Application for Tally {
    fn build() -> Self {
        build()
    }
}

/// **The program, described once** — M32 (`docs/plan-program.md`).
///
/// The window, `--headless` and the socket all build their host from
/// this, so each starts styled the same way. The style is a *function of
/// the state* — the sheet for the count's sign — which the host asks when
/// it is built and again on `adopt`. The per-sign re-cascade between
/// pumps stays in the window (`restyle_subtree`): a style re-asked every
/// frame is deferred in the plan's §4, and this example is why.
pub fn program(count: i64, w: u32, h: u32) -> lumen_static_shell::program::Program<Tally> {
    lumen_static_shell::program::application(move || Tally { count: Count { n: count }, ..build() })
        .size(w, h)
        .style(|t: &Tally| stylesheet_for(t.count.n.signum()))
}

pub fn build() -> Tally {
    let mut t = Tally::default();
    t.ui.card.caption.set("TALLY");
    t.ui.card.steps.dec10.label = "\u{2212}10".into();
    t.ui.card.steps.dec10.on_click = |t, _| t.n -= 10;
    t.ui.card.steps.dec.label = "\u{2212}1".into();
    t.ui.card.steps.dec.on_click = |t, _| t.n -= 1;
    t.ui.card.steps.inc.label = "+1".into();
    t.ui.card.steps.inc.on_click = |t, _| t.n += 1;
    t.ui.card.steps.inc10.label = "+10".into();
    t.ui.card.steps.inc10.on_click = |t, _| t.n += 10;
    t.ui.card.reset.label = "Reset".into();
    t.ui.card.reset.on_click = |t, _| t.n = 0;
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
