//! **Every widget in this crate, in one tree** — M54/T54.0.
//!
//! Shared by the role table (`a11y_roles.rs`) and the contrast harness
//! (`text_contrast.rs`), so both read the same catalogue: one field per
//! `Node` impl (the toggles in both states, each strip with a selected
//! and an unselected item), each seeded by [`build`] so it has something
//! to say and something to paint.
#![allow(dead_code, clippy::field_reassign_with_default)]

use lumen_static::prelude::*;
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::{
    Avatar, Combobox, Drawer, LinkedMarkdown, Markdown, Menu, Modal, Pad, PickList, Picker, Sheet, Strip,
    BOTTOM, COLOR, DATE, FILE, RAIL, TABS, TIME,
};

/// The application the catalogue's handlers run on — M54/T54.4: the
/// role table dispatches actions, and a handler needs a state to run on.
#[derive(lumen_static::App, Default)]
pub struct CatApp {
    #[tree]
    pub ui: Ui,
}

pub type S = CatApp;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
pub struct Ui {
    #[widget_id("Label")]
    pub label: Label<String>,
    #[widget_id("Button")]
    pub button: Button<S, String>,
    #[widget_id("Fixed")]
    pub fixed: Fixed,
    #[widget_id("Spacer")]
    pub spacer: Spacer,
    #[widget_id("CheckBox-off")]
    pub cb_off: CheckBox<S>,
    #[widget_id("CheckBox-on")]
    pub cb_on: CheckBox<S>,
    #[widget_id("Switch-off")]
    pub sw_off: Switch<S>,
    #[widget_id("Switch-on")]
    pub sw_on: Switch<S>,
    #[widget_id("Radio-off")]
    pub ra_off: Radio<S>,
    #[widget_id("Radio-on")]
    pub ra_on: Radio<S>,
    #[widget_id("Slider")]
    pub slider: Slider,
    #[widget_id("TextInput")]
    pub input: TextInput<String>,
    #[widget_id("TextArea")]
    pub area: TextArea<String>,
    #[widget_id("Image")]
    pub image: Image,
    // M84/T84.8: a video, and its seek track (`#video-track`) as a part.
    #[widget_id("Video")]
    pub video: Video<S>,
    #[widget_id("ScrollView")]
    pub scroll: ScrollView<Label<String>>,
    #[widget_id("Select")]
    pub select: Select<S>,
    #[widget_id("Popup")]
    pub popup: Popup<Label<String>>,
    #[widget_id("Glass")]
    pub glass: Glass<Label<String>>,
    #[widget_id("NumberInput")]
    pub number: NumberInput,
    #[widget_id("RangeSlider")]
    pub range: RangeSlider,
    #[widget_id("SearchField")]
    pub search: SearchField,
    #[widget_id("OptionList")]
    pub options: OptionList,
    #[widget_id("PickList")]
    pub picklist: PickList,
    #[widget_id("Combobox")]
    pub combobox: Combobox,
    #[widget_id("Menu")]
    pub menu: Menu,
    #[widget_id("ProgressBar")]
    pub progress: ProgressBar,
    #[widget_id("Spinner")]
    pub spinner: Spinner,
    #[widget_id("Skeleton")]
    pub skeleton: Skeleton,
    #[widget_id("BarChart")]
    pub bar: BarChart,
    #[widget_id("LineChart")]
    pub line: LineChart,
    #[widget_id("PieChart")]
    pub pie: PieChart,
    #[widget_id("Pad")]
    pub pad: Pad<COLOR>,
    #[widget_id("ColorPicker")]
    pub color: Picker<COLOR>,
    #[widget_id("DatePicker")]
    pub date: Picker<DATE>,
    #[widget_id("TimePicker")]
    pub time: Picker<TIME>,
    #[widget_id("FilePicker")]
    pub file: Picker<FILE>,
    #[widget_id("SplitPane")]
    pub split: SplitPane<Label<String>, Label<String>>,
    #[widget_id("Collapsible")]
    pub collapsible: Collapsible<Label<String>, String>,
    #[widget_id("PaneGrid")]
    pub panes: PaneGrid<Label<String>, Label<String>, Label<String>, Label<String>>,
    #[widget_id("Tabs")]
    pub tabs: Strip<TABS>,
    #[widget_id("BottomNav")]
    pub bottom: Strip<BOTTOM>,
    #[widget_id("NavigationRail")]
    pub rail: Strip<RAIL>,
    #[widget_id("AppBar")]
    pub appbar: AppBar<String>,
    #[widget_id("PullToRefresh")]
    pub pull: PullToRefresh<Label<String>>,
    #[widget_id("Card")]
    pub card: Card<Label<String>>,
    // M85/T85.2: a label pinned at an offset.
    #[widget_id("Pin")]
    pub pin: Pin<Label<String>>,
    // M85/T85.4: a label told when it enters the view. In the tree, not
    // an arena, so the arena refusal does not apply.
    #[widget_id("Pop")]
    pub pop: Pop<S, Label<String>>,
    #[widget_id("Toast")]
    pub toast: Toast<String>,
    #[widget_id("Tooltip")]
    pub tooltip: Tooltip<Label<String>, String>,
    #[widget_id("Modal")]
    pub modal: Modal<Label<String>>,
    #[widget_id("Sheet")]
    pub sheet: Sheet<Label<String>>,
    #[widget_id("Drawer")]
    pub drawer: Drawer<Label<String>>,
    #[widget_id("Pagination")]
    pub pagination: Pagination,
    #[widget_id("Tree")]
    pub tree: Tree,
    #[widget_id("DataGrid")]
    pub grid: DataGrid<Label<String>>,
    #[widget_id("RichText")]
    pub rich: RichText,
    #[widget_id("Markdown")]
    pub markdown: Markdown,
    #[widget_id("LinkedMarkdown")]
    pub linked: LinkedMarkdown<S>,
    #[widget_id("RichTextEditor")]
    pub editor: RichTextEditor,
    #[widget_id("PaintBox")]
    pub paint: PaintBox,
    #[widget_id("Divider")]
    pub divider: Divider,
    #[widget_id("Badge")]
    pub badge: Badge<String>,
    // Its handlers take the application (M57/B2 made a chip pressable).
    #[widget_id("Chip")]
    pub chip: Chip<String, CatApp>,
    // A control with its words beside it (M57/B11); the row reads as its
    // label.
    #[widget_id("Labelled")]
    pub labelled: lumen_static_widgets::Labelled<lumen_static_widgets::ops::Fixed>,
    #[widget_id("Avatar")]
    pub avatar: Avatar<String>,
    #[widget_id("Icon")]
    pub icon: Icon,
    // M80/T80.4: read-only text that takes focus and a selection; built
    // with its text selected, so the contrast harness measures the run
    // over its tint.
    #[widget_id("Selectable")]
    pub selectable: Selectable<String>,
    // M82/T82.8: a run drawn as outlines at an angle; under the catalogue's
    // stand-in measurer it paints its axis-aligned fallback, which the
    // contrast harness reads as text.
    #[widget_id("VectorText")]
    pub vector: VectorText<String>,
    // M83/T83.6: a node a custom shader draws, only in a build with this
    // crate's `shader`. A headless frame has no shader table, so it paints
    // its fallback: a solid fill from its first four uniform bytes.
    #[cfg(feature = "shader")]
    #[widget_id("ShaderView")]
    pub shader: lumen_static_widgets::ShaderView<16>,
}

fn bind_row(l: &mut Label<String>, i: usize) {
    l.set(format!("row {i}"));
}

fn bind_select(r: &mut lumen_static_widgets::select::Row<S>, i: usize) {
    r.label = format!("option {i}");
}

/// Every row's widget, seeded so it has something to say.
pub fn build() -> Ui {
    let mut ui = Ui::default();
    ui.label = Label::new("a label".into());
    ui.button.label = "Save".into();
    ui.cb_on.on = true;
    ui.sw_on.on = true;
    ui.ra_on.on = true;
    // Every control that can carry a name is built with one (M54/T54.3):
    // the role table requires it to reach the emitted node.
    for (t, l) in [(&mut ui.cb_off, "Subscribe"), (&mut ui.cb_on, "Agree")] {
        t.set_label(l);
    }
    for (t, l) in [(&mut ui.sw_off, "Wi-Fi"), (&mut ui.sw_on, "Bluetooth")] {
        t.set_label(l);
    }
    for (t, l) in [(&mut ui.ra_off, "Small"), (&mut ui.ra_on, "Large")] {
        t.set_label(l);
    }
    ui.slider = Slider::new(0.25);
    ui.slider.set_label("Volume");
    ui.input = TextInput::new("typed");
    ui.input.set_label("Name");
    ui.area = TextArea::new("two\nlines");
    ui.image = Image::new(lumen_static::ImageId(1), lumen_static::Size { w: 16.0, h: 16.0 });
    ui.image.set_alt("A red square");
    ui.video = Video::new(lumen_static::ImageId(1), lumen_static::Size { w: 16.0, h: 16.0 });
    ui.video.duration_ms = 65_000;
    ui.video.position_ms = 5_000;
    ui.video.set_label("A clip");
    ui.scroll.content = Label::new("scrolled".into());
    ui.select = Select::new(3, 20.0, bind_select);
    ui.select.trigger.label = "option 0".into();
    ui.select.set_label("Size");
    ui.number = NumberInput::new(3.0, 0.0, 10.0, 1.0);
    ui.number.set_label("Quantity");
    ui.range = RangeSlider::new(0.2, 0.8);
    ui.range.set_label("Price");
    ui.search = SearchField::new("query");
    ui.search.set_label("Search");
    ui.options = OptionList::new(&["one", "two"]);
    ui.picklist = PickList::new(&["one", "two"]);
    ui.combobox = Combobox::new(&["one", "two"]);
    ui.menu = Menu::new(&["one", "two"]);
    ui.picklist.set_label("Fruit");
    ui.combobox.set_label("City");
    ui.menu.set_label("Actions");
    ui.color.set_label("Colour");
    ui.date.set_label("Date");
    ui.time.set_label("Time");
    ui.file.set_label("File");
    ui.progress = ProgressBar::new(0.4);
    ui.bar = BarChart::new(&[1.0, 2.0]);
    ui.line = LineChart::new(&[1.0, 2.0]);
    ui.pie = PieChart::new(&[1.0, 2.0]);
    ui.collapsible = Collapsible::new("Details".into(), Label::new("body".into()));
    ui.tabs = Strip::new(&["One", "Two"]);
    ui.bottom = Strip::new(&["One", "Two"]);
    ui.rail = Strip::new(&["One", "Two"]);
    ui.appbar = AppBar::new("Title".into());
    ui.card = Card::new(Label::new("in a card".into()));
    ui.pin = Pin::new(Label::new("pinned".into()), 8.0, 4.0);
    ui.pop = Pop::new(Label::new("popped".into()), lumen_static_widgets::ops::ignore_click::<S>);
    ui.toast = Toast::new("Saved".into());
    ui.tooltip = Tooltip::new(Label::new("anchor".into()), "a tip".into());
    ui.pagination = Pagination::new(5);
    ui.tree = Tree::new(&[Branch::new("root", 0), Branch::new("leaf", 1)]);
    ui.grid = DataGrid::new(&["Name", "Size"], 3, 20.0, bind_row);
    ui.rich = RichText::new("some rich text");
    ui.markdown = Markdown::new("some **bold** text");
    ui.linked = LinkedMarkdown::linked("see [docs](https://a.example) now", lumen_static_widgets::ops::ignore_click::<S>);
    ui.editor = RichTextEditor::new("edit me");
    ui.badge = Badge::new("3".into());
    ui.chip = Chip::new("tag".into());
    ui.labelled = lumen_static_widgets::Labelled::new(lumen_static_widgets::ops::Fixed { w: 16.0, h: 16.0 }, "Remember me");
    ui.avatar.set("AB".into());
    ui.selectable = Selectable::new("selectable text".into());
    ui.selectable.select_all();
    ui.vector = VectorText::new("turned".into(), 30.0);
    #[cfg(feature = "shader")]
    {
        ui.shader = lumen_static_widgets::ShaderView::default().sized(16.0, 16.0);
        ui.shader.set_uniforms(&[0x25, 0x63, 0xeb, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        ui.shader.set_label("A blue plasma");
    }
    ui
}


/// The catalogue's frame: defaults for styles, laid out in `width`, and
/// painted with its overlays, as a window would.
pub fn frame(ui: &mut Ui, width: f32) -> (Vec<lumen_static::Style>, Vec<lumen_static::NodeState>, lumen_static::DisplayList) {
    use lumen_static::widget::Widget;
    let styles = lumen_static::pass::default_styles(ui);
    let mut st = vec![lumen_static::NodeState::default(); <Ui as Widget>::NODE_COUNT];
    lumen_static::pass::layout(
        ui,
        &styles,
        &mut st,
        lumen_static::Constraints { min_w: 0.0, max_w: width, min_h: 0.0, max_h: 20000.0 },
    );
    let mut list = lumen_static::DisplayList::new();
    let mut ov = lumen_static::overlay::Overlays::<8>::default();
    lumen_static::pass::paint_overlaid(&*ui, &st, &mut list, &mut ov);
    (styles, st, list)
}
