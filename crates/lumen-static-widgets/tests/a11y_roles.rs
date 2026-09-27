//! **The role table** — M54/T54.0 (`docs/plan-a11y-platform.md` §0
//! rule 2, R15-F3).
//!
//! One row per `Node` impl in this crate, every row built in one tree,
//! laid out, and read back **from the emitted `accesskit::Node`** — never
//! from the widget's type, because a check that reads the type cannot see
//! a widget that forgot to publish (the review deleted `Toggle::observe`'s
//! `set_role` and the whole suite stayed green).
//!
//! Each row states what a screen reader should be told: the role, whether
//! the control carries a name when it is built with one, and — for the
//! toggles, built in both states — the `toggled` value. The table prints
//! one line per row, then `rows checked: n / Node impls: m`, where `m` is
//! counted from this crate's source, so a new widget without a row fails
//! [`every_node_impl_has_a_row`] by name.
#![cfg(feature = "a11y")]

use std::collections::{BTreeMap, BTreeSet};

use lumen_static::observe::accesskit::{Role, Toggled};
use lumen_static::observe::{emit_root, AkNode, NodeId};
use lumen_static::widget::Widget;
use lumen_static::{pass, Constraints};

mod catalogue;
use catalogue::{build, Ui};

/// What one row must report. `impl_name` is the `impl Node for X` it
/// covers, so the coverage check compares names, not counts.
struct Row {
    wid: &'static str,
    impl_name: &'static str,
    role: Role,
    /// Built with a name, so the node must carry a non-empty one.
    named: bool,
    toggled: Option<Toggled>,
}

const fn row(wid: &'static str, impl_name: &'static str, role: Role, named: bool) -> Row {
    Row { wid, impl_name, role, named, toggled: None }
}

const fn toggle(wid: &'static str, role: Role, on: bool) -> Row {
    Row {
        wid,
        impl_name: "Toggle",
        role,
        named: true,
        toggled: Some(if on { Toggled::True } else { Toggled::False }),
    }
}

/// The table. `GenericContainer` appears only where a widget carries no
/// meaning of its own, and [`GENERIC_BOUND`] lists them, so the choice is
/// visible rather than a default nobody overrode.
const ROWS: &[Row] = &[
    row("Label", "Label", Role::Label, false),
    row("Button", "Button", Role::Button, true),
    row("Fixed", "Fixed", Role::GenericContainer, false),
    row("Spacer", "Spacer", Role::GenericContainer, false),
    toggle("CheckBox-off", Role::CheckBox, false),
    toggle("CheckBox-on", Role::CheckBox, true),
    toggle("Switch-off", Role::Switch, false),
    toggle("Switch-on", Role::Switch, true),
    toggle("Radio-off", Role::RadioButton, false),
    toggle("Radio-on", Role::RadioButton, true),
    row("Slider", "Slider", Role::Slider, true),
    row("TextInput", "TextInput", Role::TextInput, true),
    row("TextArea", "TextArea", Role::MultilineTextInput, false),
    row("Image", "Image", Role::Image, true),
    row("ScrollView", "ScrollView", Role::ScrollView, false),
    row("Select", "Select", Role::ComboBox, true),
    row("Popup", "Popup", Role::GenericContainer, false),
    row("Glass", "Glass", Role::GenericContainer, false),
    row("NumberInput", "NumberInput", Role::SpinButton, true),
    row("RangeSlider", "RangeSlider", Role::Slider, true),
    row("SearchField", "SearchField", Role::SearchInput, true),
    row("OptionList", "OptionList", Role::ListBox, false),
    row("PickList", "Chooser", Role::ComboBox, true),
    row("Combobox", "Chooser", Role::ComboBox, true),
    row("Menu", "Chooser", Role::ComboBox, true),
    row("ProgressBar", "ProgressBar", Role::ProgressIndicator, false),
    row("Spinner", "Spinner", Role::ProgressIndicator, false),
    row("Skeleton", "Skeleton", Role::GenericContainer, false),
    row("BarChart", "Chart", Role::Figure, false),
    row("LineChart", "Chart", Role::Figure, false),
    row("PieChart", "Chart", Role::Figure, false),
    row("Pad", "Pad", Role::GenericContainer, false),
    row("ColorPicker", "Picker", Role::ComboBox, true),
    row("DatePicker", "Picker", Role::ComboBox, true),
    row("TimePicker", "Picker", Role::ComboBox, true),
    row("FilePicker", "Picker", Role::ComboBox, true),
    row("SplitPane", "SplitPane", Role::Splitter, false),
    row("Collapsible", "Collapsible", Role::DisclosureTriangle, true),
    row("PaneGrid", "PaneGrid", Role::Group, false),
    row("Tabs", "Strip", Role::TabList, false),
    row("BottomNav", "Strip", Role::TabList, false),
    row("NavigationRail", "Strip", Role::TabList, false),
    row("AppBar", "AppBar", Role::Banner, false),
    row("PullToRefresh", "PullToRefresh", Role::GenericContainer, false),
    row("Card", "Card", Role::Group, false),
    row("Toast", "Toast", Role::Status, false),
    row("Tooltip", "Tooltip", Role::Group, false),
    row("Modal", "Veil", Role::Dialog, false),
    row("Sheet", "Veil", Role::Dialog, false),
    row("Drawer", "Veil", Role::Dialog, false),
    row("Pagination", "Pagination", Role::ListBox, false),
    row("Tree", "Tree", Role::Tree, false),
    row("DataGrid", "DataGrid", Role::Grid, false),
    row("RichText", "RichText", Role::Paragraph, false),
    row("Markdown", "Markdown", Role::Paragraph, false),
    row("RichTextEditor", "RichTextEditor", Role::TextInput, false),
    row("PaintBox", "PaintBox", Role::Canvas, false),
    row("Divider", "Divider", Role::Splitter, false),
    row("Badge", "Badge", Role::Label, false),
    row("Chip", "Chip", Role::Label, false),
    row("Avatar", "Avatar", Role::Image, false),
    row("Icon", "Icon", Role::Image, false),
];

/// The kinds that report `GenericContainer` on purpose: a box with a
/// fixed size, a gap, a picker's cell pad (its trigger carries the
/// combo box), two wrappers that add paint and no meaning, a loading
/// placeholder, and a gesture wrapper around content that has its own.
const GENERIC_BOUND: &[&str] =
    &["Fixed", "Spacer", "Popup", "Glass", "Skeleton", "Pad", "PullToRefresh"];

/// Lay the tree out and emit it; every node keyed by its author id.
fn snapshot(ui: &mut Ui) -> BTreeMap<&'static str, AkNode> {
    let styles = pass::default_styles(ui);
    let mut st = vec![lumen_static::NodeState::default(); <Ui as Widget>::NODE_COUNT];
    pass::layout(ui, &styles, &mut st, Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 20000.0 });
    let table = <Ui as Widget>::SUB_TABLE;
    let id = |i: usize| (i + 1) as u64;
    let mut out: Vec<(NodeId, AkNode)> = Vec::new();
    emit_root(table, &*ui, &st, &id, &mut out);
    let by_id: BTreeMap<u64, AkNode> = out.into_iter().map(|(i, n)| (i.0, n)).collect();
    let mut m = BTreeMap::new();
    for r in ROWS {
        let i = lumen_static::style::index_of_wid(table, r.wid).unwrap_or_else(|| panic!("setup: no #{}", r.wid));
        let n = by_id.get(&(i as u64 + 1)).unwrap_or_else(|| panic!("#{} was not emitted", r.wid));
        m.insert(r.wid, n.clone());
    }
    m
}

fn describe(r: &Row, n: &AkNode) -> String {
    format!(
        "{:<15} role={:?} label={:?} toggled={:?}",
        r.wid,
        n.role(),
        n.label().filter(|l| !l.is_empty()),
        n.toggled()
    )
}

/// The `impl Node for X` names in this crate's source.
fn node_impls() -> BTreeSet<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut names = BTreeSet::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        for line in std::fs::read_to_string(&p).unwrap().lines() {
            let t = line.trim_start();
            if !t.starts_with("impl") {
                continue;
            }
            if let Some(k) = t.find(" Node for ") {
                let rest = &t[k + " Node for ".len()..];
                let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                names.insert(name);
            }
        }
    }
    names
}

#[test]
fn every_node_impl_has_a_row() {
    let impls = node_impls();
    let covered: BTreeSet<String> = ROWS.iter().map(|r| r.impl_name.to_string()).collect();
    println!("rows checked: {} / Node impls: {} ({} covered)", ROWS.len(), impls.len(), covered.len());
    let missing: Vec<_> = impls.difference(&covered).collect();
    let stale: Vec<_> = covered.difference(&impls).collect();
    assert!(impls.len() > 30, "setup: only {} `impl Node for` found in src/ -- the scan is broken", impls.len());
    assert!(missing.is_empty(), "Node impls with no row in the table: {missing:?}");
    assert!(stale.is_empty(), "rows naming no Node impl in src/: {stale:?}");
}

/// **The feature seam, read back** — M54/T54.1 (§2.1). `just test` runs
/// this file with `--features a11y` alone as well as under the
/// workspace's `observe`: a toggle must report `CheckBox` either way. A
/// build whose widgets did not publish would read `GenericContainer`.
#[test]
fn a_toggle_reads_back_as_a_checkbox() {
    let mut ui = build();
    let nodes = snapshot(&mut ui);
    let n = &nodes["CheckBox-on"];
    println!(
        "CheckBox-on role={:?} toggled={:?} (core a11y {}, observe {})",
        n.role(),
        n.toggled(),
        lumen_static::A11Y,
        lumen_static::OBSERVE
    );
    assert_eq!(n.role(), Role::CheckBox, "a toggle reports {:?}", n.role());
    assert_eq!(n.toggled(), Some(Toggled::True));
}

#[test]
#[ignore = "red until T54.3: R15-F3 -- unnamed controls and GenericContainer kinds"]
fn every_widget_publishes_its_role_name_and_state() {
    let mut ui = build();
    let nodes = snapshot(&mut ui);
    let mut bad = Vec::new();
    let mut generic = Vec::new();
    for r in ROWS {
        let n = &nodes[r.wid];
        println!("{}", describe(r, n));
        if n.role() == Role::GenericContainer {
            generic.push(r.wid);
        }
        if n.role() != r.role {
            bad.push(format!("{}: role {:?}, want {:?}", r.wid, n.role(), r.role));
        }
        if r.named && n.label().is_none_or(str::is_empty) {
            bad.push(format!("{}: built with a name and reports none", r.wid));
        }
        if r.toggled.is_some() && n.toggled() != r.toggled {
            bad.push(format!("{}: toggled {:?}, want {:?}", r.wid, n.toggled(), r.toggled));
        }
    }
    println!("GenericContainer kinds: {} (bound {}) {generic:?}", generic.len(), GENERIC_BOUND.len());
    println!("rows checked: {} / Node impls: {}", ROWS.len(), node_impls().len());
    for g in &generic {
        if !GENERIC_BOUND.contains(g) {
            bad.push(format!("{g}: GenericContainer, and not in the explicit list"));
        }
    }
    assert!(bad.is_empty(), "{} row(s) wrong:\n  {}", bad.len(), bad.join("\n  "));
}
