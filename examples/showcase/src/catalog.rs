//! The catalogue: every widget the showcase can put on the stage.
//!
//! A `static` table of plain `fn` pointers, which is what lets the
//! sidebar's `Repeat` bind rows without capturing and what lets a hot
//! reload re-supply every entry.

use super::*;

/// One showcased widget.
pub struct Entry {
    /// Group heading, as it appears in the sidebar.
    pub group: &'static str,
    /// Display name.
    pub name: &'static str,
    /// One line on what the demo shows.
    pub blurb: &'static str,
    /// Whether it wants a per-frame tick.
    pub animated: bool,
    /// Put it on the stage, seeded with real data and wired to `Show`.
    pub seed: fn(&mut Show),
    /// A live readout of whatever state the demo carries.
    pub status: fn(&Show) -> String,
}

fn none(_: &Show) -> String {
    String::new()
}

macro_rules! entry {
    ($group:literal, $name:literal, $blurb:literal, $anim:expr, $seed:expr, $status:expr) => {
        Entry {
            group: $group,
            name: $name,
            blurb: $blurb,
            animated: $anim,
            seed: $seed,
            status: $status,
        }
    };
}

/// What one sidebar row is: a group's heading, or entry `k`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavItem {
    Heading(&'static str),
    Entry(usize),
}

/// Sidebar row `i`: a heading wherever the group changes, then the
/// entries under it. A walk over the static table — no allocation, so a
/// bind can call it every frame (M29/T29.5).
pub fn nav_item(i: usize) -> NavItem {
    let mut row = 0;
    for (k, e) in CATALOG.iter().enumerate() {
        if k == 0 || CATALOG[k - 1].group != e.group {
            if row == i {
                return NavItem::Heading(e.group);
            }
            row += 1;
        }
        if row == i {
            return NavItem::Entry(k);
        }
        row += 1;
    }
    NavItem::Entry(CATALOG.len() - 1)
}

/// How many sidebar rows there are: every entry, and one heading per group.
pub fn nav_len() -> usize {
    let groups = (0..CATALOG.len()).filter(|&k| k == 0 || CATALOG[k - 1].group != CATALOG[k].group).count();
    CATALOG.len() + groups
}

/// Put one demo on the stage.
///
/// **No `clear()` first.** An enum is one-of-N by construction, so
/// assigning the new variant *is* removing the old one — the whole
/// reason M24/T24.3 replaced eighteen `Option`s with a switch, and the
/// end of the one bug that shape made possible (a field left out of
/// `clear` left the previous demo underneath the new one).
fn on(s: &mut Show, demo: Demo) {
    s.ui.main.stage.row.demo = demo;
}

/// A veil's body: a title and a live control. One per veil kind,
/// because each is its own type — see `veil_body!` in `stage.rs`.
macro_rules! body_for {
    ($ty:ident, $title:expr) => {{
        let mut l = Label::<String>::default();
        l.set($title.into());
        $ty { title: l, go: btn("Confirm") }
    }};
}

/// Open a veil, because a shut one is absent and a demo of nothing is
/// not a demo.
fn veil<W: lumen_static::widget::Widget, const K: u8>(
    mut v: lumen_static_widgets::Veil<W, K>,
) -> lumen_static_widgets::Veil<W, K> {
    v.open();
    v
}

/// A small file tree, flattened — which is how `Tree` holds one.
const TREE_ROWS: [lumen_static_widgets::Branch; 7] = [
    lumen_static_widgets::Branch::new("crates", 0),
    lumen_static_widgets::Branch::new("lumen-static", 1),
    lumen_static_widgets::Branch::new("src", 2),
    lumen_static_widgets::Branch::new("tests", 2),
    lumen_static_widgets::Branch::new("lumen-static-widgets", 1),
    lumen_static_widgets::Branch::new("docs", 0),
    lumen_static_widgets::Branch::new("README.md", 0),
];

/// One grid row. A plain `fn`, so it cannot capture — which is the whole
/// reason a grid does not own its data.
fn grid_row(r: &mut GridRow, i: usize) {
    r.name.l.set(format!("file-{i:03}.rs"));
    r.size.l.set(format!("{} KB", 1 + i % 97));
}

/// What M27/T27.12's rich text says.
const RICH: &str = "A span here is a word, and a word never breaks inside \
itself; the line breaker only ever decides whether the next one fits on \
this line or opens another.";

/// And its markdown.
const DOC: &str = "# Markdown\n\nHeadings, **bold**, `code` and bullets \
are the subset a user interface actually renders.\n\n- a label\n- an \
identifier\n- a list";

/// The `PaintBox` demo's callback: a `fn`, which is the whole point.
fn sparkline(out: &mut dyn lumen_static::Painter, r: [f32; 4]) {
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let pts = [0.25f32, 0.55, 0.35, 0.8, 0.6, 0.95, 0.45, 0.7];
    let ink = lumen_static::Color(0x25, 0x63, 0xeb, 0xff);
    let step = w / (pts.len() - 1) as f32;
    for k in 1..pts.len() {
        out.line(
            lumen_static::Point { x: r[0] + step * (k - 1) as f32, y: r[3] - h * pts[k - 1] },
            lumen_static::Point { x: r[0] + step * k as f32, y: r[3] - h * pts[k] },
            2.0,
            ink,
        );
    }
    for (k, p) in pts.iter().enumerate() {
        let (x, y) = (r[0] + step * k as f32, r[3] - h * p);
        out.rounded_rect([x - 3.0, y - 3.0, x + 3.0, y + 3.0], ink, 3.0);
    }
}

/// What M27/T27.11's file picker lists.
const FILES: [&str; 7] = [
    "Cargo.toml",
    "AGENT.md",
    "justfile",
    "lib.rs",
    "main.rs",
    "style.rs",
    "widget.rs",
];

/// The options M27/T27.9's three demos share.
pub(crate) const FRUIT: [&str; 6] =
    ["Apple", "Banana", "Cherry", "Damson", "Elderberry", "apricot"];
const COMMANDS: [&str; 4] = ["Cut", "Copy", "Paste", "Select all"];

/// A `Menu` is seeded **open**, because it has no trigger and a closed
/// one shows nothing at all.
///
/// A `PickList` and a `Combobox` are seeded **closed**, which is how
/// anyone meets one — and is also the only state in which pressing the
/// trigger works: `dispatch_overlaid` dismisses every overlay on a press
/// that misses them all and *consumes* it, which is `Select`'s
/// documented dropdown behaviour and takes the press a combobox's field
/// needs. `crates/lumen-static-widgets/tests/focus_overlay.rs` pins both
/// halves.
fn opened<const K: u8>(mut c: lumen_static_widgets::Chooser<K>) -> lumen_static_widgets::Chooser<K> {
    c.open();
    c
}

/// The same for a picker: a demo that shows its cells.
fn shown<const K: u8>(mut p: lumen_static_widgets::Picker<K>) -> lumen_static_widgets::Picker<K> {
    p.open();
    p
}

/// The `Select` demo: controlled, so opening, dismissal and a choice all
/// write `Store`, and `refresh` puts them on the tree.
fn select_demo() -> lumen_static_widgets::select::Select<Store> {
    let mut d = lumen_static_widgets::select::Select::new(FRUIT.len(), 30.0, bind_fruit);
    d.trigger.on_click = |s: &mut Store, _: &EventCtx| s.select_open = true;
    d.on_dismiss = |s: &mut Store, _: &EventCtx| s.select_open = false;
    d
}

/// One option. **Writes the label only when it differs**: a bind is
/// steady-state code and runs every frame, so an unconditional
/// `String` write would allocate per row per frame (M29/T29.5).
fn bind_fruit(r: &mut lumen_static_widgets::select::Row<Store>, i: usize) {
    if r.label != FRUIT[i] {
        r.label.clear();
        r.label.push_str(FRUIT[i]);
    }
    r.on_click = |s: &mut Store, cx: &EventCtx| {
        s.pick = cx.row.map_or(0, |r| r as usize).min(FRUIT.len() - 1);
        s.select_open = false;
    };
}

fn chip(t: &str) -> Chip<String> {
    Chip::<String>::new(t.into())
}

fn gbadge(t: &str) -> Badge<String> {
    Badge::<String>::new(t.into())
}

fn lbl_of(t: &str) -> Label<String> {
    let mut l = Label::<String>::default();
    l.set(t.into());
    l
}

/// One accordion section, open on the first so the demo shows both
/// states at once.
fn section(title: &str, body: &str) -> Collapsible<Label<String>, String> {
    Collapsible::new(title.into(), lbl_of(body))
}

/// A `Button` wired to the press counter.
fn btn(label: &str) -> Button<Store, String> {
    let mut b = Button::<Store, String>::default();
    b.label = label.into();
    b.on_click = |s: &mut Store, _: &EventCtx| s.presses += 1;
    b
}

fn toggle<const K: u8>(on_: bool) -> Toggle<Store, K> {
    Toggle::<Store, K> {
        on: on_,
        on_toggle: |s: &mut Store, _: &EventCtx| s.flag = !s.flag,
        ..Default::default()
    }
}

/// Everything on the stage, in sidebar order: **each group contiguous and
/// named once** — `tests::each_group_appears_once` holds that, because the
/// sidebar prints a heading where the group changes (M56).
pub static CATALOG: &[Entry] = &[
    entry!("Text", "Label", "A line of text — the core's simplest widget.",
        false,
        |s| on(s, Demo::Label(DLabel { a: { let mut l = Label::<String>::default(); l.set("The quick brown fox".into()); l } })),
        none),
    entry!("Text", "Badge", "A count, a status and a tag.",
        false,
        |s| on(s, Demo::Badge(DBadge {
            a: Badge::new("3".into()), b: Badge::new("12".into()), c: Badge::new("99+".into()) })),
        none),
    entry!("Text", "Chip", "The same shape, in the quiet palette.",
        false,
        |s| on(s, Demo::Chip(DChip {
            a: Chip::new("rust".into()), b: Chip::new("systems".into()), c: Chip::new("no_std".into()) })),
        none),
    entry!("Text", "Avatar", "Initials, trimmed on a char boundary.",
        false,
        |s| on(s, Demo::Avatar(DAvatar {
            a: Avatar { initials: "ÅB".into() }, b: Avatar { initials: "JD".into() }, c: Avatar { initials: "K".into() } })),
        none),
    entry!("Text", "Icon", "All eight marks, drawn from primitives.",
        false,
        |s| on(s, Demo::Icon(DIcon {
            a: Icon::new(Symbol::Play), b: Icon::new(Symbol::Pause), c: Icon::new(Symbol::Stop),
            d: Icon::new(Symbol::Check), e: Icon::new(Symbol::Close), f: Icon::new(Symbol::Down),
            g: Icon::new(Symbol::Plus), h: Icon::new(Symbol::Minus) })),
        none),
    entry!("Text", "Divider", "A hairline; its thickness is its style's.",
        false,
        |s| on(s, Demo::Divider(DDivider { a: Divider })),
        none),
    entry!("Text", "RichText", "Words that wrap, and a span is a word.",
        false,
        |s| on(s, Demo::RichText(DRichText { a: RichText::new(RICH) })),
        none),
    entry!("Text", "Markdown", "Headings, bold, code, bullets.",
        false,
        |s| on(s, Demo::Markdown(DMarkdown { a: Markdown::new(DOC) })),
        none),
    entry!("Text", "RichTextEditor", "Type markup; see it rendered.",
        false,
        |s| on(s, Demo::RichTextEditor(DRichTextEditor {
            a: RichTextEditor::new("# Title\n\nSome **bold** and `code`.") })),
        none),
    entry!("Text", "PaintBox", "A box the application draws into.",
        false,
        |s| on(s, Demo::PaintBox(DPaintBox { a: PaintBox::new(sparkline) })),
        none),
    entry!("Input", "Button", "Three of them, all wired to one counter.",
        false,
        |s| on(s, Demo::Button(DButton { a: btn("Primary"), b: btn("Default"), c: btn("Ghost") })),
        |s| format!("presses: {}", s.store.presses)),
    entry!("Input", "CheckBox", "A box and a tick, both bound to one flag.",
        false,
        |s| on(s, Demo::CheckBox(DCheckBox { a: PairCheckA { t: toggle(s.store.flag), l: { let mut l = Label::<String>::default(); l.set("I agree".into()); l } }, b: PairCheckB { t: toggle(!s.store.flag), l: { let mut l = Label::<String>::default(); l.set("Send updates".into()); l } } })),
        |s| format!("checked: {}", s.store.flag)),
    entry!("Input", "Switch", "The same two states, as a sliding pill.",
        false,
        |s| on(s, Demo::Switch(DSwitch { a: PairSwitchA { t: toggle(s.store.flag), l: { let mut l = Label::<String>::default(); l.set("Wi-Fi".into()); l } }, b: PairSwitchB { t: toggle(!s.store.flag), l: { let mut l = Label::<String>::default(); l.set("Bluetooth".into()); l } } })),
        |s| format!("on: {}", s.store.flag)),
    entry!("Input", "Radio", "One of a set; the selection lives in the app.",
        false,
        |s| on(s, Demo::Radio(DRadio { a: PairRadioA { t: toggle(s.store.flag), l: { let mut l = Label::<String>::default(); l.set("Red".into()); l } }, b: PairRadioB { t: toggle(!s.store.flag), l: { let mut l = Label::<String>::default(); l.set("Green".into()); l } }, c: PairRadioC { t: toggle(false), l: { let mut l = Label::<String>::default(); l.set("Blue".into()); l } } })),
        |s| format!("selected: {}", s.store.flag)),
    entry!("Input", "Slider", "Drag it, or focus it and use the arrows.",
        false,
        |s| on(s, Demo::Slider(DSlider { a: Slider::new(s.store.fraction) })),
        |s| format!("value: {:.2}", s.store.fraction)),
    entry!("Input", "TextInput", "One line, editable. Click it and type.",
        false,
        |s| on(s, Demo::TextInput(DTextInput { a: { let mut t = TextInput::<String>::default(); t.set_str("type here"); t } })),
        none),
    entry!("Input", "ColorPicker", "Six hues, four lightnesses.",
        false,
        |s| on(s, Demo::ColorPicker(DColorPicker { a: ColorPicker::new() })),
        none),
    entry!("Input", "DatePicker", "A month; the arrows page it.",
        false,
        |s| on(s, Demo::DatePicker(DDatePicker { a: shown(DatePicker::new()) })),
        none),
    entry!("Input", "TimePicker", "Twenty-four hours over twelve marks.",
        false,
        |s| on(s, Demo::TimePicker(DTimePicker { a: TimePicker::new() })),
        none),
    entry!("Input", "FilePicker", "A list of names the application owns.",
        false,
        |s| on(s, Demo::FilePicker(DFilePicker {
            a: FilePicker::new().with_files(&FILES) })),
        none),
    entry!("Choice", "PickList", "Press it; pick one of six.",
        false,
        |s| on(s, Demo::PickList(DPickList { a: PickList::new(&FRUIT) })),
        none),
    entry!("Choice", "Select", "A dropdown the application opens, closes and fills.",
        false,
        |s| on(s, Demo::Select(DSelect { a: select_demo() })),
        |s| format!("selected: {}", FRUIT[s.store.pick])),
    entry!("Choice", "Combobox", "The same list, filtered as you type.",
        false,
        |s| on(s, Demo::Combobox(DCombobox { a: Combobox::new(&FRUIT) })),
        none),
    entry!("Choice", "Menu", "A command list with no trigger.",
        false,
        |s| on(s, Demo::Menu(DMenu { a: opened(Menu::new(&COMMANDS)) })),
        none),
    entry!("Fields", "NumberInput", "Arrows step it; letters are refused.",
        false,
        |s| on(s, Demo::Number(DNumberInput { a: NumberInput::new(s.store.fraction * 10.0, 0.0, 10.0, 0.5) })),
        |s| format!("value: {:.2}", s.store.fraction * 10.0)),
    entry!("Fields", "RangeSlider", "Two handles that cannot cross.",
        false,
        |s| on(s, Demo::Range(DRangeSlider { a: RangeSlider::new(0.25, 0.75) })),
        none),
    entry!("Fields", "SearchField", "A magnifier, then the text.",
        false,
        |s| on(s, Demo::Search(DSearchField { a: SearchField::new("search") })),
        none),
    entry!("Progress", "ProgressBar", "Determinate, driven by the tick.",
        true,
        |s| on(s, Demo::Progress(DProgressBar { a: ProgressBar::new(s.store.phase) })),
        |s| format!("{:.0}%", s.store.phase * 100.0)),
    entry!("Progress", "Spinner", "Three phases of the same ring.",
        true,
        |s| on(s, Demo::Spinner(DSpinner {
            a: Spinner::new(s.store.phase), b: Spinner::new(s.store.phase + 0.33), c: Spinner::new(s.store.phase + 0.66) })),
        |s| format!("phase {:.2}", s.store.phase)),
    entry!("Progress", "Skeleton", "A placeholder with a travelling highlight.",
        true,
        |s| on(s, Demo::Skeleton(DSkeleton { a: Skeleton::new(s.store.phase), b: Skeleton::new(s.store.phase) })),
        |s| format!("phase {:.2}", s.store.phase)),
    entry!("Charts", "BarChart", "Bars from a zero baseline.",
        false,
        |s| on(s, Demo::Bar(DBarChart { a: BarChart::new(SAMPLE) })),
        none),
    entry!("Charts", "LineChart", "A polyline through the same points.",
        false,
        |s| on(s, Demo::Line(DLineChart { a: LineChart::new(SAMPLE) })),
        none),
    entry!("Charts", "PieChart", "Proportions of a whole, as a ring.",
        false,
        |s| on(s, Demo::Pie(DPieChart { a: PieChart::new(&[5.0, 3.0, 2.0, 1.5, 1.0]) })),
        none),
    entry!("Surface", "Card", "A raised panel with a live control on it.",
        false,
        |s| on(s, Demo::Card(DCard { a: Card::new(CardBody {
            title: { let mut l = Label::<String>::default(); l.set("Storage".into()); l },
            go: btn("Manage"),
        }) })),
        |s| format!("presses: {}", s.store.presses)),
    entry!("Surface", "Toast", "A transient message, in two lengths.",
        false,
        |s| on(s, Demo::Toast(DToast {
            a: Toast::new("Saved".into()),
            b: Toast::new("Could not reach the server — retrying".into()),
        })),
        none),
    entry!("Surface", "Tooltip", "Hover the button; the tip follows.",
        false,
        |s| on(s, Demo::Tooltip(DTooltip {
            a: Tooltip::new(btn("Hover me"), "Shows while the pointer is over it".into()),
        })),
        |s| format!("presses: {}", s.store.presses)),
    entry!("Navigation", "Tabs", "Four pages; the current one is underlined.",
        false,
        |s| on(s, Demo::Tabs(DTabs { a: Tabs::new(&["Overview", "Activity", "Files", "Settings"]) })),
        none),
    entry!("Navigation", "BottomNav", "The same strip, filled instead.",
        false,
        |s| on(s, Demo::Bottom(DBottomNav { a: BottomNav::new(&["Home", "Search", "Saved", "Profile"]) })),
        none),
    entry!("Navigation", "NavigationRail", "The same strip again, down the side.",
        false,
        |s| on(s, Demo::Rail(DNavigationRail { a: NavigationRail::new(&["Home", "Search", "Saved", "Profile"]) })),
        none),
    entry!("Navigation", "AppBar", "A title and a raised ground.",
        false,
        |s| on(s, Demo::AppBar(DAppBar { a: AppBar::new("Inbox".into()) })),
        none),
    entry!("Navigation", "PullToRefresh", "Drag the content down past its top.",
        false,
        |s| on(s, Demo::Pull(DPullToRefresh { a: PullToRefresh::new({ let mut l = Label::<String>::default(); l.set("pull me down".into()); l }) })),
        none),
    entry!("Overlay", "Modal", "A centred panel over a dimmed window.",
        false,
        |s| on(s, Demo::Modal(DModal { a: veil(Modal::new(body_for!(ModalBody, "Delete this file?"))) })),
        |s| format!("dismissals: {}", s.store.presses)),
    entry!("Overlay", "Sheet", "The same panel, across the bottom.",
        false,
        |s| on(s, Demo::Sheet(DSheet { a: veil(Sheet::new(body_for!(SheetBody, "Share with"))) })),
        |s| format!("dismissals: {}", s.store.presses)),
    entry!("Overlay", "Drawer", "And down the side.",
        false,
        |s| on(s, Demo::Drawer(DDrawer { a: veil(Drawer::new(body_for!(DrawerBody, "Navigate"))) })),
        |s| format!("dismissals: {}", s.store.presses)),
    entry!("Layout", "Wrap", "Six chips that reflow onto a second line.",
        false,
        |s| on(s, Demo::Wrap(DWrap {
            a: chip("alpha"), b: chip("bravo"), c: chip("charlie"),
            d: chip("delta"), e: chip("echo"), f: chip("foxtrot") })),
        none),
    entry!("Layout", "Grid", "The same six, three to a row.",
        false,
        |s| on(s, Demo::Grid(DGrid {
            a: gbadge("1"), b: gbadge("2"), c: gbadge("3"),
            d: gbadge("4"), e: gbadge("5"), f: gbadge("6") })),
        none),
    entry!("Layout", "SplitPane", "Drag the divider between two panes.",
        false,
        |s| on(s, Demo::Split(DSplitPane { a: SplitPane::new(lbl_of("left"), lbl_of("right")) })),
        none),
    entry!("Layout", "Accordion", "Three sections; press a header.",
        false,
        |s| on(s, Demo::Accordion(DAccordion {
            a: section("Account", "Your name and email"),
            b: section("Privacy", "Who can see your activity"),
            c: section("Advanced", "Developer options") })),
        none),
    entry!("Layout", "PaneGrid", "Four panes, three dividers.",
        false,
        |s| on(s, Demo::PaneGrid(DPaneGrid { a: PaneGrid::new(
            // One word each: a `~text` selector splits its terms on
            // whitespace, so "north west" is two terms and the second
            // is read as a kind. The drive addresses these panes by
            // their text because a bare `Label` carries no id.
            lbl_of("northwest"), lbl_of("northeast"),
            lbl_of("southwest"), lbl_of("southeast")) })),
        none),
    entry!("Data", "Pagination", "Thirty pages; the window follows you.",
        false,
        |s| on(s, Demo::Pagination(DPagination { a: Pagination::new(30) })),
        none),
    entry!("Data", "Tree", "Press a chevron; the subtree folds away.",
        false,
        |s| on(s, Demo::Tree(DTree { a: Tree::new(&TREE_ROWS) })),
        none),
    entry!("Data", "DataGrid", "A header over virtualized rows.",
        false,
        |s| on(s, Demo::DataGrid(DDataGrid {
            a: DataGrid::new(&["Name", "Size"], 200, 24.0, grid_row) })),
        none),
];

/// The series the charts share, so the three demos are comparable.
pub static SAMPLE: &[f32] = &[3.0, 7.0, 4.0, 9.0, 6.0, 8.0, 5.0, 10.0, 7.5, 4.5];
