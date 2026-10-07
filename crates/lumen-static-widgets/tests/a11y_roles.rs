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
// ANCHOR: rows
const ROWS: &[Row] = &[
    row("Label", "Label", Role::Label, false),
    // M80/T80.4: a label a reader can select; it announces as one.
    row("Selectable", "Selectable", Role::Label, false),
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
    // M84/T84.8: a video, which toggles and seeks for a reader, and its
    // track, a slider over the played fraction.
    row("Video", "Video", Role::Video, true),
    row("video-track", "SeekTrack", Role::Slider, true),
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
    // A menu runs a command and holds no value (M57/B23).
    row("Menu", "Chooser", Role::Menu, true),
    row("ProgressBar", "ProgressBar", Role::ProgressIndicator, false),
    row("Spinner", "Spinner", Role::ProgressIndicator, false),
    row("Skeleton", "Skeleton", Role::GenericContainer, false),
    row("BarChart", "Chart", Role::Figure, false),
    row("LineChart", "Chart", Role::Figure, false),
    row("PieChart", "Chart", Role::Figure, false),
    // A grid of cells: swatches, days, clock marks, files (M57/A17).
    row("Pad", "Pad", Role::Grid, false),
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
    // M85/T85.2: a box that offsets its child; the child speaks.
    row("Pin", "Pin", Role::GenericContainer, false),
    // M85/T85.4: a box told when it enters the view; the child speaks.
    row("Pop", "Pop", Role::GenericContainer, false),
    // M90/T90.4: a wrapper that claims a context menu; its content
    // speaks, and it publishes `ShowContextMenu`.
    row("ContextMenu", "ContextMenu", Role::GenericContainer, false),
    // M90/T90.4: a context menu's rows, its labels as the value.
    row("CommandList", "CommandList", Role::Menu, false),
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
    // M87/T87.3: the same impl with links that activate; each link is a
    // `Link` child (`a_linked_markdown_publishes_its_links`).
    row("LinkedMarkdown", "Markdown", Role::Paragraph, false),
    row("RichTextEditor", "RichTextEditor", Role::TextInput, false),
    row("PaintBox", "PaintBox", Role::Canvas, false),
    // M82/T82.8: text at an angle is still text a reader hears.
    row("VectorText", "VectorText", Role::Label, false),
    row("Divider", "Divider", Role::GenericContainer, false),
    row("Badge", "Badge", Role::Label, false),
    row("Chip", "Chip", Role::Label, false),
    row("Labelled", "Labelled", Role::Label, false),
    row("Avatar", "Avatar", Role::Image, false),
    row("Icon", "Icon", Role::Image, false),
    // M83/T83.6: a picture a shader draws, named by its author. Only in a
    // build with the widgets' `shader`; see FEATURE_GATED.
    #[cfg(feature = "shader")]
    row("ShaderView", "ShaderView", Role::Image, true),
];
// ANCHOR_END: rows

/// **`impl Node for` names that exist only under a feature** — M83/T83.6
/// (`docs/plan-gpu-extension.md`, second review M4). [`node_impls`] reads
/// source text and cannot see a `cfg`, so a gated widget's impl is counted
/// in every build while its row exists only in the build that has it. A
/// name here is excused from `missing` while its feature is off, and must
/// have a row when it is on: `just test` runs this file with `a11y` alone
/// and with `a11y,shader`, so each direction is checked.
const FEATURE_GATED: &[(&str, &str)] = &[("ShaderView", "shader")];

/// Whether this build has `feature` — every name [`FEATURE_GATED`] uses,
/// and nothing else, so a misspelt feature is a failure rather than an
/// excuse that never expires.
fn feature_on(feature: &str) -> bool {
    match feature {
        "shader" => cfg!(feature = "shader"),
        other => panic!("FEATURE_GATED names a feature this test does not know: {other:?}"),
    }
}

/// The kinds that report `GenericContainer` on purpose: a box with a
/// fixed size, a gap, a picker's cell pad (its trigger carries the
/// combo box), two wrappers that add paint and no meaning, a loading
/// placeholder, a gesture wrapper around content that has its own, and a
/// divider, which is decoration (M48/T48.6 decided it and
/// `lumen-static-agent/tests/tree_fields.rs` allows it by name), a
/// pin, which only offsets its child (M85/T85.2), and a pop, which only
/// hears when its child enters the view (M85/T85.4), and a context
/// menu's wrapper, whose content speaks (M90/T90.4).
const GENERIC_BOUND: &[&str] = &[
    "Fixed", "Spacer", "Popup", "Glass", "Skeleton", "Pad", "PullToRefresh", "Divider", "Pin",
    "Pop", "ContextMenu",
];

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
    // A gated impl whose feature is off has no row in this build (M83/T83.6).
    let excused: Vec<&str> =
        FEATURE_GATED.iter().filter(|(_, f)| !feature_on(f)).map(|(n, _)| *n).collect();
    println!("feature-gated impls excused in this build: {excused:?} (of {:?})", FEATURE_GATED);
    let missing: Vec<_> = impls.difference(&covered).filter(|n| !excused.contains(&n.as_str())).collect();
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

/// **The name check reads the node** — M54/T54.3's accept direction. A
/// control built with an empty label is reported unnamed, not passed:
/// the check is on what was emitted, not on the row's `named` flag.
#[test]
fn a_control_built_without_a_name_is_reported_unnamed() {
    let mut ui = build();
    ui.slider.set_label("");
    let nodes = snapshot(&mut ui);
    let n = &nodes["Slider"];
    println!("Slider built with label \"\": role={:?} label={:?}", n.role(), n.label());
    assert_eq!(n.role(), Role::Slider, "setup: the slider is still a slider");
    assert!(n.label().is_none_or(str::is_empty), "an empty label was published as a name");
}

/// The framework's action for each one a node publishes.
fn core_action(a: lumen_static::observe::accesskit::Action) -> Option<lumen_static::Action> {
    use lumen_static::observe::accesskit::Action as Ak;
    use lumen_static::Action;
    Some(match a {
        Ak::Click => Action::Default,
        Ak::Increment => Action::Increment,
        Ak::Decrement => Action::Decrement,
        // A value no row starts at, inside every row's range.
        Ak::SetValue => Action::SetValue(0.5),
        Ak::Expand => Action::Expand,
        Ak::Collapse => Action::Collapse,
        Ak::Focus => Action::Focus,
        // M90/T90.4: a context menu's claim is checked like the rest.
        Ak::ShowContextMenu => Action::ContextMenu,
        _ => return None,
    })
}

/// **Every action a node publishes is handled** — M54/T54.4 (§0 rule 3).
///
/// A node that announces `Increment` and ignores it is worse than one that
/// announces nothing: the screen reader says the control can be adjusted,
/// and it cannot. Each published action is dispatched as
/// `Event::Action` on a fresh catalogue, from the row's own node, and must
/// come back handled. Prints `published: p, handled: h` per row that
/// publishes any, against the bound `h == p`.
#[test]
fn every_published_action_is_handled() {
    use lumen_static::observe::accesskit::Action as Ak;
    const ALL: [Ak; 8] =
        [Ak::Click, Ak::Increment, Ak::Decrement, Ak::SetValue, Ak::Expand, Ak::Collapse, Ak::Focus, Ak::ShowContextMenu];
    let table = <Ui as Widget>::SUB_TABLE;
    let (mut total_p, mut total_h) = (0, 0);
    let mut bad = Vec::new();
    for r in ROWS {
        let nodes = snapshot(&mut build());
        let published: Vec<Ak> = ALL.iter().copied().filter(|a| nodes[r.wid].supports_action(*a)).collect();
        if published.is_empty() {
            continue;
        }
        let i = lumen_static::style::index_of_wid(table, r.wid).unwrap();
        let mut handled = Vec::new();
        for a in &published {
            let Some(action) = core_action(*a) else { continue };
            let mut app = catalogue::CatApp { ui: build() };
            let styles = pass::default_styles(&app.ui);
            let mut st = vec![lumen_static::NodeState::default(); <Ui as Widget>::NODE_COUNT];
            pass::layout(&mut app.ui, &styles, &mut st, Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 20000.0 });
            let mut input = lumen_static::event::InputState::default();
            let ev = lumen_static::Event::Action { node: i as u32, action };
            if pass::dispatch(&mut app, &styles, &mut st, &mut input, &ev) {
                handled.push(*a);
            }
        }
        println!("{:<15} published: {} {:?}, handled: {}", r.wid, published.len(), published, handled.len());
        total_p += published.len();
        total_h += handled.len();
        if handled.len() != published.len() {
            let missed: Vec<_> = published.iter().filter(|a| !handled.contains(a)).collect();
            bad.push(format!("{}: handled {} / published {} -- not handled: {missed:?}", r.wid, handled.len(), published.len()));
        }
    }
    println!("all rows: published: {total_p}, handled: {total_h} (bound: equal)");
    assert!(total_p >= 20, "setup: only {total_p} actions published across the catalogue");
    assert!(bad.is_empty(), "{} row(s) publish what they do not handle:\n  {}", bad.len(), bad.join("\n  "));
}

/// **A `LinkedMarkdown` publishes each link as a `Link` child** — M87/T87.3:
/// the link's text as its label, the bounds of its first word (inside the
/// paragraph's), a `Click` action, and the instance id `instance_id(id, n,
/// 0)` — which is what lets the agent's `~=` address a link at all, since
/// the paragraph itself is one value. A plain `Markdown` publishes none.
/// Injection (recorded in the commit): push no link children — 0 links.
#[test]
fn a_linked_markdown_publishes_its_links() {
    let mut ui = build();
    let styles = pass::default_styles(&ui);
    let mut st = vec![lumen_static::NodeState::default(); <Ui as Widget>::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 20000.0 });
    let table = <Ui as Widget>::SUB_TABLE;
    let id = |i: usize| (i + 1) as u64;
    let mut out: Vec<(NodeId, AkNode)> = Vec::new();
    emit_root(table, &ui, &st, &id, &mut out);
    let by_id: BTreeMap<u64, AkNode> = out.into_iter().map(|(i, n)| (i.0, n)).collect();
    let i = lumen_static::style::index_of_wid(table, "LinkedMarkdown").expect("#LinkedMarkdown");
    let para = &by_id[&(i as u64 + 1)];
    let kids: Vec<u64> = para.children().iter().map(|c| c.0).collect();
    let links: Vec<&AkNode> = kids.iter().filter_map(|k| by_id.get(k)).filter(|n| n.role() == Role::Link).collect();
    let plain = lumen_static::style::index_of_wid(table, "Markdown").expect("#Markdown");
    let plain_links = by_id[&(plain as u64 + 1)].children().len();
    println!(
        "LinkedMarkdown: {} Link child(ren) (bound 1), label {:?} (bound \"docs\"), ids {kids:?} (bound [{}]); plain Markdown children {plain_links} (bound 0)",
        links.len(),
        links.first().and_then(|n| n.label()),
        lumen_static::observe::instance_id(i as u64 + 1, 0, 0)
    );
    assert_eq!(links.len(), 1, "the link was not published as a Link child");
    let l = links[0];
    assert_eq!(l.label(), Some("docs"));
    assert_eq!(kids, [lumen_static::observe::instance_id(i as u64 + 1, 0, 0)], "the link's id is not instance_id(id, 0, 0)");
    assert!(l.supports_action(lumen_static::observe::accesskit::Action::Click), "a link offers no Click");
    let (b, pb) = (l.bounds().expect("bounds"), para.bounds().expect("paragraph bounds"));
    println!("link bounds {b:?} inside the paragraph's {pb:?}");
    assert!(b.x1 > b.x0 && b.x0 >= pb.x0 && b.x1 <= pb.x1 && b.y0 >= pb.y0 && b.y1 <= pb.y1, "the link's bounds are not inside its paragraph");
    assert_eq!(plain_links, 0, "a plain Markdown published link children");
}

#[derive(lumen_static::StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Illustrated {
    #[widget_id("doc")]
    doc: lumen_static_widgets::Markdown,
}

/// **A `Markdown` image is an `Image` child** — M87/T87.4: its alt text the
/// label, its placed box the bounds, the instance id `instance_id(id, n,
/// 1)` (row `n`, template node 1, so it never collides with link `n`'s
/// `instance_id(id, n, 0)`). The paragraph's value leaves the image span
/// out, so the alt is said once; an unresolved image publishes no child,
/// because its alt is drawn as words and the value holds them. Injection
/// (recorded in the commit): no image children pushed — 0 images.
#[test]
fn a_markdown_publishes_its_images() {
    use lumen_static_widgets::ImageRef;
    let mut ui = Illustrated {
        doc: lumen_static_widgets::Markdown::new("A chart:\n\n![sales by month](chart)\n\n![missing picture](nowhere)"),
    };
    ui.doc.set_images(&[ImageRef {
        name: "chart",
        id: lumen_static::display::ImageId(3),
        size: lumen_static::Size { w: 120.0, h: 80.0 },
    }]);
    let styles = pass::default_styles(&ui);
    let mut st = vec![lumen_static::NodeState::default(); <Illustrated as Widget>::NODE_COUNT];
    pass::layout(&mut ui, &styles, &mut st, Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 2000.0 });
    let table = <Illustrated as Widget>::SUB_TABLE;
    let mut out: Vec<(NodeId, AkNode)> = Vec::new();
    emit_root(table, &ui, &st, &|i: usize| (i + 1) as u64, &mut out);
    let by_id: BTreeMap<u64, AkNode> = out.into_iter().map(|(i, n)| (i.0, n)).collect();
    let i = lumen_static::style::index_of_wid(table, "doc").expect("#doc");
    let para = &by_id[&(i as u64 + 1)];
    let kids: Vec<u64> = para.children().iter().map(|c| c.0).collect();
    let images: Vec<&AkNode> = kids.iter().filter_map(|k| by_id.get(k)).filter(|n| n.role() == Role::Image).collect();
    let value = para.value().unwrap_or("");
    println!(
        "Markdown: {} Image child(ren) (bound 1), label {:?} (bound \"sales by month\"), ids {kids:?} (bound [{}]); value {value:?}",
        images.len(),
        images.first().and_then(|n| n.label()),
        lumen_static::observe::instance_id(i as u64 + 1, 0, 1)
    );
    assert_eq!(images.len(), 1, "the resolved image was not published as one Image child");
    assert_eq!(images[0].label(), Some("sales by month"), "the image's label is not its alt text");
    assert_eq!(kids, [lumen_static::observe::instance_id(i as u64 + 1, 0, 1)], "the image's id is not instance_id(id, 0, 1)");
    let b = images[0].bounds().expect("bounds");
    let s = st[i];
    let r = ui.doc.image_rect(0).expect("image rect");
    println!("image bounds {b:?}, placed {:?}", [s.pos.x + r[0], s.pos.y + r[1], s.pos.x + r[2], s.pos.y + r[3]]);
    assert!(((b.x1 - b.x0) - 120.0).abs() < 0.01 && ((b.y1 - b.y0) - 80.0).abs() < 0.01, "the bounds are not the image's box");
    assert!(!value.contains("sales") && value.contains("missing picture"), "the value says the drawn image's alt, or drops the unresolved one's words: {value:?}");
}
