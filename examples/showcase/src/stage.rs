//! The stage: every demo in the tree at once, all but one hidden.
//!
//! # Why not `Dyn`
//!
//! The first version of this file put one `Dyn` island on the stage and
//! swapped its content per selection. It looked right and **every
//! interactive widget in it was dead**: `Dyn` is a `Box<dyn Node>`, and
//! its `Handlers<S>` impl returns `None` unconditionally, because the
//! application type is exactly what type erasure erased. A button inside
//! an island can be hit-tested, hovered and pressed, and no handler will
//! ever run.
//!
//! So the stage is what a static tree wanted all along: **every demo is
//! a typed field**, each wrapped in `Option`, and exactly one is `Some`.
//! `Option` is the core's own hide — `layout` zeroes an absent branch
//! every frame, so it holds no stale geometry for `paint`, `hit`, the
//! journal or the audit — and `Option<W>` forwards `Handlers` to `W`, so
//! a button on the stage is wired to the application like any other.
//!
//! It also buys back what the island cost: a typed field can be *read*,
//! so a `Slider`'s value reaches the readout. Through a `Dyn` it could
//! not, and the catalogue had a flag apologising for it.
//!
//! The price is the tree carries all eighteen demos. That is the trade a
//! static tree offers — space for reach — and at this size it is
//! nothing.

use super::*;

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
#[widget(row, spacing = 12.0)]
pub struct DCheckBox {
    #[widget_id("d_checkbox_a")]
    pub a: CheckBox<Show>,
    #[widget_id("d_checkbox_al")]
    pub al: Label<String>,
    #[widget_id("d_checkbox_b")]
    pub b: CheckBox<Show>,
    #[widget_id("d_checkbox_bl")]
    pub bl: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSwitch {
    #[widget_id("d_switch_a")]
    pub a: Switch<Show>,
    #[widget_id("d_switch_al")]
    pub al: Label<String>,
    #[widget_id("d_switch_b")]
    pub b: Switch<Show>,
    #[widget_id("d_switch_bl")]
    pub bl: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DRadio {
    #[widget_id("d_radio_a")]
    pub a: Radio<Show>,
    #[widget_id("d_radio_al")]
    pub al: Label<String>,
    #[widget_id("d_radio_b")]
    pub b: Radio<Show>,
    #[widget_id("d_radio_bl")]
    pub bl: Label<String>,
    #[widget_id("d_radio_c")]
    pub c: Radio<Show>,
    #[widget_id("d_radio_cl")]
    pub cl: Label<String>,
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

/// Every demo, sharing one rect. A stack, because they overlap by
/// definition — only one is ever present.
#[derive(StaticTree, Default)]
#[widget(stack)]
pub struct Demos {
    pub label: Option<DLabel>,
    pub badge: Option<DBadge>,
    pub chip: Option<DChip>,
    pub avatar: Option<DAvatar>,
    pub icon: Option<DIcon>,
    pub divider: Option<DDivider>,
    pub button: Option<DButton>,
    pub checkbox: Option<DCheckBox>,
    pub switch: Option<DSwitch>,
    pub radio: Option<DRadio>,
    pub slider: Option<DSlider>,
    pub textinput: Option<DTextInput>,
    pub progress: Option<DProgressBar>,
    pub spinner: Option<DSpinner>,
    pub skeleton: Option<DSkeleton>,
    pub bar: Option<DBarChart>,
    pub line: Option<DLineChart>,
    pub pie: Option<DPieChart>,
}

/// How many demos the stage has. Asserted against the catalogue.
pub const DEMOS: usize = 18;

impl Demos {
    /// Make **every** demo present, for the cascade's benefit.
    ///
    /// **The cascade runs once**, when the stylesheet is set, and
    /// `Option<W>::node` returns the *`Option`* when it is `None` — so a
    /// demo that was absent at that moment had its style resolved from
    /// `Spacer`'s defaults rather than the widget's. Every badge, chip,
    /// toggle and chart then painted with a transparent background and
    /// no size of its own, which on screen is a widget that looks like
    /// a label. That is exactly what it looked like.
    ///
    /// So the tree is built full, cascaded, and reduced to one demo on
    /// the first pump. The style array is indexed by node and is not
    /// touched by hiding, so the resolved styles survive.
    pub fn fill_all(&mut self) {
        self.label = Some(Default::default());
        self.badge = Some(Default::default());
        self.chip = Some(Default::default());
        self.avatar = Some(Default::default());
        self.icon = Some(Default::default());
        self.divider = Some(Default::default());
        self.button = Some(Default::default());
        self.checkbox = Some(Default::default());
        self.switch = Some(Default::default());
        self.radio = Some(Default::default());
        self.slider = Some(Default::default());
        self.textinput = Some(Default::default());
        self.progress = Some(Default::default());
        self.spinner = Some(Default::default());
        self.skeleton = Some(Default::default());
        self.bar = Some(Default::default());
        self.line = Some(Default::default());
        self.pie = Some(Default::default());
    }

    /// How many demos are present. **Exactly one, always** — and the
    /// reason this exists rather than being obvious: `clear` is a
    /// hand-written list of eighteen fields, and a field left out of it
    /// leaves the previous demo on the stage underneath the new one.
    /// That is the one bug this shape makes possible and a `Dyn` did
    /// not, so it is the one the tests gate.
    pub fn shown(&self) -> usize {
        let mut n = 0;
        n += self.label.is_some() as usize;
        n += self.badge.is_some() as usize;
        n += self.chip.is_some() as usize;
        n += self.avatar.is_some() as usize;
        n += self.icon.is_some() as usize;
        n += self.divider.is_some() as usize;
        n += self.button.is_some() as usize;
        n += self.checkbox.is_some() as usize;
        n += self.switch.is_some() as usize;
        n += self.radio.is_some() as usize;
        n += self.slider.is_some() as usize;
        n += self.textinput.is_some() as usize;
        n += self.progress.is_some() as usize;
        n += self.spinner.is_some() as usize;
        n += self.skeleton.is_some() as usize;
        n += self.bar.is_some() as usize;
        n += self.line.is_some() as usize;
        n += self.pie.is_some() as usize;
        n
    }

    /// Hide everything. Called before each `show`, so a selection cannot
    /// leave the previous demo behind.
    pub fn clear(&mut self) {
        self.label = None;
        self.badge = None;
        self.chip = None;
        self.avatar = None;
        self.icon = None;
        self.divider = None;
        self.button = None;
        self.checkbox = None;
        self.switch = None;
        self.radio = None;
        self.slider = None;
        self.textinput = None;
        self.progress = None;
        self.spinner = None;
        self.skeleton = None;
        self.bar = None;
        self.line = None;
        self.pie = None;
    }
}
