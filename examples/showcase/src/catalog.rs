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

/// A `Button` wired to the press counter.
fn btn(label: &str) -> Button<Show, String> {
    let mut b = Button::<Show, String>::default();
    b.label = label.into();
    b.on_click = |s: &mut Show, _: &EventCtx| s.presses += 1;
    b
}

fn toggle<const K: u8>(on_: bool) -> Toggle<Show, K> {
    Toggle::<Show, K> { on: on_, on_toggle: |s: &mut Show, _: &EventCtx| s.flag = !s.flag }
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
        |s| on(s, Demo::CheckBox(DCheckBox { a: PairCheck { t: toggle(s.flag), l: { let mut l = Label::<String>::default(); l.set("I agree".into()); l } }, b: PairCheck { t: toggle(!s.flag), l: { let mut l = Label::<String>::default(); l.set("Send updates".into()); l } } })),
        |s| format!("checked: {}", s.flag)),
    entry!("Input", "Switch", "The same two states, as a sliding pill.",
        "One type with three presentations, chosen by a *const* parameter — so `kind_name` and `defaults` answer at compile time.",
        false,
        |s| on(s, Demo::Switch(DSwitch { a: PairSwitch { t: toggle(s.flag), l: { let mut l = Label::<String>::default(); l.set("Wi-Fi".into()); l } }, b: PairSwitch { t: toggle(!s.flag), l: { let mut l = Label::<String>::default(); l.set("Bluetooth".into()); l } } })),
        |s| format!("on: {}", s.flag)),
    entry!("Input", "Radio", "One of a set; the selection lives in the app.",
        "There is no group container: each radio compares against a shared value, which is the honest cost of not inventing one.",
        false,
        |s| on(s, Demo::Radio(DRadio { a: PairRadio { t: toggle(s.flag), l: { let mut l = Label::<String>::default(); l.set("Red".into()); l } }, b: PairRadio { t: toggle(!s.flag), l: { let mut l = Label::<String>::default(); l.set("Green".into()); l } }, c: PairRadio { t: toggle(false), l: { let mut l = Label::<String>::default(); l.set("Blue".into()); l } } })),
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
];

/// The series the charts share, so the three demos are comparable.
pub static SAMPLE: &[f32] = &[3.0, 7.0, 4.0, 9.0, 6.0, 8.0, 5.0, 10.0, 7.5, 4.5];
