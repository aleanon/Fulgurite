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
    #[widget_id("d_label")]
    pub a: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DBadge {
    #[widget_id("d_badge")]
    pub a: Badge<String>,
    #[widget_id("d_badge")]
    pub b: Badge<String>,
    #[widget_id("d_badge")]
    pub c: Badge<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DChip {
    #[widget_id("d_chip")]
    pub a: Chip<String>,
    #[widget_id("d_chip")]
    pub b: Chip<String>,
    #[widget_id("d_chip")]
    pub c: Chip<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DAvatar {
    #[widget_id("d_avatar")]
    pub a: Avatar<String>,
    #[widget_id("d_avatar")]
    pub b: Avatar<String>,
    #[widget_id("d_avatar")]
    pub c: Avatar<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DIcon {
    #[widget_id("d_icon")]
    pub a: Icon,
    #[widget_id("d_icon")]
    pub b: Icon,
    #[widget_id("d_icon")]
    pub c: Icon,
    #[widget_id("d_icon")]
    pub d: Icon,
    #[widget_id("d_icon")]
    pub e: Icon,
    #[widget_id("d_icon")]
    pub f: Icon,
    #[widget_id("d_icon")]
    pub g: Icon,
    #[widget_id("d_icon")]
    pub h: Icon,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DDivider {
    #[widget_id("d_divider")]
    pub a: Divider,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DButton {
    #[widget_id("d_button")]
    pub a: Button<Show, String>,
    #[widget_id("d_button")]
    pub b: Button<Show, String>,
    #[widget_id("d_button")]
    pub c: Button<Show, String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DCheckBox {
    #[widget_id("d_checkbox")]
    pub a: CheckBox<Show>,
    #[widget_id("d_checkbox")]
    pub b: CheckBox<Show>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSwitch {
    #[widget_id("d_switch")]
    pub a: Switch<Show>,
    #[widget_id("d_switch")]
    pub b: Switch<Show>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DRadio {
    #[widget_id("d_radio")]
    pub a: Radio<Show>,
    #[widget_id("d_radio")]
    pub b: Radio<Show>,
    #[widget_id("d_radio")]
    pub c: Radio<Show>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSlider {
    #[widget_id("d_slider")]
    pub a: Slider,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DTextInput {
    #[widget_id("d_textinput")]
    pub a: TextInput<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DProgressBar {
    #[widget_id("d_progress")]
    pub a: ProgressBar,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSpinner {
    #[widget_id("d_spinner")]
    pub a: Spinner,
    #[widget_id("d_spinner")]
    pub b: Spinner,
    #[widget_id("d_spinner")]
    pub c: Spinner,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSkeleton {
    #[widget_id("d_skeleton")]
    pub a: Skeleton,
    #[widget_id("d_skeleton")]
    pub b: Skeleton,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DBarChart {
    #[widget_id("d_bar")]
    pub a: BarChart,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DLineChart {
    #[widget_id("d_line")]
    pub a: LineChart,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DPieChart {
    #[widget_id("d_pie")]
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
