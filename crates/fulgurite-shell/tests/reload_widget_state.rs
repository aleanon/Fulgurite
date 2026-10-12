//! **A swap keeps what the user did, or says it did not** — M49/T49.0,
//! R5-F2 (`docs/plan-reload-hardening.md` §1.4, §2.3).
//!
//! The re-triage's scratch probe, promoted. One tree holds one of every
//! widget a user can change; the session changes each, saves, and
//! restores onto a freshly built tree — the window swap's sequence
//! without the dylib. Every row must end **kept, or with a log line
//! naming the widget**: rule 1 of the plan, "a swap either keeps a value
//! or says it did not". The table is printed whole before the verdict,
//! so a LOST row is visible beside the ones that survived.
//!
//! **The inventory.** A row per widget with state a user can change, in
//! the same discipline M27 used for actions: a widget added to
//! `fulgurite-widgets` with user state gets a row here.

use fulgurite::event::EventCtx;
use fulgurite::prelude::*;
use fulgurite::reload::{take_log, Reloadable};
use fulgurite::repeat::Repeat;
use fulgurite_widgets::prelude::*;

fn no_bind(_: &mut Label, _: usize) {}

/// A widget enum: which variant is showing is the user's too.
#[derive(StaticTree, fulgurite::Reloadable)]
pub enum Page {
    Home(Label),
    Settings(Slider),
}

impl Default for Page {
    fn default() -> Self {
        Page::Home(Label::default())
    }
}

#[derive(StaticTree, Default, fulgurite::Reloadable)]
#[widget(column, spacing = 0.0)]
pub struct Ui {
    pub text: TextInput<String>,
    pub area: TextArea<String>,
    pub search: SearchField,
    pub slider: Slider,
    pub range: RangeSlider,
    pub number: NumberInput,
    pub scroll: ScrollView<Label>,
    pub list: Repeat<Label>,
    pub grid: DataGrid<Label>,
    pub popup: Popup<Label>,
    pub modal: Modal<Label>,
    pub fold: Collapsible<Label>,
    pub tabs: Tabs,
    pub pick: PickList,
    pub tree: Tree,
    pub split: SplitPane<Label, Label>,
    pub panes: PaneGrid<Label, Label, Label, Label>,
    pub toggle: Toggle<u32>,
    pub pages: Pagination,
    pub color: ColorPicker,
    pub editor: RichTextEditor,
    pub page: Page,
}

#[derive(DeriveApp, fulgurite::Reloadable)]
pub struct Probe {
    /// The default mode, keep — what T49.5 teaches. T49.0 committed this
    /// with the then-taught `#[reload(fresh)]`, under which 23 of 23
    /// rows were lost with no line.
    #[tree]
    pub ui: Ui,
    #[state]
    #[reload(keep)]
    pub count: u32,
}

fn on_toggle(n: &mut u32, _: &EventCtx) {
    *n += 1;
}

/// What the build sets. Everything a user could change starts here.
fn build() -> Probe {
    let mut ui = Ui {
        list: Repeat::new(40, 20.0, no_bind),
        grid: DataGrid::new(&["a", "b"], 40, 20.0, no_bind),
        tabs: Tabs::new(&["one", "two", "three"]),
        pick: PickList::new(&["x", "y", "z"]),
        tree: Tree::new(&[Branch::new("root", 0), Branch::new("leaf", 1)]),
        pages: Pagination::new(10),
        number: NumberInput::new(1.0, 0.0, 100.0, 1.0),
        ..Ui::default()
    };
    ui.toggle.on_toggle = on_toggle;
    Probe { ui, count: 0 }
}

/// One row: the widget and field, how to read it, and what a user did.
struct Row {
    name: &'static str,
    /// The type name a fallback line would carry.
    logs_as: &'static str,
    read: fn(&Probe) -> String,
    change: fn(&mut Probe),
}

fn rows() -> Vec<Row> {
    vec![
        Row { name: "TextInput.text", logs_as: "TextInput", read: |p| p.ui.text.as_str().to_string(), change: |p| p.ui.text.set_str("typed") },
        Row { name: "TextInput.caret", logs_as: "TextInput", read: |p| p.ui.text.caret().to_string(), change: |_| {} },
        Row { name: "TextArea.text", logs_as: "TextArea", read: |p| p.ui.area.as_str().replace('\n', "\\n"), change: |p| p.ui.area.set_str("two\nlines") },
        Row { name: "TextArea.caret", logs_as: "TextArea", read: |p| p.ui.area.caret().to_string(), change: |_| {} },
        Row { name: "SearchField.text", logs_as: "SearchField", read: |p| p.ui.search.field.as_str().to_string(), change: |p| p.ui.search.field.set_str("query") },
        Row { name: "Slider.value", logs_as: "Slider", read: |p| p.ui.slider.value().to_string(), change: |p| { p.ui.slider.set_value(0.75); } },
        Row { name: "RangeSlider.low/high", logs_as: "RangeSlider", read: |p| format!("{}..{}", p.ui.range.low(), p.ui.range.high()), change: |p| { p.ui.range.set_low(0.2); p.ui.range.set_high(0.6); } },
        Row { name: "NumberInput.value", logs_as: "NumberInput", read: |p| p.ui.number.value().to_string(), change: |p| { p.ui.number.set_value(42.0); } },
        Row { name: "ScrollView.scroll", logs_as: "ScrollView", read: |p| p.ui.scroll.scroll.to_string(), change: |p| p.ui.scroll.scroll = 40.0 },
        Row { name: "Repeat.scroll", logs_as: "Repeat", read: |p| p.ui.list.scroll.to_string(), change: |p| p.ui.list.scroll = 30.0 },
        Row { name: "DataGrid.scroll", logs_as: "DataGrid", read: |p| p.ui.grid.rows.scroll.to_string(), change: |p| p.ui.grid.rows.scroll = 25.0 },
        Row { name: "Popup.open", logs_as: "Popup", read: |p| p.ui.popup.open.to_string(), change: |p| p.ui.popup.open = true },
        Row { name: "Veil.open", logs_as: "Veil", read: |p| p.ui.modal.open.to_string(), change: |p| { p.ui.modal.open(); } },
        Row { name: "Collapsible.open", logs_as: "Collapsible", read: |p| p.ui.fold.is_open().to_string(), change: |p| { p.ui.fold.set_open(true); } },
        Row { name: "Strip.selected", logs_as: "Strip", read: |p| p.ui.tabs.selected().to_string(), change: |p| { p.ui.tabs.select(2); } },
        Row { name: "Chooser.selected", logs_as: "Chooser", read: |p| format!("{:?}", p.ui.pick.selected()), change: |p| { p.ui.pick.select(1); } },
        Row { name: "Tree.open", logs_as: "Tree", read: |p| p.ui.tree.is_open(0).to_string(), change: |p| { p.ui.tree.toggle(0); } },
        Row { name: "SplitPane.split", logs_as: "SplitPane", read: |p| p.ui.split.split().to_string(), change: |p| { p.ui.split.set_split(0.3); } },
        Row { name: "PaneGrid.dividers", logs_as: "PaneGrid", read: |p| format!("{} {} {}", p.ui.panes.vertical(), p.ui.panes.horizontal(0), p.ui.panes.horizontal(1)), change: |p| { p.ui.panes.set_divider(0, 0.3); p.ui.panes.set_divider(2, 0.7); } },
        Row { name: "Toggle.on", logs_as: "Toggle", read: |p| p.ui.toggle.on.to_string(), change: |p| p.ui.toggle.on = true },
        Row { name: "Pagination.page", logs_as: "Pagination", read: |p| p.ui.pages.page().to_string(), change: |p| { p.ui.pages.go(4); } },
        Row { name: "Picker.color", logs_as: "Picker", read: |p| p.ui.color.pad.content.color.to_string(), change: |p| p.ui.color.pad.content.color = 3 },
        Row { name: "RichTextEditor.text", logs_as: "RichTextEditor", read: |p| p.ui.editor.input.as_str().to_string(), change: |p| p.ui.editor.input.set_str("# notes") },
        Row { name: "enum variant + state", logs_as: "Page", read: |p| match &p.ui.page { Page::Home(_) => "Home".into(), Page::Settings(s) => format!("Settings {}", s.value()) }, change: |p| { let mut s = Slider::default(); s.set_value(0.25); p.ui.page = Page::Settings(s); } },
    ]
}

#[test]
fn every_widget_keeps_its_users_state_or_says_it_did_not() {
    let rows = rows();
    let mut v1 = build();
    let fresh: Vec<String> = rows.iter().map(|r| (r.read)(&v1)).collect();
    for r in &rows {
        (r.change)(&mut v1);
    }
    v1.count = 3;
    let before: Vec<String> = rows.iter().map(|r| (r.read)(&v1)).collect();
    // The setup must happen: a change that did not land makes "kept"
    // and "lost" the same row.
    for ((r, f), b) in rows.iter().zip(&fresh).zip(&before) {
        assert_ne!(f, b, "{}: the change did not land, so this row proves nothing", r.name);
    }

    // The window swap's sequence: save, a fresh build, restore — with the
    // log drained between, as `hot::boot` drains it.
    let saved = v1.save();
    let _ = take_log();
    let mut v2 = build();
    v2.restore(&saved);
    let log = take_log();

    println!("{:<22} {:>14} {:>14}  verdict", "widget.field", "user set", "after swap");
    let mut lost = Vec::new();
    for (r, b) in rows.iter().zip(&before) {
        let after = (r.read)(&v2);
        let lines = log.iter().filter(|l| l.contains(r.logs_as)).count();
        let verdict = if &after == b {
            "kept"
        } else if lines > 0 {
            "logged"
        } else {
            lost.push(r.name);
            "LOST"
        };
        println!("{:<22} {:>14} {:>14}  {verdict} ({lines} lines)", r.name, b, after);
    }
    println!("#[state] count: {} -> {}", v1.count, v2.count);
    println!("{} of {} rows lost, {} log lines", lost.len(), rows.len(), log.len());
    assert_eq!(v2.count, 3, "the #[state] value must survive");
    assert!(lost.is_empty(), "lost with no log line: {lost:?}");
}
