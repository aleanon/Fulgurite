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
use lumen_static::select::{Row as SelectRow, Select};
use lumen_static::style::Palette;
use lumen_static::Toggle;
use lumen_static_widgets::prelude::*;
use lumen_static_winit::{run, Config};

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
    picker: Select<Show>,
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
    /// The live demo's own state. One store rather than one per demo,
    /// because a showcase only ever has one demo alive.
    presses: u32,
    flag: bool,
    fraction: f32,
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
    if s.needs_seed {
        s.needs_seed = false;
        let e = &CATALOG[s.selected];
        (e.seed)(s);
        changed = true;
    }
    // **Read the typed fields back before reporting.** This is what a
    // `Dyn` island could not do: a `Slider` owns its value, and through
    // a `Box<dyn Node>` there was no handle to ask.
    if let Demo::Slider(d) = &s.ui.body.stage.row.demo {
        let v = d.a.value();
        if v != s.fraction {
            s.fraction = v;
            changed = true;
        }
    }
    let e = &CATALOG[s.selected];
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
    let e = &CATALOG[s.selected];
    if !e.animated {
        return false;
    }
    if dt_ms > 0.0 {
        s.phase = (s.phase + dt_ms / 1400.0).rem_euclid(1.0);
        (e.seed)(s);
        let text = (e.status)(s);
        s.ui.body.header.readout.set(text);
    }
    true
}

/// Put the selected entry on the stage and bring the header into step.
fn rebuild(s: &mut Show) {
    let e = &CATALOG[s.selected];
    s.ui.body.stage.note.set(e.note.into());
    s.needs_seed = true;
    // **ASCII, not `\u{25be}`.** The system font has no chevron and
    // `examples/glasswall` already paid for that lesson: a missing glyph
    // renders as the fallback box, which looks like a bug in the widget
    // rather than in the font.
    let open = s.ui.body.header.picker.is_open();
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
fn bind_row(r: &mut SelectRow<Show>, i: usize) {
    let e = &CATALOG[i];
    let first = i == 0 || CATALOG[i - 1].group != e.group;
    r.label = if first {
        format!("{} - {}", e.group, e.name)
    } else {
        format!("        {}", e.name)
    };
    r.on_click = |s: &mut Show, cx: &EventCtx| {
        let i = cx.row.map_or(0, |r| r as usize);
        choose(s, i);
    };
}

/// Put entry `i` on the stage and close the dropdown.
fn choose(s: &mut Show, i: usize) {
    s.selected = i.min(CATALOG.len() - 1);
    // Closing is the application's, deliberately: `Select` owns
    // *opening* (a press on the trigger, in `on_event`) and dismissal
    // (the pass, on Escape or a press outside), but what a chosen row
    // means is not the widget's business.
    s.ui.body.header.picker.close();
    s.phase = 0.0;
    rebuild(s);
}

/// Build the app, already on `entry`, with the dropdown `open` or not.
fn build(entry: usize, open: bool) -> Show {
    let mut app = Show::default();
    app.ui.body.header.title.set("Widget showcase");
    // **The whole wiring of the dropdown.** One `fn` for the rows, the
    // count as data, and nothing about eighteen.
    app.ui.body.header.picker = Select::new(CATALOG.len(), ROW_H, bind_row);
    app.selected = entry.min(CATALOG.len() - 1);
    if open {
        app.ui.body.header.picker.open();
    }
    // **Mid-travel, not zero.** The animated demos take their value from
    // `phase`, and a showcase opened on one of them at phase 0 shows an
    // empty bar and an unlit spinner — which reads as a broken widget
    // rather than as a starting state.
    app.phase = 0.35;
    rebuild(&mut app);
    app
}

/// One frame, headless — what the tests and the screenshots use.
fn shot(path: &str, entry: usize, open: bool) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    let mut host: lumen_static_shell::Host<Show> = lumen_static_shell::Host::new(build(entry, open), W, H);
    host.set_palette(palette());
    host.set_stylesheet(&stylesheet());
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
        lumen_static_shell::Host::new(build(0, false), W, H);
    host.set_palette(palette());
    host.set_stylesheet(&stylesheet());
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

    host.app.ui.body.header.picker.open();
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
        "   commands {before} -> {} (a length change is Damage::All today — M26/T26.11); \
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
        lumen_static_shell::Host::new(build(entry, false), W, H);
    host.set_palette(palette());
    host.set_stylesheet(&stylesheet());
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

    // The union of every node with a real box below the header — the
    // demo row and nothing else.
    let top = host.states[STAGE_NODE].pos.y;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for st in host.states.iter() {
        // Skip the `FILL` spacers that centre the row (400 px each,
        // and together they span the window) and the note beneath it.
        if st.pos.y < top || st.size.w <= 0.0 || st.size.h <= 0.0 || st.size.w > 300.0 {
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
        lumen_static_shell::Host::new(build(entry, false), W, H);
    host.set_palette(palette());
    host.set_stylesheet(&stylesheet());
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
        lumen_static_shell::Host::new(build(entry, false), W, H);
    host.set_palette(palette());
    host.set_stylesheet(&stylesheet());
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
        println!("{stem}-{tag}: presses {}", host.app.presses);
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
    let app = build(start.min(CATALOG.len() - 1), false);
    run(
        app,
        Config {
            title: "showcase",
            width: W,
            height: H,
            page: palette().page,
            palette: palette(),
            stylesheet: stylesheet(),
            // Built `--features agent`, the window can be steered from
            // outside: `input.click` on the picker, `ui.screenshot` of
            // the result. `LUMEN_AGENT` overrides the address.
            agent: agent_addr(),
            ..Config::default()
        },
    );
}

/// Where to serve the agent endpoint, or `None` without the feature.
#[cfg(feature = "agent")]
fn agent_addr() -> Option<String> {
    Some(std::env::var("LUMEN_AGENT").unwrap_or_else(|_| "127.0.0.1:7654".into()))
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
            Selector::Id(lumen_static::select::PART_TRIGGER),
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
            Selector::Id(lumen_static::select::PART_MENU),
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
            lumen_static_shell::Host::new(build(entry, open), W, H);
        h.set_palette(SHOW_LIGHT);
        h.set_stylesheet(&stylesheet());
        let mut m = FontStack::system().map(FontMetrics::new);
        for _ in 0..2 {
            match m.as_mut() {
                Some(m) => h.pump_with(m),
                None => h.pump(),
            };
        }
        h
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
        let h = host(0, true);
        let sheet = h.stylesheet();
        let found: Vec<String> = h
            .sheet_findings()
            .iter()
            .map(|f| format!("rule {} {:?}: {:?}", f.rule, sheet[f.rule].selector, f.issue))
            .collect();
        println!("{} rules, {} findings", sheet.len(), found.len());
        assert!(!sheet.is_empty(), "setup: the sheet was applied");
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
                        let kind = match arena.seeds {
                            Some(seeds) => seeds[k].kind,
                            None => arena.template.node(k).kind_name(),
                        };
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
            choose(&mut h.app, i);
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
        let rows = CATALOG.len();
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
        assert_eq!(h.app.presses, 0);

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
        println!("presses after one click: {}", h.app.presses);
        assert_eq!(h.app.presses, 1, "the click never reached the handler");

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
        let mut b = Button::<Show, String>::default();
        b.label = "live".into();
        b.on_click = |s: &mut Show, _: &EventCtx| s.presses += 1;
        let direct: Option<fn(&mut Show, &EventCtx)> =
            Handlers::<Show>::handler(&b, NodeSel { i: 0, inner: None });
        let island: lumen_static::Dyn<Show> = lumen_static::Dyn::new(b);
        let through: Option<fn(&mut Show, &EventCtx)> =
            Handlers::<Show>::handler(&island, NodeSel { i: 0, inner: None });
        println!("direct {:?}, through a Dyn {:?}", direct.is_some(), through.is_some());
        assert!(direct.is_some(), "a wired button reports no handler at all");
        assert!(through.is_some(), "a Dyn island still swallows its content's handlers");

        // And it is the *same* handler, not merely some handler.
        let mut s = Show::default();
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
        let ids: Vec<&str> = <Screen as Widget>::SUB_TABLE
            .iter()
            .map(|d| d.wid)
            .filter(|w| !w.is_empty())
            .collect();
        let sheet = stylesheet();
        let mut dead = Vec::new();
        for r in &sheet {
            if let Selector::Id(want) = r.selector {
                if !ids.contains(&want) {
                    dead.push(want);
                }
            }
        }
        println!("{} rules, {} ids in the tree, dead: {dead:?}", sheet.len(), ids.len());
        assert!(dead.is_empty(), "stylesheet rules match no node: {dead:?}");
    }

    /// **Picking a row changes what is on the stage.**
    ///
    /// Drives the real handler, with the row index arriving the way it
    /// does at runtime — in `EventCtx::row` — rather than from a table
    /// of eighteen functions that no longer exists.
    #[test]
    fn picking_a_row_swaps_the_demo() {
        let mut s = build(0, true);
        assert_eq!(s.selected, 0);
        let mut row = SelectRow::<Show>::default();
        bind_row(&mut row, 15);
        let mut cx = press_at(0.0, 0.0);
        cx.row = Some(15);
        (row.on_click)(&mut s, &cx);
        println!("after picking row 15: {} ({})", CATALOG[s.selected].name, s.selected);
        assert_eq!(s.selected, 15, "the handler picked the wrong entry");
        assert!(!s.ui.body.header.picker.is_open(), "picking a row left the dropdown open");
    }
}


