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
    r.kind.l.set(["Source", "Test", "Bench", "Example"][i % 4].to_string());
    r.size.l.set(format!("{} KB", 1 + i % 97));
    r.lines.l.set(format!("{}", 40 + (i * 37) % 900));
}

/// What M27/T27.12's rich text says.
const RICH: &str = "A span here is a word, and a word never breaks inside \
itself; the line breaker only ever decides whether the next one fits on \
this line or opens another.";

/// And its markdown.
const DOC: &str = "# Markdown\n\nHeadings, **bold**, `code` and bullets \
are the subset a user interface actually renders.\n\n- a label\n- an \
identifier\n- a list";

/// **The showcase's shader table** -- M83/T83.6, behind its `shader`
/// feature. One entry: a vertical blend between two colours packed one per
/// `u32` (R, G, B, A in byte order, which `unpack4x8unorm` reads), the
/// 16-byte block below. Registered by `program()` through
/// `Program::shaders`; the id a `ShaderView` names is its index here.
#[cfg(feature = "shader")]
pub static SHADERS: [lumen_static_shell::shader::ShaderDesc; 1] = [lumen_static_shell::shader::ShaderDesc::new(
    "blend",
    "struct U { top: u32, bottom: u32, _pad: vec2<u32> }
     @group(0) @binding(1) var<uniform> u: U;
     @fragment fn fs_main(in: LumenVarying) -> @location(0) vec4<f32> {
         return mix(unpack4x8unorm(u.top), unpack4x8unorm(u.bottom), clamp(in.local.y, 0.0, 1.0));
     }",
    16,
)];

/// The `ShaderView` demo: `SHADERS[0]` from the stage's accent to its
/// surface, 320 x 120. Its fallback is the same blend as a linear gradient,
/// so the software backend shows what the GPU would.
#[cfg(feature = "shader")]
fn shader_view() -> lumen_static_widgets::ShaderView<16> {
    let (top, bottom) = (Color(0x25, 0x63, 0xeb, 0xff), Color(0x93, 0xc5, 0xfd, 0xff));
    let mut v = lumen_static_widgets::ShaderView::new(lumen_static::display::ShaderId(0), blend).sized(320.0, 120.0);
    v.set_uniforms(&[top.0, top.1, top.2, top.3, bottom.0, bottom.1, bottom.2, bottom.3, 0, 0, 0, 0, 0, 0, 0, 0]);
    v.set_label("A blue blend");
    v
}

/// The demo's fallback: `SHADERS[0]`'s blend as a vertical linear gradient
/// from the block's two packed colours.
#[cfg(feature = "shader")]
fn blend(out: &mut dyn lumen_static::Painter, r: [f32; 4], u: &[u8]) {
    if let [r0, g0, b0, a0, r1, g1, b1, a1, ..] = *u {
        out.gradient(
            r,
            lumen_static::display::Stops::two(Color(r0, g0, b0, a0), Color(r1, g1, b1, a1)),
            lumen_static::display::GradShape::Linear { angle: core::f32::consts::FRAC_PI_2 },
            0.0,
        );
    }
}

/// The `PaintBox` demo's callback: a `fn`, which is the whole point. A
/// sparkline of lines and dots, then (M82/T82.5) a star and an arc drawn
/// as paths over it.
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
    paths(out, r);
}

/// M82/T82.5: a filled star and a stroked arc, each built once in a unit
/// frame and placed by its transform — `Painter::path`'s `xf` scales and
/// moves the points, and the stroke's width stays in px.
///
/// `scripts/drive/widgets/rich.py`'s star check recomputes the star
/// from these numbers (`STAR_*`), so the two must move together.
fn paths(out: &mut dyn lumen_static::Painter, r: [f32; 4]) {
    use lumen_static::display::{PathEl, PathPaint};
    use lumen_static::Point;
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let none = lumen_static::Color(0, 0, 0, 0);
    // A quarter circle of radius 1, as one cubic.
    let k = 0.552_284_8;
    let arc = [
        PathEl::Move(Point { x: 1.0, y: 0.0 }),
        PathEl::Cubic(Point { x: 1.0, y: k }, Point { x: k, y: 1.0 }, Point { x: 0.0, y: 1.0 }),
    ];
    let ar = 0.3 * w.min(h);
    out.path(
        &arc,
        [ar, 0.0, 0.0, -ar, r[0] + 0.12 * w, r[1] + 0.85 * h],
        PathPaint { fill: none, stroke: lumen_static::Color(0x05, 0x96, 0x69, 0xff), width: 3.0 },
    );
    let mut star = [PathEl::Close; 11];
    for (i, e) in star.iter_mut().take(10).enumerate() {
        let rr = if i % 2 == 0 { 1.0 } else { STAR_INNER };
        let a = -core::f32::consts::FRAC_PI_2 + i as f32 * core::f32::consts::PI / 5.0;
        let p = Point { x: rr * a.cos(), y: rr * a.sin() };
        *e = if i == 0 { PathEl::Move(p) } else { PathEl::Line(p) };
    }
    let sr = STAR_RADIUS * w.min(h);
    out.path(
        &star,
        [sr, 0.0, 0.0, sr, r[0] + STAR_X * w, r[1] + 0.5 * h],
        PathPaint { fill: STAR_INK, stroke: none, width: 0.0 },
    );
}

/// The star's centre (a fraction of the box's width; it is centred
/// vertically), its outer radius (a fraction of the box's shorter side),
/// its inner radius (a fraction of the outer) and its colour.
///
/// **The colour reads at 3:1 on both palettes' ground** — M82/T82.8. It was
/// `#c2410c`, 5.18:1 on the light ground and **2.67:1** on the dark one,
/// which nothing judged until the D4 walk (`every_demo_text_reads_in_both_palettes`)
/// learned to read path ink; `#e14d0a` is 3.99:1 and 3.47:1.
const STAR_X: f32 = 0.78;
const STAR_RADIUS: f32 = 0.38;
const STAR_INNER: f32 = 0.4;
const STAR_INK: lumen_static::Color = lumen_static::Color(0xe1, 0x4d, 0x0a, 0xff);

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

/// The chip demo's tags, and one `fn` per chip — a handler cannot capture
/// which it is (M57/B2, C2's shape).
const CHIP_TAGS: [&str; 3] = ["rust", "systems", "no_std"];
fn tag(t: &str, on: bool, f: fn(&mut Store, &EventCtx)) -> Chip<String, Store> {
    let mut c = Chip::<String, Store>::new(t.into());
    c.selected = on;
    c.on_click = Some(f);
    c
}
fn tag_0(s: &mut Store, _: &EventCtx) {
    s.tags ^= 1;
}
fn tag_1(s: &mut Store, _: &EventCtx) {
    s.tags ^= 2;
}
fn tag_2(s: &mut Store, _: &EventCtx) {
    s.tags ^= 4;
}
/// The toast demo's button (M57/B34): the application asks, and
/// `refresh` shows the toast for `TOAST_MS`.
fn show_toast(s: &mut Store, _: &EventCtx) {
    s.toast_request = true;
    s.toasts += 1;
}
pub const TOAST_MS: f32 = 3000.0;

fn remove_tag(s: &mut Store, _: &EventCtx) {
    s.removed += 1;
}

fn chip(t: &str) -> Chip<String, Store> {
    Chip::<String, Store>::new(t.into())
}

fn gbadge(t: &str) -> Badge<String> {
    Badge::<String>::new(t.into())
}

fn scroll_rows() -> ScrollRows {
    let mut r = ScrollRows::default();
    for (k, b) in [
        &mut r.r0, &mut r.r1, &mut r.r2, &mut r.r3, &mut r.r4, &mut r.r5,
        &mut r.r6, &mut r.r7, &mut r.r8, &mut r.r9, &mut r.r10, &mut r.r11,
    ]
    .into_iter()
    .enumerate()
    {
        *b = btn(&format!("row {}", k + 1));
    }
    r
}

fn lbl_of(t: &str) -> Label<String> {
    let mut l = Label::<String>::default();
    l.set(t.into());
    l
}

/// One accordion section. The first is opened by the entry, so the demo
/// shows both states at once — this comment said so, and until M57/C7 no
/// section started open.
fn section(title: &str, body: &str, open: bool) -> Collapsible<Label<String>, String> {
    let mut c = Collapsible::new(title.into(), lbl_of(body));
    c.set_open(open);
    c
}

/// A `Button` wired to the press counter.
fn btn(label: &str) -> Button<Store, String> {
    let mut b = Button::<Store, String>::default();
    b.label = label.into();
    b.on_click = |s: &mut Store, _: &EventCtx| s.presses += 1;
    b
}

/// A toggle whose press runs `on_toggle`. **`on` is a copy**: `refresh`
/// writes the store's value back every frame (M57/C2). Seeding it once
/// and never again was how the demos flipped the store and left the
/// drawn boxes as they were (`docs/widget-audit.md`, CheckBox).
fn toggle<const K: u8>(on_: bool, on_toggle: fn(&mut Store, &EventCtx)) -> Toggle<Store, K> {
    Toggle::<Store, K> { on: on_, on_toggle, ..Default::default() }
}

/// The `Label` demo's paragraph: long enough to wrap to several lines in
/// its 320 px demo, so a line height shows as a taller box (M80/T80.2).
// The same paragraph is cut to one line by the "ellipsis" switch
// (M80/T80.3), which is why it is longer than its 320 px box.
pub const LABEL_PROSE: &str = "The quick brown fox jumps over the lazy dog, \
    and every line of this paragraph moves when the line height changes.";

/// The `Selectable` demo's paragraph (M80/T80.4): several lines in its
/// 320 px box, so a drag can cross one.
pub const SELECTABLE_PROSE: &str = "Drag across these words to select them, then copy. \
    A double click takes one word, a triple click the whole paragraph.";

/// The right-to-left field's seed (M81/T81.6): `שלום world 123`.
pub const TEXTINPUT_RTL: &str = "\u{5e9}\u{5dc}\u{5d5}\u{5dd} world 123";

/// The `Label` demo's switch: 1.5× line height, applied by the program's
/// `style_by` (M80/T80.2).
fn flip_spacing(s: &mut Store, _: &EventCtx) {
    s.spaced = !s.spaced;
}

/// The `Label` demo's second switch: one line, cut with an ellipsis, by
/// the same `style_by` (M80/T80.3).
fn flip_ellipsis(s: &mut Store, _: &EventCtx) {
    s.ellipsis = !s.ellipsis;
}

fn flip_a(s: &mut Store, _: &EventCtx) {
    s.flag = !s.flag;
}
fn flip_b(s: &mut Store, _: &EventCtx) {
    s.flag_b = !s.flag_b;
}
/// One `fn` per option: a handler cannot capture which it is (a `fn`
/// pointer, so it survives a reload), so each writes its own index.
fn choose_0(s: &mut Store, _: &EventCtx) {
    s.choice = 0;
}
fn choose_1(s: &mut Store, _: &EventCtx) {
    s.choice = 1;
}
fn choose_2(s: &mut Store, _: &EventCtx) {
    s.choice = 2;
}

/// The radio demo's options, by index.
pub const COLOURS: [&str; 3] = ["Red", "Green", "Blue"];

/// Everything on the stage, in sidebar order: **each group contiguous and
/// named once** — `tests::each_group_appears_once` holds that, because the
/// sidebar prints a heading where the group changes (M56).
pub static CATALOG: &[Entry] = &[
    entry!("Text", "Label", "Text that wraps, at the face's spacing or a line height.",
        false,
        |s| on(s, Demo::Label(DLabel {
            spacing: Labelled::new(toggle(s.store.spaced, flip_spacing), "1.5× line height"),
            ellipsis: Labelled::new(toggle(s.store.ellipsis, flip_ellipsis), "One line, with an ellipsis"),
            a: { let mut l = Label::<String>::default(); l.set(LABEL_PROSE.into()); l } })),
        |s| format!(
            "line height: {}; overflow: {}",
            if s.store.spaced { "1.5×" } else { "the face's" },
            if s.store.ellipsis { "ellipsis" } else { "wrap" }
        )),
    // ANCHOR: selectable
    entry!("Text", "Selectable", "Read-only text: drag to select, then copy.",
        false,
        |s| on(s, Demo::Selectable(DSelectable {
            a: { let mut t = Selectable::<String>::default(); t.label.set(SELECTABLE_PROSE.into()); t.select_all(); t } })),
        |s| match &s.ui.main.stage.row.demo {
            Demo::Selectable(d) => match d.a.selected_str() {
                Some(t) => format!("selected: {} chars", t.chars().count()),
                None => "selected: nothing".into(),
            },
            _ => String::new(),
        }),
    // ANCHOR_END: selectable
    entry!("Text", "Badge", "A count, a status and a tag.",
        false,
        |s| on(s, Demo::Badge(DBadge {
            a: Badge::new("3".into()), b: Badge::new("12".into()), c: Badge::new("99+".into()) })),
        none),
    entry!("Text", "Chip", "Filter tags: click to select, × to remove.",
        false,
        |s| on(s, Demo::Chip(DChip {
            a: tag("rust", s.store.tags & 1 != 0, tag_0), b: tag("systems", s.store.tags & 2 != 0, tag_1),
            c: { let mut c = tag("no_std", s.store.tags & 4 != 0, tag_2); c.removable = true; c.on_remove = Some(remove_tag); c } })),
        |s| format!("selected: {} · removed: {}", CHIP_TAGS.iter().enumerate().filter(|(k, _)| s.store.tags & (1 << k) != 0).map(|(_, t)| *t).collect::<Vec<_>>().join(" "), s.store.removed)),
    entry!("Text", "Avatar", "Initials, trimmed on a char boundary.",
        false,
        |s| on(s, Demo::Avatar(DAvatar {
            a: Avatar::new("ÅB".into()), b: Avatar::new("JD".into()), c: Avatar::new("K".into()) })),
        none),
    entry!("Text", "Icon", "All nine marks, drawn from primitives.",
        false,
        |s| on(s, Demo::Icon(DIcon {
            a: Icon::new(Symbol::Play), b: Icon::new(Symbol::Pause), c: Icon::new(Symbol::Stop),
            d: Icon::new(Symbol::Check), e: Icon::new(Symbol::Close), f: Icon::new(Symbol::Down),
            g: Icon::new(Symbol::Plus), h: Icon::new(Symbol::Minus), i: Icon::new(Symbol::Search) })),
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
    // ANCHOR: vectortext
    entry!("Text", "VectorText", "Text drawn as outlines, at any angle.",
        false,
        |s| on(s, Demo::VectorText(DVectorText {
            a: VectorText::new("Level", 0.0),
            b: VectorText::new("Tilted", 30.0),
            c: VectorText::new("Vertical", 90.0) })),
        none),
    // ANCHOR_END: vectortext
    // M83/T83.6: with the `shader` feature. The GPU runs `SHADERS[0]`; the
    // default software backend, `--shot` and every test draw the fallback.
    #[cfg(feature = "shader")]
    entry!("Text", "ShaderView", "A box a custom shader draws; its fallback without one.",
        false,
        |s| on(s, Demo::ShaderView(DShaderView { a: shader_view() })),
        none),
    entry!("Input", "Button", "Three of them, all wired to one counter.",
        false,
        |s| on(s, Demo::Button(DButton { a: btn("Primary"), b: btn("Default"), c: btn("Ghost") })),
        |s| format!("presses: {}", s.store.presses)),
    entry!("Input", "CheckBox", "Two boxes, each bound to its own flag.",
        false,
        |s| on(s, Demo::CheckBox(DCheckBox { a: Labelled::new(toggle(s.store.flag, flip_a), "I agree"), b: Labelled::new(toggle(s.store.flag_b, flip_b), "Send updates") })),
        |s| format!("checked: {} {}", s.store.flag, s.store.flag_b)),
    entry!("Input", "Switch", "The same two states, as a sliding pill.",
        false,
        |s| on(s, Demo::Switch(DSwitch { a: Labelled::new(toggle(s.store.flag, flip_a), "Wi-Fi"), b: Labelled::new(toggle(s.store.flag_b, flip_b), "Bluetooth") })),
        |s| format!("on: {} {}", s.store.flag, s.store.flag_b)),
    entry!("Input", "Radio", "One of three; the choice lives in the app.",
        false,
        |s| on(s, Demo::Radio(DRadio { a: Labelled::new(toggle(s.store.choice == 0, choose_0), "Red"), b: Labelled::new(toggle(s.store.choice == 1, choose_1), "Green"), c: Labelled::new(toggle(s.store.choice == 2, choose_2), "Blue") })),
        |s| format!("selected: {}", COLOURS[s.store.choice])),
    entry!("Input", "Slider", "Drag it, or focus it and use the arrows.",
        false,
        |s| on(s, Demo::Slider(DSlider { a: Slider::new(s.store.fraction) })),
        |s| format!("value: {:.2}", s.store.fraction)),
    entry!("Input", "TextInput", "One line, editable. Click it and type.",
        false,
        |s| on(s, Demo::TextInput(DTextInput { a: { let mut t = TextInput::<String>::default(); t.set_str("type here"); t } })),
        none),
    // M81/T81.6: the field right to left. Hebrew first, so the paragraph
    // is right to left; then Latin and digits, which run left to right
    // inside it -- so its logical end is at the visual left.
    entry!("Input", "TextInput RTL", "Right to left: Hebrew, then Latin and digits. Click its left end and type.",
        false,
        |s| on(s, Demo::TextInputRtl(DTextInputRtl { a: { let mut t = TextInput::<String>::default(); t.set_str(TEXTINPUT_RTL); t } })),
        none),
    entry!("Input", "TextArea", "Several lines, editable. Enter breaks a line; a paste keeps its lines.",
        false,
        |s| on(s, Demo::TextArea(DTextArea { a: { let mut t = TextArea::<String>::default(); t.set_str("first line"); t } })),
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
        |s| on(s, Demo::Menu(DMenu {
            open: { let mut b = btn("Commands"); b.on_click = |s: &mut Store, _: &EventCtx| s.menu_open = true; b },
            a: opened(Menu::new(&COMMANDS)) })),
        |s| format!("ran: {}", s.store.command.map_or("nothing yet", |k| COMMANDS[k]))),
    entry!("Fields", "NumberInput", "Arrows step it; letters are refused.",
        false,
        |s| on(s, Demo::Number(DNumberInput { a: NumberInput::new(s.store.fraction * 10.0, 0.0, 10.0, 0.5) })),
        |s| format!("value: {:.2}", s.store.fraction * 10.0)),
    entry!("Fields", "RangeSlider", "Two handles that cannot cross.",
        false,
        |s| on(s, Demo::Range(DRangeSlider { a: RangeSlider::new(0.25, 0.75) })),
        |s| format!("range: {:.2}..{:.2}", s.store.range.0, s.store.range.1)),
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
    entry!("Charts", "BarChart", "Bars on a scale; hover one for its value.",
        false,
        |s| on(s, Demo::Bar(DBarChart { a: { let mut c = BarChart::new(SAMPLE); c.axes = true; c.labels = SAMPLE_LABELS; c } })),
        none),
    entry!("Charts", "LineChart", "The same points as a line; hover one.",
        false,
        |s| on(s, Demo::Line(DLineChart { a: { let mut c = LineChart::new(SAMPLE); c.axes = true; c.labels = SAMPLE_LABELS; c } })),
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
            show: { let mut b = btn("Save"); b.on_click = show_toast; b },
            a: {
                let mut t = Toast::new("Saved".into());
                t.closable = true;
                t.hide();
                let mut p = Popup::new(ToastSlot { t });
                p.anchor = lumen_static_widgets::Anchor::WindowBottom;
                p.open = false;
                p.passive = true;
                p
            },
            b: Toast::new("Could not reach the server — retrying".into()),
        })),
        |s| format!("toasts shown: {}", s.store.toasts)),
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
    entry!("Navigation", "AppBar", "A title between a leading and an action slot.",
        false,
        |s| on(s, Demo::AppBar(DAppBar { a: AppBar::with("Inbox".into(), btn("Back"), btn("Save")) })),
        |s| format!("presses: {}", s.store.presses)),
    entry!("Navigation", "PullToRefresh", "Drag the content down past its top.",
        false,
        |s| on(s, Demo::Pull(DPullToRefresh { a: PullToRefresh::new({ let mut l = Label::<String>::default(); l.set("pull me down".into()); l }) })),
        |s| format!("refreshes: {}", s.store.refreshes)),
    entry!("Overlay", "Modal", "A centred panel over a dimmed window.",
        false,
        |s| on(s, Demo::Modal(DModal { a: veil(Modal::new(body_for!(ModalBody, "Delete this file?"))) })),
        |s| format!("dismissals: {} · confirmed: {}", s.store.dismissals, s.store.presses)),
    entry!("Overlay", "Sheet", "The same panel, across the bottom.",
        false,
        |s| on(s, Demo::Sheet(DSheet { a: veil(Sheet::new(body_for!(SheetBody, "Share with"))) })),
        |s| format!("dismissals: {} · confirmed: {}", s.store.dismissals, s.store.presses)),
    entry!("Overlay", "Drawer", "And down the side.",
        false,
        |s| on(s, Demo::Drawer(DDrawer { a: veil(Drawer::new(body_for!(DrawerBody, "Navigate"))) })),
        |s| format!("dismissals: {} · confirmed: {}", s.store.dismissals, s.store.presses)),
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
    entry!("Layout", "Accordion", "Three sections, one open at a time.",
        false,
        |s| on(s, Demo::Accordion(DAccordion {
            a: section("Account", "Your name and email", true),
            b: section("Privacy", "Who can see your activity", false),
            c: section("Advanced", "Developer options", false) })),
        none),
    entry!("Layout", "ScrollView", "Twelve rows in a view for four.",
        false,
        |s| on(s, Demo::Scroll(DScrollView { a: ScrollView::new(scroll_rows()) })),
        |s| match &s.ui.main.stage.row.demo {
            Demo::Scroll(d) => format!("scroll: {:.0} px", d.a.scroll),
            _ => String::new(),
        }),
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
    entry!("Data", "DataGrid", "A sheet over virtualized rows; press a cell, or use the arrows.",
        false,
        |s| on(s, Demo::DataGrid(DDataGrid {
            a: {
                let mut g = DataGrid::new(&["Name", "Type", "Size", "Lines"], 200, 26.0, grid_row);
                // Sizes and line counts are numbers: their headers sit at
                // the end of the column, where `EndCell` puts them (B51).
                g.align[2] = lumen_static::style::Align::End;
                g.align[3] = lumen_static::style::Align::End;
                g
            } })),
        // The cell in a sheet's own notation: a column letter and a
        // one-based row, as the gutter numbers it.
        |s| match s.store.grid_cell {
            Some((r, c)) => format!("selected: file-{r:03}.rs, cell {}{}", (b'A' + c as u8) as char, r + 1),
            None => "selected: none".into(),
        }),
];

/// The series the charts share, so the three demos are comparable.
pub static SAMPLE: &[f32] = &[3.0, 7.0, 4.0, 9.0, 6.0, 8.0, 5.0, 10.0, 7.5, 4.5];
/// One per sample, under its bar or point (M57/B30).
pub static SAMPLE_LABELS: &[&str] = &["J", "F", "M", "A", "M", "J", "J", "A", "S", "O"];
