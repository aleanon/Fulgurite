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
/// One labelled control per *instance*, stamped from one macro.
///
/// **Seven types where there were three, and that is not duplication —
/// it is what an author id means here.** A `#[widget_id]` lives on a
/// *type's* field, so a composite instantiated twice carries the same id
/// twice; `PairCheck` was used for both checkboxes and both of them
/// answered to `d_checkbox_a`. `ui.resolve` refuses that ambiguity, so
/// neither control could be clicked by name, no stylesheet rule could
/// reach one without the other, and nothing said so.
///
/// Found on the **first run** of `just widget-drive` (M27/T27.3), which
/// could not address either checkbox. Eight ids were duplicated across
/// the three demos. `W1011` now reports it for everyone.
///
/// The labels keep unique ids *and* share a class: an id is an address
/// and must be unique, a class is a style handle and is meant to repeat.
/// The sheet uses the class.
macro_rules! pair {
    ($name:ident, $ctrl:ty, $cid:literal, $lid:literal) => {
        #[derive(StaticTree, Default)]
        #[widget(row, spacing = 10.0)]
        pub struct $name {
            #[widget_id($cid)]
            pub t: $ctrl,
            #[widget_id($lid)]
            #[widget_class("pair-label")]
            pub l: Label<String>,
        }
    };
}

pair!(PairCheckA, CheckBox<Store>, "d_checkbox_a", "d_checkbox_al");
pair!(PairCheckB, CheckBox<Store>, "d_checkbox_b", "d_checkbox_bl");
pair!(PairSwitchA, Switch<Store>, "d_switch_a", "d_switch_al");
pair!(PairSwitchB, Switch<Store>, "d_switch_b", "d_switch_bl");
pair!(PairRadioA, Radio<Store>, "d_radio_a", "d_radio_al");
pair!(PairRadioB, Radio<Store>, "d_radio_b", "d_radio_bl");
pair!(PairRadioC, Radio<Store>, "d_radio_c", "d_radio_cl");

/// A card with something on it — the point being that the something is
/// live. `docs/plan-widgets-gallery.md` §3 records what a wrapper that
/// does not route handlers looks like: every control inside it is
/// hit-testable, hoverable, pressable and wired to nothing.
/// M27/T27.8's fifth, which the tranche's table named and its as-built
/// section did not: four panes and three dividers.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DPaneGrid {
    #[widget_id("d_panegrid_a")]
    pub a: PaneGrid<Label<String>, Label<String>, Label<String>, Label<String>>,
}

/// M27/T27.12's four: the text tail and the draw callback.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DRichText {
    #[widget_id("d_richtext_a")]
    pub a: RichText,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DMarkdown {
    #[widget_id("d_markdown_a")]
    pub a: Markdown,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DRichTextEditor {
    #[widget_id("d_editor_a")]
    pub a: RichTextEditor,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DPaintBox {
    #[widget_id("d_paintbox_a")]
    pub a: PaintBox,
}

/// M27/T27.11's four. Each is a trigger and a popup of cells, and the
/// only thing that differs is what a cell says — which is why they are
/// one type with a `KIND` and four demos rather than four widgets.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DColorPicker {
    #[widget_id("d_colorpicker_a")]
    pub a: ColorPicker,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DDatePicker {
    #[widget_id("d_datepicker_a")]
    pub a: DatePicker,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DTimePicker {
    #[widget_id("d_timepicker_a")]
    pub a: TimePicker,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DFilePicker {
    #[widget_id("d_filepicker_a")]
    pub a: FilePicker,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DPagination {
    #[widget_id("d_pagination_a")]
    pub a: Pagination,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DTree {
    #[widget_id("d_tree_a")]
    pub a: Tree,
}

/// A grid's rows are the **application's**, bound by the `fn` `Repeat`
/// takes — which is why this demo declares a row type and the chooser
/// demos do not.
/// One cell of a grid row.
///
/// **A container, not a bare `Label`.** A derived container is `FILL`
/// across its main axis, so a row of these divides its width evenly —
/// which is exactly the column the grid's header is drawn over. A bare
/// `Label` is `Shrink`, so cell 1 starts wherever cell 0's text ended
/// and the values sat 72 px left of their heading (M27/T27.11, finding
/// 4). Declared in the type rather than in the stylesheet because a
/// rule naming a *repeated row's* node matches nothing while its demo
/// is off stage, and an ambient `W1010` for a rule that is right is the
/// false positive the audit exists to avoid.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct GridCell {
    #[widget_id("d_grid_cell")]
    pub l: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
pub struct GridRow {
    #[widget_id("d_grid_name")]
    pub name: GridCell,
    #[widget_id("d_grid_size")]
    pub size: GridCell,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DDataGrid {
    #[widget_id("d_datagrid_a")]
    pub a: DataGrid<GridRow>,
}

/// A `Select` on the stage — M44/T44.10. The header's picker is one too,
/// but it is the drive's own navigation; this one is a demo it can
/// drive, controlled like any application's.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DSelect {
    #[widget_id("d_select_a")]
    pub a: Select<Store>,
}

/// Twelve buttons in a `ScrollView` too short for them — M44/T44.10.
/// The first carries an id, so a drive can put focus inside the view.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
pub struct ScrollRows {
    #[widget_id("d_scroll_first")]
    pub r0: Button<Store, String>,
    pub r1: Button<Store, String>,
    pub r2: Button<Store, String>,
    pub r3: Button<Store, String>,
    pub r4: Button<Store, String>,
    pub r5: Button<Store, String>,
    pub r6: Button<Store, String>,
    pub r7: Button<Store, String>,
    pub r8: Button<Store, String>,
    pub r9: Button<Store, String>,
    pub r10: Button<Store, String>,
    pub r11: Button<Store, String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DScrollView {
    #[widget_id("d_scrollview_a")]
    pub a: ScrollView<ScrollRows>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DPickList {
    #[widget_id("d_picklist_a")]
    pub a: PickList,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DCombobox {
    #[widget_id("d_combobox_a")]
    pub a: Combobox,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DMenu {
    #[widget_id("d_menu_a")]
    pub a: Menu,
}

/// A wrap: six chips that reflow when the box narrows.
#[derive(StaticTree, Default)]
#[widget(wrap, spacing = 8.0)]
pub struct DWrap {
    #[widget_id("d_wrap_a")]
    pub a: Chip<String>,
    #[widget_id("d_wrap_b")]
    pub b: Chip<String>,
    #[widget_id("d_wrap_c")]
    pub c: Chip<String>,
    #[widget_id("d_wrap_d")]
    pub d: Chip<String>,
    #[widget_id("d_wrap_e")]
    pub e: Chip<String>,
    #[widget_id("d_wrap_f")]
    pub f: Chip<String>,
}

/// A grid: the same six, in three columns however wide the box.
#[derive(StaticTree, Default)]
#[widget(grid, columns = 3, spacing = 8.0)]
pub struct DGrid {
    #[widget_id("d_grid_a")]
    pub a: Badge<String>,
    #[widget_id("d_grid_b")]
    pub b: Badge<String>,
    #[widget_id("d_grid_c")]
    pub c: Badge<String>,
    #[widget_id("d_grid_d")]
    pub d: Badge<String>,
    #[widget_id("d_grid_e")]
    pub e: Badge<String>,
    #[widget_id("d_grid_f")]
    pub f: Badge<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DSplitPane {
    #[widget_id("d_split_a")]
    pub a: SplitPane<Label<String>, Label<String>>,
}

/// An accordion is a **column of `Collapsible`s**, not a widget: a
/// container's children are its fields here, so the one-open rule is
/// three lines in the application rather than a type per section count.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
pub struct DAccordion {
    #[widget_id("d_acc_a")]
    pub a: Collapsible<Label<String>, String>,
    #[widget_id("d_acc_b")]
    pub b: Collapsible<Label<String>, String>,
    #[widget_id("d_acc_c")]
    pub c: Collapsible<Label<String>, String>,
}

/// The three veils. Each is seeded **open**, because a closed one is
/// absent by construction — `layout` zeroes it — and a demo of a modal
/// that shows nothing is a demo of nothing.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DModal {
    #[widget_id("d_modal_a")]
    pub a: Modal<ModalBody>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DSheet {
    #[widget_id("d_sheet_a")]
    pub a: Sheet<SheetBody>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct DDrawer {
    #[widget_id("d_drawer_a")]
    pub a: Drawer<DrawerBody>,
}

/// What sits on a veil: a title and a control, so the demo shows that a
/// wrapper routes handlers as well as that it paints.
///
/// **One type per instance**, stamped from a macro, for the reason
/// `pair!` above gives: a `#[widget_id]` lives on a *type's* field, so a
/// composite used three times carries each id three times and `W1011`
/// reports all of them as unaddressable. `just widget-drive` found this
/// the moment it tried to press a veil's scrim — `#d_veil_title` matched
/// three nodes and `ui.resolve` refused. That is the **third** time this
/// shape has appeared in M27; the first was `PairCheck`.
macro_rules! veil_body {
    ($name:ident, $tid:literal, $gid:literal) => {
        #[derive(StaticTree, Default)]
        #[widget(column, spacing = 10.0)]
        #[widget_class("veil-body")]
        pub struct $name {
            #[widget_id($tid)]
            #[widget_class("veil-title")]
            pub title: Label<String>,
            #[widget_id($gid)]
            #[widget_class("veil-go")]
            pub go: Button<Store, String>,
        }
    };
}

veil_body!(ModalBody, "d_modal_title", "d_modal_go");
veil_body!(SheetBody, "d_sheet_title", "d_sheet_go");
veil_body!(DrawerBody, "d_drawer_title", "d_drawer_go");

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
pub struct DTabs {
    #[widget_id("d_tabs_a")]
    pub a: Tabs,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
pub struct DBottomNav {
    #[widget_id("d_bottomnav_a")]
    pub a: BottomNav,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DNavigationRail {
    #[widget_id("d_rail_a")]
    pub a: NavigationRail,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
pub struct DAppBar {
    #[widget_id("d_appbar_a")]
    pub a: AppBar<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
pub struct DPullToRefresh {
    #[widget_id("d_pull_a")]
    pub a: PullToRefresh<Label<String>>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DNumberInput {
    #[widget_id("d_number_a")]
    pub a: NumberInput,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
pub struct DRangeSlider {
    #[widget_id("d_range_a")]
    pub a: RangeSlider,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DSearchField {
    #[widget_id("d_search_a")]
    pub a: SearchField,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DCard {
    #[widget_id("d_card_a")]
    pub a: Card<CardBody>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
pub struct CardBody {
    #[widget_id("d_card_title")]
    pub title: Label<String>,
    #[widget_id("d_card_go")]
    pub go: Button<Store, String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DToast {
    #[widget_id("d_toast_a")]
    pub a: Toast<String>,
    #[widget_id("d_toast_b")]
    pub b: Toast<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DTooltip {
    #[widget_id("d_tooltip_a")]
    pub a: Tooltip<Button<Store, String>, String>,
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
    pub a: Button<Store, String>,
    #[widget_id("d_button_b")]
    pub b: Button<Store, String>,
    #[widget_id("d_button_c")]
    pub c: Button<Store, String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 34.0)]
pub struct DCheckBox {
    pub a: PairCheckA,
    pub b: PairCheckB,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 34.0)]
pub struct DSwitch {
    pub a: PairSwitchA,
    pub b: PairSwitchB,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 34.0)]
pub struct DRadio {
    pub a: PairRadioA,
    pub b: PairRadioB,
    pub c: PairRadioC,
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

/// M45/T45.7: the multi-line field, on the stage so the drive harness
/// can paste into it (`scripts/drive/widgets/text_and_slider.py`).
#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct DTextArea {
    #[widget_id("d_textarea_a")]
    pub a: TextArea<String>,
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
/// **`clippy::large_enum_variant` is allowed here, and cannot be
/// obeyed.** Its advice is to box the big variant; a `StaticTree` enum
/// lays its *active* variant out in place, in the parent's table, which
/// is the whole point — a widget behind a pointer is the dynamic tree
/// this project exists not to have. The variant that trips it is
/// `DTree`, whose 32 `Branch`es are inline for the same reason
/// (M27/T27.10). The cost is real and is the enum's by design: a
/// one-of-N stage is as large as its largest demo.
#[allow(clippy::large_enum_variant)]
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
    TextArea(DTextArea),
    Progress(DProgressBar),
    Spinner(DSpinner),
    Skeleton(DSkeleton),
    Bar(DBarChart),
    Line(DLineChart),
    Pie(DPieChart),
    Card(DCard),
    Toast(DToast),
    Tooltip(DTooltip),
    Number(DNumberInput),
    Range(DRangeSlider),
    Search(DSearchField),
    Tabs(DTabs),
    Bottom(DBottomNav),
    Rail(DNavigationRail),
    AppBar(DAppBar),
    Pull(DPullToRefresh),
    Modal(DModal),
    Sheet(DSheet),
    Drawer(DDrawer),
    Wrap(DWrap),
    Grid(DGrid),
    Split(DSplitPane),
    Accordion(DAccordion),
    PickList(DPickList),
    Combobox(DCombobox),
    Menu(DMenu),
    PaneGrid(DPaneGrid),
    RichText(DRichText),
    Markdown(DMarkdown),
    RichTextEditor(DRichTextEditor),
    PaintBox(DPaintBox),
    ColorPicker(DColorPicker),
    DatePicker(DDatePicker),
    TimePicker(DTimePicker),
    FilePicker(DFilePicker),
    Pagination(DPagination),
    Tree(DTree),
    DataGrid(DDataGrid),
    Select(DSelect),
    Scroll(DScrollView),
}

impl Default for Demo {
    fn default() -> Self {
        Demo::Label(DLabel::default())
    }
}

/// How many demos the stage has. Asserted against the catalogue.
pub const DEMOS: usize = 54;
