//! The catalogue: every widget the showcase can put on the stage.
//!
//! A `static` table of plain `fn` pointers, which is what lets the
//! dropdown's `Repeat` bind rows without capturing and what lets a hot
//! reload re-supply every entry.

use super::*;

/// One showcased widget.
pub struct Entry {
    /// Group heading, as it appears in the dropdown.
    pub group: &'static str,
    /// Display name.
    pub name: &'static str,
    /// One line on what the demo shows.
    pub blurb: &'static str,
    /// A subtlety worth a sentence, printed under the demo — Lumen's
    /// showcase does this and it is most of what makes it teach rather
    /// than merely list.
    pub note: &'static str,
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
    ($group:literal, $name:literal, $blurb:literal, $note:literal, $anim:expr, $seed:expr, $status:expr) => {
        Entry {
            group: $group,
            name: $name,
            blurb: $blurb,
            note: $note,
            animated: $anim,
            seed: $seed,
            status: $status,
        }
    };
}

/// Put one demo on the stage.
///
/// **No `clear()` first.** An enum is one-of-N by construction, so
/// assigning the new variant *is* removing the old one — the whole
/// reason M24/T24.3 replaced eighteen `Option`s with a switch, and the
/// end of the one bug that shape made possible (a field left out of
/// `clear` left the previous demo underneath the new one).
fn on(s: &mut Show, demo: Demo) {
    s.ui.body.stage.row.demo = demo;
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
    r.name.set(format!("file-{i:03}.rs"));
    r.size.set(format!("{} KB", 1 + i % 97));
}

/// The options M27/T27.9's three demos share.
const FRUIT: [&str; 6] =
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

fn chip(t: &str) -> Chip<String> {
    Chip::<String> { text: t.into() }
}

fn gbadge(t: &str) -> Badge<String> {
    Badge::<String> { text: t.into() }
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
fn btn(label: &str) -> Button<Show, String> {
    let mut b = Button::<Show, String>::default();
    b.label = label.into();
    b.on_click = |s: &mut Show, _: &EventCtx| s.presses += 1;
    b
}

fn toggle<const K: u8>(on_: bool) -> Toggle<Show, K> {
    Toggle::<Show, K> {
        on: on_,
        on_toggle: |s: &mut Show, _: &EventCtx| s.flag = !s.flag,
        ..Default::default()
    }
}

/// Everything on the stage, in dropdown order.
pub static CATALOG: &[Entry] = &[
    entry!("Text", "Label", "A line of text — the core's simplest widget.",
        "A `Label`'s text is a presentation copy: the application owns it and `#[bind]` writes it here.",
        false,
        |s| on(s, Demo::Label(DLabel { a: { let mut l = Label::<String>::default(); l.set("The quick brown fox".into()); l } })),
        none),
    entry!("Text", "Badge", "A count, a status and a tag.",
        "It shrinks to its text, so `IS_RIGID` is false — a changed count is a relayout boundary, not just a repaint.",
        false,
        |s| on(s, Demo::Badge(DBadge {
            a: Badge { text: "3".into() }, b: Badge { text: "12".into() }, c: Badge { text: "99+".into() } })),
        none),
    entry!("Text", "Chip", "The same shape, in the quiet palette.",
        "A separate type rather than a field, because `kind_name` feeds the cascade and the agent's selectors.",
        false,
        |s| on(s, Demo::Chip(DChip {
            a: Chip { text: "rust".into() }, b: Chip { text: "systems".into() }, c: Chip { text: "no_std".into() } })),
        none),
    entry!("Text", "Avatar", "Initials, trimmed on a char boundary.",
        "Two *chars*, not two bytes: \"ÅB\" sliced at byte two would panic, and an avatar is decoration.",
        false,
        |s| on(s, Demo::Avatar(DAvatar {
            a: Avatar { initials: "ÅB".into() }, b: Avatar { initials: "JD".into() }, c: Avatar { initials: "K".into() } })),
        none),
    entry!("Text", "Icon", "All eight marks, drawn from primitives.",
        "A closed enum, not a font codepoint — a glyph icon set fails as the missing-glyph box, which is how `examples/glasswall` lost twenty-two icons.",
        false,
        |s| on(s, Demo::Icon(DIcon {
            a: Icon::new(Symbol::Play), b: Icon::new(Symbol::Pause), c: Icon::new(Symbol::Stop),
            d: Icon::new(Symbol::Check), e: Icon::new(Symbol::Close), f: Icon::new(Symbol::Down),
            g: Icon::new(Symbol::Plus), h: Icon::new(Symbol::Minus) })),
        none),
    entry!("Text", "Divider", "A hairline; its thickness is its style's.",
        "Named `Divider` where Lumen says `Rule`, because `lumen_static::Rule` is a stylesheet rule and is in the prelude.",
        false,
        |s| on(s, Demo::Divider(DDivider { a: Divider })),
        none),
    entry!("Input", "Button", "Three of them, all wired to one counter.",
        "An `on_click` is a plain `fn` pointer: it cannot capture, which is what lets a hot reload re-supply it.",
        false,
        |s| on(s, Demo::Button(DButton { a: btn("Primary"), b: btn("Default"), c: btn("Ghost") })),
        |s| format!("presses: {}", s.presses)),
    entry!("Input", "CheckBox", "A box and a tick, both bound to one flag.",
        "A toggle does not flip itself: the app owns the boolean and the binding writes this copy back, or the two could disagree.",
        false,
        |s| on(s, Demo::CheckBox(DCheckBox { a: PairCheckA { t: toggle(s.flag), l: { let mut l = Label::<String>::default(); l.set("I agree".into()); l } }, b: PairCheckB { t: toggle(!s.flag), l: { let mut l = Label::<String>::default(); l.set("Send updates".into()); l } } })),
        |s| format!("checked: {}", s.flag)),
    entry!("Input", "Switch", "The same two states, as a sliding pill.",
        "One type with three presentations, chosen by a *const* parameter — so `kind_name` and `defaults` answer at compile time.",
        false,
        |s| on(s, Demo::Switch(DSwitch { a: PairSwitchA { t: toggle(s.flag), l: { let mut l = Label::<String>::default(); l.set("Wi-Fi".into()); l } }, b: PairSwitchB { t: toggle(!s.flag), l: { let mut l = Label::<String>::default(); l.set("Bluetooth".into()); l } } })),
        |s| format!("on: {}", s.flag)),
    entry!("Input", "Radio", "One of a set; the selection lives in the app.",
        "There is no group container: each radio compares against a shared value, which is the honest cost of not inventing one.",
        false,
        |s| on(s, Demo::Radio(DRadio { a: PairRadioA { t: toggle(s.flag), l: { let mut l = Label::<String>::default(); l.set("Red".into()); l } }, b: PairRadioB { t: toggle(!s.flag), l: { let mut l = Label::<String>::default(); l.set("Green".into()); l } }, c: PairRadioC { t: toggle(false), l: { let mut l = Label::<String>::default(); l.set("Blue".into()); l } } })),
        |s| format!("selected: {}", s.flag)),
    entry!("Input", "Slider", "Drag it, or focus it and use the arrows.",
        "A slider only draggable by mouse is not an accessible slider, so `step` is a field.",
        false,
        |s| on(s, Demo::Slider(DSlider { a: Slider::new(s.fraction) })),
        |s| format!("value: {:.2}", s.fraction)),
    entry!("Input", "TextInput", "One line, editable. Click it and type.",
        "The one place a widget owning state is right: the data originates *at* the widget, because the user typed it.",
        false,
        |s| on(s, Demo::TextInput(DTextInput { a: { let mut t = TextInput::<String>::default(); t.set_str("type here"); t } })),
        none),
    entry!("Progress", "ProgressBar", "Determinate, driven by the tick.",
        "An empty bar paints nothing at all: a zero-width rounded rect still covers `2 × radius` px once the radius is clamped.",
        true,
        |s| on(s, Demo::Progress(DProgressBar { a: ProgressBar::new(s.phase) })),
        |s| format!("{:.0}%", s.phase * 100.0)),
    entry!("Progress", "Spinner", "Three phases of the same ring.",
        "It does not drive its own clock — there is no per-node tick, so the application advances the phase.",
        true,
        |s| on(s, Demo::Spinner(DSpinner {
            a: Spinner::new(s.phase), b: Spinner::new(s.phase + 0.33), c: Spinner::new(s.phase + 0.66) })),
        |s| format!("phase {:.2}", s.phase)),
    entry!("Progress", "Skeleton", "A placeholder with a travelling highlight.",
        "The shimmer is clipped to the block, because the renderer's clip is a hard test and the arithmetic would have to repeat the rounded corner.",
        true,
        |s| on(s, Demo::Skeleton(DSkeleton { a: Skeleton::new(s.phase), b: Skeleton::new(s.phase) })),
        |s| format!("phase {:.2}", s.phase)),
    entry!("Charts", "BarChart", "Bars from a zero baseline.",
        "`Series` is a fixed array, so a chart allocates nothing and is `Copy`; over-long input is truncated rather than refused.",
        false,
        |s| on(s, Demo::Bar(DBarChart { a: BarChart::new(SAMPLE) })),
        none),
    entry!("Charts", "LineChart", "A polyline through the same points.",
        "A non-finite sample costs one segment, not the whole scale — a chart is a view of numbers it did not produce.",
        false,
        |s| on(s, Demo::Line(DLineChart { a: LineChart::new(SAMPLE) })),
        none),
    entry!("Charts", "PieChart", "Proportions of a whole, as a ring.",
        "Wedges are runs of radial lines: the renderer has no arc and no polygon, and 96 segments hides the joins at any size.",
        false,
        |s| on(s, Demo::Pie(DPieChart { a: PieChart::new(&[5.0, 3.0, 2.0, 1.5, 1.0]) })),
        none),
    // ---- M27/T27.4 ------------------------------------------------
    entry!("Surface", "Card", "A raised panel with a live control on it.",
        "A wrapper that does not route `Handlers<S>` leaves every control inside it hit-testable, hoverable, pressable and wired to nothing — which is what `Dyn` shipped as.",
        false,
        |s| on(s, Demo::Card(DCard { a: Card::new(CardBody {
            title: { let mut l = Label::<String>::default(); l.set("Storage".into()); l },
            go: btn("Manage"),
        }) })),
        |s| format!("presses: {}", s.presses)),
    entry!("Surface", "Toast", "A transient message, in two lengths.",
        "It does not dismiss itself: there is no per-node tick here, so the application owns the lifetime — the same reason `Spinner` does not drive its own phase.",
        false,
        |s| on(s, Demo::Toast(DToast {
            a: Toast::new("Saved".into()),
            b: Toast::new("Could not reach the server — retrying".into()),
        })),
        none),
    entry!("Surface", "Tooltip", "Hover the button; the tip follows.",
        "The tip keys on the *content's* hover, not the wrapper's: `dispatch` marks the deepest node it hits, so a wrapper never sees one.",
        false,
        |s| on(s, Demo::Tooltip(DTooltip {
            a: Tooltip::new(btn("Hover me"), "Shows while the pointer is over it".into()),
        })),
        |s| format!("presses: {}", s.presses)),
    // ---- M27/T27.5 ------------------------------------------------
    entry!("Fields", "NumberInput", "Arrows step it; letters are refused.",
        "Not a Stepper: no +/- buttons. Blur commits, because focus is *state* — there is no blur event, and `layout` is the one method that can see it change.",
        false,
        |s| on(s, Demo::Number(DNumberInput { a: NumberInput::new(s.fraction * 10.0, 0.0, 10.0, 0.5) })),
        |s| format!("value: {:.2}", s.fraction * 10.0)),
    entry!("Fields", "RangeSlider", "Two handles that cannot cross.",
        "A pair of sliders is not a range: nothing stops the low one passing the high one, and the moment it does the span between them is inverted.",
        false,
        |s| on(s, Demo::Range(DRangeSlider { a: RangeSlider::new(0.25, 0.75) })),
        none),
    entry!("Fields", "SearchField", "A magnifier, then the text.",
        "A click in the padding focuses the *field*, not the container — falling through would focus nothing and the control would feel dead at its edges.",
        false,
        |s| on(s, Demo::Search(DSearchField { a: SearchField::new("search") })),
        none),
    // ---- M27/T27.6 ------------------------------------------------
    entry!("Navigation", "Tabs", "Four pages; the current one is underlined.",
        "An underline, not a fill: a filled tab reads as a button, and the underline says \"this is the page you are on\" without competing with the content.",
        false,
        |s| on(s, Demo::Tabs(DTabs { a: Tabs::new(&["Overview", "Activity", "Files", "Settings"]) })),
        none),
    entry!("Navigation", "BottomNav", "The same strip, filled instead.",
        "One type, three presentations, chosen by a *const* parameter — so `kind_name` and `defaults` answer at compile time. `Chart` is built the same way.",
        false,
        |s| on(s, Demo::Bottom(DBottomNav { a: BottomNav::new(&["Home", "Search", "Saved", "Profile"]) })),
        none),
    entry!("Navigation", "NavigationRail", "The same strip again, down the side.",
        "A rail reads a press down `local.y` where the others read `local.x` — the one thing a shared implementation can get wrong and the only test that can tell.",
        false,
        |s| on(s, Demo::Rail(DNavigationRail { a: NavigationRail::new(&["Home", "Search", "Saved", "Profile"]) })),
        none),
    entry!("Navigation", "AppBar", "A title and a raised ground.",
        "Not a selector, so not a strip: nothing in it is selected. Its height is its style's, never its title's, or a title change relays out the whole screen below it.",
        false,
        |s| on(s, Demo::AppBar(DAppBar { a: AppBar::new("Inbox".into()) })),
        none),
    entry!("Navigation", "PullToRefresh", "Drag the content down past its top.",
        "It does not refresh anything: `take_triggered` hands the gesture over once, and taking rather than reading is what stops a double refresh.",
        false,
        |s| on(s, Demo::Pull(DPullToRefresh { a: PullToRefresh::new({ let mut l = Label::<String>::default(); l.set("pull me down".into()); l }) })),
        none),
    // ---- M27/T27.7 ------------------------------------------------
    entry!("Overlay", "Modal", "A centred panel over a dimmed window.",
        "A shut veil is *absent*, not merely unpainted — one that only skipped `paint` would still hit-test at full size and swallow every click in the window.",
        false,
        |s| on(s, Demo::Modal(DModal { a: veil(Modal::new(body_for!(ModalBody, "Delete this file?"))) })),
        |s| format!("dismissals: {}", s.presses)),
    entry!("Overlay", "Sheet", "The same panel, across the bottom.",
        "Its panel is measured against what its kind will give it, not against the window: a sheet measured full-height would size its content to a box it never gets.",
        false,
        |s| on(s, Demo::Sheet(DSheet { a: veil(Sheet::new(body_for!(SheetBody, "Share with"))) })),
        |s| format!("dismissals: {}", s.presses)),
    entry!("Overlay", "Drawer", "And down the side.",
        "Dismissal is a *request*: pressing the scrim asks, and the application decides — one that must confirm before discarding cannot undo a close that already happened.",
        false,
        |s| on(s, Demo::Drawer(DDrawer { a: veil(Drawer::new(body_for!(DrawerBody, "Navigate"))) })),
        |s| format!("dismissals: {}", s.presses)),
    // ---- M27/T27.8 ------------------------------------------------
    entry!("Layout", "Wrap", "Six chips that reflow onto a second line.",
        "A derive axis, not a widget: a container's children are its *fields* here, so a wrapping row is a way of arranging fields rather than something that holds them.",
        false,
        |s| on(s, Demo::Wrap(DWrap {
            a: chip("alpha"), b: chip("bravo"), c: chip("charlie"),
            d: chip("delta"), e: chip("echo"), f: chip("foxtrot") })),
        none),
    entry!("Layout", "Grid", "The same six, three to a row.",
        "`wrap` and `grid` are one lowering: a grid is a wrap whose line break is a *count* instead of a width, and two lowerings would be two places for the cross-axis arithmetic to disagree.",
        false,
        |s| on(s, Demo::Grid(DGrid {
            a: gbadge("1"), b: gbadge("2"), c: gbadge("3"),
            d: gbadge("4"), e: gbadge("5"), f: gbadge("6") })),
        none),
    entry!("Layout", "SplitPane", "Drag the divider between two panes.",
        "The divider stops short of both edges: a pane with no area cannot be clicked to get it back, so the control would be one-way.",
        false,
        |s| on(s, Demo::Split(DSplitPane { a: SplitPane::new(lbl_of("left"), lbl_of("right")) })),
        none),
    entry!("Layout", "Accordion", "Three sections; press a header.",
        "Not a widget — a column of `Collapsible`s. A press in the *body* must not collapse the section, or clicking what is inside it would close it.",
        false,
        |s| on(s, Demo::Accordion(DAccordion {
            a: section("Account", "Your name and email"),
            b: section("Privacy", "Who can see your activity"),
            c: section("Advanced", "Developer options") })),
        none),
    // ---- M27/T27.9 ------------------------------------------------
    entry!("Choice", "PickList", "Press it; pick one of six.",
        "It owns its options, which is what `Select` cannot: `Repeat::bind` is a plain `fn` and cannot capture the widget holding them — so these draw their rows the way `Strip` does.",
        false,
        |s| on(s, Demo::PickList(DPickList { a: PickList::new(&FRUIT) })),
        none),
    entry!("Choice", "Combobox", "The same list, filtered as you type.",
        "An empty query shows everything: a filter that hid every row when the field was cleared would make the control unusable the moment anyone deleted what they typed.",
        false,
        |s| on(s, Demo::Combobox(DCombobox { a: Combobox::new(&FRUIT) })),
        none),
    entry!("Choice", "Menu", "A command list with no trigger.",
        "A release chooses, a press does not — every menu in every toolkit commits on release, and choosing on the press also closes the list under a finger that is still down.",
        false,
        |s| on(s, Demo::Menu(DMenu { a: opened(Menu::new(&COMMANDS)) })),
        none),
    // ---- M27/T27.10 -----------------------------------------------
    entry!("Data", "Pagination", "Thirty pages; the window follows you.",
        "Centred, not paged in blocks: a block-paged strip jumps under the finger — you press 5 and the numbers shift so 5 is somewhere else.",
        false,
        |s| on(s, Demo::Pagination(DPagination { a: Pagination::new(30) })),
        none),
    entry!("Data", "Tree", "Press a chevron; the subtree folds away.",
        "Collapsing hides a whole subtree, not one row — and a *leaf* does not toggle, which is the difference between a chevron that does nothing and one that is not drawn.",
        false,
        |s| on(s, Demo::Tree(DTree { a: Tree::new(&TREE_ROWS) })),
        none),
    entry!("Data", "DataGrid", "A header over virtualized rows.",
        "It owns no data: its rows are a `Repeat` the application binds, because a grid is a view of a table someone else holds and copying it in would be a second copy to keep in step.",
        false,
        |s| on(s, Demo::DataGrid(DDataGrid {
            a: DataGrid::new(&["Name", "Size"], 200, 24.0, grid_row) })),
        none),
];

/// The series the charts share, so the three demos are comparable.
pub static SAMPLE: &[f32] = &[3.0, 7.0, 4.0, 9.0, 6.0, 8.0, 5.0, 10.0, 7.5, 4.5];
