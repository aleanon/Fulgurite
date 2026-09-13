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
use lumen_static::Toggle;
use lumen_static_widgets::prelude::*;
use lumen_static_winit::{run, Config};

mod catalog;
mod stage;
use catalog::CATALOG;
pub use stage::*;

// ---- palette ---------------------------------------------------------

const PAGE: Color = Color(0xf4, 0xf6, 0xfa, 0xff);
const BAND: Color = Color(0xff, 0xff, 0xff, 0xff);
const INK: Color = Color(0x1b, 0x22, 0x30, 0xff);
const MUTED: Color = Color(0x6b, 0x74, 0x88, 0xff);
const ACCENT: Color = Color(0x2f, 0x6b, 0xff, 0xff);
const HAIRLINE: Color = Color(0xdf, 0xe4, 0xed, 0xff);

const W: u32 = 960;
const H: u32 = 720;
/// Rows the dropdown shows before it scrolls.
const PANEL_H: f32 = 560.0;

// ---- the tree --------------------------------------------------------

/// One row of the dropdown. A `Button` because it is a tap target and
/// the core gives a button its hover, press and focus for free.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Row {
    #[widget_id("opt")]
    pick: Button<Show, String>,
}

/// The dropdown panel.
///
/// **`Repeat`, not a field per entry.** The catalogue's length is data,
/// not a fact about the tree's shape, and `Repeat` is virtualized: its
/// `bind` is a plain `fn` that cannot capture, and a click reports which
/// row through `EventCtx::row` — recomputed by hit testing every frame,
/// so it cannot go stale the way a captured index can.
/// The dropdown's rows, **one field per catalogue entry**.
///
/// Verbose, and the static tree's own answer. `Repeat` and `List` are
/// the off-table containers built for runtime-sized collections, and a
/// catalogue is not one: its length is known when the binary is. Both
/// were tried here first and neither painted a row — their instances
/// live in an arena rather than the main state array, which is a seam
/// worth understanding before leaning on it for a *menu*.
///
/// The cost is real and worth stating: adding a widget to `CATALOG`
/// means adding a field here, and `ROWS` is asserted against the
/// catalogue's length at startup so the two cannot drift silently.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct Panel {
    #[widget_id("opt")]
    r0: Row,
    #[widget_id("opt")]
    r1: Row,
    #[widget_id("opt")]
    r2: Row,
    #[widget_id("opt")]
    r3: Row,
    #[widget_id("opt")]
    r4: Row,
    #[widget_id("opt")]
    r5: Row,
    #[widget_id("opt")]
    r6: Row,
    #[widget_id("opt")]
    r7: Row,
    #[widget_id("opt")]
    r8: Row,
    #[widget_id("opt")]
    r9: Row,
    #[widget_id("opt")]
    r10: Row,
    #[widget_id("opt")]
    r11: Row,
    #[widget_id("opt")]
    r12: Row,
    #[widget_id("opt")]
    r13: Row,
    #[widget_id("opt")]
    r14: Row,
    #[widget_id("opt")]
    r15: Row,
    #[widget_id("opt")]
    r16: Row,
    #[widget_id("opt")]
    r17: Row,
}

/// How many rows the panel has. Checked against the catalogue.
const ROWS: usize = 18;

/// The header band: the trigger, what it is, and a live readout.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
pub struct Header {
    #[widget_id("title")]
    title: Label,
    #[widget_id("trigger")]
    trigger: Button<Show, String>,
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
    demos: Demos,
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

/// **A stack, and the panel is declared last.**
///
/// The dropdown paints *over* the stage and must win its clicks. A stack
/// paints in field order and hit-tests in reverse, so declaring the
/// panel last gives both — the rule `examples/glass` records after an
/// invisible `FILL` spacer in the wrong layer swallowed every click.
/// The dropdown layer: the panel, pushed under the trigger.
///
/// **A stack has one alignment for every child**, so a panel that has to
/// sit at a particular place inside it is positioned by spacers — the
/// same technique the glass dock uses to float itself. A fixed spacer
/// above puts it under the header band; a fixed one to the left lines it
/// up with the trigger.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
pub struct PanelRow {
    #[widget_id("pl")]
    l: Spacer,
    /// **`Option`, which is how the core hides a subtree.** `layout`
    /// zeroes an absent branch every frame, so it holds no stale
    /// geometry for `paint`, `hit`, the journal or the audit to read —
    /// and an absent panel cannot swallow a click meant for the stage.
    #[widget_id("panelwrap")]
    panel: Option<Panel>,
    #[widget_id("pr")]
    r: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
pub struct PanelLayer {
    #[widget_id("pt")]
    top: Spacer,
    row: PanelRow,
    #[widget_id("pb")]
    bottom: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(stack)]
pub struct Screen {
    body: Body,
    layer: PanelLayer,
}

#[derive(DeriveApp, Default)]
#[app(tick = advance, after_update = refresh)]
pub struct Show {
    #[tree]
    ui: Screen,
    /// Which catalogue entry is on the stage.
    selected: usize,
    /// Whether the dropdown is down.
    open: bool,
    /// Turns, for the demos that animate. Advanced by `App::tick`;
    /// nothing here drives its own clock.
    phase: f32,
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
fn refresh(s: &mut Show) {
    // **Read the typed fields back before reporting.** This is what a
    // `Dyn` island could not do: a `Slider` owns its value, and through
    // a `Box<dyn Node>` there was no handle to ask.
    if let Some(d) = &s.ui.body.stage.row.demos.slider {
        s.fraction = d.a.value();
    }
    let e = &CATALOG[s.selected];
    let text = (e.status)(s);
    s.ui.body.header.readout.set(text);
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
    // **ASCII, not `\u{25be}`.** The system font has no chevron and
    // `examples/glasswall` already paid for that lesson: a missing glyph
    // renders as the fallback box, which looks like a bug in the widget
    // rather than in the font.
    s.ui.body.header.trigger.label = format!("{}      {}", e.name, if s.open { "^" } else { "v" });
    s.ui.body.header.blurb.set(e.blurb.into());
    (e.seed)(s);
    let text = (e.status)(s);
    s.ui.body.header.readout.set(text);
}

/// **One handler per row, generated.**
///
/// An `on_click` is a plain `fn` pointer: it cannot close over the row's
/// index, and it is a `fn` on purpose — a captured index is exactly what
/// goes stale when a list changes under it, and a hot reload can
/// re-supply a `fn` where it cannot re-supply a closure. `EventCtx::row`
/// answers this for *off-table* rows; these are ordinary nodes, so the
/// index is baked into the handler instead.
fn pick0(s: &mut Show, _: &EventCtx) {
    choose(s, 0);
}
fn pick1(s: &mut Show, _: &EventCtx) {
    choose(s, 1);
}
fn pick2(s: &mut Show, _: &EventCtx) {
    choose(s, 2);
}
fn pick3(s: &mut Show, _: &EventCtx) {
    choose(s, 3);
}
fn pick4(s: &mut Show, _: &EventCtx) {
    choose(s, 4);
}
fn pick5(s: &mut Show, _: &EventCtx) {
    choose(s, 5);
}
fn pick6(s: &mut Show, _: &EventCtx) {
    choose(s, 6);
}
fn pick7(s: &mut Show, _: &EventCtx) {
    choose(s, 7);
}
fn pick8(s: &mut Show, _: &EventCtx) {
    choose(s, 8);
}
fn pick9(s: &mut Show, _: &EventCtx) {
    choose(s, 9);
}
fn pick10(s: &mut Show, _: &EventCtx) {
    choose(s, 10);
}
fn pick11(s: &mut Show, _: &EventCtx) {
    choose(s, 11);
}
fn pick12(s: &mut Show, _: &EventCtx) {
    choose(s, 12);
}
fn pick13(s: &mut Show, _: &EventCtx) {
    choose(s, 13);
}
fn pick14(s: &mut Show, _: &EventCtx) {
    choose(s, 14);
}
fn pick15(s: &mut Show, _: &EventCtx) {
    choose(s, 15);
}
fn pick16(s: &mut Show, _: &EventCtx) {
    choose(s, 16);
}
fn pick17(s: &mut Show, _: &EventCtx) {
    choose(s, 17);
}

static PICKERS: [fn(&mut Show, &EventCtx); ROWS] = [
    pick0,
    pick1,
    pick2,
    pick3,
    pick4,
    pick5,
    pick6,
    pick7,
    pick8,
    pick9,
    pick10,
    pick11,
    pick12,
    pick13,
    pick14,
    pick15,
    pick16,
    pick17,
];

/// Put entry `i` on the stage and close the dropdown.
fn choose(s: &mut Show, i: usize) {
    s.selected = i.min(CATALOG.len() - 1);
    s.open = false;
    s.phase = 0.0;
    rebuild(s);
    sync_open(s);
}

/// A row's caption, with its group folded in so the list reads as
/// grouped without the panel needing heading nodes of a second type.
fn bind_row(r: &mut Row, i: usize) {
    let e = &CATALOG[i];
    let first = i == 0 || CATALOG[i - 1].group != e.group;
    r.pick.label = if first {
        format!("{} - {}", e.group, e.name)
    } else {
        format!("        {}", e.name)
    };
    // **The index comes from the node, not from a capture.** An
    // `on_click` is a plain `fn` pointer — it cannot close over `i` —
    // and `EventCtx::row` is for off-table rows, which these are not.
    // So the handler reads back which of the panel's nodes was hit.
    r.pick.on_click = PICKERS[i];
}

/// The panel is shown by giving it a height and hidden by taking it
/// away — `Style::visible` is the cascade's, and the cascade is static.
fn sync_open(s: &mut Show) {
    let e = &CATALOG[s.selected];
    s.ui.body.header.trigger.label =
        format!("{}      {}", e.name, if s.open { "^" } else { "v" });
    s.ui.layer.row.panel = s.open.then(|| {
        {
            let mut p = Panel::default();
            let rows: [&mut Row; ROWS] = [
                &mut p.r0,
                &mut p.r1,
                &mut p.r2,
                &mut p.r3,
                &mut p.r4,
                &mut p.r5,
                &mut p.r6,
                &mut p.r7,
                &mut p.r8,
                &mut p.r9,
                &mut p.r10,
                &mut p.r11,
                &mut p.r12,
                &mut p.r13,
                &mut p.r14,
                &mut p.r15,
                &mut p.r16,
                &mut p.r17,
            ];
            for (i, r) in rows.into_iter().enumerate() {
                bind_row(r, i);
            }
            p
        }
    });
}

/// Build the app, already on `entry`, with the dropdown `open` or not.
fn build(entry: usize, open: bool) -> Show {
    let mut app = Show::default();
    app.ui.body.header.title.set("Widget showcase");
    app.ui.body.header.trigger.on_click = |s: &mut Show, _: &EventCtx| {
        s.open = !s.open;
        sync_open(s);
    };
    app.selected = entry.min(CATALOG.len() - 1);
    app.open = open;
    rebuild(&mut app);
    sync_open(&mut app);
    app
}

/// One frame, headless — what the tests and the screenshots use.
fn shot(path: &str, entry: usize, open: bool) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    let mut host: lumen_static_shell::Host<Show> = lumen_static_shell::Host::new(build(entry, open), W, H);
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
    c.clear(PAGE);
    lumen_static::Renderer::render(&mut c, &host.list, W, H);
    let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
    for p in c.pixels.chunks_exact(4) {
        out.extend_from_slice(&p[..3]);
    }
    std::fs::write(path, out).expect("write");
    println!("{path}: {} ({}), dropdown {}", CATALOG[entry.min(CATALOG.len()-1)].name,
        entry, if open { "open" } else { "closed" });
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--shot") {
        let path = args.get(1).cloned().unwrap_or_else(|| "showcase.ppm".into());
        let entry = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let open = args.get(3).map(|s| s == "open").unwrap_or(false);
        shot(&path, entry, open);
        return;
    }
    let mut app = Show::default();
    app.ui.body.header.title.set("Widget showcase");
    app.ui.body.header.trigger.on_click = |s: &mut Show, _: &EventCtx| {
        s.open = !s.open;
        sync_open(s);
    };
    rebuild(&mut app);
    sync_open(&mut app);
    run(
        app,
        Config {
            title: "showcase",
            width: W,
            height: H,
            page: PAGE,
            stylesheet: stylesheet(),
            ..Config::default()
        },
    );
}

/// One dropdown row's height.
const ROW_H: f32 = 30.0;

fn stylesheet() -> Vec<StyleRule> {
    use lumen_static::style::{Patch, Selector};
    vec![
        StyleRule::new(Selector::Kind("Screen"), Patch::new().background(PAGE)),
        StyleRule::new(
            Selector::Id("header"),
            Patch::new()
                .background(BAND)
                .padding(Edges { left: 28.0, right: 28.0, top: 20.0, bottom: 18.0 }),
        ),
        StyleRule::new(Selector::Id("title"), Patch::new().color(INK).font_size(20.0).font_weight(700.0)),
        StyleRule::new(
            Selector::Id("trigger"),
            Patch::new()
                .width(Length::Fixed(300.0))
                .background(Color(0xf7, 0xf9, 0xfd, 0xff))
                .color(INK)
                .radius(8.0)
                .font_size(15.0)
                .padding(Edges { left: 14.0, right: 14.0, top: 9.0, bottom: 9.0 }),
        ),
        StyleRule::new(Selector::Id("blurb"), Patch::new().color(MUTED).font_size(13.0)),
        StyleRule::new(Selector::Id("readout"), Patch::new().color(ACCENT).font_size(13.0).font_weight(600.0)),
        StyleRule::new(Selector::Id("hr"), Patch::new().background(HAIRLINE)),
        // The stage: the demo centred in what is left.
        StyleRule::new(Selector::Kind("Stage"), Patch::new().height(Length::FILL)),
        StyleRule::new(Selector::Id("st"), Patch::new().height(Length::FILL)),
        StyleRule::new(Selector::Id("sb"), Patch::new().height(Length::FILL)),
        StyleRule::new(Selector::Id("sl"), Patch::new().width(Length::FILL)),
        StyleRule::new(Selector::Id("sr"), Patch::new().width(Length::FILL)),
        // The dropdown layer: spacers put the panel under the trigger.
        StyleRule::new(Selector::Id("pt"), Patch::new().height(Length::Fixed(118.0))),
        StyleRule::new(Selector::Id("pb"), Patch::new().height(Length::FILL)),
        StyleRule::new(Selector::Id("pl"), Patch::new().width(Length::Fixed(28.0))),
        StyleRule::new(Selector::Id("pr"), Patch::new().width(Length::FILL)),
        // The dropdown panel, pinned under the trigger.
        // **No `ScrollView` around it**, and that is a scope note
        // rather than a preference: a scroller lays its content out
        // against the limits *it* was given, not against its own
        // resolved box, so an 18-row list inside a 300 px scroller came
        // out 932 px wide. The catalogue fits the panel today; a list
        // that outgrows it needs that asymmetry addressed first.
        StyleRule::new(
            Selector::Id("panelwrap"),
            Patch::new()
                .width(Length::Fixed(300.0))
                .height(Length::Fixed(PANEL_H))
                .background(BAND)
                .radius(10.0),
        ),
        StyleRule::new(
            Selector::Id("opt"),
            Patch::new()
                .width(Length::FILL)
                .height(Length::Fixed(ROW_H))
                .background(Color(0, 0, 0, 0))
                .color(INK)
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

    /// The `Stage` node's index. Its `pos.y` is where the header stops
    /// and the demo's own commands start.
    const STAGE: usize = 8;

    fn host(entry: usize, open: bool) -> lumen_static_shell::Host<Show> {
        use lumen_static_shell::text::{FontMetrics, FontStack};
        let mut h: lumen_static_shell::Host<Show> =
            lumen_static_shell::Host::new(build(entry, open), W, H);
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
    /// The one place this file can drift: `Panel` is a field per
    /// catalogue entry, so adding a widget to `CATALOG` without adding a
    /// field would silently drop it off the end of the menu — visible
    /// only to someone who counted.
    #[test]
    fn the_menu_has_a_row_for_every_widget() {
        println!("{} rows, {} demos, {} entries", ROWS, stage::DEMOS, CATALOG.len());
        assert_eq!(ROWS, CATALOG.len(), "`Panel`'s fields and `CATALOG` disagree");
        assert_eq!(stage::DEMOS, CATALOG.len(), "`Demos`'s fields and `CATALOG` disagree");
    }

    /// **Every entry puts something on the stage.**
    ///
    /// Seeded, laid out and painted for real — not merely constructed.
    /// A demo that built a widget and gave it no size would pass a
    /// smoke test that stopped at `seed`.
    #[test]
    fn every_entry_paints_a_demo() {
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
            // **Exactly one demo on the stage.** `Demos::clear` is a
            // hand-written list of eighteen fields; one left out of it
            // leaves the previous demo underneath the new one, which
            // paints and hit-tests and looks almost right.
            assert_eq!(
                h.app.ui.body.stage.row.demos.shown(),
                1,
                "{} left {} demos on the stage",
                e.name,
                h.app.ui.body.stage.row.demos.shown()
            );
            assert!(blurb, "{}'s blurb never reached the header", e.name);
            assert!(demo > 0, "{} put nothing on the stage", e.name);
        }
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
        assert!(open.app.ui.layer.row.panel.is_some(), "an open picker has no panel");
        assert!(shut.app.ui.layer.row.panel.is_none(), "a closed picker kept its panel");
        assert!(b > a + ROWS, "an open panel drew {} more commands, not {ROWS}+", b - a);
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
            h.app.ui.body.stage.row.demos.button.is_some(),
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

    /// **A `Dyn` island reports no handler — pinned, so nobody reaches
    /// for one again.**
    ///
    /// This is the framework fact that made the first version of the
    /// stage silently dead, and it is not a defect: `Dyn` is a
    /// `Box<dyn Node>` and the application type is exactly what the
    /// erasure erased, so there is nothing for `Handlers<S>` to forward
    /// to. It is a *cost* of the hatch, and one its own rustdoc does not
    /// spell out. Asserted here rather than remembered.
    #[test]
    fn a_dyn_island_cannot_route_a_handler() {
        use lumen_static::event::{Handlers, NodeSel};
        let mut b = Button::<Show, String>::default();
        b.label = "live".into();
        b.on_click = |s: &mut Show, _: &EventCtx| s.presses += 1;
        // The same button, reachable directly...
        let direct: Option<fn(&mut Show, &EventCtx)> =
            Handlers::<Show>::handler(&b, NodeSel { i: 0, inner: None });
        // ...and through an island.
        let island = lumen_static::Dyn::new(b);
        let through: Option<fn(&mut Show, &EventCtx)> =
            Handlers::<Show>::handler(&island, NodeSel { i: 0, inner: None });
        println!("direct {:?}, through a Dyn {:?}", direct.is_some(), through.is_some());
        assert!(direct.is_some(), "a wired button reports no handler at all");
        assert!(
            through.is_none(),
            "a Dyn now routes handlers — the stage could use one again, and this test should go"
        );
    }

    /// **Picking a row changes what is on the stage.**
    ///
    /// Drives the real handler rather than setting `selected` directly,
    /// so the wiring between a row and its entry is what is tested.
    #[test]
    fn picking_a_row_swaps_the_demo() {
        let mut s = build(0, true);
        assert_eq!(s.selected, 0);
        let cx = EventCtx {
            event: Event::down(0.0, 0.0),
            node: 0,
            row: None,
            inner: 0,
            pos: lumen_static::Point { x: 0.0, y: 0.0 },
            local: lumen_static::Point { x: 0.0, y: 0.0 },
            button: PointerButton::Left,
            clicks: 1,
            mods: Mods::NONE,
        };
        PICKERS[15](&mut s, &cx);
        println!("after picking row 15: {} ({})", CATALOG[s.selected].name, s.selected);
        assert_eq!(s.selected, 15, "the handler picked the wrong entry");
        assert!(!s.open, "picking a row left the dropdown open");
    }
}
