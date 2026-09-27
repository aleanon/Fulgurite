//! **The widget showcase** — every built-in widget, one at a time.
//!
//! `just example showcase`. A picker at the top lists the whole
//! catalogue grouped by kind; choosing one drops it, seeded with real
//! data, into the middle of the stage. Widgets that carry state print a
//! live readout under the picker, so a slider drag or a checkbox toggle
//! can be watched landing in the store.
//!
//! Modelled on Lumen's `examples/widget_showcase`, which is where the
//! shape of this comes from.
//!
//! # How "one of N" works in a static tree
//!
//! This is the interesting part, and it is the one place a static tree
//! needs a hatch. The tree's topology is its type, so a stage that can
//! hold any of twenty demos cannot simply *become* each of them.
//!
//! Three ways out, and the third is the one used here:
//!
//! 1. **Every demo in the tree at once, all but one hidden.** Honest and
//!    allocation-free, and what `Either` does for two. For twenty it
//!    means nineteen nested `Either`s, and the merged const table is
//!    bounded by `MAX_EITHER_NODES` (64) per instantiation — the nesting
//!    blows it long before twenty.
//! 2. **A stylesheet rebuilt on every selection**, flipping
//!    `Style::visible`. The cascade can express it, but the whole tree
//!    still pays layout for twenty demos on every frame.
//! 3. **`Dyn`** — a boxed `dyn Node` island with `replace`. One node in
//!    the parent's table, its content swapped when the picker changes.
//!    This is precisely what M13/T13.4 put the escape hatch behind the
//!    `alloc` feature *for*: the cost of dynamism is named in the type
//!    system, and a `--no-default-features` build does not have it.
//!
//! So the showcase is a demonstration of the hatch as much as of the
//! widgets, and an application that needs a fixed set of screens should
//! still reach for `Either`.

use lumen_static::display::Color;
use lumen_static::prelude::*;
use lumen_static::Rule as StyleRule;
use lumen_static_widgets::select::{Row as SelectRow, Select};
use lumen_static::style::Palette;
use lumen_static_widgets::Toggle;
use lumen_static_widgets::prelude::*;
use lumen_static_winit::{application, Program, Run};

mod catalog;
mod stage;
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
    // A field's and the trigger's pale ground: `Style` has no border, so
    // the fill is what separates a control from the page.
    control: Color(0xf0, 0xf2, 0xf6, 0xff),
    text: Color(0x1b, 0x22, 0x30, 0xff),
    muted: Color(0x6b, 0x74, 0x88, 0xff),
    accent: Color(0x2f, 0x6b, 0xff, 0xff),
    on_accent: Color(0xff, 0xff, 0xff, 0xff),
    focus: Color(0x2f, 0x6b, 0xff, 0xff),
};

const SHOW_DARK: Palette = Palette {
    page: Color(0x12, 0x15, 0x1c, 0xff),
    surface: Color(0x1c, 0x21, 0x2b, 0xff),
    control: Color(0x2a, 0x30, 0x3c, 0xff),
    text: Color(0xe6, 0xea, 0xf2, 0xff),
    muted: Color(0x94, 0xa0, 0xb4, 0xff),
    accent: Color(0x5b, 0x8c, 0xff, 0xff),
    on_accent: Color(0x0a, 0x0e, 0x18, 0xff),
    focus: Color(0x82, 0xaa, 0xff, 0xff),
};

/// The palette to start in: [`SHOW_DARK`] under `LUMEN_PALETTE=dark`.
fn palette() -> Palette {
    match std::env::var("LUMEN_PALETTE").as_deref() {
        Ok("dark") => SHOW_DARK,
        _ => SHOW_LIGHT,
    }
}

/// The colours with no role, kept as values on purpose: a hairline is
/// neither text nor a control's body, and a ghost button's tint is one
/// button's. They stay put under every palette, which the
/// `every_role_rule_follows_the_palette` test prints rather than hides.
const HAIRLINE: Color = Color(0xdf, 0xe4, 0xed, 0xff);
const GHOST: Color = Color(0xea, 0xef, 0xfb, 0xff);

const W: u32 = 960;
const H: u32 = 720;
/// Rows the dropdown shows before it scrolls.
const PANEL_H: f32 = 560.0;

// ---- the tree --------------------------------------------------------

/// The header band: the trigger, what it is, and a live readout.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
pub struct Header {
    #[widget_id("title")]
    title: Label,
    /// **The dropdown, as one field.** Its trigger, its popup and its
    /// rows are `Select`'s four nodes; the option count is data, so
    /// adding a widget to `CATALOG` no longer adds a field here.
    #[widget_id("picker")]
    picker: Select<Store>,
    #[widget_id("blurb")]
    blurb: Label<String>,
    #[widget_id("readout")]
    readout: Label<String>,
}

/// The stage: whatever the picker chose, centred by two `FILL` spacers.
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
#[widget(column, spacing = 18.0)]
pub struct Stage {
    #[widget_id("st")]
    top: Spacer,
    row: StageRow,
    /// The subtlety under the demo — what makes Lumen's showcase teach
    /// rather than merely list.
    #[widget_id("note")]
    note: Label<String>,
    #[widget_id("sb")]
    bottom: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Body {
    #[widget_id("header")]
    header: Header,
    #[widget_id("hr")]
    hr: Divider,
    stage: Stage,
}

/// The whole window.
///
/// **No overlay layer any more.** The dropdown used to be a second
/// child of a stack, positioned under the trigger by four `Spacer`s,
/// because nothing else could put a panel over the stage. `Select`'s
/// menu is a `Popup`: out of flow, painted in the overlay phase, hit
/// first. So the tree is the body, and the four spacers, the wrapper row
/// and the wrapper column are gone with it.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Screen {
    body: Body,
}

#[derive(DeriveApp, Default)]
#[app(tick = advance, after_update = refresh)]
pub struct Show {
    #[tree]
    ui: Screen,
    /// **What a handler may touch** — M33/T33.4. Every handler in this
    /// example — the demos' buttons and toggles, the picker's trigger,
    /// its rows and its dismissal — takes `&mut Store`, and the tree is
    /// not reachable from it. What a handler used to write into the tree
    /// (the note, the trigger's label, the blurb, the readout, the demo on
    /// the stage) is written by `refresh`, the `after_update` hook, which
    /// keeps `&mut Show` — so showcase stays on the conservative path M29
    /// §2.4 records for `after_update`, by design.
    #[state]
    #[bind(ui.body.header.picker.menu.open, menu_open)]
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
    /// Set by a handler that changed what the header shows — the
    /// selection or the dropdown's state — so `refresh` rebuilds it. A
    /// handler cannot write the header itself: it is on the tree.
    needs_rebuild: bool,
    /// **Whether the dropdown is open — the application's, since
    /// `Select` became controlled** (M33/T33.3). Bound into the popup; the
    /// trigger's `on_click` sets it, a chosen row and the pass's
    /// dismissal (`on_dismiss`) clear it.
    menu_open: bool,
    /// The live demo's own state. One store rather than one per demo,
    /// because a showcase only ever has one demo alive.
    presses: u32,
    flag: bool,
    fraction: f32,
    /// The stage's own `Select` (M44/T44.10): whether it is open — the
    /// application's, as for the header's — and what it last chose.
    select_open: bool,
    select_choice: Option<usize>,
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
    // **The toggles show the flag they are bound to** (M48/T48.4). A
    // toggle does not flip itself: its `on` is a presentation copy the
    // application writes back. The seed wrote it once, on selection, and
    // nothing wrote it again -- so a click flipped `flag`, the readout
    // followed, and the box, the switch and the radio stayed as seeded.
    // Found when the drive began failing a repainting action that painted
    // nothing over its widget; the old damage check skipped it.
    let flag = s.store.flag;
    let pair = |a: &mut bool, b: &mut bool| {
        let moved = *a != flag || *b == flag;
        *a = flag;
        *b = !flag;
        moved
    };
    changed |= match &mut s.ui.body.stage.row.demo {
        Demo::CheckBox(d) => pair(&mut d.a.t.on, &mut d.b.t.on),
        Demo::Switch(d) => pair(&mut d.a.t.on, &mut d.b.t.on),
        Demo::Radio(d) => pair(&mut d.a.t.on, &mut d.b.t.on),
        _ => false,
    };
    // **Read the typed fields back before reporting.** This is what a
    // `Dyn` island could not do: a `Slider` owns its value, and through
    // a `Box<dyn Node>` there was no handle to ask.
    if let Demo::Slider(d) = &s.ui.body.stage.row.demo {
        let v = d.a.value();
        if v != s.store.fraction {
            s.store.fraction = v;
            changed = true;
        }
    }
    // **The dismissal request, taken.** A veil never closes itself — it
    // asks, and the application decides. Polling here is where an
    // application does it: `after_update` runs on every pump, which is
    // exactly when the request can have been made (M27/T27.7).
    // The stage's `Select` is controlled: its `open` is bound here by
    // hand, because it lives in one variant of the demo switch and a
    // `#[bind]` path cannot name a variant.
    if let Demo::Select(d) = &mut s.ui.body.stage.row.demo {
        if d.a.menu.open != s.store.select_open {
            d.a.menu.open = s.store.select_open;
            changed = true;
        }
        let want = s.store.select_choice.map_or("Choose a fruit", |k| catalog::FRUIT[k]);
        if d.a.trigger.label != want {
            d.a.trigger.label = want.into();
            changed = true;
        }
    }
    let asked = match &mut s.ui.body.stage.row.demo {
        Demo::Modal(d) => d.a.take_dismissed(),
        Demo::Sheet(d) => d.a.take_dismissed(),
        Demo::Drawer(d) => d.a.take_dismissed(),
        _ => false,
    };
    if asked {
        s.store.presses += 1;
        changed = true;
    }
    let e = &CATALOG[s.store.selected];
    let text = (e.status)(s);
    // `set_str` compares before writing and answers whether the text
    // differed, so the readout reports for itself rather than being
    // assumed dirty — and the measurement memo survives an unchanged one.
    changed | s.ui.body.header.readout.set_str(&text)
}

/// Advance what animates, and keep the live demo in step with the store.
///
/// A `dt` of zero must change nothing and report whether anything is
/// moving — `App::tick`'s contract, which the frame loop relies on to
/// decide whether to ask for another frame.
fn advance(s: &mut Show, dt_ms: f32) -> bool {
    let e = &CATALOG[s.store.selected];
    if !e.animated {
        return false;
    }
    if dt_ms > 0.0 {
        s.store.phase = (s.store.phase + dt_ms / 1400.0).rem_euclid(1.0);
        (e.seed)(s);
        let text = (e.status)(s);
        s.ui.body.header.readout.set(text);
    }
    true
}

/// Put the selected entry on the stage and bring the header into step.
fn rebuild(s: &mut Show) {
    let e = &CATALOG[s.store.selected];
    s.ui.body.stage.note.set(e.note.into());
    s.store.needs_seed = true;
    // **ASCII, not `\u{25be}`.** The system font has no chevron and
    // `examples/glasswall` already paid for that lesson: a missing glyph
    // renders as the fallback box, which looks like a bug in the widget
    // rather than in the font.
    let open = s.store.menu_open;
    s.ui.body.header.picker.trigger.label =
        format!("{}      {}", e.name, if open { "^" } else { "v" });
    s.ui.body.header.blurb.set(e.blurb.into());
    (e.seed)(s);
    let text = (e.status)(s);
    s.ui.body.header.readout.set(text);
}

/// A row's caption, with its group folded in so the list reads as
/// grouped without the panel needing heading nodes of a second type.
///
/// **One `fn` for every row.** It cannot capture — `Repeat::bind` is a
/// plain `fn` pointer, which is what lets a hot reload re-supply it —
/// and it does not need to: [`EventCtx::row`] tells the handler which
/// row was hit, recomputed by hit testing every frame, so it cannot go
/// stale the way a captured index can.
///
/// This replaces `pick0`..`pick17` and a `PICKERS` table: eighteen
/// functions whose only difference was a literal.
fn bind_row(r: &mut SelectRow<Store>, i: usize) {
    let e = &CATALOG[i];
    let first = i == 0 || CATALOG[i - 1].group != e.group;
    r.label = if first {
        format!("{} - {}", e.group, e.name)
    } else {
        format!("        {}", e.name)
    };
    r.on_click = |s: &mut Store, cx: &EventCtx| {
        let i = cx.row.map_or(0, |r| r as usize);
        choose(s, i);
    };
}

/// Put entry `i` on the stage and close the dropdown.
///
/// **State only** (M33/T33.4): the header and the stage are on the tree,
/// which a handler cannot reach, so `refresh` rebuilds them on this same
/// pump.
fn choose(s: &mut Store, i: usize) {
    s.selected = i.min(CATALOG.len() - 1);
    s.menu_open = false;
    s.phase = 0.0;
    s.needs_rebuild = true;
}

/// Build the app, already on `entry`, with the dropdown `open` or not.
fn build(entry: usize, open: bool) -> Show {
    let mut app = Show::default();
    app.ui.body.header.title.set("Widget showcase");
    // **The whole wiring of the dropdown.** One `fn` for the rows, the
    // count as data, and nothing about eighteen.
    app.ui.body.header.picker = Select::new(CATALOG.len(), ROW_H, bind_row);
    // Controlled (M33/T33.3): opening and dismissal write the state, and
    // the header's chevron follows it.
    app.ui.body.header.picker.trigger.on_click = |s: &mut Store, _: &EventCtx| {
        s.menu_open = true;
        s.needs_rebuild = true;
    };
    app.ui.body.header.picker.on_dismiss = |s: &mut Store, _: &EventCtx| {
        s.menu_open = false;
        s.needs_rebuild = true;
    };
    app.store.selected = entry.min(CATALOG.len() - 1);
    app.store.menu_open = open;
    // **Mid-travel, not zero.** The animated demos take their value from
    // `phase`, and a showcase opened on one of them at phase 0 shows an
    // empty bar and an unlit spinner — which reads as a broken widget
    // rather than as a starting state.
    app.store.phase = 0.35;
    rebuild(&mut app);
    app
}

/// One frame, headless — what the tests and the screenshots use.
fn shot(path: &str, entry: usize, open: bool) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    let mut host: lumen_static_shell::Host<Show> = program(entry, open).host();
    let mut metrics = FontStack::system().map(FontMetrics::new);
    for _ in 0..2 {
        match metrics.as_mut() {
            Some(m) => host.pump_with(m),
            None => host.pump(),
        };
    }
    let mut c = match FontStack::system() {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    };
    c.clear(palette().page);
    lumen_static::Renderer::render(&mut c, &host.list, W, H);
    // M50/T50.2: the glyph cache on a real screen. A second, warm frame
    // must rasterise nothing and draw nothing uncached
    // (`docs/plan-raster-allocation.md` §5's risk).
    let cold = c.glyph_cache_stats();
    c.clear(palette().page);
    lumen_static::Renderer::render(&mut c, &host.list, W, H);
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

/// The frame a popup opens on — M26/T26.0.
///
/// `just profile` is fixed to counter-window's tally and takes no
/// arguments, so M26/T26.11 — whose whole subject is that opening a
/// popup damages the entire window — had no arm at all. This is one: it
/// settles a closed frame, opens the picker, pumps, and prints the
/// damaged area beside the window's. Today that ratio is 100%, because a
/// command-count change answers `Damage::All`; after T26.11 it must be
/// the popup's rect, and this is the number that says so.
fn profile_popup() {
    let mut host: lumen_static_shell::Host<Show> =
        program(0, false).host();
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

    host.app.store.menu_open = true;
    rebuild(&mut host.app);
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
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    let mut host: lumen_static_shell::Host<Show> =
        program(entry, false).host();
    let mut metrics = FontStack::system().map(FontMetrics::new);
    for _ in 0..2 {
        match metrics.as_mut() {
            Some(m) => host.pump_with(m),
            None => host.pump(),
        };
    }
    let mut c = match FontStack::system() {
        Some(f) => Canvas::with_font(W, H, f),
        None => Canvas::new(W, H),
    };
    c.clear(palette().page);
    lumen_static::Renderer::render(&mut c, &host.list, W, H);

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
    let (cw, chh) = (x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1));
    let mut out = format!("P6\n{cw} {chh}\n255\n").into_bytes();
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * W as usize + x) * 4;
            out.extend_from_slice(&c.pixels[i..i + 3]);
        }
    }
    std::fs::write(path, out).expect("write");
    println!("{path}: {} — {cw}x{chh} at ({x0},{y0})", CATALOG[entry].name);
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

    let top = host.states[STAGE_NODE].pos.y;
    let mut targets: Vec<(f32, f32, usize)> = host
        .states
        .iter()
        .enumerate()
        .filter(|(_, st)| st.pos.y >= top && st.size.w > 0.0 && st.size.w <= 300.0 && st.size.h > 0.0)
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
    let top = host.states[STAGE_NODE].pos.y;
    let btns: Vec<usize> = (0..<Screen as Widget>::NODE_COUNT)
        .filter(|&i| {
            let s = &host.states[i];
            s.pos.y >= top && s.size.w > 40.0 && s.size.w <= 300.0 && s.size.h > 20.0
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
    let mut program = program(start.min(CATALOG.len() - 1), false).title("showcase");
    // Built `--features agent`, the window can be steered from outside:
    // `input.click` on the picker, `ui.screenshot` of the result.
    // `LUMEN_AGENT_ADDR` overrides the address, read by the window
    // (M48/T48.11).
    if let Some(addr) = agent_addr() {
        program = program.agent(addr);
    }
    program.run();
}

/// **The program, described once** — M32 (`docs/plan-program.md`).
///
/// The window, every `--shot`/`--demo`/`--press` frame, the profile and
/// the tests are all built by `program(..).host()`, so a shot is the
/// window's frame. The boot captures `entry` and `open`: showcase opens
/// on an entry read from argv, which is why a boot may capture.
fn program(entry: usize, open: bool) -> Program<Show> {
    application(move || build(entry, open))
        .size(W, H)
        .page(palette().page)
        .palette(palette())
        .style(|_: &Show| stylesheet())
}

/// Where to serve the agent endpoint, or `None` without the feature.
#[cfg(feature = "agent")]
fn agent_addr() -> Option<String> {
    Some("127.0.0.1:7654".into())
}

#[cfg(not(feature = "agent"))]
fn agent_addr() -> Option<String> {
    None
}

/// The `Stage` node's index — where the header stops.
const STAGE_NODE: usize = 8;

/// One dropdown row's height.
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

fn stylesheet() -> Vec<StyleRule> {
    use lumen_static::style::{Patch, Role, Selector};
    vec![
        StyleRule::new(Selector::Kind("Screen"), Patch::new().background_role(Role::Page)),
        StyleRule::new(
            Selector::Id("header"),
            Patch::new()
                .background_role(Role::Surface)
                .padding(Edges { left: 28.0, right: 28.0, top: 20.0, bottom: 18.0 }),
        ),
        StyleRule::new(Selector::Id("title"), Patch::new().color_role(Role::Text).font_size(20.0).font_weight(700.0)),
        // **The picker's parts, by the ids `Select` stamps.** A
        // composite is more than one node, and the derive only names a
        // *field* — so a widget that is several nodes has to publish its
        // own ids or nothing can reach inside it, not a stylesheet and
        // not the agent. `select::PART_*` are those names.
        StyleRule::new(
            Selector::Id(lumen_static_widgets::select::PART_TRIGGER),
            Patch::new()
                .width(Length::Fixed(300.0))
                .background_role(Role::Control)
                .color_role(Role::Text)
                .radius(8.0)
                .font_size(15.0)
                .padding(Edges { left: 14.0, right: 14.0, top: 9.0, bottom: 9.0 }),
        ),
        StyleRule::new(Selector::Id("blurb"), Patch::new().color_role(Role::Muted).font_size(13.0)),
        StyleRule::new(
            Selector::Id("readout"),
            Patch::new().color_role(Role::Accent).font_size(13.0).font_weight(600.0),
        ),
        StyleRule::new(Selector::Id("hr"), Patch::new().background(HAIRLINE)),
        // The stage: the demo centred in what is left.
        StyleRule::new(Selector::Kind("Stage"), Patch::new().height(Length::FILL)),
        StyleRule::new(Selector::Id("st"), Patch::new().height(Length::FILL)),
        StyleRule::new(Selector::Id("sb"), Patch::new().height(Length::FILL)),
        StyleRule::new(Selector::Id("sl"), Patch::new().width(Length::FILL)),
        StyleRule::new(Selector::Id("sr"), Patch::new().width(Length::FILL)),
        // The menu panel. **No spacers.** It used to take four — two
        // to push a layer down and across to meet the trigger — because
        // the panel lived in a stack beside the body and had to be
        // aimed by hand. A `Popup` anchors itself on its parent, so the
        // position is the widget's and the sheet only says how big.
        //
        // **No `ScrollView` around it**, and that is a scope note
        // rather than a preference: a scroller lays its content out
        // against the limits *it* was given, not against its own
        // resolved box, so an 18-row list inside a 300 px scroller came
        // out 932 px wide. The catalogue fits the panel today; a list
        // that outgrows it needs that asymmetry addressed first.
        StyleRule::new(
            Selector::Id(lumen_static_widgets::select::PART_MENU),
            Patch::new()
                .width(Length::Fixed(300.0))
                .height(Length::Fixed(PANEL_H))
                .background_role(Role::Surface)
                .radius(10.0),
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
            btn().background_role(Role::Surface).color_role(Role::Text),
        ),
        // **Ghost has a ground, faint as it is.** `pressed_dim` and
        // `hover_dim` *scale* a background; they cannot invent one. A
        // fully transparent button therefore shows no press at all —
        // which is a third of the reported "clicking does nothing".
        StyleRule::new(
            Selector::Id("d_button_c"),
            btn().background(GHOST).color_role(Role::Accent),
        ),
        // ---- M27/T27.4's three -------------------------------------
        // The card is sized so the stage can show it whole; its default
        // width is FILL, which is right for a card in a column and wrong
        // for one on a centring stage.
        StyleRule::new(
            Selector::Id("d_card_a"),
            Patch::new().width(Length::Fixed(280.0)),
        ),
        StyleRule::new(Selector::Id("d_card_title"), Patch::new().font_weight(600.0)),
        // **Buttons need a ground.** `pressed_dim` and `hover_dim` scale
        // a background; they cannot invent one, and a fully transparent
        // button shows no press at all — a third of the showcase's
        // "clicking does nothing" (`docs/plan-widgets-gallery.md` §3b).
        StyleRule::new(Selector::Id("d_card_go"), btn().background_role(Role::Accent)
            .color_role(Role::OnAccent)),
        StyleRule::new(
            Selector::Id(lumen_static_widgets::surface::PART_CONTENT),
            btn().background_role(Role::Accent).color_role(Role::OnAccent),
        ),
        // The field: a ground, a rounded edge, room for a caret.
        StyleRule::new(
            Selector::Id("d_textinput_a"),
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
                .background_role(Role::Control)
                .color_role(Role::Text)
                .border_width(1)
                .border_color_role(Role::Muted)
                .radius(8.0)
                .font_size(15.0)
                .padding(Edges { left: 12.0, right: 12.0, top: 9.0, bottom: 9.0 }),
        ),
        // The multi-line field, the same ground, three lines tall
        // (M45/T45.7).
        StyleRule::new(
            Selector::Id("d_textarea_a"),
            Patch::new()
                .width(Length::Fixed(260.0))
                .height(Length::Fixed(96.0))
                .background_role(Role::Control)
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
        StyleRule::new(Selector::Class("pair-label"), lbl()),
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
                .background_role(Role::Control)
                .color_role(Role::Text)
                .border_width(1)
                .border_color_role(Role::Muted)
                .radius(8.0)
                .padding(Edges { left: 12.0, right: 12.0, top: 8.0, bottom: 8.0 }),
        ),
        // M27/T27.8's four. A wrap needs a box narrow enough to wrap in,
        // a grid needs one wide enough to show that it does *not*, and a
        // split pane is as big as it is given.
        StyleRule::new(Selector::Kind("DWrap"), Patch::new().width(Length::Fixed(260.0))),
        StyleRule::new(Selector::Kind("DGrid"), Patch::new().width(Length::Fixed(420.0))),
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
        StyleRule::new(Selector::Id("d_datagrid_a"), Patch::new().height(Length::Fixed(220.0))),
        // A tree is a *pane*: a file tree lives in a sidebar with a size,
        // and it is the sidebar that stops a collapse from moving the
        // rest of the window. Without one the stage centres a
        // shrink-wrapped tree, so folding two rows away moves the demo
        // 26 px up and repaints everything below the header — 11.94x the
        // stage, measured (M27/T27.10).
        StyleRule::new(Selector::Id("d_tree_a"), Patch::new().height(Length::Fixed(200.0))),
        // A pane grid fills what it is given, and what the stage gives
        // it is the whole column — which squeezed the note under it to
        // zero height and `W1001` said so, on the first drive. A pane
        // grid in an application fills a window; in a catalogue of
        // fifty-two demos it gets a pane's worth.
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
        // **The menu rows, by kind.** A `Repeat`'s rows live in an
        // arena, off the main table, and are styled from the *template*
        // — which carries no `#[widget_id]`, so `Kind` is the only
        // selector that reaches them. It works here because every other
        // `Button` in this sheet is addressed by id and an id beats a
        // kind; it would not survive an un-named button being added.
        // `docs/plan-select.md` T24.4 is the per-row addressing that
        // would make this exact rather than merely true.
        StyleRule::new(
            Selector::Kind("button"),
            Patch::new()
                .width(Length::FILL)
                .height(Length::Fixed(ROW_H))
                .background(Color(0, 0, 0, 0))
                .color_role(Role::Text)
                .font_size(13.0)
                .radius(0.0)
                .hover_dim(0.0)
                .padding(Edges { left: 14.0, right: 10.0, top: 6.0, bottom: 6.0 }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumen_static_shell::text::{FontMetrics as FontMetrics2, FontStack as FontStack2};
    use lumen_static::display::Cmd;
    use lumen_static::style::Selector;

    /// An `EventCtx` for a press, for calling a handler directly.
    fn press_at(x: f32, y: f32) -> EventCtx {
        EventCtx {
            event: Event::down(x, y),
            node: 0,
            row: None,
            inner: 0,
            pos: lumen_static::Point { x, y },
            local: lumen_static::Point { x, y },
            button: PointerButton::Left,
            clicks: 1,
            mods: Mods::NONE,
        }
    }

    /// The `Stage` node's index. Its `pos.y` is where the header stops
    /// and the demo's own commands start.
    const STAGE: usize = 8;

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
        let Demo::DataGrid(d) = &h.app.ui.body.stage.row.demo else {
            panic!("choosing entry {entry} did not put a grid on the stage");
        };
        // Node 1 of `GridRow` is the first cell, node 3 the second.
        let want = d.a.column_width(320.0);
        let (c0, c1) = (d.a.rows.instance_state(0, 1), d.a.rows.instance_state(0, 3));
        println!("column {want}, cells {} and {} px wide", c0.size.w, c1.size.w);
        assert!(want > 0.0, "setup: the grid reported a zero-wide column");
        for (k, c) in [(0, c0), (1, c1)] {
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
        let before = match &h.app.ui.body.stage.row.demo {
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
        let after = match &h.app.ui.body.stage.row.demo {
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
    fn the_menu_has_a_row_for_every_widget() {
        let app = build(0, true);
        let rows = app.ui.body.header.picker.count();
        println!("{rows} rows, {} demos, {} entries", stage::DEMOS, CATALOG.len());
        assert_eq!(rows, CATALOG.len(), "the menu lists a different number of options");
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

        const ROLES: [Role; 8] = [
            Role::Page,
            Role::Surface,
            Role::Control,
            Role::Text,
            Role::Muted,
            Role::Accent,
            Role::OnAccent,
            Role::Focus,
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
            let top = h.states[STAGE].pos.y;
            let demo = h
                .list
                .cmds
                .iter()
                .filter(|c| match c {
                    Cmd::Rect { rect, .. }
                    | Cmd::RoundedRect { rect, .. }
                    | Cmd::Text { rect, .. } => rect[1] >= top,
                    Cmd::Line { a, .. } => a.y >= top,
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
            seen.push((e.name, core::mem::discriminant(&h.app.ui.body.stage.row.demo)));
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
            if e.name != "Label" {
                let shapes = h
                    .list
                    .cmds
                    .iter()
                    .filter(|c| match c {
                        Cmd::Rect { rect, .. } | Cmd::RoundedRect { rect, .. } => rect[1] >= top,
                        Cmd::Line { a, .. } => a.y >= top,
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
        let (shut, open) = (host(0, false), host(0, true));
        let (a, b) = (shut.list.cmds.len(), open.list.cmds.len());
        println!("closed {a} commands, open {b}");
        assert!(open.app.ui.body.header.picker.is_open(), "an open picker is not open");
        assert!(!shut.app.ui.body.header.picker.is_open(), "a closed picker is open");
        // **The rows the menu *materialized*, not the catalogue's length.**
        // Those were the same number until M27/T27.4 pushed the
        // catalogue past what the menu can show; after that this bound
        // was computed from the wrong quantity and failed at 23 extra
        // commands for 24 entries — which is the honest answer, because
        // only eighteen rows exist to draw. `Repeat` reports its own
        // window, so the bound now comes from the thing it bounds.
        let rows = open.app.ui.body.header.picker.menu.content.window_len();
        assert!(
            rows > 0 && rows <= CATALOG.len(),
            "the menu materialized {rows} rows of {} — setup",
            CATALOG.len()
        );
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
            matches!(h.app.ui.body.stage.row.demo, Demo::Button(_)),
            "the Button demo is not on the stage"
        );
        let node = (0..<Screen as Widget>::NODE_COUNT)
            .find(|&i| {
                let s = &h.states[i];
                s.size.w > 40.0 && s.size.h > 20.0 && s.pos.y > 200.0 && s.size.w < 200.0
            })
            .expect("no button-shaped node on the stage");
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
        let top = h.states[STAGE_NODE].pos.y;
        let btns: Vec<usize> = (0..<Screen as Widget>::NODE_COUNT)
            .filter(|&i| {
                let s = &h.states[i];
                s.pos.y >= top && s.size.w > 40.0 && s.size.w <= 300.0 && s.size.h > 20.0
            })
            .collect();
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
        let top = h.states[STAGE_NODE].pos.y;
        let btns: Vec<usize> = (0..<Screen as Widget>::NODE_COUNT)
            .filter(|&i| {
                let s = &h.states[i];
                s.pos.y >= top && s.size.w > 40.0 && s.size.w <= 300.0 && s.size.h > 20.0
            })
            .collect();
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
    #[test]
    fn picking_a_row_swaps_the_demo() {
        let mut s = build(0, true);
        assert_eq!(s.store.selected, 0);
        let mut row = SelectRow::<Store>::default();
        bind_row(&mut row, 15);
        let mut cx = press_at(0.0, 0.0);
        cx.row = Some(15);
        // The handler gets the state (M33); the binding step then closes
        // the popup, as the next pump would.
        (row.on_click)(&mut s.store, &cx);
        lumen_static::event::App::update(&mut s);
        println!("after picking row 15: {} ({})", CATALOG[s.store.selected].name, s.store.selected);
        assert_eq!(s.store.selected, 15, "the handler picked the wrong entry");
        assert!(!s.ui.body.header.picker.is_open(), "picking a row left the dropdown open");
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
