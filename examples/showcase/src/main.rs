//! **The widget showcase** — every built-in widget, one at a time.
//!
//! `just example showcase`. A sidebar lists the whole catalogue under
//! its groups; choosing an entry puts it, seeded with real data, on the
//! stage card beside it. The page above the card names the entry, says in
//! one line what the demo shows, and — for a widget that carries state —
//! prints a live readout as a chip, so a slider drag or a checkbox toggle
//! can be watched landing in the store. `docs/plan-showcase-design.md`
//! (M56) is the layout's design and what building it found.
//!
//! Modelled on Lumen's `examples/widget_showcase`, which is where the
//! shape of this comes from.
//!
//! # How "one of N" works in a static tree
//!
//! The tree's topology is its type, so a stage that can hold any of
//! fifty-two demos cannot simply *become* each of them. It is an enum
//! (`stage::Demo`, M24/T24.3): one-of-N by construction, its active
//! variant laid out in place in the parent's table, and the `Dyn` hatch
//! this example once used left for content whose shape is genuinely
//! unknown.

use lumen_static::display::Color;
use lumen_static::prelude::*;
use lumen_static::Rule as StyleRule;
use lumen_static::style::Palette;
use lumen_static_widgets::Toggle;
use lumen_static_widgets::prelude::*;
use lumen_static_winit::{application, Program, Run};

mod catalog;
mod chip;
mod nav;
mod stage;
use lumen_static::repeat::Repeat;
use chip::StatusChip;
use nav::{NavRow, RowKind};
use catalog::CATALOG;
pub use stage::*;

// ---- palette ---------------------------------------------------------

/// The showcase's colours, by role (M25/T25.11). The sheet names roles
/// rather than these values, so one `set_palette` re-themes every rule
/// that does. `LUMEN_PALETTE=dark` starts in [`SHOW_DARK`]; a running
/// window built with `--features agent` also takes `app.setPalette`, which
/// swaps to the core's `Palette::LIGHT` or `Palette::DARK`.
const SHOW_LIGHT: Palette = Palette {
    page: Color(0xf4, 0xf6, 0xfa, 0xff),
    surface: Color(0xff, 0xff, 0xff, 0xff),
    // A toggle's resting body, at 3:1 on the stage (WCAG 1.4.11) — M57/A8.
    // This was the fields' pale ground (0xf0f2f6), which every toggle then
    // drew as its off state at 1.1:1; the fields moved to `raised`.
    control: Color(0x8a, 0x94, 0xa6, 0xff),
    text: Color(0x1b, 0x22, 0x30, 0xff),
    // #6b7488 until M56/T56.3: 4.33:1 on the page, under AA's 4.5 for
    // the blurb and breadcrumb it colours (`chrome_text_meets_contrast`).
    muted: Color(0x63, 0x6c, 0x80, 0xff),
    // M54/T54.6 (R16-F9): `#2f6bff` was under AA on three grounds; this
    // is the first value that passes all of them
    // (`docs/results-a11y-platform.md`). The focus ring shares it. M57/A8
    // had raised it to #2b64f5 for the same reason (5.0:1 under white);
    // this is the darker of the two.
    accent: Color(0x26, 0x5c, 0xe6, 0xff),
    on_accent: Color(0xff, 0xff, 0xff, 0xff),
    focus: Color(0x26, 0x5c, 0xe6, 0xff),
    // M57/A8's roles: every widget's own colours now follow these, with
    // no rule in the sheet.
    raised: Color(0xff, 0xff, 0xff, 0xff),
    border: Color(0x83, 0x8d, 0x9f, 0xff),
    track: Color(0xd5, 0xdb, 0xe4, 0xff),
    placeholder: Color(0xe4, 0xe9, 0xf0, 0xff),
    scrim: Color(0x0f, 0x17, 0x2a, 110),
    danger: Color(0xdc, 0x26, 0x26, 0xff),
    on_danger: Color(0xff, 0xff, 0xff, 0xff),
};

const SHOW_DARK: Palette = Palette {
    page: Color(0x12, 0x15, 0x1c, 0xff),
    surface: Color(0x1c, 0x21, 0x2b, 0xff),
    control: Color(0x6b, 0x75, 0x89, 0xff),
    text: Color(0xe6, 0xea, 0xf2, 0xff),
    muted: Color(0x94, 0xa0, 0xb4, 0xff),
    // 5.1:1 on `raised` — M57/A8; #5b8cff was 4.37:1 as the pagination's
    // page numbers.
    accent: Color(0x6b, 0x98, 0xff, 0xff),
    on_accent: Color(0x0a, 0x0e, 0x18, 0xff),
    focus: Color(0x82, 0xaa, 0xff, 0xff),
    // Raised a step *above* the stage card (`surface`), which is how a
    // dark theme shows elevation.
    raised: Color(0x26, 0x2d, 0x3a, 0xff),
    border: Color(0x6b, 0x75, 0x89, 0xff),
    track: Color(0x3a, 0x42, 0x54, 0xff),
    placeholder: Color(0x2e, 0x35, 0x44, 0xff),
    scrim: Color(0, 0, 0, 150),
    danger: Color(0xf8, 0x71, 0x71, 0xff),
    on_danger: Color(0x1a, 0x0a, 0x0a, 0xff),
};

/// The palette to start in: [`SHOW_DARK`] under `LUMEN_PALETTE=dark`.
fn palette() -> Palette {
    match std::env::var("LUMEN_PALETTE").as_deref() {
        Ok("dark") => SHOW_DARK,
        _ => SHOW_LIGHT,
    }
}

/// The chrome's colours with no role — M56/T56.3. A hairline is neither
/// text nor a control's body, and a chip's or a ghost button's tint is one
/// widget's, so they are values; but they are **picked with the palette**,
/// because the light values on the dark page were a bright rule and a
/// glaring chip. Chosen once, by the same `LUMEN_PALETTE` as
/// [`palette`]. A palette swapped at run time through the agent's
/// `app.setPalette` re-resolves the sheet's roles and not these —
/// `every_role_rule_follows_the_palette` prints them rather than hides
/// them.
#[derive(Clone, Copy)]
struct Chrome {
    /// The sidebar's edge, the stage card's and a popup's border.
    hairline: Color,
    /// The readout chip's ground.
    chip: Color,
    /// Accent-coloured text on an accent tint — the chip's, the ghost
    /// button's: the accent deepened (light) or lifted (dark) until it
    /// reads there. The raw accent was 3.90:1 on the chip and 3.91:1 on
    /// the ghost button in the light palette.
    accent_text: Color,
    /// The ghost button's faint ground.
    ghost: Color,
    /// A secondary button's edge on the white stage card.
    outline: Color,
}

const CHROME_LIGHT: Chrome = Chrome {
    hairline: Color(0xe1, 0xe6, 0xee, 0xff),
    chip: Color(0xe8, 0xef, 0xff, 0xff),
    accent_text: Color(0x1f, 0x4f, 0xd1, 0xff),
    ghost: Color(0xea, 0xef, 0xfb, 0xff),
    outline: Color(0xcf, 0xd6, 0xe2, 0xff),
};

const CHROME_DARK: Chrome = Chrome {
    hairline: Color(0x2c, 0x33, 0x40, 0xff),
    chip: Color(0x1f, 0x2b, 0x47, 0xff),
    accent_text: Color(0x9b, 0xb8, 0xff, 0xff),
    ghost: Color(0x23, 0x2f, 0x4d, 0xff),
    outline: Color(0x3d, 0x46, 0x57, 0xff),
};

/// The chrome's colours for the palette in force at start.
fn chrome() -> Chrome {
    match std::env::var("LUMEN_PALETTE").as_deref() {
        Ok("dark") => CHROME_DARK,
        _ => CHROME_LIGHT,
    }
}

/// The window: a 248 px sidebar and the page beside it (M56).
const W: u32 = 1120;
const H: u32 = 760;
/// One sidebar row's height — an entry or a group heading.
const NAV_ROW_H: f32 = 32.0;

// ---- the tree --------------------------------------------------------

/// The sidebar: the brand, then every entry under its group — M56
/// (`docs/plan-showcase-design.md`). One `Repeat` of [`NavRow`]s, so the
/// catalogue is data here too, and the list scrolls by wheel (M27/T27.4)
/// when it is taller than the window.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Sidebar {
    #[widget_id("brand")]
    brand: Label,
    #[widget_id("brand_sub")]
    sub: Label,
    #[widget_id("nav")]
    nav: Repeat<NavRow<Store>>,
}

/// The page title and, at the far end of its line, the live readout as a
/// chip.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 12.0)]
pub struct TitleRow {
    #[widget_id("title")]
    title: Label<String>,
    #[widget_id("tgap")]
    gap: Spacer,
    #[widget_id("readout")]
    readout: StatusChip,
}

/// What is on the page: where it sits in the catalogue, its name, and the
/// one line on what the demo shows. **No note under the demo** — M56
/// removed it (`docs/plan-showcase-design.md` T56.1).
#[derive(StaticTree, Default)]
#[widget(column, spacing = 6.0)]
pub struct Header {
    #[widget_id("crumb")]
    crumb: Label<String>,
    heading: TitleRow,
    #[widget_id("blurb")]
    blurb: Label<String>,
}

/// The stage: whatever the sidebar chose, centred by two `FILL` spacers.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
pub struct StageRow {
    #[widget_id("sl")]
    l: Spacer,
    #[widget_id("demo")]
    demo: Demo,
    #[widget_id("sr")]
    r: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Stage {
    #[widget_id("st")]
    top: Spacer,
    row: StageRow,
    #[widget_id("sb")]
    bottom: Spacer,
}

/// The page beside the sidebar: the header, then the stage as a card.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 20.0)]
pub struct Main {
    #[widget_id("header")]
    header: Header,
    #[widget_id("stage")]
    stage: Stage,
}

/// The whole window: sidebar, a hairline, the page.
///
/// **No overlay layer.** A `Select`'s menu — the stage's `Select` demo,
/// since M56 moved navigation into the sidebar — is a `Popup`: out of
/// flow, painted in the overlay phase, hit first.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
pub struct Screen {
    #[widget_id("sidebar")]
    side: Sidebar,
    #[widget_id("rule")]
    rule: Divider,
    #[widget_id("main")]
    main: Main,
}

#[derive(DeriveApp, Default)]
#[app(tick = advance, after_update = refresh, wake = wake, images = app_images)]
// M84/T84.8: the Video demo's frames arrive as messages from a subscription.
#[app(msg = lumen_static_shell::video::VideoFrame, apply = catalog::apply_clip)]
#[app(subscriptions = subscriptions, jobs = catalog::clip_jobs)]
pub struct Show {
    #[tree]
    ui: Screen,
    /// **What a handler may touch** — M33/T33.4. Every handler in this
    /// example — the demos' buttons and toggles, the sidebar's rows, the
    /// `Select` demo's trigger, rows and dismissal — takes `&mut Store`,
    /// and the tree is not reachable from it. What a handler used to write
    /// into the tree (the header, the readout, the demo on the stage, the
    /// `Select` demo's label and open state) is written by `refresh`, the
    /// `after_update` hook, which keeps `&mut Show` — so showcase stays on
    /// the conservative path M29 §2.4 records for `after_update`, by
    /// design, and that whole-tree mark is also what re-binds the sidebar
    /// when the selection moves (`nav.rs`).
    #[state]
    store: Store,
}

/// What the handlers own.
#[derive(Default)]
pub struct Store {
    /// Which catalogue entry is on the stage.
    selected: usize,
    /// Turns, for the demos that animate. Advanced by `App::tick`;
    /// nothing here drives its own clock.
    phase: f32,
    /// Set when the selection changed and the stage needs re-seeding.
    /// Not every pump: a `TextInput` owns its text, and re-seeding it
    /// each frame would overwrite what was typed on the frame after it
    /// was typed.
    needs_seed: bool,
    /// Set by a handler that changed the selection, so `refresh` rebuilds
    /// the header and the stage. A handler cannot write them itself: they
    /// are on the tree.
    needs_rebuild: bool,
    /// **Whether the `Select` demo is open — the application's, since
    /// `Select` became controlled** (M33/T33.3). Its trigger sets it, a
    /// chosen row and the pass's dismissal (`on_dismiss`) clear it, and
    /// `refresh` puts it on the tree.
    select_open: bool,
    /// Which of the `Select` demo's options is chosen.
    pick: usize,
    /// Scroll the sidebar so the selected row is in view, once its
    /// viewport is known — a window opened on the fifty-second entry
    /// should show it highlighted, not the top of the list.
    reveal: bool,
    /// The live demo's own state. One store rather than one per demo,
    /// because a showcase only ever has one demo alive.
    presses: u32,
    flag: bool,
    /// The second box's and switch's own flag (M57/C2): two controls
    /// bound to one flag could only ever disagree.
    flag_b: bool,
    /// The `Label` demo's 1.5× line height (M80/T80.2). The program's
    /// sheet is a function of it (`style_by`), so the switch restyles the
    /// label without a widget field.
    pub(crate) spaced: bool,
    /// The `Label` demo's one-line ellipsis (M80/T80.3), by the same
    /// `style_by` as `spaced`.
    pub(crate) ellipsis: bool,
    /// Which radio option is chosen, 0..=2 (M57/C2). It was a `bool`
    /// shared by three radios, so "Blue" could never be selected.
    choice: usize,
    fraction: f32,
    /// The range slider's two bounds, read back each frame (M57/C4).
    range: (f32, f32),
    /// Veil dismissal requests, apart from the panel's presses (M57/C6).
    dismissals: u32,
    /// The accordion's open section, if any (M57/C7).
    section: Option<usize>,
    /// Refreshes the pull-to-refresh demo has fired (M57/B40).
    refreshes: u32,
    /// The menu demo's reopen request, and the last command it ran (C3).
    menu_open: bool,
    command: Option<usize>,
    /// The chip demo's selected tags, a bit each, and its × presses (B2).
    tags: u8,
    removed: u32,
    /// The linked markdown demo's last followed link (M87/T87.3).
    link: Option<usize>,
    /// **The Pop demo** (M85/T85.4): whether its placeholder has been
    /// scrolled into view and so "loaded". Set by the pop's `on_view`;
    /// `refresh` writes the placeholder's text from it, and the entry's
    /// seed clears it.
    pop_loaded: bool,
    /// The toast demo: a show request, and how many were shown (B34).
    toast_request: bool,
    toasts: u32,
    /// The data grid's selected row, taken from the grid (B51).
    /// The grid's active cell, `(row, column)`, as its last selection
    /// left it.
    grid_cell: Option<(usize, usize)>,
    /// **The Image demo's clock** (M84/T84.3): the GIF's delays, its
    /// frame and whether it plays. The pause button flips `playing`,
    /// `advance` writes its frame into the image, and `wake` answers its
    /// next change.
    spin: lumen_static::Playback,
    /// **The Video demo** (M84/T84.8): what the widget shows, the
    /// application's own image store its feed fills (`App::images`), and
    /// the subscription set its play button and seeks declare.
    clip: catalog::Clip,
    clip_images: lumen_static_shell::image::ImageStore,
    subs: lumen_static::spawn::Subs,
}

/// **The application's images** — `#[app(images = app_images)]`
/// (M84/T84.6): the Video demo's store, which the renderer reads through
/// the frame before the window's own (`Program::images`, the Image demo's
/// GIF).
fn app_images(s: &Show) -> Option<&dyn lumen_static::ImageSource> {
    Some(&s.store.clip_images)
}

/// **What should be running** — `#[app(subscriptions = subscriptions)]`
/// (M84/T84.8): the Video demo's feed while it plays.
fn subscriptions(s: &mut Show) -> Option<&mut lumen_static::spawn::Subs> {
    Some(&mut s.store.subs)
}

/// After every pump: rebuild the demos whose state lives in the store,
/// and refresh the readout.
///
/// **`after_update`, not the tick.** A button press changes `Show` and
/// asks for no animation, so a tick-only refresh would show the count
/// one interaction late. `after_update` runs on every pump, which is
/// exactly when the store can have changed.
/// **Returns whether it changed anything** (M27/T27.3). It used to
/// return `()`, which forced the derive to mark every pump changed, so
/// the showcase's frame never settled and every interaction damaged the
/// whole window. A drive script measuring per-widget damage read 170x
/// the widget's own box before this.
fn refresh(s: &mut Show) -> bool {
    let mut changed = false;
    // What a handler asked for and could not do itself (M33/T33.4).
    if s.store.needs_rebuild {
        s.store.needs_rebuild = false;
        rebuild(s);
        changed = true;
    }
    if s.store.needs_seed {
        s.store.needs_seed = false;
        let e = &CATALOG[s.store.selected];
        (e.seed)(s);
        changed = true;
    }
    // The selected row, into view — once, after the first layout has
    // told the list how tall its viewport is.
    if s.store.reveal && s.ui.side.nav.viewport() > 0.0 {
        s.store.reveal = false;
        let nav = &mut s.ui.side.nav;
        let row = (0..catalog::nav_len())
            .find(|&r| catalog::nav_item(r) == catalog::NavItem::Entry(s.store.selected))
            .unwrap_or(0);
        let (vp, y) = (nav.viewport(), row as f32 * NAV_ROW_H);
        if y < nav.scroll || y + NAV_ROW_H > nav.scroll + vp {
            nav.scroll = (y - (vp - NAV_ROW_H) / 2.0).clamp(0.0, nav.max_scroll(vp));
            changed = true;
        }
    }
    // **The `Select` demo, controlled**: its open state and its label are
    // the store's, put on the tree here — compared first, so an unchanged
    // frame writes nothing and allocates nothing.
    if let Demo::Select(d) = &mut s.ui.main.stage.row.demo {
        if d.a.menu.open != s.store.select_open {
            d.a.menu.open = s.store.select_open;
            changed = true;
        }
        // The current choice, for the menu to mark (M57/B21).
        d.a.current = Some(s.store.pick);
        let want = catalog::FRUIT[s.store.pick];
        if d.a.trigger.label != want {
            d.a.trigger.label.clear();
            d.a.trigger.label.push_str(want);
            changed = true;
        }
    }
    // **The Pop demo's placeholder** (M85/T85.4): its text is the
    // store's, so the frame after the pop is scrolled into view draws it
    // loaded. Compared first, so an unchanged frame writes nothing.
    if let Demo::Pop(d) = &mut s.ui.main.stage.row.demo {
        let want = if s.store.pop_loaded { "loaded" } else { "loads when it scrolls into view" };
        if d.a.content.item.content.text.text.as_str() != want {
            d.a.content.item.content.text.set(want.into());
            changed = true;
        }
    }
    // **The toggles, controlled** (M57/C2): `on` is a copy, written back
    // from the store every frame, as the `Select` above is. Compared
    // first, so an unchanged frame writes nothing.
    {
        let (a, b, c) = (s.store.flag, s.store.flag_b, s.store.choice);
        let mut put = |t: &mut bool, v: bool| {
            if *t != v {
                *t = v;
                changed = true;
            }
        };
        match &mut s.ui.main.stage.row.demo {
            Demo::CheckBox(d) => {
                put(&mut d.a.control.on, a);
                put(&mut d.b.control.on, b);
            }
            Demo::Switch(d) => {
                put(&mut d.a.control.on, a);
                put(&mut d.b.control.on, b);
            }
            Demo::Label(d) => {
                put(&mut d.spacing.control.on, s.store.spaced);
                put(&mut d.ellipsis.control.on, s.store.ellipsis);
            }
            Demo::Radio(d) => {
                put(&mut d.a.control.on, c == 0);
                put(&mut d.b.control.on, c == 1);
                put(&mut d.c.control.on, c == 2);
            }
            Demo::Chip(d) => {
                let t = s.store.tags;
                put(&mut d.a.selected, t & 1 != 0);
                put(&mut d.b.selected, t & 2 != 0);
                put(&mut d.c.selected, t & 4 != 0);
            }
            _ => {}
        }
    }
    // **One section open at a time** — the application's rule, as
    // `Collapsible`'s rustdoc says: an accordion is a column of them, and
    // the "three lines in the application" are these (M57/C7). A section
    // that opened since last frame becomes the open one; the others shut.
    if let Demo::Accordion(d) = &mut s.ui.main.stage.row.demo {
        let opens = [d.a.is_open(), d.b.is_open(), d.c.is_open()];
        if let Some(k) = (0..3).find(|&k| opens[k] && Some(k) != s.store.section) {
            s.store.section = Some(k);
        } else if s.store.section.is_some_and(|j| !opens[j]) {
            s.store.section = None;
        }
        let want = s.store.section;
        changed |= d.a.set_open(want == Some(0)) | d.b.set_open(want == Some(1)) | d.c.set_open(want == Some(2));
    }
    // **A menu runs a command** (M57/B23, C3): taken once, shown in the
    // readout; its button opens it again.
    if let Demo::Menu(d) = &mut s.ui.main.stage.row.demo {
        if s.store.menu_open {
            s.store.menu_open = false;
            changed |= d.a.open();
        }
        if let Some(k) = d.a.take_chosen() {
            s.store.command = Some(k);
            changed = true;
        }
    }
    // **A toast shown on request** (M57/B34): the button asks, this
    // shows it for `TOAST_MS`, and `advance` runs its countdown.
    if let Demo::Toast(d) = &mut s.ui.main.stage.row.demo {
        if core::mem::take(&mut s.store.toast_request) {
            d.a.content.t.show(Some(catalog::TOAST_MS));
            changed = true;
        }
        // The overlay is open exactly while the toast shows, so a closed
        // or timed-out toast leaves no empty overlay taking Escape.
        let shown = d.a.content.t.is_shown();
        if d.a.open != shown {
            d.a.open = shown;
            changed = true;
        }
    }
    // A press on a grid row, taken once (M57/B51).
    if let Demo::DataGrid(d) = &mut s.ui.main.stage.row.demo {
        // A press or a key; the column comes with it (`DataGrid::active`).
        if d.a.take_selected().is_some() {
            s.store.grid_cell = d.a.active();
            changed = true;
        }
    }
    // **The Image demo's pause button says what it will do** (M84/T84.3):
    // a `&'static str`, compared first, so an unchanged frame writes nothing.
    if let Demo::Image(d) = &mut s.ui.main.stage.row.demo {
        let want = if s.store.spin.playing { "Pause" } else { "Play" };
        if d.pause.label != want {
            d.pause.label = want;
            changed = true;
        }
    }
    // **The Video demo, controlled** (M84/T84.8): every field it shows is
    // the store's, compared first, so a frame with no new picture writes
    // nothing.
    if let Demo::Video(d) = &mut s.ui.main.stage.row.demo {
        let c = &s.store.clip;
        let v = &mut d.a;
        if (v.playing, v.position_ms, v.duration_ms, v.frame) != (c.playing, c.position_ms, c.duration_ms, c.frame) {
            v.playing = c.playing;
            v.position_ms = c.position_ms;
            v.duration_ms = c.duration_ms;
            v.frame = c.frame;
            changed = true;
        }
    }
    // **A pull that fired is a refresh** (M57/B40): taken once, counted,
    // so the readout shows the widget doing what it is for.
    if let Demo::Pull(d) = &mut s.ui.main.stage.row.demo {
        if d.a.take_triggered() {
            s.store.refreshes += 1;
            changed = true;
        }
    }
    // **Read the typed fields back before reporting.** This is what a
    // `Dyn` island could not do: a `Slider` owns its value, and through
    // a `Box<dyn Node>` there was no handle to ask.
    if let Demo::Slider(d) = &s.ui.main.stage.row.demo {
        let v = d.a.value();
        if v != s.store.fraction {
            s.store.fraction = v;
            changed = true;
        }
    }
    // The number field and the range slider own their values the same way,
    // and their readouts never moved until these read them back (M57/C4:
    // `value: 0.00` through every edit, `docs/widget-audit.md`).
    if let Demo::Number(d) = &s.ui.main.stage.row.demo {
        let v = d.a.value() / 10.0;
        if v != s.store.fraction {
            s.store.fraction = v;
            changed = true;
        }
    }
    if let Demo::Range(d) = &s.ui.main.stage.row.demo {
        let r = (d.a.low(), d.a.high());
        if r != s.store.range {
            s.store.range = r;
            changed = true;
        }
    }
    // **The dismissal request, taken.** A veil never closes itself — it
    // asks, and the application decides. Polling here is where an
    // application does it: `after_update` runs on every pump, which is
    // exactly when the request can have been made (M27/T27.7).
    let asked = match &mut s.ui.main.stage.row.demo {
        Demo::Modal(d) => d.a.take_dismissed(),
        Demo::Sheet(d) => d.a.take_dismissed(),
        Demo::Drawer(d) => d.a.take_dismissed(),
        _ => false,
    };
    if asked {
        // Its own count (M57/C6): this bumped `presses`, which the panel's
        // Confirm button bumps too, so a confirm read as a dismissal.
        s.store.dismissals += 1;
        changed = true;
    }
    let e = &CATALOG[s.store.selected];
    let text = (e.status)(s);
    // `set_str` compares before writing and answers whether the text
    // differed, so the readout reports for itself rather than being
    // assumed dirty — and the measurement memo survives an unchanged one.
    changed | s.ui.main.header.heading.readout.set_str(&text)
}

/// Advance what animates, and keep the live demo in step with the store.
///
/// A `dt` of zero must change nothing and report whether anything is
/// moving — `App::tick`'s contract, which the frame loop relies on to
/// decide whether to ask for another frame.
fn advance(s: &mut Show, dt_ms: f32) -> bool {
    // **An animated image, at its own rate** (M84/T84.3): the frame is
    // written only when the `Playback` answers a new one, so the tick
    // answers `true` (and the tree is marked) ten times a second for a
    // ten-frame-a-second GIF, and `wake` says when the next is due. A zero
    // `dt` answers `None`: nothing moves and nothing is reported.
    // ANCHOR: image_tick
    if let Demo::Image(d) = &mut s.ui.main.stage.row.demo {
        return match s.store.spin.tick(dt_ms) {
            Some(f) => {
                d.a.frame = f;
                true
            }
            None => false,
        };
    }
    // ANCHOR_END: image_tick
    // The Video demo's frames are messages, not ticks (M84/T84.8).
    if let Demo::Video(_) = &s.ui.main.stage.row.demo {
        return false;
    }
    // A toast's countdown runs on this clock, animated entry or not: a
    // widget has none of its own (M57/B34, `Toast::tick`).
    if let Demo::Toast(d) = &mut s.ui.main.stage.row.demo {
        if d.a.content.t.is_counting() {
            d.a.content.t.tick(dt_ms);
            return true;
        }
    }
    let e = &CATALOG[s.store.selected];
    if !e.animated {
        return false;
    }
    if dt_ms > 0.0 {
        s.store.phase = (s.store.phase + dt_ms / 1400.0).rem_euclid(1.0);
        (e.seed)(s);
        let text = (e.status)(s);
        s.ui.main.header.heading.readout.set_str(&text);
    }
    true
}

/// **When the next frame is due** — `App::wake_in_ms`, through
/// `#[app(wake = wake)]` (M84/T84.3): the Image demo's next frame change
/// while it plays, and nothing otherwise. A paused image, and every other
/// demo, asks for no frame; the per-frame demos answer `true` from
/// `advance` instead.
// ANCHOR: image_wake
fn wake(s: &Show) -> Option<f32> {
    match &s.ui.main.stage.row.demo {
        Demo::Image(_) => s.store.spin.next_change_ms(),
        _ => None,
    }
}
// ANCHOR_END: image_wake

/// Put the selected entry on the stage and bring the header into step.
fn rebuild(s: &mut Show) {
    let e = &CATALOG[s.store.selected];
    // The sidebar's highlight: its bind reads this (`nav.rs`).
    nav::set_selected(s.store.selected);
    s.store.needs_seed = true;
    let h = &mut s.ui.main.header;
    h.crumb.set(format!("{} / {}", e.group, e.name));
    h.heading.title.set(e.name.into());
    h.blurb.set(e.blurb.into());
    (e.seed)(s);
    let text = (e.status)(s);
    s.ui.main.header.heading.readout.set_str(&text);
}

/// One sidebar row: a group heading, or an entry under one.
///
/// **Writes only `&'static str`s and flags** — a bind runs every frame,
/// and a `format!` here would be one allocation per row per frame, the
/// defect M29/T29.5 found in a fixture's bind. The heading rows are
/// computed from `CATALOG`'s order, which `each_group_appears_once` keeps
/// honest.
fn bind_nav(r: &mut NavRow<Store>, i: usize) {
    match catalog::nav_item(i) {
        catalog::NavItem::Heading(g) => {
            r.label = g;
            r.kind = RowKind::Heading;
            r.selected = false;
        }
        catalog::NavItem::Entry(k) => {
            r.label = CATALOG[k].name;
            r.kind = RowKind::Item;
            r.selected = k == nav::selected();
        }
    }
    r.on_click = |s: &mut Store, cx: &EventCtx| {
        if let Some(catalog::NavItem::Entry(k)) = cx.row.map(|r| catalog::nav_item(r as usize)) {
            choose(s, k);
        }
    };
}

/// Put entry `i` on the stage.
///
/// **State only** (M33/T33.4): the header and the stage are on the tree,
/// which a handler cannot reach, so `refresh` rebuilds them on this same
/// pump.
fn choose(s: &mut Store, i: usize) {
    s.selected = i.min(CATALOG.len() - 1);
    // A video left on stage stops: nothing would show its frames.
    s.clip.playing = false;
    s.subs.clear();
    s.select_open = false;
    s.phase = 0.0;
    s.needs_rebuild = true;
}

/// Build the app, already on `entry`, with the `Select` demo's menu
/// `open` or not (it only shows on the `Select` entry).
fn build(entry: usize, open: bool) -> Show {
    let mut app = Show::default();
    app.ui.side.brand.set("lumen-static");
    app.ui.side.sub.set("Widgets");
    // **The whole wiring of the sidebar.** One `fn` for the rows, the
    // count as data.
    app.ui.side.nav = Repeat::new(catalog::nav_len(), NAV_ROW_H, bind_nav);
    app.store.selected = entry.min(CATALOG.len() - 1);
    app.store.select_open = open;
    app.store.reveal = true;
    // **Mid-travel, not zero.** The animated demos take their value from
    // `phase`, and a showcase opened on one of them at phase 0 shows an
    // empty bar and an unlit spinner — which reads as a broken widget
    // rather than as a starting state.
    app.store.phase = 0.35;
    // The Image demo's clock, from the decoded GIF's own delays (M84/T84.3).
    app.store.spin = lumen_static::Playback::new(&catalog::spin().delays_ms);
    rebuild(&mut app);
    app
}

/// **The stack a headless frame of `p` measures and draws with**: the
/// system face, laid out by the program's `Program::shaping` — M81/T81.4
/// (`docs/plan-shaped-paint.md`, second review C1). The window builds its
/// stack the same way (from `Session::shaping`), so `--shot` and `--demo`
/// stay the window's frame, which M57's D5 parity compares them against.
/// Built `FontStack::system()` directly, a `Complex` window's `--demo`
/// would differ from it by construction.
fn stack_of(p: &Program<Show>) -> Option<lumen_static_shell::text::FontStack> {
    lumen_static_shell::text::FontStack::system().map(|f| f.with_shaping(p.shaping))
}

/// **A headless frame as the window draws it** — the application's images
/// (`App::images`, the Video demo's store, M84/T84.8) resolved before the
/// canvas's own, as a session's present does (`FrameSource::images`).
fn render_app(c: &mut lumen_static_shell::software::Canvas, host: &lumen_static_shell::Host<Show>) {
    c.render_within_from(&host.list, [0.0, 0.0, W as f32, H as f32], lumen_static::event::App::images(&host.app));
}

/// One frame, headless — what the tests and the screenshots use.
fn shot(path: &str, entry: usize, open: bool) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::FontMetrics;
    let p = program(entry, open);
    let stack = stack_of(&p);
    let mut host: lumen_static_shell::Host<Show> = p.host();
    let mut metrics = stack.clone().map(FontMetrics::new);
    for _ in 0..2 {
        match metrics.as_mut() {
            Some(m) => host.pump_with(m),
            None => host.pump(),
        };
    }
    let mut c = match stack {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    };
    // The window's images, so the shot draws what the window draws.
    c.images = Some(Box::new(catalog::images()));
    c.clear(palette().page);
    render_app(&mut c, &host);
    // M50/T50.2: the glyph cache on a real screen. A second, warm frame
    // must rasterise nothing and draw nothing uncached
    // (`docs/plan-raster-allocation.md` §5's risk).
    let cold = c.glyph_cache_stats();
    c.clear(palette().page);
    render_app(&mut c, &host);
    let warm = c.glyph_cache_stats();
    println!(
        "glyph cache: first frame {cold:?}; second frame misses {}, uncached {} (bound 0, 0)",
        warm.misses - cold.misses,
        warm.uncached - cold.uncached
    );
    let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
    for p in c.pixels.chunks_exact(4) {
        out.extend_from_slice(&p[..3]);
    }
    std::fs::write(path, out).expect("write");
    println!("{path}: {} ({}), dropdown {}", CATALOG[entry.min(CATALOG.len()-1)].name,
        entry, if open { "open" } else { "closed" });
}

/// The node indices of the `#demo` subtree — **by the table, not by a
/// position.** The helpers below used to take "every node below the
/// stage's top edge", which a full-height sidebar (M56) now also is.
fn demo_span() -> core::ops::Range<usize> {
    let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
    let demo = lumen_static::style::index_of_wid(table, "demo").expect("the stage has no `#demo` node");
    lumen_static::style::subtree_range(table, demo)
}

/// The leaves of the `#demo` subtree — its controls, not the rows that
/// hold them. A size filter alone could not tell them apart once a demo
/// shrank to its content (M56).
fn demo_leaves() -> Vec<usize> {
    let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
    demo_span().filter(|&i| table[i].child_count == 0).collect()
}

/// The frame a popup opens on — M26/T26.0.
///
/// `just profile` is fixed to counter-window's tally and takes no
/// arguments, so M26/T26.11 — whose whole subject is that opening a
/// popup damages the entire window — had no arm at all. This is one: it
/// settles a closed frame, opens the `Select` demo's menu (the showcase's
/// own picker until M56), pumps, and prints the
/// damaged area beside the window's. Today that ratio is 100%, because a
/// command-count change answers `Damage::All`; after T26.11 it must be
/// the popup's rect, and this is the number that says so.
fn profile_popup() {
    let select = CATALOG.iter().position(|e| e.name == "Select").expect("a Select entry");
    let mut host: lumen_static_shell::Host<Show> =
        program(select, false).host();
    let mut metrics = lumen_static_shell::text::FontStack::system()
        .map(lumen_static_shell::text::FontMetrics::new);
    let mut pump = |h: &mut lumen_static_shell::Host<Show>| match metrics.as_mut() {
        Some(m) => {
            h.pump_with(m);
        }
        None => {
            h.pump();
        }
    };
    let viewport = [0.0, 0.0, W as f32, H as f32];
    // `.max(0.0)`: an empty region set sums to a negative zero, which
    // prints as "-0 px" and reads like a measurement error.
    let area = |d: lumen_static::Damage| -> f32 {
        let px: f32 = {
        // `regions` already yields rects inside the viewport; `clip_to`
        // is a different thing — it answers the clipped *bounding* rect
        // as an `Option`, which is what counter-window's headless prints.
        d.regions(viewport).map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum()
        };
        px.max(0.0)
    };
    // **Bank the seed frame before measuring anything**, the way
    // `examples/counter-window`'s headless profile does — "or the damage
    // is the whole viewport". The first version of this arm measured the
    // banked frame and reported the settled frame as 100% damaged, which
    // made it unable to tell a still frame from a popup-open one: an arm
    // that cannot say no (M26/T26.0's own gate).
    pump(&mut host);
    pump(&mut host);
    let _seed = host.take_damage_for_age(1);
    // Now a genuinely still frame: nothing changed since the bank.
    pump(&mut host);
    let settled = host.take_damage_for_age(1);
    let before = host.list.len();

    // What the trigger's handler does; `refresh` puts it on the tree.
    host.app.store.select_open = true;
    host.invalidate_all();
    pump(&mut host);

    let opened = area(host.take_damage_for_age(1));
    let window = W as f32 * H as f32;
    println!(
        "showcase popup frame: damaged {opened:.0} px of {window:.0} ({:.1}% of the window)",
        100.0 * opened / window
    );
    println!(
        "   commands {before} -> {} (a length change is aligned since M26/T26.11; it was `Damage::All`, and this frame was 100%); \
         settled frame damaged {:.0} px",
        host.list.len(),
        area(settled)
    );
}

/// One entry, cropped tight to what the demo actually drew.
///
/// **The crop comes from the laid-out geometry, not from a guess.**
/// Eyeballing coordinates off a full-window screenshot is how a review
/// ends up looking at the wrong 240 px and concluding a widget is
/// broken — or worse, that it is fine.
fn demo_shot(path: &str, entry: usize) {
    let p = program(entry, false);
    // The stack's own mode, printed for M57's D5 parity, which checks it
    // equals the live leg's (M81/T81.6): `none` without a system face.
    let mode = stack_of(&p).map_or("none".to_owned(), |f| format!("{:?}", f.shaping()));
    let (pixels, [x0, y0, x1, y1]) = demo_frame(&p, |_| {});
    let (cw, chh) = (x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1));
    let mut out = format!("P6\n{cw} {chh}\n255\n").into_bytes();
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * W as usize + x) * 4;
            out.extend_from_slice(&pixels[i..i + 3]);
        }
    }
    std::fs::write(path, out).expect("write");
    println!("{path}: {} — {cw}x{chh} at ({x0},{y0}), shaping {mode}", CATALOG[entry].name);
}

/// [`demo_shot`]'s frame: the whole canvas, and the crop `[x0, y0, x1,
/// y1]` around the `#demo` subtree. `prep` runs on the host between the
/// two pumps, once the first has seeded the stage — a test's hook;
/// `demo_shot` passes nothing. Measured and drawn with [`stack_of`]`(p)`,
/// the program's own `Shaping`.
fn demo_frame(p: &Program<Show>, prep: impl FnOnce(&mut lumen_static_shell::Host<Show>)) -> (Vec<u8>, [usize; 4]) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::FontMetrics;
    let stack = stack_of(p);
    let mut host: lumen_static_shell::Host<Show> = p.host();
    let mut metrics = stack.clone().map(FontMetrics::new);
    let mut prep = Some(prep);
    for _ in 0..2 {
        match metrics.as_mut() {
            Some(m) => host.pump_with(m),
            None => host.pump(),
        };
        if let Some(f) = prep.take() {
            f(&mut host);
        }
    }
    let mut c = match stack {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    };
    // The window's images (M84/T84.3), as `shot` hands them.
    c.images = Some(Box::new(catalog::images()));
    c.clear(palette().page);
    render_app(&mut c, &host);

    // **The union of the demo's own subtree** — by the table, not by a
    // size.
    //
    // It was "every node below the header narrower than 300 px", meant
    // to exclude the two `FILL` spacers that centre the row. Any demo
    // *at least* 300 px wide was excluded from its own crop: a
    // `Pagination` and a `DataGrid` both lay out at 320, so `--demo 39`
    // and `--demo 41` rendered the caption text and no widget at all —
    // in a function whose whole purpose is that "a review ends up
    // looking at the wrong 240 px" cannot happen (M27/T27.11, finding
    // 11). The subtree is what the crop always meant.
    let table = <<Show as lumen_static::event::App>::Tree as
        lumen_static::widget::Widget>::SUB_TABLE;
    let demo = table
        .iter()
        .position(|d| d.wid == "demo")
        .expect("the stage has no `#demo` node");
    let span = lumen_static::style::subtree_range(table, demo);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for st in host.states[span.clone()].iter() {
        if st.size.w <= 0.0 || st.size.h <= 0.0 {
            continue;
        }
        x0 = x0.min(st.pos.x);
        y0 = y0.min(st.pos.y);
        x1 = x1.max(st.pos.x + st.size.w);
        y1 = y1.max(st.pos.y + st.size.h);
    }
    let pad = 16.0;
    let (x0, y0) = ((x0 - pad).max(0.0) as usize, (y0 - pad).max(0.0) as usize);
    let (x1, y1) = ((x1 + pad).min(W as f32) as usize, (y1 + pad).min(H as f32) as usize);
    (c.pixels, [x0, y0, x1, y1])
}

/// An entry with its `which`-th control **held down**, rendered while
/// the pointer is still on it.
///
/// A press is a state that exists between two events, so a screenshot
/// taken after the release shows nothing. This posts the down, pumps,
/// and renders — which is the only way to see what a user sees mid-click.
fn press_shot(path: &str, entry: usize, which: usize) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    let mut host: lumen_static_shell::Host<Show> =
        program(entry, false).host();
    let mut metrics = FontStack::system().map(FontMetrics::new);
    let mut pump = |h: &mut lumen_static_shell::Host<Show>| match metrics.as_mut() {
        Some(m) => {
            h.pump_with(m);
        }
        None => {
            h.pump();
        }
    };
    pump(&mut host);
    pump(&mut host);

    let leaves = demo_leaves();
    let mut targets: Vec<(f32, f32, usize)> = host
        .states
        .iter()
        .enumerate()
        .filter(|(i, st)| leaves.contains(i) && st.size.w > 0.0 && st.size.h > 0.0)
        .map(|(i, st)| (st.pos.x, st.pos.y, i))
        .collect();
    targets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let Some(&(_, _, node)) = targets.get(which) else {
        println!("{path}: no control {which}");
        return;
    };
    let st = host.states[node];
    let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
    host.post(Event::down(x, y));
    pump(&mut host);

    let mut c = match FontStack::system() {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    };
    c.clear(palette().page);
    lumen_static::Renderer::render(&mut c, &host.list, W, H);
    // Centred on the control that is held, not on the stage's top —
    // the first version cropped y 150..270 while the buttons sit at 423
    // and produced three identical blank images.
    let (cw, chh) = (460usize, 72usize);
    let x0 = ((x - cw as f32 / 2.0).max(0.0) as usize).min(W as usize - cw);
    let y0 = ((y - chh as f32 / 2.0).max(0.0) as usize).min(H as usize - chh);
    let mut out = format!("P6\n{cw} {chh}\n255\n").into_bytes();
    for yy in y0..(y0 + chh).min(H as usize) {
        for xx in x0..(x0 + cw).min(W as usize) {
            let i = (yy * W as usize + xx) * 4;
            out.extend_from_slice(&c.pixels[i..i + 3]);
        }
    }
    std::fs::write(path, out).expect("write");
    println!(
        "{path}: {} control {which} (node {node}) held at ({x:.0},{y:.0}), pressed={}",
        CATALOG[entry].name, host.states[node].pressed
    );
}

/// The whole click sequence a user performs, rendered frame by frame:
/// press the first button, release it, then press the second.
///
/// **This is the only way to see a press.** It exists between two events
/// and a screenshot taken after the release shows the resting frame —
/// which is why "nothing happens when I click" is a report that a
/// still image cannot confirm or deny.
fn seq_shot(stem: String) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    let entry = CATALOG.iter().position(|e| e.name == "Button").unwrap();
    let mut host: lumen_static_shell::Host<Show> =
        program(entry, false).host();
    let mut metrics = FontStack::system().map(FontMetrics::new);
    let mut pump = |h: &mut lumen_static_shell::Host<Show>| match metrics.as_mut() {
        Some(m) => {
            h.pump_with(m);
        }
        None => {
            h.pump();
        }
    };
    pump(&mut host);
    pump(&mut host);
    let btns: Vec<usize> = demo_leaves()
        .into_iter()
        .filter(|&i| {
            let s = &host.states[i];
            s.size.w > 40.0 && s.size.h > 20.0
        })
        .collect();
    let at = |h: &lumen_static_shell::Host<Show>, i: usize| {
        let s = &h.states[i];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    };
    let (x0c, y0c) = at(&host, btns[0]);
    let (x1c, y1c) = at(&host, btns[1]);
    let steps: [(&str, Option<Event>); 4] = [
        ("0-rest", None),
        ("1-down-a", Some(Event::down(x0c, y0c))),
        ("2-up-a", Some(Event::up(x0c, y0c))),
        ("3-down-b", Some(Event::down(x1c, y1c))),
    ];
    for (tag, ev) in steps {
        if let Some(e) = ev {
            host.post(e);
            pump(&mut host);
        }
        let mut c = match FontStack::system() {
            Some(f) => Canvas::with_font(W, H, f),
            None => Canvas::new(W, H),
        };
        c.clear(palette().page);
        lumen_static::Renderer::render(&mut c, &host.list, W, H);
        let (cw, chh) = (400usize, 64usize);
        let x0 = ((x0c - 60.0).max(0.0) as usize).min(W as usize - cw);
        let y0 = ((y0c - chh as f32 / 2.0).max(0.0) as usize).min(H as usize - chh);
        let mut out = format!("P6\n{cw} {chh}\n255\n").into_bytes();
        for yy in y0..y0 + chh {
            for xx in x0..x0 + cw {
                let i = (yy * W as usize + xx) * 4;
                out.extend_from_slice(&c.pixels[i..i + 3]);
            }
        }
        std::fs::write(format!("{stem}-{tag}.ppm"), out).expect("write");
        println!("{stem}-{tag}: presses {}", host.app.store.presses);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--seq") {
        seq_shot(args.get(1).cloned().unwrap_or_else(|| "seq".into()));
        return;
    }
    if args.first().map(String::as_str) == Some("--press") {
        let entry: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(6);
        let which: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let path = args.get(3).cloned().unwrap_or_else(|| "press.ppm".into());
        press_shot(&path, entry, which);
        return;
    }
    // The catalogue this binary was compiled with, one `name<TAB>animated`
    // line per entry in index order: what `--demo N` and a bare `N` mean.
    // The drives (`scripts/drive/parity.py`, `run.py`, `a11y_drive.sh`,
    // `gpu_parity.sh`) take their entry list from here, never from
    // `catalog.rs`'s text: an entry behind `#[cfg(feature = ..)]` is in the
    // source and not in a build without the feature, and counting the
    // source shifted every later index by one (M83/T83.6's `ShaderView`;
    // `docs/plan-gpu-extension.md`, "What building found").
    if args.first().map(String::as_str) == Some("--entries") {
        for e in CATALOG {
            println!("{}\t{}", e.name, e.animated);
        }
        return;
    }
    if args.first().map(String::as_str) == Some("--demo") {
        let entry: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let path = args.get(2).cloned().unwrap_or_else(|| "demo.ppm".into());
        demo_shot(&path, entry.min(CATALOG.len() - 1));
        return;
    }
    if args.first().map(String::as_str) == Some("--profile") {
        profile_popup();
        return;
    }
    if args.first().map(String::as_str) == Some("--shot") {
        let path = args.get(1).cloned().unwrap_or_else(|| "showcase.ppm".into());
        let entry = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let open = args.get(3).map(|s| s == "open").unwrap_or(false);
        shot(&path, entry, open);
        return;
    }
    // A bare number opens the window on that entry, so a review can
    // look at one widget without clicking through the picker.
    let start: usize = args.first().and_then(|a| a.parse::<usize>().ok()).unwrap_or(0);
    // ANCHOR: accessible
    // A screen reader can see this window: the manifest asks for the
    // window crate's `a11y` feature, which builds the platform adapter,
    // and `accessible` is the run-time switch -- `false` builds none.
    let mut program = program(start.min(CATALOG.len() - 1), false)
        .title("showcase")
        .accessible(true);
    // ANCHOR_END: accessible
    // Built `--features agent`, the window can be steered from outside:
    // `input.click` on the picker, `ui.screenshot` of the result.
    // `LUMEN_AGENT_ADDR` overrides the address, read by the window
    // (M48/T48.11).
    if let Some(addr) = agent_addr() {
        program = program.agent(addr);
    }
    // M55/T55.4: the drive's `Everything` leg is this application's
    // choice, made here -- not a feature of the window crate.
    #[cfg(feature = "everything")]
    program.invalidation::<lumen_static::Everything>().run();
    #[cfg(not(feature = "everything"))]
    program.run();
}

/// **The program, described once** — M32 (`docs/plan-program.md`).
///
/// The window, every `--shot`/`--demo`/`--press` frame, the profile and
/// the tests are all built by `program(..).host()`, so a shot is the
/// window's frame. The boot captures `entry` and `open`: showcase opens
/// on an entry read from argv, which is why a boot may capture.
fn program(entry: usize, open: bool) -> Program<Show> {
    let p = application(move || build(entry, open))
        .size(W, H)
        .page(palette().page)
        .palette(palette())
        // M82/T82.3: the Card demo's ground, by index (`#d_card_a`'s
        // `background_fill(1)`).
        .fills(&FILLS)
        // **Keyed on the `Label` demo's switches** (M80/T80.2, T80.3): the
        // sheet is `stylesheet()` plus, while a switch is on, a 1.5× line
        // height or a one-line ellipsis on the demo's label — style fields
        // restyled at run time, which is what `Program::style_by` is for.
        .style_by(|s: &Show| (s.store.spaced, s.store.ellipsis), sheet_for)
        .shaping(SHAPING)
        // M84/T84.3: the Image demo's GIF, decoded once (`catalog::SPIN`).
        .images(catalog::images());
    // M83/T83.6: the `ShaderView` demo's shader, with the `shader` feature.
    #[cfg(feature = "shader")]
    let p = p.shaders(&catalog::SHADERS);
    p
}

/// **The fill table** — M82/T82.3 (`docs/plan-paint-vocabulary.md`).
/// Entry 0 is the Card demo's ground: a radial from the stage's own
/// `raised` at the centre out to `track` at the farthest corner, both
/// named by role, so it follows the palette as the rest of the chrome
/// does (`Host::set_palette` re-resolves it). The card's background is
/// transparent, because a fill paints *under* the flat background, and
/// its corners round the fill by the card's radius.
// ANCHOR: card_fill
const FILLS: [lumen_static::style::Fill; 1] = [lumen_static::style::Fill::new(
    lumen_static::display::Stops::two(Color(255, 255, 255, 255), Color(0xd5, 0xdb, 0xe4, 255)),
    lumen_static::display::GradShape::Radial { center: [0.5, 0.5], radius: 1.0 },
)
.role(0, lumen_static::style::Role::Raised)
.role(1, lumen_static::style::Role::Track)];
// ANCHOR_END: card_fill

/// How the showcase lays text out: `Complex` when built with its
/// `shaping` feature, `Basic` otherwise (M81/T81.4). The window, `--shot`
/// and `--demo` all follow it through [`program`].
#[cfg(feature = "shaping")]
const SHAPING: lumen_static_shell::text::Shaping = lumen_static_shell::text::Shaping::Complex;
#[cfg(not(feature = "shaping"))]
const SHAPING: lumen_static_shell::text::Shaping = lumen_static_shell::text::Shaping::Basic;

/// The program's sheet for the `Label` demo's switches (M80/T80.2, T80.3).
// ANCHOR: line_height
fn sheet_for((spaced, ellipsis): (bool, bool)) -> Vec<StyleRule> {
    use lumen_static::style::{Patch, Selector};
    let mut rules = stylesheet();
    if spaced {
        // A ratio of the font size; the cascade keeps it in sixteenths.
        rules.push(StyleRule::new(Selector::Id("d_label_a"), Patch::new().line_height(1.5)));
    }
    // ANCHOR: text_overflow
    if ellipsis {
        // One line, cut at a grapheme boundary so the prefix and `…` fit.
        let cut = lumen_static::TextOverflow::Ellipsis;
        rules.push(StyleRule::new(Selector::Id("d_label_a"), Patch::new().overflow(cut)));
    }
    // ANCHOR_END: text_overflow
    rules
}
// ANCHOR_END: line_height

/// Where to serve the agent endpoint, or `None` without the feature.
#[cfg(feature = "agent")]
fn agent_addr() -> Option<String> {
    Some("127.0.0.1:7654".into())
}

#[cfg(not(feature = "agent"))]
fn agent_addr() -> Option<String> {
    None
}

/// The `Select` demo's row height.
///
/// `Repeat` is uniform-extent, so this single number decides both where
/// a row is drawn and which row a click at `y` lands on — they cannot
/// disagree, which is the half the eighteen-field panel had to keep in
/// step by hand.
const ROW_H: f32 = 30.0;

/// A control's own label — the text beside a checkbox or a switch.
fn lbl() -> lumen_static::style::Patch {
    use lumen_static::style::{Patch, Role};
    Patch::new().color_role(Role::Text).font_size(15.0)
}



/// The shape all three button variants share; only the colours differ.
///
/// **`width` and `height` are named here on purpose.** The menu's rows
/// are styled by `Kind("button")` — the only selector that reaches an
/// arena's template — and that rule sets a `FILL` width and a fixed
/// height. An id beats a kind, so these three win; leaving either out
/// let the row rule through and the demo buttons came out full-width.
fn btn() -> lumen_static::style::Patch {
    use lumen_static::style::Patch;
    Patch::new()
        // **The hover shade, restated.** The menu rows' `Kind("button")`
        // rule below sets `hover_dim(0.0)`, and an id rule beats a kind
        // only for the fields it sets -- so every id-addressed button here
        // inherited the rows' 0.0 and showed no hover at all. Found by
        // M42/T42.6, when the drive's "hovering is visible" check had to
        // see the button itself change (`Button`'s type default is 0.07).
        .hover_dim(0.07)
        .width(Length::Shrink)
        .height(Length::Shrink)
        .radius(8.0)
        .font_size(15.0)
        .font_weight(600.0)
        .padding(Edges { left: 18.0, right: 18.0, top: 10.0, bottom: 10.0 })
}

/// The one-line fields' look: `#d_textinput_a` and, since M81/T81.6,
/// `#d_textinput_rtl`.
fn field() -> lumen_static::style::Patch {
    use lumen_static::style::{Edges, Patch, Role};
    Patch::new()
        .width(Length::Fixed(260.0))
        .height(Length::Fixed(38.0))
        // Lumen's field is a pale fill with a hairline border.
        // **Both halves, since M27/T27.1.** Until then `Style`
        // had no border and `Cmd` no stroked rect, so the ground
        // was the only thing separating a field from the page —
        // recorded as a gap in `docs/plan-widgets-gallery.md`
        // §3c, where our field was put beside Lumen's and read
        // as text on the page.
        .background_role(Role::Raised)
        .color_role(Role::Text)
        .border_width(1)
        .border_color_role(Role::Muted)
        .radius(8.0)
        .font_size(15.0)
        .padding(Edges { left: 12.0, right: 12.0, top: 9.0, bottom: 9.0 })
}

fn stylesheet() -> Vec<StyleRule> {
    stylesheet_with(chrome())
}

/// The sheet, with its role-less colours taken from `c` — so a test can
/// build the dark sheet without the `LUMEN_PALETTE` environment variable
/// (M57/A8's demo contrast gate).
fn stylesheet_with(c: Chrome) -> Vec<StyleRule> {
    use lumen_static::style::{Patch, Role, Selector};
    let mut rules = vec![
        // ---- the chrome (M56, `docs/plan-showcase-design.md`) -------
        //
        // A 4/8 px spacing grid, a 12/14/16/24 type scale, radii of 8
        // for controls and rows, 12 for popups and 16 for the stage card.
        // The window is a row, and a row's height shrinks to its content
        // unless told otherwise: without `FILL` here every full-height
        // child — sidebar, rule, page — laid out 0 px tall.
        StyleRule::new(
            Selector::Kind("Screen"),
            Patch::new().background_role(Role::Page).width(Length::FILL).height(Length::FILL),
        ),
        // The sidebar: a surface column, its rows inset by `NavRow`.
        StyleRule::new(
            Selector::Id("sidebar"),
            Patch::new()
                .width(Length::Fixed(248.0))
                .height(Length::FILL)
                .background_role(Role::Surface)
                .padding(Edges { left: 0.0, right: 0.0, top: 24.0, bottom: 12.0 }),
        ),
        StyleRule::new(
            Selector::Id("brand"),
            Patch::new()
                .color_role(Role::Text)
                .font_size(16.0)
                .font_weight(700.0)
                .padding(Edges { left: 20.0, right: 20.0, top: 0.0, bottom: 2.0 }),
        ),
        StyleRule::new(
            Selector::Id("brand_sub"),
            Patch::new()
                .color_role(Role::Muted)
                .font_size(12.0)
                .font_weight(600.0)
                .padding(Edges { left: 20.0, right: 20.0, top: 0.0, bottom: 16.0 }),
        ),
        StyleRule::new(Selector::Id("nav"), Patch::new().width(Length::FILL).height(Length::FILL)),
        // **Every sidebar row, by kind** — a `Repeat`'s rows are arena
        // nodes, which a kind reaches and an id cannot. The accent is the
        // border colour: `NavRow` paints its selected tint and bar from
        // it and never strokes the border (`nav.rs`).
        StyleRule::new(
            Selector::Kind("navrow"),
            Patch::new()
                .width(Length::FILL)
                .height(Length::Fixed(NAV_ROW_H))
                .background_role(Role::Surface)
                .color_role(Role::Text)
                .border_color_role(Role::Accent)
                .font_size(14.0)
                .radius(8.0),
        ),
        StyleRule::new(
            Selector::Id("rule"),
            Patch::new().width(Length::Fixed(1.0)).height(Length::FILL).background(c.hairline),
        ),
        // The page: the header on the page colour, then the stage card.
        StyleRule::new(
            Selector::Id("main"),
            Patch::new()
                .width(Length::FILL)
                .height(Length::FILL)
                .padding(Edges { left: 32.0, right: 32.0, top: 28.0, bottom: 32.0 }),
        ),
        StyleRule::new(Selector::Id("header"), Patch::new().width(Length::FILL)),
        StyleRule::new(
            Selector::Id("crumb"),
            Patch::new().color_role(Role::Muted).font_size(12.0).font_weight(600.0),
        ),
        StyleRule::new(Selector::Id("title"), Patch::new().color_role(Role::Text).font_size(24.0).font_weight(700.0)),
        StyleRule::new(Selector::Id("tgap"), Patch::new().width(Length::FILL)),
        // The live readout as a chip at the end of the title's line.
        StyleRule::new(
            Selector::Id("readout"),
            Patch::new()
                .color(c.accent_text)
                .background(c.chip)
                .radius(12.0)
                .font_size(12.0)
                .font_weight(600.0)
                .padding(Edges { left: 10.0, right: 10.0, top: 4.0, bottom: 4.0 }),
        ),
        StyleRule::new(Selector::Id("blurb"), Patch::new().color_role(Role::Muted).font_size(14.0)),
        // The stage: a surface card, the demo centred in what is left.
        StyleRule::new(
            Selector::Id("stage"),
            Patch::new()
                .width(Length::FILL)
                .height(Length::FILL)
                .background_role(Role::Surface)
                .border_width(1)
                .border_color(c.hairline)
                .radius(16.0)
                .shadow(6.0)
                .padding(Edges::all(32.0)),
        ),
        StyleRule::new(Selector::Id("st"), Patch::new().height(Length::FILL)),
        StyleRule::new(Selector::Id("sb"), Patch::new().height(Length::FILL)),
        // **The demo takes its natural width; the spacers centre it** —
        // with every demo struct `Shrink` too (the loop at the end).
        StyleRule::new(Selector::Id("demo"), Patch::new().width(Length::Shrink)),
        StyleRule::new(Selector::Id("sl"), Patch::new().width(Length::FILL)),
        StyleRule::new(Selector::Id("sr"), Patch::new().width(Length::FILL)),
        // The `Select` demo's parts, by the ids `Select` stamps. A
        // composite is more than one node, and the derive only names a
        // *field* — so a widget that is several nodes has to publish its
        // own ids or nothing can reach inside it, not a stylesheet and
        // not the agent. `select::PART_*` are those names.
        StyleRule::new(
            Selector::Id(lumen_static_widgets::select::PART_TRIGGER),
            Patch::new()
                .width(Length::Fixed(220.0))
                .background_role(Role::Raised)
                .color_role(Role::Text)
                .radius(8.0)
                .font_size(15.0)
                .padding(Edges { left: 14.0, right: 14.0, top: 9.0, bottom: 9.0 }),
        ),
        // Its menu. A `Popup` anchors itself on its parent, so the sheet
        // only says how big: six rows, the whole list.
        StyleRule::new(
            Selector::Id(lumen_static_widgets::select::PART_MENU),
            Patch::new()
                .width(Length::Fixed(220.0))
                .height(Length::Fixed(6.0 * ROW_H))
                .background_role(Role::Surface)
                .border_width(1)
                .border_color(c.hairline)
                .radius(12.0)
                .shadow(12.0),
        ),
        // ---- the demos -------------------------------------------
        //
        // **A widget's `defaults()` is a floor, not a look.** It is what
        // makes a control visible with no stylesheet at all — the
        // contract `Toggle`'s module note records — and for a badge or a
        // switch that is already the whole picture. For a button and a
        // field it is not: the core ships them plain on purpose, because
        // a toolkit that bakes in a house style is a toolkit you fight.
        // A showcase has to supply the style it wants to show, and the
        // first version of this file supplied none — so a `TextInput`
        // rendered as its text and nothing else, which is to say as a
        // `Label`.
        StyleRule::new(
            Selector::Id("d_button_a"),
            btn().background_role(Role::Accent).color_role(Role::OnAccent),
        ),
        StyleRule::new(
            Selector::Id("d_button_b"),
            // An edge, because its ground is the card's own: white on
            // white until M56/T56.3 gave it one.
            btn().background_role(Role::Surface).color_role(Role::Text).border_width(1).border_color(c.outline),
        ),
        // **Ghost has a ground, faint as it is.** `pressed_dim` and
        // `hover_dim` *scale* a background; they cannot invent one. A
        // fully transparent button therefore shows no press at all —
        // which is a third of the reported "clicking does nothing".
        StyleRule::new(
            Selector::Id("d_button_c"),
            btn().background(c.ghost).color(c.accent_text),
        ),
        // ---- M27/T27.4's three -------------------------------------
        // The card is sized so the stage can show it whole; its default
        // width is FILL, which is right for a card in a column and wrong
        // for one on a centring stage.
        // M82/T82.3: and its ground is the program's fill 1 -- a rounded
        // radial (`FILLS`), under a transparent background, with no drop
        // shadow so its rounded corners show the stage behind them.
        StyleRule::new(
            Selector::Id("d_card_a"),
            Patch::new()
                .width(Length::Fixed(280.0))
                .background(Color(0, 0, 0, 0))
                .shadow(0.0)
                .background_fill(1),
        ),
        StyleRule::new(Selector::Id("d_card_title"), Patch::new().font_weight(600.0)),
        // **Buttons need a ground.** `pressed_dim` and `hover_dim` scale
        // a background; they cannot invent one, and a fully transparent
        // button shows no press at all — a third of the showcase's
        // "clicking does nothing" (`docs/plan-widgets-gallery.md` §3b).
        StyleRule::new(Selector::Id("d_card_go"), btn().background_role(Role::Accent)
            .color_role(Role::OnAccent)),
        // M84/T84.3: the Image demo's pause button, the same ground, and
        // a fixed width: its label flips between "Pause" and "Play", and a
        // shrinking button would re-centre the row and move the image.
        StyleRule::new(Selector::Id("d_image_pause"), btn().background_role(Role::Accent)
            .color_role(Role::OnAccent).width(Length::Fixed(96.0))),
        StyleRule::new(
            Selector::Id(lumen_static_widgets::surface::PART_CONTENT),
            btn().background_role(Role::Accent).color_role(Role::OnAccent),
        ),
        // The field: a ground, a rounded edge, room for a caret.
        StyleRule::new(Selector::Id("d_textinput_a"), field()),
        // The right-to-left field (M81/T81.6), the same ground.
        StyleRule::new(Selector::Id("d_textinput_rtl"), field()),
        // The multi-line field, the same ground as the text fields below,
        // three lines tall (M45/T45.7). `Raised`, as theirs is since
        // M57/A8: `Control` put its text at 3.84:1 in the dark palette.
        StyleRule::new(
            Selector::Id("d_textarea_a"),
            Patch::new()
                .width(Length::Fixed(260.0))
                .height(Length::Fixed(96.0))
                .background_role(Role::Raised)
                .color_role(Role::Text)
                .border_width(1)
                .border_color_role(Role::Muted)
                .radius(8.0)
                .font_size(15.0)
                .padding(Edges { left: 12.0, right: 12.0, top: 9.0, bottom: 9.0 }),
        ),
        // The ones whose width is the demo: a bar, a divider, a
        // skeleton and a slider all fill whatever they are given, and on
        // an empty stage that is the whole window.
        // The toggles' labels, which Lumen's controls carry and ours
        // do not: `Toggle` has no text of its own, so the demo pairs it
        // with a `Label` — which is also how an application writes it.
        // **A class, not seven ids** (M27/T27.3). Every labelled pair is
        // its own type so its control has a unique address; the labels
        // all want the same look, which is what a class is for and what
        // an id is not.
        StyleRule::new(Selector::Kind("labelled"), lbl()),
        StyleRule::new(Selector::Id("d_slider_a"), Patch::new().width(Length::Fixed(280.0))),
        // M44/T44.10: a view shorter than its twelve rows, so it scrolls.
        StyleRule::new(
            Selector::Id("d_scrollview_a"),
            Patch::new().width(Length::Fixed(260.0)).height(Length::Fixed(180.0)),
        ),

        // The same, for M27/T27.5's two that fill: a range and a search
        // box are both as wide as they are given, and on a centring
        // stage that is nothing at all.
        StyleRule::new(Selector::Id("d_range_a"), Patch::new().width(Length::Fixed(280.0))),
        StyleRule::new(Selector::Id("d_search_a"), Patch::new().width(Length::Fixed(320.0))),
        // M27/T27.9's three. A chooser's trigger is a `TextInput` and
        // ships plain, like every other field here — the sheet is what
        // says it is a control. `Kind`, because all three share one
        // part id and styling them apart is not the point.
        // **A `Kind` rule, not the part id.** Three choosers carry the
        // same part id, so a rule keyed on it styles all three — which
        // is what is wanted here, and is also why `Selector::Id` is the
        // wrong tool: an id is an address and these are not unique. The
        // trigger is a `TextInput`, so its kind reaches exactly them
        // plus the showcase's own field, which wants the same look.
        StyleRule::new(
            Selector::Kind("textinput"),
            Patch::new()
                .width(Length::Fixed(220.0))
                .height(Length::Fixed(36.0))
                .background_role(Role::Raised)
                .color_role(Role::Text)
                .border_width(1)
                .border_color_role(Role::Muted)
                .radius(8.0)
                .padding(Edges { left: 12.0, right: 12.0, top: 8.0, bottom: 8.0 }),
        ),
        // M27/T27.8's four. A wrap needs a box narrow enough to wrap in,
        // a grid needs one wide enough to show that it does *not*, and a
        // split pane is as big as it is given.
        // Centred lines (M57/C9): a flow reads `align` for its lines since
        // A19, and a left-packed wrap sat 31 px off the card's centre.
        StyleRule::new(
            Selector::Kind("DWrap"),
            Patch::new().width(Length::Fixed(260.0)).align(lumen_static::style::Align::Center),
        ),
        // Tall enough for any one section open, so opening one does not
        // re-centre the column and move the header out from under the
        // pointer that pressed it (M57/C7: 15 px up, measured).
        StyleRule::new(Selector::Kind("DAccordion"), Patch::new().height(Length::Fixed(200.0))),
        StyleRule::new(Selector::Kind("DGrid"), Patch::new().width(Length::Fixed(420.0))),
        // M85/T85.1: `flexible(1)` beside `fill` in a 300 px row, the case
        // `crates/lumen-static/tests/flexible.rs` gates at 80/220.
        StyleRule::new(Selector::Kind("DLayout"), Patch::new().width(Length::Fixed(300.0))),
        StyleRule::new(
            Selector::Id("d_layout_flex"),
            Patch::new().width(Length::Flexible(1)).height(Length::Fixed(32.0)).color_role(Role::Accent),
        ),
        StyleRule::new(
            Selector::Id("d_layout_fill"),
            Patch::new().width(Length::FILL).height(Length::Fixed(32.0)).color_role(Role::Muted),
        ),
        // M85/T85.2: the Pin demo's box, raised so its edges show, and
        // its button on the accent ground the Card's has.
        StyleRule::new(
            Selector::Kind("DPin"),
            Patch::new()
                .width(Length::Fixed(240.0))
                .height(Length::Fixed(120.0))
                .background_role(Role::Raised)
                .radius(8.0),
        ),
        // The pinned body shrinks to its button, so the offset is the
        // button's and not a FILL row's.
        StyleRule::new(Selector::Kind("PinBody"), Patch::new().width(Length::Shrink)),
        StyleRule::new(Selector::Id("d_pin_go"), btn().background_role(Role::Accent)
            .color_role(Role::OnAccent)),
        // M85/T85.4: the Pop demo's view, as short as the ScrollView
        // demo's, and its placeholder an image-sized raised box.
        StyleRule::new(
            Selector::Id("d_pop_a"),
            Patch::new().width(Length::Fixed(260.0)).height(Length::Fixed(180.0)),
        ),
        StyleRule::new(
            Selector::Id("d_pop_img"),
            Patch::new()
                .width(Length::Fixed(220.0))
                .height(Length::Fixed(120.0))
                .background_role(Role::Raised)
                .radius(8.0),
        ),
        StyleRule::new(
            Selector::Id("d_split_a"),
            Patch::new().width(Length::Fixed(360.0)).height(Length::Fixed(140.0)),
        ),
        StyleRule::new(Selector::Kind("DAccordion"), Patch::new().width(Length::Fixed(320.0))),
        // M27/T27.7's three. **Classes, not ids.** The three bodies are
        // three types (one per veil, so no id is duplicated — `W1011`),
        // and all three want the same look, which is what a class is for
        // and what an id is not. The same split `pair-label` made in
        // T27.3.
        StyleRule::new(
            Selector::Class("veil-body"),
            Patch::new()
                .background_role(Role::Surface)
                .padding(Edges::all(20.0))
                .radius(12.0)
                .shadow(12.0),
        ),
        // A modal is sized to its content (M57/B41): its body is a
        // derived column, FILL like any, which filled the veil's 80% cap.
        StyleRule::new(
            Selector::Kind("ModalBody"),
            Patch::new().width(Length::Shrink).height(Length::Shrink),
        ),
        StyleRule::new(Selector::Class("veil-title"), Patch::new().font_weight(600.0)),
        StyleRule::new(
            Selector::Class("veil-go"),
            btn().background_role(Role::Accent).color_role(Role::OnAccent),
        ),
        // M27/T27.6's five. A navigation strip is as wide as the screen
        // it navigates; on a centring stage that is nothing, so each one
        // is given a plausible screen's worth.
        StyleRule::new(Selector::Id("d_menu_a"), Patch::new().height(Length::Fixed(40.0))),
        // M27/T27.10's three. A grid fills whatever it is given and a
        // centring stage gives nothing.
        // **No width.** It was `Fixed(380.0)`, and the laid-out box was
        // 320 every time: the stage's card is narrower, so the rule
        // changed nothing and the header's two-column arithmetic was
        // tuned against a width that does not occur (M27/T27.11,
        // finding 12). The sheet lint cannot catch this one — the rule
        // *matches*, and the node *does* read `WIDTH`.
        StyleRule::new(Selector::Id("d_datagrid_a"), Patch::new().height(Length::Fixed(260.0))),
        // A tree is a *pane*: a file tree lives in a sidebar with a size,
        // and it is the sidebar that stops a collapse from moving the
        // rest of the window. Without one the stage centres a
        // shrink-wrapped tree, so folding two rows away moves the demo
        // 26 px up and repaints everything below the header — 11.94x the
        // stage, measured (M27/T27.10).
        StyleRule::new(Selector::Id("d_tree_a"), Patch::new().height(Length::Fixed(200.0))),
        // A pane grid fills what it is given, and what the stage gives
        // it is the whole column — which squeezed the label then under
        // it to zero height and `W1001` said so, on the first drive. A
        // pane grid in an application fills a window; in a catalogue of
        // fifty-four demos it gets a pane's worth.
        StyleRule::new(Selector::Id("d_panegrid_a"), Patch::new().height(Length::Fixed(260.0))),
        StyleRule::new(Selector::Id("d_tabs_a"), Patch::new().width(Length::Fixed(420.0))),
        StyleRule::new(Selector::Id("d_bottomnav_a"), Patch::new().width(Length::Fixed(420.0))),
        StyleRule::new(Selector::Id("d_appbar_a"), Patch::new().width(Length::Fixed(420.0))),
        // A rail is the other way round: its width is its own, its
        // height is the screen's.
        StyleRule::new(Selector::Id("d_rail_a"), Patch::new().height(Length::Fixed(260.0))),
        StyleRule::new(
            Selector::Id("d_pull_a"),
            Patch::new().width(Length::Fixed(280.0)).height(Length::Fixed(120.0)),
        ),
        // **A pull area needs a ground.** At rest the widget draws only
        // its content, so the demo read as a line of text; M56's stricter
        // stage filter said so (the old one had credited it with the
        // header's hairline). The demo's own container is the area.
        // A raised ground under the pull area and the split pane's
        // halves, following the palette. They were a light panel "in both
        // palettes" because their content is bare `Label`s and a label's
        // text was dark whatever the palette; since M57/A8 a label is the
        // palette's text, and the light panel made it light-on-light in
        // dark (`every_demo_text_reads_in_both_palettes`). The four
        // `Kind` rules that painted icon, rich text, markdown and text
        // area in the text role went the same way: that is their default.
        StyleRule::new(
            Selector::Kind("DPullToRefresh"),
            // Padded (M57/C8): the label sat jammed in the corner.
            Patch::new()
                .width(Length::Shrink)
                .background_role(Role::Raised)
                .radius(12.0)
                .padding(Edges::all(16.0)),
        ),
        StyleRule::new(
            Selector::Kind("DSplitPane"),
            Patch::new().width(Length::Shrink).background_role(Role::Raised).radius(8.0),
        ),
        StyleRule::new(Selector::Id("d_progress_a"), Patch::new().width(Length::Fixed(280.0))),
        StyleRule::new(Selector::Id("d_divider_a"), Patch::new().width(Length::Fixed(280.0))),
        StyleRule::new(
            Selector::Id("d_skeleton_a"),
            Patch::new().width(Length::Fixed(180.0)).height(Length::Fixed(18.0)),
        ),
        StyleRule::new(
            Selector::Id("d_skeleton_b"),
            Patch::new().width(Length::Fixed(120.0)).height(Length::Fixed(18.0)),
        ),
        // **No rule for the menu rows** — M57/C1. The showcase styled
        // `Select`'s rows with `Kind("button")`, the only selector that
        // reaches arena rows, and its `hover_dim(0.0)` switched hover off on
        // every demo button. A row is its own kind (`selectrow`) now, whose
        // look is the widget's and whose height is the repeater's row
        // extent. A `Kind("selectrow")` rule was tried in its place, and the
        // sheet lint reported it unmatched on every entry but Select's:
        // arena rows exist only while their demo is on the stage.
    ];
    // **Every demo shrinks to what it draws** (M56). A derived struct is
    // `FILL` wide by default, so a demo's own row spanned whatever share
    // the stage gave it and its content sat at the share's left edge;
    // `#demo` could only be centred by being one `FILL` third of the
    // stage row, which fit three buttons at the old 960 px window by
    // luck and wrapped "Ghost" on M56's card. With each demo `Shrink`,
    // the stage measures its real width and the two spacers centre it. A
    // demo that already has a rule of its own keeps it: a later rule of
    // the same specificity would win.
    // **Demos whose widget fills what it is given** get a width of their
    // own: inside a `Shrink` demo a `FILL` widget has nothing to fill and
    // lays out 0 px wide — which is what the one-third share above used
    // to paper over. The veils cover the window whatever their node's
    // box, so theirs simply fills.
    for (kind, w) in [
        // A paragraph that wraps, so a line height shows (M80/T80.2).
        ("DLabel", Length::Fixed(320.0)),
        // A paragraph that wraps, so a drag can cross a line (M80/T80.4).
        ("DSelectable", Length::Fixed(320.0)),
        ("DRichText", Length::Fixed(420.0)),
        ("DMarkdown", Length::Fixed(420.0)),
        ("DMdLinks", Length::Fixed(420.0)),
        ("DRichTextEditor", Length::Fixed(420.0)),
        ("DPaintBox", Length::Fixed(320.0)),
        ("DPaneGrid", Length::Fixed(480.0)),
        ("DDataGrid", Length::Fixed(560.0)),
        ("DModal", Length::FILL),
        ("DSheet", Length::FILL),
        ("DDrawer", Length::FILL),
    ] {
        rules.push(StyleRule::new(Selector::Kind(kind), Patch::new().width(w)));
    }
    // A toggle and its label were a derived row of their own, `FILL`
    // like any derived struct, and needed a rule each to shrink. A
    // `Labelled` shrinks by default (M57/B11).
    // The floating toast's slot shrinks to the toast: the popup centres
    // its content, and a FILL slot would put the toast at its left (B34).
    rules.push(StyleRule::new(Selector::Kind("ToastSlot"), Patch::new().width(Length::Shrink)));
    for kind in stage::DEMO_KINDS {
        let styled = rules.iter().any(|r| matches!(r.selector, Selector::Kind(k) if k == *kind));
        if !styled {
            rules.push(StyleRule::new(Selector::Kind(kind), Patch::new().width(Length::Shrink)));
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The showcase's sheet as `.lss`** — M61/T61.4
    /// (`docs/plan-lss.md`). `showcase.lss` mirrors [`stylesheet`] rule
    /// for rule, and `include_lss!` checks every `#id` and `.class` in it
    /// against `Screen` at compile time: this compiling is the accept
    /// direction of those checks on a real sheet. The comparison is by
    /// value, rule by rule, so a mirror that drifted from the sheet the
    /// showcase runs is named here rather than trusted.
    #[test]
    fn the_lss_mirror_is_the_hand_written_sheet() {
        const LSS: &[StyleRule] = lumen_static::include_lss!(Screen, "../showcase.lss");
        // M83/T83.6: the `ShaderView` demo's kind rule exists only with the
        // `shader` feature, and the mirror is the default build's sheet.
        let hand: Vec<StyleRule> = stylesheet()
            .into_iter()
            .filter(|r| !(cfg!(feature = "shader") && r.selector == Selector::Kind("DShaderView")))
            .collect();
        println!("showcase.lss: {} rules; stylesheet(): {} rules", LSS.len(), hand.len());
        let differ: Vec<String> = LSS
            .iter()
            .zip(&hand)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| format!("rule {i}:\n  lss:  {a:?}\n  hand: {b:?}"))
            .collect();
        assert_eq!(LSS.len(), hand.len(), "rule counts differ");
        assert!(differ.is_empty(), "{} rules differ:\n{}", differ.len(), differ.join("\n"));
        // T61.5's parity: the run-time reader lowers the file to the same
        // rules the macro emitted.
        let read = lumen_static_shell::lss::load(concat!(env!("CARGO_MANIFEST_DIR"), "/showcase.lss"), None)
            .unwrap_or_else(|e| panic!("{e}"));
        println!("lss::load: {} rules", read.len());
        assert_eq!(read, LSS, "include_lss! and lss::load disagree on showcase.lss");
    }
    use lumen_static_shell::text::{FontMetrics as FontMetrics2, FontStack as FontStack2};
    use lumen_static::display::Cmd;
    use lumen_static::style::Selector;

    /// An `EventCtx` for a press, for calling a handler directly.
    fn press_at(x: f32, y: f32) -> EventCtx {
        EventCtx {
            event: Event::down(x, y),
            node: 0,
            target: 0,
            target_is_control: false,
            row: None,
            inner: 0,
            pos: lumen_static::Point { x, y },
            local: lumen_static::Point { x, y },
            button: PointerButton::Left,
            clicks: 1,
            mods: Mods::NONE,
        }
    }

    /// The stage card's top-left corner — where the header and the sidebar
    /// stop. A command whose own corner lies **strictly** past it is the
    /// demo's: the card's own background starts exactly there, and a
    /// bound that counted it would pass with nothing on the stage (M56:
    /// this was a `y` threshold until the sidebar ran the window's full
    /// height).
    /// The Button demo's three buttons, **by id** — M56 made the demo
    /// exactly as wide as a "button-shaped" size filter's 300 px bound,
    /// and the filter then counted the demo and its row as buttons too.
    fn button_ids() -> Vec<usize> {
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        ["d_button_a", "d_button_b", "d_button_c"]
            .iter()
            .map(|w| lumen_static::style::index_of_wid(table, w).expect("a demo button"))
            .collect()
    }

    fn stage_origin(h: &lumen_static_shell::Host<Show>) -> (f32, f32) {
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let i = lumen_static::style::index_of_wid(table, "stage").expect("no `#stage` node");
        (h.states[i].pos.x, h.states[i].pos.y)
    }

    /// **The `Label` demo's switch restyles its paragraph** — M80/T80.2,
    /// headless: the press lands on the switch, the program's `style_by`
    /// re-asks the sheet, and the label is re-measured at `lines × pitch`.
    /// The live half is `just widget-drive Label`.
    #[test]
    fn the_label_demos_switch_sets_a_line_height() {
        use lumen_static_shell::text::{FontMetrics, FontStack};
        let Some(font) = FontStack::system() else {
            eprintln!("no system font; skipping");
            return;
        };
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let label = lumen_static::style::index_of_wid(table, "d_label_a").expect("#d_label_a");
        let entry = CATALOG.iter().position(|e| e.name == "Label").unwrap();
        let mut h = host(entry, false);
        let mut m = FontMetrics::new(font);
        let before = h.states[label].size.h;
        // The switch is the first control in `#d_label_spacing`.
        let sw = (0..table.len())
            .find(|&i| h.states[i].size.w > 0.0 && table[i].is_focusable()
                && h.states[i].pos.y < h.states[label].pos.y)
            .expect("a focusable switch above the label");
        let st = h.states[sw];
        let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
        h.post(lumen_static::Event::down(x, y));
        h.post(lumen_static::Event::up(x, y));
        h.pump_with(&mut m);
        h.pump_with(&mut m);
        let after = h.states[label].size.h;
        let px = h.styles[label].font_size;
        let pitch = 1.5 * px;
        let lines = (after / pitch).round();
        println!(
            "label: {before} -> {after} px (line_height byte {}; {lines} lines x pitch {pitch} = {})",
            h.styles[label].line_height,
            lines * pitch
        );
        assert!(h.app.store.spaced, "setup: the press did not reach the switch");
        assert_eq!(h.styles[label].line_height, 24, "the sheet did not set 1.5x on the label");
        assert!(lines >= 2.0, "setup: the paragraph did not wrap");
        assert_eq!(after, lines * pitch, "the label is not lines x pitch tall");
        assert!(after > before, "the label did not grow");
    }

    /// **The `Label` demo's ellipsis switch cuts its paragraph to one
    /// line** — M80/T80.3, headless: the press lands on the second switch,
    /// `style_by` re-asks the sheet, and the label comes out one line tall,
    /// its shown text a prefix ending at a boundary. The live half is
    /// `just widget-drive Label`.
    #[test]
    fn the_label_demos_ellipsis_switch_cuts_to_one_line() {
        use lumen_static_shell::text::{FontMetrics, FontStack};
        let Some(font) = FontStack::system() else {
            eprintln!("no system font; skipping");
            return;
        };
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let label = lumen_static::style::index_of_wid(table, "d_label_a").expect("#d_label_a");
        let entry = CATALOG.iter().position(|e| e.name == "Label").unwrap();
        let mut h = host(entry, false);
        let mut m = FontMetrics::new(font.clone());
        let before = h.states[label].size.h;
        // The second focusable control above the label is the ellipsis switch.
        let sw = (0..table.len())
            .filter(|&i| h.states[i].size.w > 0.0 && table[i].is_focusable()
                && h.states[i].pos.y < h.states[label].pos.y)
            .nth(1)
            .expect("a second focusable switch above the label");
        let st = h.states[sw];
        let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
        h.post(lumen_static::Event::down(x, y));
        h.post(lumen_static::Event::up(x, y));
        h.pump_with(&mut m);
        h.pump_with(&mut m);
        let after = h.states[label].size.h;
        let one = lumen_static::TextMeasure::measure_weighted(
            &mut FontMetrics::new(font),
            "The",
            h.styles[label].font_size,
            h.styles[label].font_weight,
            f32::INFINITY,
        )
        .h;
        let Demo::Label(d) = &h.app.ui.main.stage.row.demo else { panic!("setup: the stage is not the Label demo") };
        let (shown, cut) = d.a.shown();
        println!("label: {before} -> {after} px (one line {one} px); shown {shown:?}, cut {cut}");
        assert!(h.app.store.ellipsis, "setup: the press did not reach the ellipsis switch");
        assert!(before > 1.5 * one, "setup: the paragraph did not wrap before the press");
        assert!(cut, "the label was not cut");
        assert_eq!(after, one, "the label is not one line tall");
        assert!(catalog::LABEL_PROSE.starts_with(shown), "the shown text is not a prefix of the paragraph");
    }

    pub(super) fn host(entry: usize, open: bool) -> lumen_static_shell::Host<Show> {
        use lumen_static_shell::text::{FontMetrics, FontStack};
        let mut h: lumen_static_shell::Host<Show> =
            program(entry, open).palette(SHOW_LIGHT).host();
        let mut m = FontStack::system().map(FontMetrics::new);
        for _ in 0..2 {
            match m.as_mut() {
                Some(m) => h.pump_with(m),
                None => h.pump(),
            };
        }
        h
    }

    /// **A partial repaint matches a full one at scale 2, on the real
    /// layout** — found by M57/D6's scale-2 drive. Opening a picker left
    /// one physical row of the trigger's text stale: the popup's damage
    /// ended on a half-pixel logical edge, and the damaged and painted
    /// extents rounded it differently. The framework's own test lays out
    /// on whole pixels, which is why only the showcase's layout showed it.
    #[test]
    fn a_popup_repaints_partially_to_the_same_pixels_at_scale_2() {
        use lumen_static::retain::Damage;
        use lumen_static_shell::software::Canvas;
        use lumen_static_shell::text::{FontMetrics, FontStack};
        let Some(font) = FontStack::system() else {
            eprintln!("no system font; skipping");
            return;
        };
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let mut worst = Vec::new();
        for name in ["DatePicker", "FilePicker", "PickList"] {
            let entry = CATALOG.iter().position(|e| e.name == name).unwrap();
            let mut h = host(entry, false);
            // The size the scale-2 drive's window really had: the window
            // manager fitted the requested 2240x1520 to the screen, and the
            // defect needs that layout's half-pixel edge. At 2240x1520 every
            // edge here was whole and this test was green.
            let (w, hh) = (1920, 1008);
            h.resize(w, hh);
            h.set_scale(2.0);
            let mut m = FontMetrics::new(font.clone());
            h.pump_with(&mut m);
            h.post(Event::Key { key: lumen_static::event::Key::Escape, down: true });
            h.pump_with(&mut m);
            let (mut full, mut part) = (Canvas::with_font(w, hh, font.clone()), Canvas::with_font(w, hh, font.clone()));
            for c in [&mut full, &mut part] {
                c.clear(palette().page);
                lumen_static::Renderer::render(c, &h.list, w, hh);
            }
            h.take_damage_for_age(1);
            // The trigger, found by part id under the demo.
            let demo = lumen_static::style::index_of_wid(table, "demo").unwrap();
            let trig = lumen_static::style::subtree_range(table, demo)
                .find(|&k| matches!(table[k].wid, "picker-trigger" | "chooser-trigger") && h.states[k].size.w > 0.0)
                .expect("setup: no trigger on stage");
            let (mut partial, mut differ) = (0, 0);
            for _ in 0..3 {
                let t = h.states[trig];
                let (x, y) = (t.pos.x + 20.0, t.pos.y + t.size.h / 2.0);
                h.post(Event::down(x, y));
                h.pump_with(&mut m);
                h.post(Event::up(x, y));
                h.pump_with(&mut m);
                full.clear(palette().page);
                lumen_static::Renderer::render(&mut full, &h.list, w, hh);
                match h.take_damage_for_age(1) {
                    Damage::None => {}
                    Damage::All => {
                        part.clear(palette().page);
                        lumen_static::Renderer::render(&mut part, &h.list, w, hh);
                    }
                    d @ Damage::Regions(_) => {
                        partial += 1;
                        for r in d.regions([0.0, 0.0, w as f32, hh as f32]) {
                            part.clear_within(r, palette().page);
                            part.render_within(&h.list, r);
                        }
                    }
                }
                differ = differ.max(full.pixels.chunks(4).zip(part.pixels.chunks(4)).filter(|(a, b)| a != b).count());
            }
            println!("{name} at scale 2: {partial} partial repaints, worst {differ} px differ");
            assert!(partial >= 2, "setup: {name} repainted partially {partial} times");
            worst.push((name, differ));
        }
        assert!(worst.iter().all(|w| w.1 == 0), "stale pixels after a partial repaint: {worst:?}");
    }

    /// **A grid row's cells take their column** (M27/T27.11, finding 4).
    ///
    /// The header is drawn over `column_width` bands of a box the rows
    /// are handed already inset; a row whose children are `FILL`
    /// divides that box the same way, so the two agree by construction.
    /// A bare `Label` is `Shrink` and does not — which is what shipped,
    /// and the `Size` values sat 72 px left of the `Size` heading.
    #[test]
    fn a_grid_rows_cells_take_their_column() {
        let entry = CATALOG.iter().position(|e| e.name == "DataGrid").unwrap();
        let mut h = host(0, false);
        choose(&mut h.app.store, entry);
        let mut m = FontStack2::system().map(FontMetrics2::new);
        match m.as_mut() {
            Some(m) => h.pump_with(m),
            None => h.pump(),
        };
        let Demo::DataGrid(d) = &h.app.ui.main.stage.row.demo else {
            panic!("choosing entry {entry} did not put a grid on the stage");
        };
        // **The grid's own laid-out width, not a literal.** This was
        // `column_width(320.0)`: 320 px was the stage's one-third share
        // at the old 960 px window, true only by coincidence — M56 moved
        // it, and the literal went on describing a grid that was no
        // longer there.
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let grid = lumen_static::style::index_of_wid(table, "d_datagrid_a").expect("no `#d_datagrid_a`");
        let width = h.states[grid].size.w;
        let want = d.a.column_width(width);
        // The cells by id within the row's own table, not by index: the
        // cells pad themselves now, which moved every index after the first.
        let row = <stage::GridRow as lumen_static::widget::Widget>::SUB_TABLE;
        let cells: Vec<_> = ["d_grid_name", "d_grid_kind", "d_grid_size", "d_grid_lines"]
            .iter()
            .map(|id| d.a.rows.instance_state(0, row.iter().position(|n| n.wid == *id).expect("no cell id")))
            .collect();
        println!("grid {width} px, column {want}, cells {:?} px wide", cells.iter().map(|c| c.size.w).collect::<Vec<_>>());
        assert!(want > 0.0, "setup: the grid reported a zero-wide column");
        assert_eq!(cells.len(), d.a.columns(), "setup: a cell per column");
        for (k, c) in cells.into_iter().enumerate() {
            assert!(
                (c.size.w - want).abs() < 0.5,
                "cell {k} is {:.1} px wide, not the column's {want} — the row \
                 does not divide the box the header is drawn over",
                c.size.w
            );
        }
    }

    /// **A wheel over the grid scrolls it — through the real host.**
    ///
    /// `crates/lumen-static-widgets/tests/data.rs`'s
    /// `a_wheel_over_the_body_scrolls_it` walks the same path on a bare
    /// `DataGrid` and is green; `just widget-drive` found the live
    /// window ignoring the wheel anyway. The difference is everything
    /// between them — the enum variant, the stage, the overlay-aware
    /// dispatch — so this asserts it where the difference lives.
    #[test]
    fn a_wheel_over_the_grid_scrolls_it_on_the_stage() {
        let entry = CATALOG.iter().position(|e| e.name == "DataGrid").unwrap();
        let mut h = host(entry, false);
        let before = match &h.app.ui.main.stage.row.demo {
            Demo::DataGrid(d) => d.a.rows.first_row(),
            other => panic!("entry {entry} put {:?} on the stage", other.kind_name()),
        };
        let (x, y) = {
            let table =
                <<Show as lumen_static::event::App>::Tree as
                    lumen_static::widget::Widget>::SUB_TABLE;
            let i = table
                .iter()
                .position(|d| d.wid == "d_datagrid_a")
                .expect("the grid is not in the table");
            let s = &h.states[i];
            (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
        };
        let hit = h.probe(x, y).expect("nothing is under the middle of the grid");
        h.post(Event::Scroll { x, y, dy: -120.0 });
        h.pump();
        let after = match &h.app.ui.main.stage.row.demo {
            Demo::DataGrid(d) => d.a.rows.first_row(),
            _ => unreachable!(),
        };
        println!("hit node {} at ({x}, {y}); first row {before} -> {after}", hit.node);
        assert!(
            after > before,
            "a 120 px wheel over the grid's body left the first visible row at \
             {after} — the hit named node {}",
            hit.node
        );
    }

    /// **The stage has a demo for every entry, and the menu a row.**
    ///
    /// **The menu's length is data now.** It used to be a field per
    /// catalogue entry, so adding a widget to `CATALOG` without adding a
    /// field silently dropped it off the end of the menu — visible only
    /// to someone who counted. `Select` takes the count from
    /// `CATALOG.len()`, so only the stage can still drift.
    #[test]
    fn the_sidebar_has_a_row_for_every_entry() {
        let app = build(0, false);
        let rows = app.ui.side.nav.count;
        let groups = (0..rows).filter(|&r| matches!(catalog::nav_item(r), catalog::NavItem::Heading(_))).count();
        println!("{rows} rows: {} entries and {groups} headings; {} demos", CATALOG.len(), stage::DEMOS);
        assert_eq!(rows, catalog::nav_len(), "the sidebar lists a different number of rows");
        assert_eq!(rows, CATALOG.len() + groups, "the rows are not the entries plus one heading per group");
        // Every entry exactly once, in catalogue order.
        let entries: Vec<usize> = (0..rows)
            .filter_map(|r| match catalog::nav_item(r) {
                catalog::NavItem::Entry(k) => Some(k),
                catalog::NavItem::Heading(_) => None,
            })
            .collect();
        assert_eq!(entries, (0..CATALOG.len()).collect::<Vec<_>>(), "an entry is missing, repeated or out of order");
        assert_eq!(stage::DEMOS, CATALOG.len(), "`Demo`'s variants and `CATALOG` disagree");
    }

    /// **Every entry puts something on the stage.**
    ///
    /// Seeded, laid out and painted for real — not merely constructed.
    /// A demo that built a widget and gave it no size would pass a
    /// smoke test that stopped at `seed`.
    /// M25/T25.2: no rule in the showcase's sheet is a no-op — its
    /// selector matches, and every field it sets is read by a node it
    /// matches. Prints each finding, so a failure says which rule.
    #[test]
    fn every_stylesheet_rule_changes_something() {
        // `Unread` is wrong wherever it happens: the rule *matched* a
        // node and set a field that node does not read. `Unmatched` is
        // `every_rule_matches_a_node`'s job, because a rule for the demo
        // that is not on the stage matches nothing *here* and is not
        // thereby dead.
        let mut found: Vec<String> = Vec::new();
        let mut rules = 0;
        for entry in [0usize, CATALOG.iter().position(|e| e.name == "DataGrid").unwrap()] {
            let h = host(entry, entry == 0);
            let sheet = h.stylesheet();
            rules = sheet.len();
            for f in h.sheet_findings() {
                if matches!(f.issue, lumen_static::style::SheetIssue::Unread(_)) {
                    found.push(format!(
                        "entry {entry}: rule {} {:?}: {:?}",
                        f.rule, sheet[f.rule].selector, f.issue
                    ));
                }
            }
        }
        println!("{rules} rules, {} findings", found.len());
        assert!(rules > 0, "setup: the sheet was applied");
        assert!(found.is_empty(), "stylesheet rules that change nothing:\n{}", found.join("\n"));
    }

    /// M25/T25.11 — T25.9's "every themed node changes", for what a
    /// stylesheet themes. For every catalogue entry, each colour field a
    /// **role** rule decides is resolved from `SHOW_LIGHT`, and after one
    /// `set_palette` from `SHOW_DARK`; each one a **value** rule decides
    /// keeps its colour. The menu rows and the demo live in arenas, so the
    /// walk reaches them the way the cascade does — a check over the main
    /// table alone would miss most of the sheet.
    #[test]
    fn every_role_rule_follows_the_palette() {
        use lumen_static::style::{Fields, Role, Rule};
        use lumen_static::widget::Node as _;

        const ROLES: [Role; 15] = [
            Role::Page,
            Role::Surface,
            Role::Control,
            Role::Text,
            Role::Muted,
            Role::Accent,
            Role::OnAccent,
            Role::Focus,
            Role::Raised,
            Role::Border,
            Role::Track,
            Role::Placeholder,
            Role::Scrim,
            Role::Danger,
            Role::OnDanger,
        ];
        for r in ROLES {
            assert_ne!(SHOW_LIGHT.get(r), SHOW_DARK.get(r), "setup: the palettes differ at {r:?}");
        }

        /// Every node's author id, **classes**, kind and resolved style:
        /// the main table, then every arena under it.
        ///
        /// Classes carried since M27/T27.3, when the labelled pairs
        /// started sharing one (`pair-label`) instead of each carrying an
        /// id. `winner` below used to `panic!("this check does not model
        /// classes")` rather than quietly ignore them — which is the
        /// difference between a guard and a hole, and is why adding the
        /// class turned this test red instead of leaving it green and
        /// wrong.
        type Row = (&'static str, &'static [&'static str], &'static str, Style);

        fn nodes(h: &mut lumen_static_shell::Host<Show>) -> Vec<Row> {
            let table = <Screen as Widget>::SUB_TABLE;
            let mut out: Vec<_> = table
                .iter()
                .enumerate()
                .map(|(i, d)| (d.wid, d.classes, <Screen as Widget>::SEEDS[i].kind, h.styles[i]))
                .collect();
            for i in 0..table.len() {
                let node = h.app.ui.node_mut(i);
                for a in 0..node.arena_count() {
                    let Some(arena) = node.arena_mut(a) else { continue };
                    for (k, d) in arena.table.iter().enumerate() {
                        // Every arena carries its seeds since
                        // M26/T26.10; the `None` arm that asked a live
                        // template for its kind is gone.
                        let kind = arena.seeds[k].kind;
                        out.push((d.wid, d.classes, kind, arena.style[k]));
                    }
                }
            }
            out
        }

        /// The rule that decides a field for a node, in the cascade's
        /// order: highest specificity, then the last declared
        /// (`max_by_key` keeps the last of equals).
        fn winner<'s>(
            sheet: &'s [Rule],
            wid: &str,
            classes: &[&str],
            kind: &str,
            field: Fields,
        ) -> Option<&'s Rule> {
            sheet
                .iter()
                .filter(|r| r.patch.fields().contains(field))
                .filter(|r| match r.selector {
                    Selector::Id(id) => id == wid,
                    Selector::Kind(k) => k == kind,
                    Selector::Class(c) => classes.contains(&c),
                    Selector::Any => true,
                })
                .max_by_key(|r| r.selector.specificity())
        }

        let sheet = stylesheet();
        let (mut themed, mut kept) = (0usize, 0usize);
        let mut wrong: Vec<String> = Vec::new();
        for (entry, e) in CATALOG.iter().enumerate() {
            let mut h = host(entry, true);
            let light = nodes(&mut h);
            h.set_palette(SHOW_DARK);
            let dark = nodes(&mut h);
            assert_eq!(light.len(), dark.len(), "setup: both walks saw the same nodes");
            for ((wid, classes, kind, l), (.., d)) in light.iter().zip(&dark) {
                let fields = [
                    (Fields::COLOR, "color", l.color, d.color),
                    (Fields::BACKGROUND, "background", l.background, d.background),
                ];
                for (field, name, lc, dc) in fields {
                    let Some(rule) = winner(&sheet, wid, classes, kind, field) else { continue };
                    let role =
                        if field == Fields::COLOR { rule.patch.color_role } else { rule.patch.background_role };
                    let at = || format!("{} {kind} #{wid} {name}", e.name);
                    match role {
                        Some(r) => {
                            themed += 1;
                            if (lc, dc) != (SHOW_LIGHT.get(r), SHOW_DARK.get(r)) {
                                wrong.push(format!("{}: {r:?} painted {lc:?} then {dc:?}", at()));
                            }
                        }
                        None => {
                            kept += 1;
                            if lc != dc {
                                wrong.push(format!("{}: a value changed with the palette", at()));
                            }
                        }
                    }
                }
            }
        }
        println!(
            "{themed} role-decided fields followed the palette and {kept} value-decided fields \
             kept theirs, over {} entries",
            CATALOG.len()
        );
        assert!(themed > 0 && kept > 0, "setup: both halves were exercised");
        assert!(wrong.is_empty(), "fields that did not follow their rule:\n{}", wrong.join("\n"));
    }

    /// The catalogue's painted text, entry by entry, under `palette`, with
    /// the real stylesheet (`lumen_static_shell::contrast`) — M54/T54.0,
    /// T54.5. Returns the runs under `bound`, the runs measured, the
    /// unmeasured count and the catalogue's count of text commands.
    fn catalogue_contrast(palette: Palette, bound: f32) -> (Vec<String>, usize, usize, usize) {
        use lumen_static_shell::contrast::text_contrast;
        let mut h = host(0, false);
        h.set_palette(palette);
        let (mut measured, mut texts, mut unmeasured) = (0usize, 0usize, 0usize);
        let mut below = Vec::new();
        for (i, e) in CATALOG.iter().enumerate() {
            choose(&mut h.app.store, i);
            let mut m = FontStack2::system().map(FontMetrics2::new);
            for _ in 0..2 {
                match m.as_mut() {
                    Some(m) => h.pump_with(m),
                    None => h.pump(),
                };
            }
            texts += h.list.cmds.iter().filter(|c| matches!(c, Cmd::Text { text, .. } if !h.list.str_of(*text).trim().is_empty())).count();
            let r = text_contrast(&h.list, palette.page);
            measured += r.measured.len();
            unmeasured += r.unmeasured.len();
            for x in r.below(bound) {
                below.push(format!("{}: {:?} {:.2}:1 (fg {:?} on {:?})", e.name, x.text, x.ratio, x.fg, x.ground));
            }
        }
        (below, measured, unmeasured, texts)
    }

    fn three_to_one(palettes: &[(&str, Palette)]) {
        let mut bad = Vec::new();
        for (name, p) in palettes {
            let (below, measured, unmeasured, texts) = catalogue_contrast(*p, 3.0);
            println!("{name}: runs below 3.0: {} / {measured} measured ({unmeasured} unmeasured, {texts} text commands)", below.len());
            assert!(measured * 2 >= texts, "setup: {name}: {measured} of {texts} text commands measured");
            bad.extend(below.into_iter().map(|b| format!("{name}: {b}")));
        }
        assert!(bad.is_empty(), "{} run(s) under 3:1:\n  {}", bad.len(), bad.join("\n  "));
    }

    /// **Every text run the catalogue paints reaches 3:1**, the non-text
    /// minimum, under the showcase's light palette and the core's —
    /// M54/T54.5 (R16-F8; the core's is what `app.setPalette` swaps to).
    /// It fails if fewer than half the catalogue's text commands were
    /// measured, so a harness that found no ground cannot pass.
    #[test]
    fn every_catalogue_text_run_reaches_three_to_one_in_light() {
        three_to_one(&[("SHOW_LIGHT", SHOW_LIGHT), ("Palette::LIGHT", Palette::LIGHT)]);
    }

    /// **The same under the dark palettes** — M54/T54.5 measured it red
    /// (120 of 446 runs under 3:1, 2026-09-27): the widgets' own defaults
    /// were light-theme *values*, not palette roles. M57/A8 made them
    /// palette roles, and at the merge of the two programs this reads
    /// 0 of 1672 runs under 3:1 in `SHOW_DARK` and in `Palette::DARK`, so
    /// it runs rather than being ignored
    /// (`docs/plan-a11y-platform.md`, "What building found", T54.5).
    #[test]
    fn every_catalogue_text_run_reaches_three_to_one_in_dark() {
        three_to_one(&[("SHOW_DARK", SHOW_DARK), ("Palette::DARK", Palette::DARK)]);
    }

    /// **And WCAG AA, 4.5:1, for text** — M54/T54.0, green since T54.6
    /// (R16-F9), under the showcase's light palette and the core's.
    #[test]
    fn every_catalogue_text_run_reaches_aa() {
        let mut bad = Vec::new();
        for (name, p) in [("SHOW_LIGHT", SHOW_LIGHT), ("Palette::LIGHT", Palette::LIGHT)] {
            let (below, measured, unmeasured, texts) = catalogue_contrast(p, 4.5);
            println!("{name}: runs below 4.5: {} / {measured} measured ({unmeasured} unmeasured, {texts} text commands)", below.len());
            assert!(measured * 2 >= texts, "setup: {name}: {measured} of {texts} text commands measured");
            bad.extend(below.into_iter().map(|b| format!("{name}: {b}")));
        }
        assert!(bad.is_empty(), "{} run(s) under 4.5:1:\n  {}", bad.len(), bad.join("\n  "));
    }

    /// The opening of every note the showcase used to print under its
    /// demo, removed by M56/T56.1 (`docs/plan-showcase-design.md`). Kept
    /// only so a test can say none of them comes back.
    const FORMER_NOTES: [(&str, &str); 51] = [
        ("Label", "A `Label`'s text is a presentati"),
        ("Badge", "It shrinks to its text, so `IS_R"),
        ("Chip", "A separate type rather than a fi"),
        ("Avatar", "Two *chars*, not two bytes: \"ÅB\""),
        ("Icon", "A closed enum, not a font codepo"),
        ("Divider", "Named `Divider` where Lumen says"),
        ("Button", "An `on_click` is a plain `fn` po"),
        ("CheckBox", "A toggle does not flip itself: t"),
        ("Switch", "One type with three presentation"),
        ("Radio", "There is no group container: eac"),
        ("Slider", "A slider only draggable by mouse"),
        ("TextInput", "The one place a widget owning st"),
        ("ProgressBar", "An empty bar paints nothing at a"),
        ("Spinner", "It does not drive its own clock "),
        ("Skeleton", "The shimmer is clipped to the bl"),
        ("BarChart", "`Series` is a fixed array, so a "),
        ("LineChart", "A non-finite sample costs one se"),
        ("PieChart", "Wedges are runs of radial lines:"),
        ("Card", "A wrapper that does not route `H"),
        ("Toast", "It does not dismiss itself: ther"),
        ("Tooltip", "The tip keys on the *content's* "),
        ("NumberInput", "Not a Stepper: no +/- buttons. B"),
        ("RangeSlider", "A pair of sliders is not a range"),
        ("SearchField", "A click in the padding focuses t"),
        ("Tabs", "An underline, not a fill: a fill"),
        ("BottomNav", "One type, three presentations, c"),
        ("NavigationRail", "A rail reads a press down `local"),
        ("AppBar", "Not a selector, so not a strip: "),
        ("PullToRefresh", "It does not refresh anything: `t"),
        ("Modal", "A shut veil is *absent*, not mer"),
        ("Sheet", "Its panel is measured against wh"),
        ("Drawer", "Dismissal is a *request*: pressi"),
        ("Wrap", "A derive axis, not a widget: a c"),
        ("Grid", "`wrap` and `grid` are one loweri"),
        ("SplitPane", "The divider stops short of both "),
        ("Accordion", "Not a widget — a column of `Coll"),
        ("PickList", "It owns its options, which is wh"),
        ("Combobox", "An empty query shows everything:"),
        ("Menu", "A release chooses, a press does "),
        ("PaneGrid", "iced's is a *tree* the user re-a"),
        ("RichText", "The measure hook answers `how wi"),
        ("Markdown", "No italic, deliberately: `Painte"),
        ("RichTextEditor", "It edits *markup*, not a documen"),
        ("PaintBox", "A plain `fn` pointer and not a c"),
        ("ColorPicker", "A fixed palette, not a wheel: a "),
        ("DatePicker", "A month grid is a lead-in of bla"),
        ("TimePicker", "Hours and five-minute marks in o"),
        ("FilePicker", "It owns no data either — the nam"),
        ("Pagination", "Centred, not paged in blocks: a "),
        ("Tree", "Collapsing hides a whole subtree"),
        ("DataGrid", "It owns no data: its rows are a "),
    ];

    /// WCAG relative luminance of an sRGB colour.
    fn luminance(c: Color) -> f64 {
        let lin = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
    }

    fn contrast(a: Color, b: Color) -> f64 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// `fg` at `alpha` over an opaque `bg`, as the rasteriser blends it.
    fn over(fg: Color, alpha: u8, bg: Color) -> Color {
        let a = alpha as f64 / 255.0;
        let m = |f: u8, b: u8| (f as f64 * a + b as f64 * (1.0 - a)).round() as u8;
        Color(m(fg.0, bg.0), m(fg.1, bg.1), m(fg.2, bg.2), 255)
    }

    /// **The chrome's text reads, in both palettes** — M56/T56.3.
    ///
    /// Every text/ground pair the chrome paints, from the values it
    /// paints them with: the palettes' roles, the `Chrome` values, and
    /// `NavRow`'s own heading and tint alphas. AA's 4.5:1 for all of it:
    /// the chrome's text is 11–16 px, and the blurb and breadcrumb are
    /// content, not decoration. The first pass measured three pairs
    /// under — light muted text on the page at 4.33, and the chip's
    /// accent-on-tint at 3.90 (light) and 4.44 (dark) — and the ghost
    /// demo button's, a pair it was added to cover, at 3.91 and 4.19.
    #[test]
    fn chrome_text_meets_contrast() {
        let mut failed = Vec::new();
        for (name, p, c) in [("light", SHOW_LIGHT, CHROME_LIGHT), ("dark", SHOW_DARK, CHROME_DARK)] {
            let selected = over(p.accent, nav::SELECTED_TINT_ALPHA, p.surface);
            let heading = over(p.text, nav::HEADING_ALPHA, p.surface);
            let pairs = [
                ("title on the page", p.text, p.page),
                ("blurb and breadcrumb on the page", p.muted, p.page),
                ("brand subtitle on the sidebar", p.muted, p.surface),
                ("a sidebar row", p.text, p.surface),
                ("the selected row", p.text, selected),
                ("a group heading", heading, p.surface),
                ("the readout chip", c.accent_text, c.chip),
                ("a ghost button", c.accent_text, c.ghost),
            ];
            for (what, fg, bg) in pairs {
                let r = contrast(fg, bg);
                println!("  {name:5} {what:34} {r:5.2}:1 (bound 4.50)");
                if r < 4.5 {
                    failed.push(format!("{name}: {what} is {r:.2}:1"));
                }
            }
        }
        assert!(failed.is_empty(), "chrome text under AA's 4.5:1 — {failed:?}");
    }

    /// **Every demo's text reads on what it is painted over, in both
    /// palettes** — M57/A8 (`docs/plan-widget-fixes.md`, D4).
    ///
    /// `chrome_text_meets_contrast` above checks the chrome and nothing
    /// else, which is how a Label demo shipped at 1.30:1 in the dark
    /// palette and a modal's title black on its dark panel
    /// (`docs/widget-audit.md` X6). This reads what was *painted*: every
    /// text command on the stage, against the opaque fills painted before
    /// it that contain its centre, composited in paint order over the
    /// stage card. AA's 4.5:1, or 3:1 for large text (24 px, or 18.66 px
    /// bold).
    #[test]
    fn every_demo_text_reads_in_both_palettes() {
        use lumen_static_shell::text::{FontMetrics, FontStack};
        let mut failed = Vec::new();
        let mut checked = 0usize;
        let mut on_gradient = 0usize;
        let (mut paths, mut vector_paths) = (0usize, 0usize);
        let mut m_found = false;
        for (name, p, c) in [("light", SHOW_LIGHT, CHROME_LIGHT), ("dark", SHOW_DARK, CHROME_DARK)] {
            let mut h = host(0, false);
            h.set_palette(p);
            h.set_stylesheet(&stylesheet_with(c));
            let mut m = FontStack::system().map(FontMetrics::new);
            m_found = m.is_some();
            for (e, entry) in CATALOG.iter().enumerate() {
                choose(&mut h.app.store, e);
                for _ in 0..2 {
                    match m.as_mut() {
                        Some(m) => h.pump_with(m),
                        None => h.pump(),
                    };
                }
                let (sx, sy) = stage_origin(&h);
                let inside = |r: &[f32; 4], x: f32, y: f32| r[0] <= x && x <= r[2] && r[1] <= y && y <= r[3];
                for (k, cmd) in h.list.cmds.iter().enumerate() {
                    let Cmd::Text { rect, text, size, weight, line_height: _, color } = cmd else { continue };
                    if rect[0] <= sx || rect[1] <= sy || color.3 == 0 {
                        continue;
                    }
                    let (cx, cy) = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
                    // The ground at one point, composited in paint order.
                    // **A gradient is a ground too** (M82/T82.3): the Card
                    // demo's text sits on its fill, which is sampled where
                    // the point falls, clamped into the gradient's rect.
                    let ground_at = |x: f32| {
                        let mut ground = p.surface;
                        for prev in &h.list.cmds[..k] {
                            match prev {
                                Cmd::Rect { rect: r, color: f } | Cmd::RoundedRect { rect: r, color: f, .. }
                                    if f.3 > 0 && inside(r, cx, cy) =>
                                {
                                    ground = over(*f, f.3, ground);
                                }
                                Cmd::Gradient { rect: r, stops, kind, geom, .. } if inside(r, cx, cy) => {
                                    let axis = lumen_static::display::GradAxis::new(*r, *kind, *geom);
                                    let f = stops.sample(axis.t(x.clamp(r[0], r[2]), cy));
                                    ground = over(f, f.3, ground);
                                }
                                _ => {}
                            }
                        }
                        ground
                    };
                    let large = *size >= 24.0 || (*size >= 18.66 && *weight >= 700.0);
                    let bound = if large { 3.0 } else { 4.5 };
                    // The worst of the run's centre and its two ends: on a
                    // flat ground all three agree, on a gradient they do not.
                    let (r, fg, ground) = [cx, rect[0], rect[2]]
                        .into_iter()
                        .map(|x| {
                            let ground = ground_at(x);
                            let fg = over(*color, color.3, ground);
                            (contrast(fg, ground), fg, ground)
                        })
                        .fold((f64::INFINITY, *color, p.surface), |a, b| if b.0 < a.0 { b } else { a });
                    if h.list.cmds[..k].iter().any(|c| matches!(c, Cmd::Gradient { rect: g, .. } if inside(g, cx, cy))) {
                        on_gradient += 1;
                        println!("  {name} {}: {:?} on a gradient, worst {r:.2}:1 ({fg:?} on {ground:?})", entry.name, h.list.str_of(*text));
                    }
                    checked += 1;
                    if r < bound {
                        failed.push(format!(
                            "{name} {}: {:?} is {r:.2}:1 ({fg:?} on {ground:?}), under {bound}",
                            entry.name,
                            h.list.str_of(*text)
                        ));
                    }
                }
                // **And path ink, at 3:1** — M82/T82.8. `VectorText`
                // paints its run as one filled path, which no `Cmd::Text`
                // walk sees, and `PaintBox` draws its star and arc as paths;
                // a path is non-text ink (and VectorText's run is display
                // size), so WCAG's 3:1. Its colour is the fill, or the
                // stroke where there is no fill, against the opaque fills
                // painted before it at the centre of its ink.
                for (k, cmd) in h.list.cmds.iter().enumerate() {
                    let Cmd::Path { rect, fill, stroke, .. } = cmd else { continue };
                    let color = if fill.3 > 0 { *fill } else { *stroke };
                    if rect[0] <= sx || rect[1] <= sy || color.3 == 0 {
                        continue;
                    }
                    let (cx, cy) = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
                    let mut ground = p.surface;
                    for prev in &h.list.cmds[..k] {
                        if let Cmd::Rect { rect: r, color: f } | Cmd::RoundedRect { rect: r, color: f, .. } = prev {
                            if f.3 > 0 && inside(r, cx, cy) {
                                ground = over(*f, f.3, ground);
                            }
                        }
                    }
                    let fg = over(color, color.3, ground);
                    let r = contrast(fg, ground);
                    paths += 1;
                    if entry.name == "VectorText" {
                        vector_paths += 1;
                        println!("  {name} VectorText: path ink {r:.2}:1 ({fg:?} on {ground:?}) (bound 3)");
                    }
                    if r < 3.0 {
                        failed.push(format!("{name} {}: path ink is {r:.2}:1 ({fg:?} on {ground:?}), under 3", entry.name));
                    }
                }
            }
        }
        println!("  {checked} text runs checked across both palettes; {} under their bound", failed.len());
        for f in &failed {
            println!("  {f}");
        }
        assert!(checked > 200, "setup: only {checked} text runs on the stage — the walk found no demos");
        // M82/T82.3: the Card demo's title and button label, in both
        // palettes, are judged on its fill — a walk that never met a
        // gradient would judge them on the stage instead.
        println!("  {on_gradient} text runs judged on a gradient (want >= 4: the Card's two, both palettes)");
        assert!(on_gradient >= 4, "setup: the Card's text was not judged on its fill");
        // M82/T82.8: VectorText's three runs, in both palettes, are judged
        // as path ink -- which they are only with outlines, so a system
        // face is the setup (without one the demo paints text, judged above).
        println!("  {paths} paths judged at 3:1, {vector_paths} of them VectorText's (want 6 with a system face)");
        if m_found {
            assert_eq!(vector_paths, 6, "setup: VectorText's runs were not judged as path ink");
        }
        assert!(failed.is_empty(), "{} demo text runs under their bound", failed.len());
    }

    /// The sidebar prints a heading where the group changes, so a group
    /// that recurs later in the catalogue would be headed twice (M56).
    #[test]
    fn each_group_appears_once() {
        let mut seen: Vec<&str> = Vec::new();
        for e in CATALOG {
            if seen.last() != Some(&e.group) {
                assert!(!seen.contains(&e.group), "group {:?} recurs at {:?}: groups must be contiguous", e.group, e.name);
                seen.push(e.group);
            }
        }
        println!("  {} groups, each once: {:?}", seen.len(), seen);
    }

    #[test]
    fn every_entry_paints_a_demo() {
        let mut seen: Vec<(&str, core::mem::Discriminant<Demo>)> = Vec::new();
        // **One host, selected through in turn** — not a fresh one per
        // entry. A `Demos::clear` that forgets a field leaves the
        // previous demo on the stage, and from a fresh app there is
        // never a previous demo to leave: the first version of this
        // loop rebuilt each time and could not see it.
        let mut h = host(0, false);
        for (i, e) in CATALOG.iter().enumerate() {
            choose(&mut h.app.store, i);
            h.pump();
            // **Commands on the stage, not in the frame.** The header
            // alone draws ten, so a whole-frame count is cleared by the
            // header whatever the stage does — an entry whose `seed`
            // did nothing passed a `> 6` bound on the total.
            let (sx, sy) = stage_origin(&h);
            let demo = h
                .list
                .cmds
                .iter()
                .filter(|c| match c {
                    Cmd::Rect { rect, .. }
                    | Cmd::RoundedRect { rect, .. }
                    | Cmd::Text { rect, .. } => rect[0] > sx && rect[1] > sy,
                    Cmd::Line { a, .. } => a.x > sx && a.y > sy,
                    // M83/T83.6: the `ShaderView` demo is one bracket (its
                    // fallback a gradient, which this count does not read).
                    // The showcase's `shader` turns the core's on, so the
                    // variant exists whenever this arm is compiled.
                    #[cfg(feature = "shader")]
                    Cmd::Shader { rect, .. } => rect[0] > sx && rect[1] > sy,
                    _ => false,
                })
                .count();
            let blurb = h
                .list
                .cmds
                .iter()
                .filter_map(|c| match c {
                    Cmd::Text { text, .. } => Some(h.list.str_of(*text).to_string()),
                    _ => None,
                })
                .any(|t| t == e.blurb);
            // **No note under the demo** (M56/T56.1): no text anywhere on
            // the frame opens the way a removed note did.
            for c in &h.list.cmds {
                if let Cmd::Text { text, .. } = c {
                    let t = h.list.str_of(*text);
                    if let Some((whose, _)) = FORMER_NOTES.iter().find(|(_, p)| t.starts_with(p)) {
                        panic!("{}: a removed note is on screen again ({whose}'s): {t:?}", e.name);
                    }
                }
            }
            println!("  {:>12}  {demo} commands", e.name);
            // **Exactly one demo on the stage — now by construction.**
            // This used to assert `shown() == 1` over eighteen
            // `Option`s, because `clear()` was a hand-written list and
            // one field left out of it left the previous demo underneath
            // the new one. An enum cannot hold two variants, so that
            // assertion has no content left.
            //
            // What replaces it is the failure the enum *does* still
            // allow: a catalogue entry seeding the wrong variant, which
            // is a copy-paste away. Collected here, checked below — no
            // per-entry literal to go stale, and two entries landing on
            // one variant is exactly what it catches.
            seen.push((e.name, core::mem::discriminant(&h.app.ui.main.stage.row.demo)));
            assert!(blurb, "{}'s blurb never reached the header", e.name);
            assert!(demo > 0, "{} put nothing on the stage", e.name);

            // **A widget that draws only text is a label.** That is the
            // bug this gate exists for: the demos got no stylesheet
            // rules at first, so a `TextInput` rendered as its text and
            // nothing else — no ground, no edge, no caret room — and so
            // did a `Button`. `defaults()` is a floor that keeps a
            // control *visible*, not a look; a showcase has to supply
            // the style it means to show.
            //
            // `Label` is the one entry exempt, because text is all it
            // is.
            //
            // `RichText` is exempt too, and for the same reason: its
            // spans are words and it draws nothing else. **Until M56 this
            // check passed for it by counting the chrome**: its filter
            // took everything below the stage's top edge, and the
            // header's hairline divider sat exactly there, so a
            // text-only demo was credited with the divider's rect.
            // The filter now takes only what is strictly inside the
            // stage card, which is what made that visible.
            //
            // And `Selectable` (M80/T80.4): its one shape is the tint of a
            // selection, placed from shaped clusters, and this pumps the
            // `BlockMetrics` stand-in, which reports none — so it draws
            // the text alone here, as a label does.
            //
            // And `VectorText` (M82/T82.8), for the same reason: the
            // stand-in has no outlines either, so it paints its fallback,
            // the axis-aligned string. Its turned outlines are held by
            // `lumen-static-shell/tests/vector_text.rs` and its live drive.
            if e.name != "Label" && e.name != "RichText" && e.name != "Selectable" && e.name != "VectorText" {
                let shapes = h
                    .list
                    .cmds
                    .iter()
                    .filter(|c| match c {
                        Cmd::Rect { rect, .. } | Cmd::RoundedRect { rect, .. } => rect[0] > sx && rect[1] > sy,
                        Cmd::Line { a, .. } => a.x > sx && a.y > sy,
                        // A gradient is a shape (M83/T83.6: the
                        // `ShaderView` demo's fallback is one).
                        Cmd::Gradient { rect, .. } => rect[0] > sx && rect[1] > sy,
                        _ => false,
                    })
                    .count();
                assert!(
                    shapes > 0,
                    "{} drew only text on the stage — it looks like a label",
                    e.name
                );
            }
        }

        // No two entries put the same variant on the stage.
        assert_eq!(seen.len(), CATALOG.len());
        for (i, (na, da)) in seen.iter().enumerate() {
            for (nb, db) in &seen[i + 1..] {
                assert_ne!(da, db, "{na} and {nb} both seed the same demo variant");
            }
        }
        println!("{} entries, {} distinct variants", seen.len(), seen.len());
    }

    /// **The dropdown is absent when closed and present when open**, and
    /// an absent one paints nothing.
    ///
    /// `Option` is the core's hide: `layout` zeroes an absent branch
    /// every frame, so it holds no geometry for `paint` or `hit`. A
    /// panel that merely painted off-screen would still swallow clicks.
    #[test]
    fn the_dropdown_is_absent_when_closed() {
        // The `Select` demo — the showcase's own picker until M56.
        let select = CATALOG.iter().position(|e| e.name == "Select").unwrap();
        let (shut, open) = (host(select, false), host(select, true));
        let (a, b) = (shut.list.cmds.len(), open.list.cmds.len());
        println!("closed {a} commands, open {b}");
        let menu = |h: &lumen_static_shell::Host<Show>| match &h.app.ui.main.stage.row.demo {
            Demo::Select(d) => (d.a.is_open(), d.a.menu.content.window_len()),
            _ => panic!("the Select demo is not on the stage"),
        };
        assert!(menu(&open).0, "an open select is not open");
        assert!(!menu(&shut).0, "a closed select is open");
        // **The rows the menu *materialized*, not the option count** —
        // `Repeat` reports its own window, so the bound comes from the
        // thing it bounds (M27/T27.4).
        let rows = menu(&open).1;
        assert!(rows > 0 && rows <= catalog::FRUIT.len(), "the menu materialized {rows} rows — setup");
        assert!(b > a + rows, "an open menu drew {} more commands, not {rows}+", b - a);
    }

    /// **A widget on the stage actually receives its clicks.**
    ///
    /// The bug this test exists for: the stage was a `Dyn` island, whose
    /// `Handlers<S>` returns `None` unconditionally — the application
    /// type is exactly what type erasure erased. Every button, checkbox
    /// and radio in the showcase could be hit-tested, hovered and
    /// pressed, and no handler ever ran. Nothing that stopped at "the
    /// demo paints" could see it.
    ///
    /// Driven through `Host`: a real pointer event, at the widget's real
    /// laid-out position, and the assertion is on the application's own
    /// state.
    #[test]
    fn pressing_a_demo_button_reaches_the_application() {
        let button = CATALOG.iter().position(|e| e.name == "Button").unwrap();
        let mut h = host(button, false);
        assert_eq!(h.app.store.presses, 0);

        // The first button of the `Button` demo, found by geometry
        // rather than by a counted index.
        assert!(
            matches!(h.app.ui.main.stage.row.demo, Demo::Button(_)),
            "the Button demo is not on the stage"
        );
        let node = button_ids()[0];
        let s = &h.states[node];
        let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
        println!("clicking node {node} at ({x:.0}, {y:.0}), size {:?}", s.size);

        h.post(Event::down(x, y));
        h.post(Event::up(x, y));
        h.pump();
        println!("presses after one click: {}", h.app.store.presses);
        assert_eq!(h.app.store.presses, 1, "the click never reached the handler");

        // And the readout followed, which is the other half of the
        // wiring: `after_update` refreshes it on the same pump.
        let shown = h
            .list
            .cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::Text { text, .. } => Some(h.list.str_of(*text).to_string()),
                _ => None,
            })
            .any(|t| t == "presses: 1");
        assert!(shown, "the readout did not follow the press");
    }

    /// **The Image demo plays at its own rate, and its button flips it** —
    /// M84/T84.3. A tick short of the GIF's 100 ms delay changes nothing
    /// and reports nothing; the tick at the delay writes frame 1 into the
    /// image and reports it; `wake_in_ms` answers what is left of the
    /// frame. A press on the button pauses (no wake, label "Play"), and a
    /// second press resumes: the button *flips* `playing`. *Injection:* the
    /// handler sets `playing = false` instead of flipping it -- the second
    /// press leaves it paused, by name.
    #[test]
    fn the_image_demo_plays_at_its_own_rate_and_its_button_flips_it() {
        use lumen_static::event::App as _;
        let entry = CATALOG.iter().position(|e| e.name == "Image").expect("an Image entry");
        let mut h = host(entry, false);
        let frame = |h: &lumen_static_shell::Host<Show>| match &h.app.ui.main.stage.row.demo {
            Demo::Image(d) => d.a.frame,
            _ => panic!("setup: the Image demo is not on the stage"),
        };
        let g = catalog::spin();
        println!("the GIF: {}x{}, {} frames, delays {:?} ms", g.w, g.h, g.frames.len(), g.delays_ms);
        assert_eq!((g.frames.len(), &g.delays_ms[..]), (4, &[100u32; 4][..]), "setup: assets/spin.gif is four 100 ms frames");
        assert_eq!(frame(&h), 0, "setup: the demo starts at frame 0");
        let short = h.tick(60.0);
        let left = h.app.wake_in_ms();
        let at = h.tick(40.0);
        println!("tick(60) -> {short}, wake then {left:?} ms; tick(40) -> {at}, frame {}", frame(&h));
        assert!(!short, "a tick short of the delay reported a change");
        assert!(left.is_some_and(|l| (l - 40.0).abs() < 0.01), "wake_in_ms is {left:?}, want 40");
        assert!(at && frame(&h) == 1, "the tick at the delay did not write frame 1 and report it");

        let press = |h: &mut lumen_static_shell::Host<Show>| {
            let i = lumen_static::style::index_of_wid(
                <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE,
                "d_image_pause",
            )
            .expect("the pause button has an id");
            let s = h.states[i];
            let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
            h.post(Event::down(x, y));
            h.post(Event::up(x, y));
            h.pump();
            match &h.app.ui.main.stage.row.demo {
                Demo::Image(d) => d.pause.label,
                _ => "",
            }
        };
        let label = press(&mut h);
        let (wake, ticked) = (h.app.wake_in_ms(), h.tick(500.0));
        println!("paused: label {label:?}, wake {wake:?}, tick(500) -> {ticked}, frame {}", frame(&h));
        assert_eq!((label, wake, ticked), ("Play", None, false), "the first press did not pause the image");
        let label = press(&mut h);
        let resumed = h.app.store.spin.playing;
        println!("second press: label {label:?}, playing {resumed}, wake {:?}", h.app.wake_in_ms());
        assert!(resumed && label == "Pause", "the second press did not resume the image: the button does not flip `playing`");
    }

    /// **A `Dyn` island routes a handler into its content** — M23.
    ///
    /// This test said the opposite one commit ago, and pinned it: `Dyn`
    /// boxed a `dyn Node`, which has no `Handlers<S>` because the
    /// application type is exactly what the erasure erased, so every
    /// interactive widget in an island was dead. The fix is a supertrait
    /// — `NodeFor<S>: Node` with a blanket impl — and the island boxes
    /// that instead.
    ///
    /// The stage does **not** use one: a typed `Option` field can be
    /// *read* as well as clicked, which is what lets a `Slider`'s value
    /// reach the readout. The hatch is for content whose shape is
    /// genuinely unknown, and this is what it now costs to use.
    #[test]
    fn a_dyn_island_routes_a_handler() {
        use lumen_static::event::{Handlers, NodeSel};
        let mut b = Button::<Store, String>::default();
        b.label = "live".into();
        b.on_click = |s: &mut Store, _: &EventCtx| s.presses += 1;
        let direct: Option<fn(&mut Store, &EventCtx)> =
            Handlers::<Store>::handler(&b, NodeSel { i: 0, inner: None });
        let island: lumen_static::Dyn<Store> = lumen_static::Dyn::new(b);
        let through: Option<fn(&mut Store, &EventCtx)> =
            Handlers::<Store>::handler(&island, NodeSel { i: 0, inner: None });
        println!("direct {:?}, through a Dyn {:?}", direct.is_some(), through.is_some());
        assert!(direct.is_some(), "a wired button reports no handler at all");
        assert!(through.is_some(), "a Dyn island still swallows its content's handlers");

        // And it is the *same* handler, not merely some handler.
        let mut s = Store::default();
        (through.unwrap())(&mut s, &press_at(0.0, 0.0));
        assert_eq!(s.presses, 1, "the routed handler is not the button's");
    }

    /// Drive a click on one button, then another, reading back what each
    /// button's resolved background is at every step.
    #[test]
    fn a_press_shows_on_the_button_being_pressed() {
        let entry = CATALOG.iter().position(|e| e.name == "Button").unwrap();
        let mut h = host(entry, false);
        let btns = button_ids();
        assert_eq!(btns.len(), 3, "expected three buttons, found {}", btns.len());
        let at = |h: &lumen_static_shell::Host<Show>, i: usize| {
            let s = &h.states[i];
            (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
        };
        let show = |h: &lumen_static_shell::Host<Show>, tag: &str| {
            let v: Vec<String> = btns
                .iter()
                .map(|&i| {
                    let s = &h.states[i];
                    format!("{}{}", if s.pressed { "P" } else { "-" }, if s.hovered { "H" } else { "-" })
                })
                .collect();
            println!("  {tag:<18} {v:?}");
        };
        show(&h, "start");
        let (x, y) = at(&h, btns[0]);
        h.post(Event::down(x, y));
        h.pump();
        show(&h, "down on 0");
        assert!(h.states[btns[0]].pressed, "button 0 is not pressed while held");
        h.post(Event::up(x, y));
        h.pump();
        show(&h, "up on 0");
        assert!(!h.states[btns[0]].pressed, "button 0 stayed pressed after release");

        let (x2, y2) = at(&h, btns[1]);
        h.post(Event::down(x2, y2));
        h.pump();
        show(&h, "down on 1");
        assert!(h.states[btns[1]].pressed, "button 1 is not pressed while held");
        assert!(
            !h.states[btns[0]].pressed,
            "button 0 still reads pressed while button 1 is held — the stale press"
        );
    }

    /// **A press repaints the button it is on** — through the *damage*
    /// path the window actually uses.
    ///
    /// The reported symptom was that clicking did nothing visible, and
    /// that clicking a second button seemed to animate the first. Every
    /// test so far rendered the whole frame, which is not what a window
    /// does: it renders `Host::damage`'s regions and leaves the rest.
    /// A colour change the damage does not cover is a change nobody
    /// sees until something else forces a repaint of that area — which
    /// is exactly what "it happens when I click the next one" describes.
    #[test]
    fn a_press_damages_the_button_it_is_on() {
        use lumen_static::retain::Damage;
        let entry = CATALOG.iter().position(|e| e.name == "Button").unwrap();
        let mut h = host(entry, false);
        let btns = button_ids();
        assert_eq!(btns.len(), 3);
        // **This frame does not settle, and that is a known cost.**
        // `#[app(after_update = ...)]` reports nothing, so the derive
        // marks every pump as changed (`docs/plan-invalidation.md`
        // T16.10) — the showcase uses the hook to keep a `Slider`'s
        // readout following a drag, since a slider owns its value and
        // has no change callback to hang it on. So the assertion here is
        // *coverage*, not exclusivity: the press must damage the button,
        // whatever else is also damaged.
        let _ = h.take_damage_for_age(1);
        h.pump();

        let s0 = h.states[btns[0]];
        let (x, y) = (s0.pos.x + s0.size.w / 2.0, s0.pos.y + s0.size.h / 2.0);
        h.post(Event::down(x, y));
        h.pump();
        let d = h.damage();
        println!("press damage: {d:?}");
        println!("button 0 box: {:?} {:?}", s0.pos, s0.size);
        assert_ne!(d, Damage::None, "a press reported no damage at all");
        // And it must actually cover the button, not merely be non-empty.
        let covers = match d {
            Damage::All => true,
            Damage::Regions(r) => r.as_slice().iter().any(|g| {
                g[0] <= s0.pos.x + 1.0
                    && g[1] <= s0.pos.y + 1.0
                    && g[2] >= s0.pos.x + s0.size.w - 1.0
                    && g[3] >= s0.pos.y + s0.size.h - 1.0
            }),
            Damage::None => false,
        };
        assert!(covers, "the press damaged something, but not the button it was on");
    }

    /// **The wrap demo's lines are centred** — M57/C9.
    ///
    /// A flow packed every line from the left, so the demo's chips sat off
    /// the card's centre (`docs/widget-audit.md`, Wrap). The first line's
    /// margins, left and right, must match.
    #[test]
    fn the_wrap_demos_lines_are_centred() {
        let entry = CATALOG.iter().position(|e| e.name == "Wrap").unwrap();
        let h = host(entry, false);
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let idx = |w: &str| lumen_static::style::index_of_wid(table, w).expect(w);
        let (a, c) = (h.states[idx("d_wrap_a")], h.states[idx("d_wrap_c")]);
        let wrap = h.states[table[idx("d_wrap_a")].parent as usize];
        let left = a.pos.x - wrap.pos.x;
        let right = (wrap.pos.x + wrap.size.w) - (c.pos.x + c.size.w);
        println!("first line: {left:.1} px left, {right:.1} px right");
        assert!(left > 1.0 && (left - right).abs() <= 1.0, "the first line is not centred ({left:.1} / {right:.1})");
    }

    /// **An accordion keeps one section open, and its headers still** —
    /// M57/C7.
    ///
    /// No section started open, although the demo's comment said the first
    /// did; the one-open rule the rustdoc gives the application was not
    /// written; and the stage re-centred the column when a section opened,
    /// moving the pressed header 15 px (`docs/widget-audit.md`, Accordion).
    #[test]
    fn an_accordion_keeps_one_section_open_and_its_headers_still() {
        let entry = CATALOG.iter().position(|e| e.name == "Accordion").unwrap();
        let mut h = host(entry, false);
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let b = lumen_static::style::index_of_wid(table, "d_acc_b").expect("the second section");
        let open = |h: &lumen_static_shell::Host<Show>| match &h.app.ui.main.stage.row.demo {
            Demo::Accordion(d) => [d.a.is_open(), d.b.is_open(), d.c.is_open()],
            _ => panic!("setup: not the Accordion demo"),
        };
        assert_eq!(open(&h), [true, false, false], "the first section does not start open");
        let a = lumen_static::style::index_of_wid(table, "d_acc_a").unwrap();
        let (y0, ya0) = (h.states[b].pos.y, h.states[a].pos.y);
        let (x, y) = (h.states[b].pos.x + 30.0, y0 + 10.0);
        h.post(Event::down(x, y));
        h.post(Event::up(x, y));
        // **The measure `host` laid out with**, not the default: since
        // M45/T45.2 a text memo is keyed on the measurer, so a bare `pump`
        // re-measured the page header with block metrics, shrank it 20 px,
        // and re-centred the stage under a column that had not moved.
        let mut m = FontStack2::system().map(FontMetrics2::new);
        for _ in 0..2 {
            match m.as_mut() {
                Some(m) => h.pump_with(m),
                None => h.pump(),
            };
        }
        println!("after pressing the second header: {:?}; header y {y0} -> {}", open(&h), h.states[b].pos.y);
        assert_eq!(open(&h), [false, true, false], "more or other than the pressed section is open");
        // The second header moves up by the first section's closing, which
        // is right; what must not happen is a re-centring, which would move
        // the *first* header too.
        println!("first header y {ya0} -> {}", h.states[a].pos.y);
        assert_eq!(h.states[a].pos.y, ya0, "the column re-centred when a section opened");
    }

    /// **A confirm is not a dismissal** — M57/C6.
    ///
    /// The veil demos' readout counted both on one counter, so pressing
    /// the panel's Confirm read as "dismissals: 1"
    /// (`docs/widget-audit.md`, Modal).
    #[test]
    fn a_confirm_is_counted_apart_from_a_dismissal() {
        let entry = CATALOG.iter().position(|e| e.name == "Modal").unwrap();
        let mut h = host(entry, false);
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let idx = |w: &str| lumen_static::style::index_of_wid(table, w).expect(w);
        let press = |h: &mut lumen_static_shell::Host<Show>, x: f32, y: f32| {
            h.post(Event::down(x, y));
            h.post(Event::up(x, y));
            h.pump();
            h.pump();
        };
        let go = h.states[idx("d_modal_go")];
        press(&mut h, go.pos.x + go.size.w / 2.0, go.pos.y + go.size.h / 2.0);
        let after_go = h.app.ui.main.header.heading.readout.as_str().to_string();
        let veil = h.states[idx("d_modal_a")];
        press(&mut h, veil.pos.x + 4.0, veil.pos.y + 4.0);
        let after_scrim = h.app.ui.main.header.heading.readout.as_str().to_string();
        println!("after Confirm: {after_go:?}; after the scrim: {after_scrim:?}");
        assert_eq!(after_go, "dismissals: 0 · confirmed: 1", "a confirm was counted as something else");
        assert_eq!(after_scrim, "dismissals: 1 · confirmed: 1", "the scrim press was not counted as a dismissal");
    }

    /// **A search field is one pill, not a box inside a box** — M57/C5.
    ///
    /// The sheet's `Kind("textinput")` rule reached the field inside the
    /// search pill too, and gave it a 220 px bordered box of its own
    /// (`docs/widget-audit.md`, SearchField). The inner field is kind
    /// `searchinput` now; the rule must not reach it.
    #[test]
    fn a_search_field_has_no_box_inside_it() {
        let entry = CATALOG.iter().position(|e| e.name == "SearchField").unwrap();
        let h = host(entry, false);
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let i = table
            .iter()
            .position(|d| d.wid == "search-field")
            .expect("setup: no search field part");
        let st = h.styles[i];
        println!("inner field: border {} px, width {:?}", st.border_width, st.width);
        assert_eq!(st.border_width, 0, "the search pill's field draws a border of its own");
        assert_ne!(st.width, Length::Fixed(220.0), "the text field rule sized the search pill's field");
    }

    /// **The number field's readout follows it** — M57/C4.
    ///
    /// `refresh` read the `Slider` back and nothing else, so the number
    /// field's readout said `value: 0.00` through every edit
    /// (`docs/widget-audit.md`, NumberInput).
    #[test]
    fn the_number_readout_follows_the_field() {
        use lumen_static::event::Key;
        let entry = CATALOG.iter().position(|e| e.name == "NumberInput").unwrap();
        let mut h = host(entry, false);
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let i = lumen_static::style::index_of_wid(table, "d_number_a").expect("the number field");
        h.input.focus = Some(i as u32);
        h.post(Event::Key { key: Key::Up, down: true });
        h.post(Event::Key { key: Key::Up, down: false });
        h.pump();
        h.pump();
        let text = h.app.ui.main.header.heading.readout.as_str().to_string();
        let Demo::Number(d) = &h.app.ui.main.stage.row.demo else { panic!("setup: not the NumberInput demo") };
        println!("field {}, readout {text:?}", d.a.value());
        assert_eq!(d.a.value(), 0.5, "setup: Up did not step the field");
        assert_eq!(text, "value: 0.50", "the readout did not follow the field");
    }

    /// **A toggle's drawn state follows its click** — M57/C2.
    ///
    /// The demos flipped the store and never wrote the toggles' `on` back,
    /// so the readout changed while the boxes stayed as drawn; and the
    /// three radios shared one `bool`, so Blue could never be chosen
    /// (`docs/widget-audit.md`, CheckBox, Switch, Radio). The drive read
    /// only the readout, which is how it passed. This reads what the
    /// widget will paint.
    #[test]
    fn a_toggles_drawn_state_follows_its_click() {
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let idx = |w: &str| lumen_static::style::index_of_wid(table, w).expect(w);
        let click = |h: &mut lumen_static_shell::Host<Show>, i: usize| {
            let s = h.states[i];
            let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
            h.post(Event::down(x, y));
            h.post(Event::up(x, y));
            h.pump();
            h.pump();
        };

        let entry = CATALOG.iter().position(|e| e.name == "CheckBox").unwrap();
        let mut h = host(entry, false);
        click(&mut h, idx("d_checkbox_a"));
        let Demo::CheckBox(d) = &h.app.ui.main.stage.row.demo else { panic!("setup: not the CheckBox demo") };
        println!("after one click: store {} drawn a={} b={}", h.app.store.flag, d.a.control.on, d.b.control.on);
        assert!(h.app.store.flag, "setup: the click did not reach the store");
        assert!(d.a.control.on, "the store says checked and the box is drawn unchecked");
        assert!(!d.b.control.on, "the other box followed a click that was not on it");

        let entry = CATALOG.iter().position(|e| e.name == "Radio").unwrap();
        let mut h = host(entry, false);
        click(&mut h, idx("d_radio_c"));
        let Demo::Radio(d) = &h.app.ui.main.stage.row.demo else { panic!("setup: not the Radio demo") };
        println!("after clicking Blue: choice {} drawn {} {} {}", h.app.store.choice, d.a.control.on, d.b.control.on, d.c.control.on);
        assert_eq!(h.app.store.choice, 2, "clicking Blue did not choose it");
        assert!(d.c.control.on && !d.a.control.on && !d.b.control.on, "Blue is chosen and not the one drawn");
    }

    /// **The toast demo shows a floating toast on Save, and it goes away
    /// by itself** — M57/B34, through the showcase's own wiring: the
    /// button asks, `refresh` shows it, `advance` runs its countdown.
    #[test]
    fn save_shows_a_toast_that_times_out() {
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let idx = |w: &str| lumen_static::style::index_of_wid(table, w).expect(w);
        let entry = CATALOG.iter().position(|e| e.name == "Toast").unwrap();
        let mut h = host(entry, false);
        let shown = |h: &lumen_static_shell::Host<Show>| match &h.app.ui.main.stage.row.demo {
            Demo::Toast(d) => d.a.content.t.is_shown(),
            _ => panic!("setup: not the Toast demo"),
        };
        assert!(!shown(&h), "setup: the toast is showing before Save");
        let s = h.states[idx("d_toast_show")];
        let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
        h.post(Event::down(x, y));
        h.post(Event::up(x, y));
        h.pump();
        h.pump();
        let t = h.states[idx("d_toast_a")];
        println!("after Save: shown {}, toasts {}, box {:?}+{:?}", shown(&h), h.app.store.toasts, t.pos, t.size);
        assert_eq!(h.app.store.toasts, 1, "setup: the Save click did not land");
        assert!(shown(&h) && t.size.w > 0.0, "Save did not show the toast");
        h.tick(3200.0);
        h.pump();
        println!("after 3.2 s: shown {}", shown(&h));
        assert!(!shown(&h), "the toast outlived its 3000 ms");
    }

    /// **Every demo button shows hover** — M57/C1.
    ///
    /// The `Kind("button")` rule meant for `Select`'s menu rows set
    /// `hover_dim(0.0)`, and an id rule that does not set the field keeps
    /// the kind's: no demo button changed under the pointer
    /// (`docs/widget-audit.md`, Button). Hover each and compare the
    /// painted background.
    #[test]
    fn every_demo_button_shows_hover() {
        let entry = CATALOG.iter().position(|e| e.name == "Button").unwrap();
        let mut h = host(entry, false);
        let btns = button_ids();
        assert_eq!(btns.len(), 3, "setup: the Button demo has three buttons");
        for &b in &btns {
            let s = h.states[b];
            let rest = s.bg;
            h.post(Event::PointerMove { x: s.pos.x + s.size.w / 2.0, y: s.pos.y + s.size.h / 2.0 });
            h.pump();
            let hover = h.states[b].bg;
            println!("button {b}: rest {rest:?}, hovered {hover:?}");
            assert!(h.states[b].hovered, "setup: the pointer is not over button {b}");
            assert_ne!(rest, hover, "button {b} painted the same background under the pointer");
            h.post(Event::PointerMove { x: 1.0, y: 1.0 });
            h.pump();
        }
    }

    /// **Every stylesheet rule matches a node.**
    ///
    /// A rule whose id matches nothing is silent: the cascade skips it,
    /// the widget keeps its bare `defaults()`, and what reaches the
    /// screen is a control with no size or ground of its own. That is
    /// what happened to `ProgressBar` — its field is `progress`, so its
    /// id is `d_progress_a`, and the rule said d_progressbar_a. The
    /// bar rendered full-window and flat, which is indistinguishable
    /// from a widget that does not work.
    ///
    /// `SUB_TABLE` carries every node's `wid`, so the two sets can
    /// simply be compared.
    #[test]
    fn every_rule_matches_a_node() {
        // **Per entry, because one demo is live at a time.**
        //
        // This read `Screen::SUB_TABLE`'s `wid`s and nothing else, which
        // works for every demo — the stage is an enum and its inactive
        // variants stay in the table — and is blind to the one place a
        // node is *not* in the table: a `Repeat`'s row template lives in
        // an arena that exists only while its demo is on the stage. A
        // rule naming a repeated row's id was reported dead however
        // correct it was (M27/T27.11, finding 4's fix).
        let sheet = stylesheet();
        let mut ever_matched = vec![false; sheet.len()];
        for entry in 0..CATALOG.len() {
            let h = host(entry, false);
            let mut unmatched = vec![false; sheet.len()];
            for f in h.sheet_findings() {
                if matches!(f.issue, lumen_static::style::SheetIssue::Unmatched) {
                    unmatched[f.rule] = true;
                }
            }
            for (r, m) in ever_matched.iter_mut().enumerate() {
                *m |= !unmatched[r];
            }
        }
        let dead: Vec<String> = ever_matched
            .iter()
            .enumerate()
            .filter(|(_, m)| !**m)
            .map(|(r, _)| format!("rule {r} {:?}", sheet[r].selector))
            .collect();
        println!("{} rules over {} entries, dead: {}", sheet.len(), CATALOG.len(), dead.len());
        assert!(
            dead.is_empty(),
            "stylesheet rules that match no node under any entry:\n{}",
            dead.join("\n")
        );
    }

    /// **Picking a row changes what is on the stage.**
    ///
    /// Drives the real handler, with the row index arriving the way it
    /// does at runtime — in `EventCtx::row` — rather than from a table
    /// of eighteen functions that no longer exists.
    /// **Every sidebar row, clicked for real, picks its own entry** — and
    /// a heading picks nothing (M56/T56.2).
    ///
    /// Real wheel events bring each row into the window, and a real press
    /// and release land on the centre of its materialized instance. For an
    /// entry the store's selection, the header's title and the row's own
    /// highlight must all name it; for a heading, `pump` must answer that
    /// nothing handled the press, and the selection must not move. The
    /// walk starts away from the first entry, so "selected 0" cannot be a
    /// row that did nothing.
    #[test]
    fn every_sidebar_row_picks_its_own_entry() {
        let start = CATALOG.len() - 1;
        let mut h = host(start, false);
        let table = <<Show as lumen_static::event::App>::Tree as lumen_static::widget::Widget>::SUB_TABLE;
        let nav = lumen_static::style::index_of_wid(table, "nav").expect("no `#nav`");
        let (nx, ny, nh) = (h.states[nav].pos.x, h.states[nav].pos.y, h.states[nav].size.h);
        let (mut items, mut headings, mut dismissals) = (0, 0, 0);
        for r in 0..catalog::nav_len() {
            // Scroll until row `r` is in the materialized window.
            for _ in 0..200 {
                let s = &h.app.ui.side.nav;
                let (first, len) = (s.first_row(), s.window_len());
                // Fully inside the viewport, not merely materialized: a row
                // half under the edge is clipped where the press would land.
                let k = r.wrapping_sub(first);
                if k < len {
                    let st = s.row_state(k);
                    if st.pos.y >= ny && st.pos.y + st.size.h <= ny + nh {
                        break;
                    }
                }
                // A positive `dy` is a wheel turned toward the user, which
                // moves a list back toward its top (`PullToRefresh` pulls
                // on the same sign) — so reaching a lower row is negative.
                //
                // **Up, too, for a row materialized above the viewport's
                // top** — M80/T80.4. Deciding by `r < first` alone sent a
                // row that was first in the window but cut by the top edge
                // *down*: the walk oscillated between two offsets 32 px
                // apart and never reached it. Latent while the list's
                // starting offset happened to be a multiple of the step;
                // one more catalogue entry moved the reveal off it.
                let above = k < len && s.row_state(k).pos.y < ny;
                let dy = if r < first || above { NAV_ROW_H } else { -NAV_ROW_H };
                h.post(Event::Scroll { x: nx + 100.0, y: ny + nh / 2.0, dy });
                h.pump();
            }
            let s = &h.app.ui.side.nav;
            let k = r - s.first_row();
            let st = s.row_state(k);
            let (x, y) = (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0);
            let before = h.app.store.selected;
            let open_before = h.open_overlays();
            h.post(Event::down(x, y));
            h.post(Event::up(x, y));
            let mut handled = h.pump();
            // **A demo seeded with its popup open** (a date picker, a
            // menu) takes the next press anywhere outside it: the press
            // closes the popup and is consumed — the documented dropdown
            // behaviour — so a user clicks again, and so does this. Only
            // when a popup *was* open and the press closed it: a retry
            // anywhere else would hide exactly the miss this test is for.
            // (A veil is different: its dismissal is a request the
            // showcase counts and declines, and the press goes on.)
            if h.app.store.selected == before && open_before > 0 && h.open_overlays() < open_before {
                dismissals += 1;
                h.post(Event::down(x, y));
                h.post(Event::up(x, y));
                handled = h.pump();
            }
            match catalog::nav_item(r) {
                catalog::NavItem::Heading(g) => {
                    headings += 1;
                    assert!(!handled, "a press on the {g:?} heading (row {r}) was handled");
                    assert_eq!(h.app.store.selected, before, "a press on the {g:?} heading moved the selection");
                }
                catalog::NavItem::Entry(e) => {
                    items += 1;
                    let name = CATALOG[e].name;
                    assert!(handled, "a press on row {r} ({name}) was not handled at ({x:.0}, {y:.0})");
                    assert_eq!(
                        h.app.store.selected, e,
                        "row {r} is labelled {name:?} and picked {:?}",
                        CATALOG[h.app.store.selected].name
                    );
                    let title = &h.app.ui.main.header.heading.title;
                    assert_eq!(title.as_str(), name, "the header did not follow row {r}");
                    let s = &h.app.ui.side.nav;
                    let row = s.instance(r - s.first_row());
                    assert!(
                        row.selected && row.label == name,
                        "row {r} was clicked for {name:?} and now reads {:?}, selected: {}",
                        row.label,
                        row.selected
                    );
                }
            }
        }
        println!("clicked {items} entry rows and {headings} headings ({dismissals} presses first closed a demo's popup); every entry picked itself");
        assert_eq!(items, CATALOG.len(), "setup: the walk did not reach every entry");
    }

    #[test]
    fn picking_a_row_swaps_the_demo() {
        let mut s = build(0, false);
        assert_eq!(s.store.selected, 0);
        let target = 15;
        let r = (0..catalog::nav_len())
            .find(|&r| catalog::nav_item(r) == catalog::NavItem::Entry(target))
            .expect("entry 15 has no sidebar row");
        let mut row = NavRow::<Store>::default();
        bind_nav(&mut row, r);
        assert_eq!(row.label, CATALOG[target].name, "the row is not labelled with its entry");
        let mut cx = press_at(0.0, 0.0);
        cx.row = Some(r as u64);
        // The handler gets the state (M33); `refresh` then rebuilds the
        // header and moves the highlight, as the next pump would.
        (row.on_click)(&mut s.store, &cx);
        lumen_static::event::App::update(&mut s);
        println!("after picking sidebar row {r}: {} ({})", CATALOG[s.store.selected].name, s.store.selected);
        assert_eq!(s.store.selected, target, "the handler picked the wrong entry");
        assert_eq!(nav::selected(), target, "the sidebar's highlight did not follow");
    }
}

/// **What the flip to `Boundaries` buys, by frame class** — M34/T34.2
/// (`docs/plan-boundaries-default.md`). Ignored: it is a measurement,
/// not a gate. `cargo test --release -p showcase measure_the_flip --
/// --ignored --nocapture`; eight runs a side, the median frame of each.
///
/// **Two figures per cell since M53/T53.1**: the whole pump
/// (`Host::stage_timings`), which is the frame, and layout + place +
/// paint (`Host::timings3`), which is what T34.2 published as the frame
/// and leaves out input, the arena cascade and `finish`
/// (`docs/results-damage-precision.md`, T53.1).
#[cfg(test)]
mod flip {
    use super::*;
    use lumen_static::invalidate::{Boundaries, Everything, Invalidation};
    use lumen_static::retain::Incremental;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    use lumen_static_shell::Host;

    const RUNS: usize = 8;
    const ITERS: usize = 40;

    /// Median (whole pump, layout + place + paint), µs.
    fn run<I: Invalidation>(entry: &str, class: &str, m: &mut FontMetrics) -> (f64, f64) {
        let e = CATALOG.iter().position(|x| x.name == entry).unwrap();
        let mut h: Host<Show, Incremental, I> = program(e, false).host();
        for _ in 0..3 {
            h.pump_with(m);
        }
        let table = <Screen as lumen_static::Widget>::SUB_TABLE;
        let at = |h: &Host<Show, Incremental, I>, wid: &str| {
            let i = lumen_static::style::index_of_wid(table, wid).unwrap();
            let s = h.states[i];
            (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
        };
        let (mut v, mut whole) = (Vec::new(), Vec::new());
        for k in 0..ITERS {
            match class {
                "idle" => {}
                "hover" => {
                    let (x, y) = at(&h, if k % 2 == 0 { "d_button_a" } else { "d_button_b" });
                    h.post(lumen_static::Event::PointerMove { x, y });
                }
                "press" => {
                    let (x, y) = at(&h, "d_button_a");
                    h.post(if k % 2 == 0 { lumen_static::Event::down(x, y) } else { lumen_static::Event::up(x + 400.0, y + 400.0) });
                }
                "tick" => {
                    h.tick(16.0);
                }
                "click" => {
                    let (x, y) = at(&h, "d_button_a");
                    h.click(x, y);
                }
                _ => unreachable!(),
            }
            h.pump_with(m);
            let (l, p, pa) = h.timings3();
            v.push((l + p + pa) as f64 / 1000.0);
            whole.push(h.stage_timings().pump as f64 / 1000.0);
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        whole.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (whole[whole.len() / 2], v[v.len() / 2])
    }

    #[test]
    #[ignore]
    fn measure_the_flip() {
        let Some(face) = FontStack::system() else { return };
        let mut m = FontMetrics::new(face);
        println!(
            "{:<8} {:<13} {:>16} {:>16} {:>16} {:>16}",
            "class", "entry", "Everything pump", "(l+p+p)", "Boundaries pump", "(l+p+p)"
        );
        for (class, entry) in [("idle", "Button"), ("hover", "Button"), ("press", "Button"), ("tick", "ProgressBar"), ("click", "Button")] {
            let e: Vec<(f64, f64)> = (0..RUNS).map(|_| run::<Everything>(entry, class, &mut m)).collect();
            let b: Vec<(f64, f64)> = (0..RUNS).map(|_| run::<Boundaries<256>>(entry, class, &mut m)).collect();
            let f = |v: &[(f64, f64)], k: usize| {
                let mut x: Vec<f64> = v.iter().map(|p| if k == 0 { p.0 } else { p.1 }).collect();
                x.sort_by(|a, b| a.partial_cmp(b).unwrap());
                format!("{:.2}-{:.2}", x[0], x[x.len() - 1])
            };
            println!("{class:<8} {entry:<13} {:>16} {:>16} {:>16} {:>16}", f(&e, 0), f(&e, 1), f(&b, 0), f(&b, 1));
        }
    }
}

/// **M37/T37.0 — what a byte boundary would cost per frame**
/// (`docs/plan-window-reload.md` §2.3). The kill rule, written before
/// this ran: if copying showcase's display list costs less than the frame
/// arm's fast-cluster spread, the library boundary is bytes; otherwise it
/// is borrowed pointers behind an ABI stamp.
///
/// The copy measured is a `memcpy` of the commands and the text into
/// buffers already allocated — the **lower bound** of any byte encoding,
/// since a stable encoding must also visit every field. If the lower
/// bound loses, every encoding loses.
#[cfg(test)]
mod boundary {
    use super::*;
    use lumen_static::display::Cmd;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    use lumen_static_shell::Host;

    const RUNS: usize = 8;
    const ITERS: usize = 40;

    #[test]
    #[ignore]
    fn measure_the_copy() {
        let Some(face) = FontStack::system() else { return };
        let mut m = FontMetrics::new(face);
        println!(
            "{:<13} {:>5} {:>7} {:>6} {:>18} {:>14} {:>14} {:>6}",
            "entry", "cmds", "mem B", "text", "frame µs", "memcpy µs", "wire µs", "wire B"
        );
        for entry in ["Button", "DataGrid", "Markdown", "ColorPicker"] {
            let e = CATALOG.iter().position(|x| x.name == entry).unwrap();
            let mut frames = Vec::new();
            let mut copies = Vec::new();
            let mut encodes = Vec::new();
            let mut wire_len = 0;
            let mut shape = (0, 0, 0);
            for _ in 0..RUNS {
                let mut h: Host<Show> = program(e, false).host();
                for _ in 0..3 {
                    h.pump_with(&mut m);
                }
                let mut cmds: Vec<Cmd> = Vec::with_capacity(4096);
                let mut text = String::with_capacity(1 << 16);
                let (mut f, mut c, mut en) = (Vec::new(), Vec::new(), Vec::new());
                let mut wire = Vec::with_capacity(1 << 16);
                let mut back = lumen_static::DisplayList::new();
                for k in 0..ITERS {
                    // The frame arm: a hover that moves, as T34.2's.
                    let (x, y) = if k % 2 == 0 { (40.0, 120.0) } else { (300.0, 200.0) };
                    h.post(lumen_static::Event::PointerMove { x, y });
                    h.pump_with(&mut m);
                    let (l, p, pa) = h.timings3();
                    f.push((l + p + pa) as f64 / 1000.0);
                    let t = std::time::Instant::now();
                    cmds.clear();
                    cmds.extend_from_slice(&h.list.cmds);
                    text.clear();
                    text.push_str(&h.list.text);
                    std::hint::black_box((&cmds, &text));
                    c.push(t.elapsed().as_nanos() as f64 / 1000.0);
                    let t = std::time::Instant::now();
                    wire.clear();
                    lumen_static::wire::encode(&h.list, &mut wire);
                    lumen_static::wire::decode_into(&wire, &mut back).unwrap();
                    std::hint::black_box(&back);
                    en.push(t.elapsed().as_nanos() as f64 / 1000.0);
                    wire_len = wire.len();
                    shape = (
                        h.list.cmds.len(),
                        h.list.cmds.len() * std::mem::size_of::<Cmd>() + h.list.text.len(),
                        h.list.text.len(),
                    );
                }
                let med = |mut v: Vec<f64>| {
                    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    v[v.len() / 2]
                };
                frames.push(med(f));
                copies.push(med(c));
                encodes.push(med(en));
            }
            frames.sort_by(|a, b| a.partial_cmp(b).unwrap());
            copies.sort_by(|a, b| a.partial_cmp(b).unwrap());
            encodes.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let r = |v: &[f64]| format!("{:.3}-{:.3}", v[0], v[v.len() - 1]);
            println!(
                "{entry:<13} {:>5} {:>7} {:>6} {:>18} {:>14} {:>14} {:>6}   spread {:.3} µs | memcpy min {:.3} | wire min {:.3}",
                shape.0, shape.1, shape.2, r(&frames), r(&copies), r(&encodes), wire_len,
                frames[RUNS - 1] - frames[0], copies[0], encodes[0]
            );
        }
        println!("size_of::<Cmd>() = {} B", std::mem::size_of::<Cmd>());
    }
}

/// **M40/T40.3's kill rule** (`docs/plan-physical-damage.md` §5), written
/// before the snap was built: showcase's *paint* time at scale 1.0, eight
/// runs a side, before and after `Scaled::rect` snaps its edges. If the
/// after-median lies outside the before-arm's spread, the rounding is
/// restricted to scales other than 1.0. Paint alone, because the snap is
/// in paint and nowhere else. `docs/results-gpu.md` §2a has the numbers.
#[cfg(test)]
mod snap_cost {
    use super::*;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    use lumen_static_shell::Host;

    const RUNS: usize = 8;
    const ITERS: usize = 40;

    #[test]
    #[ignore]
    fn measure_the_snap() {
        let Some(face) = FontStack::system() else { return };
        let mut m = FontMetrics::new(face);
        for entry in ["Button", "DataGrid"] {
            let e = CATALOG.iter().position(|x| x.name == entry).unwrap();
            let mut runs = Vec::new();
            let mut rects = (0, 0);
            for _ in 0..RUNS {
                let mut h: Host<Show> = program(e, false).host();
                for _ in 0..3 {
                    h.pump_with(&mut m);
                }
                let mut v = Vec::new();
                for k in 0..ITERS {
                    // A hover that moves, the frame arm T34.2 and T37.0 use.
                    let (x, y) = if k % 2 == 0 { (40.0, 120.0) } else { (300.0, 200.0) };
                    h.post(lumen_static::Event::PointerMove { x, y });
                    h.pump_with(&mut m);
                    v.push(h.timings3().2 as f64 / 1000.0);
                }
                rects = (
                    h.list.cmds.iter().filter(|c| matches!(c, lumen_static::Cmd::Rect { .. })).count(),
                    h.list.cmds.len(),
                );
                v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                runs.push(v[v.len() / 2]);
            }
            runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = (runs[RUNS / 2 - 1] + runs[RUNS / 2]) / 2.0;
            println!(
                "{entry:<9} Rect cmds {} of {}   paint µs per run (sorted) {:?}   spread {:.3}-{:.3}   median {:.3}",
                rects.0,
                rects.1,
                runs.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
                runs[0],
                runs[RUNS - 1],
                median
            );
        }

        // Showcase paints almost no plain rects (the count above), so the
        // entries cannot see a per-rect cost. The worst case, isolated:
        // 2 000 `rect` calls through `Scaled` at 1.0 into a list.
        let mut runs = Vec::new();
        let mut list = lumen_static::DisplayList::new();
        for _ in 0..RUNS {
            let mut v = Vec::new();
            for _ in 0..ITERS {
                list.clear();
                let t = std::time::Instant::now();
                {
                    let mut s = lumen_static::Scaled::new(&mut list, 1.0);
                    let p: &mut dyn lumen_static::Painter = &mut s;
                    for k in 0..2000 {
                        let y = (k % 400) as f32 * 1.7;
                        p.rect([3.0, y, 180.3, y + 1.0], lumen_static::Color(40, 40, 40, 255));
                    }
                }
                std::hint::black_box(&list);
                v.push(t.elapsed().as_nanos() as f64 / 1000.0);
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            runs.push(v[v.len() / 2]);
        }
        runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "2000 rects through Scaled(1.0): µs per run (sorted) {:?}   spread {:.3}-{:.3}   median {:.3}",
            runs.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
            runs[0],
            runs[RUNS - 1],
            (runs[RUNS / 2 - 1] + runs[RUNS / 2]) / 2.0
        );
    }
}


/// **`--demo` renders with the program's `Shaping`** — M81/T81.4
/// (`docs/plan-shaped-paint.md`, second review C1). Since T81.6 the
/// catalogue has a right-to-left entry, `TextInput RTL`, seeded with
/// [`catalog::TEXTINPUT_RTL`]: the `Complex` program's frame of it must
/// differ from the `Basic` one's. (Until then the `TextInput` demo was
/// given a Hebrew value through [`demo_frame`]'s hook.) The accept
/// direction: the Latin `TextInput` entry needs no shaping, and the two
/// modes draw it byte for byte.
#[cfg(all(test, feature = "shaping"))]
mod shaping_shot {
    use super::*;
    use lumen_static_shell::text::{FontStack, Shaping};

    fn differing(a: &(Vec<u8>, [usize; 4]), b: &(Vec<u8>, [usize; 4])) -> usize {
        assert_eq!(a.1, b.1, "the two crops differ, so the frames were not of one layout");
        let [x0, y0, x1, y1] = a.1;
        let mut n = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * W as usize + x) * 4;
                n += usize::from(a.0[i..i + 4] != b.0[i..i + 4]);
            }
        }
        n
    }

    #[test]
    fn demo_frames_follow_the_programs_shaping() {
        if FontStack::system().is_none() {
            assert!(std::env::var("LUMEN_REQUIRE_FONTS").as_deref() != Ok("1"), "LUMEN_REQUIRE_FONTS=1 but no system face");
            println!("no system face -- skipped");
            return;
        }
        let rtl = CATALOG.iter().position(|e| e.name == "TextInput RTL").expect("a TextInput RTL entry");
        let latin = CATALOG.iter().position(|e| e.name == "TextInput").expect("a TextInput entry");
        let seeded = {
            let mut h: lumen_static_shell::Host<Show> = program(rtl, false).host();
            h.pump();
            h.pump();
            matches!(&h.app.ui.main.stage.row.demo, Demo::TextInputRtl(d) if d.a.as_str() == catalog::TEXTINPUT_RTL)
        };
        assert!(seeded, "setup: the TextInput RTL demo does not hold {:?}", catalog::TEXTINPUT_RTL);
        let frame = |k: usize, mode: Shaping| demo_frame(&program(k, false).shaping(mode), |_| {});
        println!("program shaping: {:?} and {:?}; built-in default {SHAPING:?}",
                 program(rtl, false).shaping(Shaping::Complex).shaping, program(rtl, false).shaping(Shaping::Basic).shaping);
        let he = differing(&frame(rtl, Shaping::Complex), &frame(rtl, Shaping::Basic));
        let la = differing(&frame(latin, Shaping::Complex), &frame(latin, Shaping::Basic));
        println!("--demo {rtl} (TextInput RTL): Complex vs Basic {he} px differ (bound > 0); \
                  --demo {latin} (TextInput, Latin): {la} px differ (bound 0)");
        assert!(he > 0, "the Complex program's --demo frame equals the Basic one: demo_frame ignores the program's Shaping");
        assert_eq!(la, 0, "a Latin frame moved under Complex, which must lay out plain Latin as Basic does");
    }
}
