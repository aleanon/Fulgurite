//! The stage: every demo in the tree at once, all but one hidden.
//!
//! # Why not `Dyn`
//!
//! The first version of this file put one `Dyn` island on the stage and
//! swapped its content per selection. It looked right and **every
//! interactive widget in it was dead**: `Dyn` was a `Box<dyn Node>`, and
//! its `Handlers<S>` impl returned `None` unconditionally, because the
//! application type is exactly what type erasure erased. A button inside
//! an island could be hit-tested, hovered and pressed, and no handler
//! would ever run. (`Dyn` was given a `NodeFor<S>` supertrait in M23 and
//! does route handlers now; the stage still does not use one, for the
//! reason below.)
//!
//! # Why an enum
//!
//! A typed variant can be *read*, so a `Slider`'s value reaches the
//! readout. Through a `Dyn` it could not, and the catalogue had a flag
//! apologising for it.
//!
//! # What M24 removed from this file
//!
//! Eighteen `Option` fields and the `fill_all` / `clear` / `shown`
//! trio — 111 lines whose content was the number eighteen, and a
//! `clear` with one field left out was the one bug that shape made
//! possible.
//!
//! The `Option` stack existed because the enum switch **did not work**:
//! the cascade read a node's kind and defaults off the *live tree*, and
//! an inactive variant has no live node, so every hidden demo resolved
//! the container's defaults and painted as a bare text run. The test
//! that catches it says so in as many words: *"Badge drew only text on
//! the stage — it looks like a label"*.
//!
//! `Option` could paper over that by being briefly all-`Some` when the
//! stylesheet was set; an enum holds one variant by definition and has
//! no such moment, which is why the workaround could not be carried
//! across. M24/T24.1 moved both facts to the type (`Widget::SEEDS`), and
//! the enum this file always wanted became writable.
//! `docs/plan-select.md`.
//!
//! The price is the tree carries all eighteen demos. That is the trade a
//! static tree offers — space for reach — and at this size it is
//! nothing.

use super::*;

/// One labelled control: the box and its text, kept together.
///
/// **A nested row, not padding.** The grouping gap has to be wider
/// between pairs than inside one, and neither lever on a flat row can do
/// it: padding on the *label* is subtracted from the box its text is
/// measured against (it wrapped "Blue"), and padding on a *fixed-size*
/// control is ignored outright — `resolve_self` returns the `Fixed`
/// width whatever the padding says. So the pair is its own row with its
/// own spacing, and the outer row spaces the pairs.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 10.0)]
pub struct PairCheck {
    #[widget_id("d_checkbox_a")]
    pub t: CheckBox<Show>,
    #[widget_id("d_checkbox_al")]
    pub l: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 10.0)]
pub struct PairSwitch {
    #[widget_id("d_switch_a")]
    pub t: Switch<Show>,
    #[widget_id("d_switch_al")]
    pub l: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 10.0)]
pub struct PairRadio {
    #[widget_id("d_radio_a")]
    pub t: Radio<Show>,
    #[widget_id("d_radio_al")]
    pub l: Label<String>,
}


#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DLabel {
    #[widget_id("d_label_a")]
    pub a: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DBadge {
    #[widget_id("d_badge_a")]
    pub a: Badge<String>,
    #[widget_id("d_badge_b")]
    pub b: Badge<String>,
    #[widget_id("d_badge_c")]
    pub c: Badge<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DChip {
    #[widget_id("d_chip_a")]
    pub a: Chip<String>,
    #[widget_id("d_chip_b")]
    pub b: Chip<String>,
    #[widget_id("d_chip_c")]
    pub c: Chip<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DAvatar {
    #[widget_id("d_avatar_a")]
    pub a: Avatar<String>,
    #[widget_id("d_avatar_b")]
    pub b: Avatar<String>,
    #[widget_id("d_avatar_c")]
    pub c: Avatar<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DIcon {
    #[widget_id("d_icon_a")]
    pub a: Icon,
    #[widget_id("d_icon_b")]
    pub b: Icon,
    #[widget_id("d_icon_c")]
    pub c: Icon,
    #[widget_id("d_icon_d")]
    pub d: Icon,
    #[widget_id("d_icon_e")]
    pub e: Icon,
    #[widget_id("d_icon_f")]
    pub f: Icon,
    #[widget_id("d_icon_g")]
    pub g: Icon,
    #[widget_id("d_icon_h")]
    pub h: Icon,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DDivider {
    #[widget_id("d_divider_a")]
    pub a: Divider,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DButton {
    #[widget_id("d_button_a")]
    pub a: Button<Show, String>,
    #[widget_id("d_button_b")]
    pub b: Button<Show, String>,
    #[widget_id("d_button_c")]
    pub c: Button<Show, String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 34.0)]
pub struct DCheckBox {
    pub a: PairCheck,
    pub b: PairCheck,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 34.0)]
pub struct DSwitch {
    pub a: PairSwitch,
    pub b: PairSwitch,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 34.0)]
pub struct DRadio {
    pub a: PairRadio,
    pub b: PairRadio,
    pub c: PairRadio,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSlider {
    #[widget_id("d_slider_a")]
    pub a: Slider,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DTextInput {
    #[widget_id("d_textinput_a")]
    pub a: TextInput<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DProgressBar {
    #[widget_id("d_progress_a")]
    pub a: ProgressBar,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSpinner {
    #[widget_id("d_spinner_a")]
    pub a: Spinner,
    #[widget_id("d_spinner_b")]
    pub b: Spinner,
    #[widget_id("d_spinner_c")]
    pub c: Spinner,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSkeleton {
    #[widget_id("d_skeleton_a")]
    pub a: Skeleton,
    #[widget_id("d_skeleton_b")]
    pub b: Skeleton,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DBarChart {
    #[widget_id("d_bar_a")]
    pub a: BarChart,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DLineChart {
    #[widget_id("d_line_a")]
    pub a: LineChart,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DPieChart {
    #[widget_id("d_pie_a")]
    pub a: PieChart,
}

/// Every demo, as **one enum**.
///
/// `#[derive(StaticTree)]` on an enum is a variant switch: each variant
/// gets its own range of the node table, inactive variants are zeroed
/// rather than left stale, and `Handlers<S>` forwards to the active one.
/// So a demo on the stage is wired to the application exactly as a
/// typed field was.
///
/// **This is what the file wanted from the start** and could not have
/// until M24/T24.1. Before that the cascade read a node's kind and
/// defaults off the *live tree*, and an inactive variant has no live
/// node — so every hidden demo resolved the enum's defaults and painted
/// as a bare text run. The `Option` stack could paper over it by being
/// briefly all-`Some` at cascade time (`fill_all`); an enum holds one
/// variant by definition and has no such moment. `docs/plan-select.md`.
///
/// What it removes: `fill_all`, `clear` and `shown` — 111 lines whose
/// content was the number eighteen, and a `clear` with one field left
/// out was the one bug the `Option` shape made possible. One-of-N is
/// now the type's job.
#[derive(StaticTree)]
pub enum Demo {
    Label(DLabel),
    Badge(DBadge),
    Chip(DChip),
    Avatar(DAvatar),
    Icon(DIcon),
    Divider(DDivider),
    Button(DButton),
    CheckBox(DCheckBox),
    Switch(DSwitch),
    Radio(DRadio),
    Slider(DSlider),
    TextInput(DTextInput),
    Progress(DProgressBar),
    Spinner(DSpinner),
    Skeleton(DSkeleton),
    Bar(DBarChart),
    Line(DLineChart),
    Pie(DPieChart),
}

impl Default for Demo {
    fn default() -> Self {
        Demo::Label(DLabel::default())
    }
}

/// How many demos the stage has. Asserted against the catalogue.
pub const DEMOS: usize = 18;
