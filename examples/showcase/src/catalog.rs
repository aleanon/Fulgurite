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
    /// One line on what the seeded demo shows.
    pub blurb: &'static str,
    /// Whether it wants a per-frame tick.
    pub animated: bool,
    /// Whether its state lives in the **application**.
    ///
    /// The core's own split, and the reason this flag exists rather than
    /// a blanket rule: a checkbox's boolean belongs to the app and is
    /// written back through a binding, so the demo is rebuilt from the
    /// store after every pump. A text field's text and a slider's value
    /// originate *at the widget*, because the user typed or dragged
    /// them — rebuilding those from the store would overwrite the
    /// interaction on the very next frame.
    ///
    /// A `Dyn` island has no typed handle, so a widget that owns its own
    /// value cannot be read back here at all. That is the honest cost of
    /// the hatch, and it is why those entries print no readout.
    pub live: bool,
    /// Put it on the stage, seeded with real data.
    ///
    /// **Rebuilds the island rather than updating it**, which is the
    /// cost of the hatch rather than a shortcut: a `Dyn` is a
    /// `Box<dyn Node>`, so once a widget is inside one there is no typed
    /// handle left to write a field through. For a showcase that is one
    /// small allocation per change; a real application would reach for
    /// `Either` or a typed field.
    pub seed: fn(&mut Show),
    /// A live readout of whatever state the demo carries.
    pub status: fn(&Show) -> String,
}

fn none(_: &Show) -> String {
    String::new()
}

macro_rules! entry {
    ($group:literal, $name:literal, $blurb:literal, $anim:expr, $live:expr, $seed:expr, $status:expr) => {
        Entry {
            group: $group,
            name: $name,
            blurb: $blurb,
            animated: $anim,
            live: $live,
            seed: $seed,
            status: $status,
        }
    };
}

/// Everything on the stage, in dropdown order.
pub static CATALOG: &[Entry] = &[
    // ---- text & display ------------------------------------------
    entry!("Text", "Label", "A line of text — the core's simplest widget.", false, false,
        |s| stage(s, Label::<String>::default().tap(|l| l.set("The quick brown fox".into()))), none),
    entry!("Text", "Badge", "A count, as a pill.", false, false,
        |s| stage(s, Badge { text: "128".to_string() }), none),
    entry!("Text", "Chip", "A token in the quiet palette.", false, false,
        |s| stage(s, Chip { text: "rust · systems".to_string() }), none),
    entry!("Text", "Avatar", "Initials, trimmed on a char boundary.", false, false,
        |s| stage(s, Avatar { initials: "ÅB".to_string() }), none),
    entry!("Text", "Icon", "A mark drawn from primitives — never a missing glyph.", false, false,
        |s| stage(s, Icon::new(Symbol::Check)), none),
    entry!("Text", "Divider", "A hairline; its thickness is its style's.", false, false,
        |s| stage(s, Divider), none),
    // ---- input ---------------------------------------------------
    entry!("Input", "Button", "Press it; the readout counts.", false, true,
        |s| {
            let mut b = Button::<Show, String>::default();
            b.label = "Press me".into();
            b.on_click = |s: &mut Show, _: &EventCtx| s.value += 1.0;
            stage(s, b);
        },
        |s| format!("pressed {} times", s.value as i64)),
    entry!("Input", "CheckBox", "A box and a tick. The app owns the flag.", false, true,
        |s| {
            stage(
                s,
                CheckBox::<Show> {
                    on: s.flag,
                    on_toggle: |s: &mut Show, _: &EventCtx| s.flag = !s.flag,
                },
            )
        },
        |s| format!("checked: {}", s.flag)),
    entry!("Input", "Switch", "The same two states, as a sliding pill.", false, true,
        |s| {
            stage(
                s,
                Switch::<Show> {
                    on: s.flag,
                    on_toggle: |s: &mut Show, _: &EventCtx| s.flag = !s.flag,
                },
            )
        },
        |s| format!("on: {}", s.flag)),
    entry!("Input", "Radio", "One of a set; the selection lives in the app.", false, true,
        |s| {
            stage(
                s,
                Radio::<Show> {
                    on: s.flag,
                    on_toggle: |s: &mut Show, _: &EventCtx| s.flag = !s.flag,
                },
            )
        },
        |s| format!("selected: {}", s.flag)),
    entry!("Input", "Slider", "Drag it. It owns its value — see `Entry::live`.", false, false,
        |s| stage(s, Slider::new(0.35)), none),
    entry!("Input", "TextInput", "One line, editable. The widget owns the text.", false, false,
        |s| {
            let mut t = TextInput::<String>::default();
            t.set_str("type here");
            stage(s, t)
        },
        none),
    // ---- progress ------------------------------------------------
    entry!("Progress", "ProgressBar", "Determinate, driven by the tick.", true, false,
        |s| stage(s, ProgressBar::new(s.phase)), |s| format!("{:.0}%", s.phase * 100.0)),
    entry!("Progress", "Spinner", "Indeterminate: a ring with a travelling gap.", true, false,
        |s| stage(s, Spinner::new(s.phase)), |s| format!("phase {:.2}", s.phase)),
    entry!("Progress", "Skeleton", "A placeholder with a shimmer.", true, false,
        |s| stage(s, Skeleton::new(s.phase)), |s| format!("phase {:.2}", s.phase)),
    // ---- charts --------------------------------------------------
    entry!("Charts", "BarChart", "Bars from a zero baseline.", false, false,
        |s| stage(s, BarChart::new(SAMPLE)), none),
    entry!("Charts", "LineChart", "A polyline through the same points.", false, false,
        |s| stage(s, LineChart::new(SAMPLE)), none),
    entry!("Charts", "PieChart", "Proportions of a whole, as a ring.", false, false,
        |s| stage(s, PieChart::new(&[5.0, 3.0, 2.0, 1.5, 1.0])), none),
];

/// The series the charts share, so the three demos are comparable.
pub static SAMPLE: &[f32] = &[3.0, 7.0, 4.0, 9.0, 6.0, 8.0, 5.0, 10.0, 7.5, 4.5];
