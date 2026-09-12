//! **A phone home screen made of glass** — what the material is *for*.
//!
//! `just example glasswall` (add `--features gpu`; see below). Twenty-six
//! backdrop filters laid out the way a mobile screen actually is: a
//! status bar across the top, a 4 × 6 grid of clear app icons, and a
//! floating tab bar at the bottom.
//!
//! # Clear icons are the point
//!
//! The icons carry almost no tint. Nearly everything you see in one is
//! the wallpaper behind it, refracted through a bevelled edge and lit
//! from one side — which is why the grid reads as coloured glass rather
//! than as twenty-four grey squares, and why the wallpaper under it is
//! a hue ramp rather than a flat colour. A backdrop filter over a flat
//! background is a flat background.
//!
//! This is also why the icons sit *over* the wallpaper in a
//! `#[widget(stack)]` and not above it in a column: a column allocates
//! non-overlapping rects, so an icon laid out that way would have the
//! page behind it and nothing to read.
//!
//! # Build it with `--features gpu`
//!
//! Twenty-six panels is far past what the software filter is for.
//! Measured, `just glasswall-bench`: **139 ms** a frame on the software
//! canvas against **2.2 ms** on a GPU — seven frames a second against
//! four hundred (`docs/results-glass.md` §0h).
//!
//! # It is past what *damage* is for too, and that is the honest part
//!
//! `MAX_DEPS` is 4: a frame with more backdrop filters than that reports
//! `Damage::All` and redraws everything, every frame. `retain.rs` states
//! the reasoning — a glass panel serialises the rasteriser, so "a frame
//! with more than a handful is a design problem rather than a case to
//! optimise". A home screen full of glass icons is exactly that design
//! problem, and the benchmark prints the damage so the cliff has a
//! number beside it rather than only a comment.

use lumen_static::display::{Color, Material};
use lumen_static::prelude::*;
use lumen_static::Glass;
use lumen_static_winit::{run, Config};

// ---- the device ------------------------------------------------------

/// A phone at 3×, which is where a 24 px design lands as 72 physical
/// pixels and a bevel has room to be seen.
const W: u32 = 1290;
const H: u32 = 2796;

const ICON: f32 = 224.0;
/// `(1290 − 4·224 − 3·104) / 2`, where 104 is the row spacing below.
/// The gap is a literal there because `#[widget(row, spacing = ...)]` is
/// an attribute and takes one.
const MARGIN: f32 = 41.0;

// ---- palette ---------------------------------------------------------

const PAGE: Color = Color(0x08, 0x0a, 0x12, 0xff);
const INK: Color = Color(0xff, 0xff, 0xff, 0xff);
const DIM: Color = Color(0xe6, 0xea, 0xf6, 0xff);

/// **Almost nothing.** An app icon is a piece of clear glass: the
/// wallpaper is supposed to come through it, and a tint heavy enough to
/// read as a colour would make twenty-four identical squares.
const ICON_TINT: Color = Color(0xff, 0xff, 0xff, 0x14);
/// The bars carry more, because text sits on them and text needs a
/// ground. Dark rather than white — the wallpaper below is bright and
/// saturated, and white on pastel is unreadable.
const BAR_TINT: Color = Color(0x08, 0x0a, 0x12, 0x59);

/// **`LIQUID`, scaled for a 3× phone.**
///
/// `Material`'s lengths are in *physical* pixels. `Scaled` multiplies
/// them by the DPI factor, but this window is 1290 × 2796 at scale 1 —
/// a large window, not a scaled one — so the preset's 13 px rim would be
/// a hairline on a 224 px icon. Everything here is the preset × 1.8.
/// The unitless fields (specular, saturation, dispersion, adapt) are
/// **not** multiplied: a fraction has no length in it.
fn icon_glass() -> Material {
    Material {
        rim: 24.0,
        refraction: 20.0,
        blur: 2.0,
        tint: ICON_TINT,
        // Clear glass shows its edge or it shows nothing, so the rim
        // works harder here than on a tinted panel.
        specular: 1.0,
        shadow: 110,
        ..Material::LIQUID
    }
}

// ---- the wallpaper ---------------------------------------------------

/// A hue ramp, not a photograph, and not a flat colour.
///
/// Diagonal so neither a row nor a column of icons sits over one hue —
/// the refraction at an icon's edge is only visible where there is a
/// gradient to bend.
const WALL: [Color; 28] = [
    Color(0x17, 0x50, 0x84, 0xff),
    Color(0x1c, 0x36, 0x8e, 0xff),
    Color(0x28, 0x21, 0x98, 0xff),
    Color(0x52, 0x26, 0xa2, 0xff),
    Color(0x1d, 0x2e, 0x91, 0xff),
    Color(0x34, 0x22, 0x9b, 0xff),
    Color(0x5e, 0x27, 0xa5, 0xff),
    Color(0x8c, 0x2d, 0xaf, 0xff),
    Color(0x40, 0x24, 0x9e, 0xff),
    Color(0x6b, 0x29, 0xa8, 0xff),
    Color(0x99, 0x2f, 0xb2, 0xff),
    Color(0xbc, 0x35, 0xae, 0xff),
    Color(0x78, 0x2b, 0xab, 0xff),
    Color(0xa7, 0x31, 0xb5, 0xff),
    Color(0xbf, 0x37, 0xa5, 0xff),
    Color(0xc9, 0x3d, 0x86, 0xff),
    Color(0xb5, 0x32, 0xb8, 0xff),
    Color(0xc2, 0x39, 0x9d, 0xff),
    Color(0xcc, 0x3f, 0x7d, 0xff),
    Color(0xd5, 0x47, 0x5c, 0xff),
    Color(0xc4, 0x3b, 0x94, 0xff),
    Color(0xce, 0x41, 0x74, 0xff),
    Color(0xd8, 0x49, 0x52, 0xff),
    Color(0xe2, 0x71, 0x50, 0xff),
    Color(0xd1, 0x43, 0x6a, 0xff),
    Color(0xdb, 0x4d, 0x4b, 0xff),
    Color(0xe5, 0x80, 0x52, 0xff),
    Color(0xef, 0xb4, 0x5b, 0xff)
];

const WALL_IDS: [&str; 28] = [
    "w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10", "w11", "w12", "w13",
    "w14", "w15", "w16", "w17", "w18", "w19", "w20", "w21", "w22", "w23", "w24", "w25", "w26",
    "w27",
];

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Patch4 {
    #[widget_id("wtile")]
    pad: Spacer,
}

/// Four across, **and seven distinct row types rather than one used
/// seven times**. A `#[widget_id]` lives on a field of its parent, so a
/// shared row type gives every row the same four ids — a selector
/// matches every node carrying one, and the wallpaper comes out as four
/// full-height stripes with twenty-four of the twenty-eight colours
/// never reaching the screen.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct WallRow0 {
    #[widget_id("w0")]
    a: Patch4,
    #[widget_id("w1")]
    b: Patch4,
    #[widget_id("w2")]
    c: Patch4,
    #[widget_id("w3")]
    d: Patch4,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct WallRow1 {
    #[widget_id("w4")]
    a: Patch4,
    #[widget_id("w5")]
    b: Patch4,
    #[widget_id("w6")]
    c: Patch4,
    #[widget_id("w7")]
    d: Patch4,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct WallRow2 {
    #[widget_id("w8")]
    a: Patch4,
    #[widget_id("w9")]
    b: Patch4,
    #[widget_id("w10")]
    c: Patch4,
    #[widget_id("w11")]
    d: Patch4,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct WallRow3 {
    #[widget_id("w12")]
    a: Patch4,
    #[widget_id("w13")]
    b: Patch4,
    #[widget_id("w14")]
    c: Patch4,
    #[widget_id("w15")]
    d: Patch4,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct WallRow4 {
    #[widget_id("w16")]
    a: Patch4,
    #[widget_id("w17")]
    b: Patch4,
    #[widget_id("w18")]
    c: Patch4,
    #[widget_id("w19")]
    d: Patch4,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct WallRow5 {
    #[widget_id("w20")]
    a: Patch4,
    #[widget_id("w21")]
    b: Patch4,
    #[widget_id("w22")]
    c: Patch4,
    #[widget_id("w23")]
    d: Patch4,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct WallRow6 {
    #[widget_id("w24")]
    a: Patch4,
    #[widget_id("w25")]
    b: Patch4,
    #[widget_id("w26")]
    c: Patch4,
    #[widget_id("w27")]
    d: Patch4,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Wallpaper {
    r0: WallRow0,
    r1: WallRow1,
    r2: WallRow2,
    r3: WallRow3,
    r4: WallRow4,
    r5: WallRow5,
    r6: WallRow6,
}

// ---- the icons -------------------------------------------------------

/// What is *inside* the glass: one glyph.
///
/// **Centred by the icon's padding, not by spacers**, and the reason is
/// worth knowing before you reach for the obvious thing.
///
/// `Style` has `align`, which is the *cross* axis, and no main-axis
/// equivalent — so in a column nothing centres a child top to bottom,
/// and the first version of this had twenty-four letters in their
/// icons' top-left corners. The usual fix is a `FILL` `Spacer` either
/// side, which is how the dock floats itself, and here it made things
/// worse: the glyphs escaped their icons entirely and landed between
/// them.
///
/// The reason is general rather than anything to do with `Glass`.
/// `inner_limits` derives a child's box from the limits its parent was
/// *given*, while `resolve_self` picks the parent's own `Fixed` size
/// separately — so **a `FILL` child of a `Fixed` container fills the
/// space available to the container, not the container**. Two `FILL`
/// spacers in a 224 px icon therefore split the whole row's height, and
/// a `FILL`-width column does the same sideways: the next version had
/// every glyph drifting a column to the right of its own icon, by
/// exactly the width each one had left over.
///
/// **A `stack` is the container that does constrain its children.** Its
/// `place` stretches a `FILL` child to the box the container settled on
/// and then aligns it — on *both* axes, because a stack has no cross
/// axis — which is the one place `align` centres something vertically.
/// So the glyph sits in a fixed-size stack, and `Align::Center` does
/// the rest.
#[derive(StaticTree, Default)]
#[widget(stack)]
struct Glyph {
    #[widget_id("glyph")]
    ch: Label<String>,
}

/// One app: the glass tile, and its name **underneath** rather than on
/// it — which is how a home screen is laid out, and which keeps the
/// icon itself clear.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 14.0)]
struct App {
    #[widget_id("icon")]
    art: Glass<Glyph>,
    #[widget_id("name")]
    name: Label<String>,
}

/// Four across. **One shared row type is right here**, unlike the
/// wallpaper: every icon is the same size and the same material, and
/// what differs between them — the glyph and the name — is set in
/// `build` rather than by a selector.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 104.0)]
struct AppRow {
    a: App,
    b: App,
    c: App,
    d: App,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 58.0)]
struct Apps {
    #[widget_id("atop")]
    top: Spacer,
    r0: AppRow,
    r1: AppRow,
    r2: AppRow,
    r3: AppRow,
    r4: AppRow,
    r5: AppRow,
    #[widget_id("abot")]
    bottom: Spacer,
}

// ---- the two bars ----------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(row, spacing = 20.0)]
struct Status {
    #[widget_id("clock")]
    clock: Label<String>,
    #[widget_id("sgap")]
    gap: Spacer,
    #[widget_id("meta")]
    meta: Label<String>,
}

/// The tab bar's four buttons. Buttons rather than labels because this
/// is the surface a finger actually lands on, and pressing it is what
/// drives the deformation — M22/T22.4.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 74.0)]
struct Tabs {
    #[widget_id("tab")]
    t0: Button<Home, String>,
    #[widget_id("tab")]
    t1: Button<Home, String>,
    #[widget_id("tab")]
    t2: Button<Home, String>,
    #[widget_id("tab")]
    t3: Button<Home, String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct DockRow {
    #[widget_id("dockl")]
    l: Spacer,
    #[widget_id("dock")]
    dock: Glass<Tabs>,
    #[widget_id("dockr")]
    r: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Floating {
    #[widget_id("push")]
    push: Spacer,
    row: DockRow,
}

/// **A stack, and the field order in it is the whole feature.**
///
/// The stack's children share one rect and paint in order, so each reads
/// everything declared before it: the icons read the wallpaper, and the
/// two bars read the wallpaper *and* the icons. A `column` would put
/// them side by side, and a panel with nothing behind it is a tinted
/// rectangle.
#[derive(StaticTree, Default)]
#[widget(stack)]
struct Screen {
    #[widget_id("wall")]
    wall: Wallpaper,
    apps: Apps,
    #[widget_id("status")]
    status: Glass<Status>,
    dock: Floating,
}

/// Advance what the tree animates — M22/T22.4.
///
/// There is no framework-walked per-node tick (`App::tick` records why),
/// so the application names what moves: twenty-four icons, the status
/// bar and the dock.
///
/// **`|=`, not `any` or an early return.** Both short-circuit, and a
/// short circuit here stops calling `tick` on every panel after the
/// first one moving — on screen, an icon that ignores you while another
/// is still settling. `|` on a `bool` evaluates both sides; `||` does
/// not, and that is the whole difference.
fn advance(d: &mut Home, dt_ms: f32) -> bool {
    let a = &mut d.ui.apps;
    let rows = [&mut a.r0, &mut a.r1, &mut a.r2, &mut a.r3, &mut a.r4, &mut a.r5];
    let mut moved = false;
    for r in rows {
        for icon in [&mut r.a, &mut r.b, &mut r.c, &mut r.d] {
            moved |= icon.art.tick(dt_ms);
        }
    }
    moved | d.ui.status.tick(dt_ms) | d.ui.dock.row.dock.tick(dt_ms)
}

#[derive(DeriveApp, Default)]
#[app(tick = advance)]
struct Home {
    #[tree]
    ui: Screen,
}

// ---- style -----------------------------------------------------------

fn stylesheet() -> Vec<Rule> {
    let mut rules = vec![
        Rule::new(Selector::Kind("Screen"), Patch::new().background(PAGE)),
        // The wallpaper: seven rows, each filling its share, each tile
        // filling its share of the row.
        Rule::new(
            Selector::Id("wtile"),
            Patch::new().width(Length::FILL).height(Length::FILL),
        ),
        Rule::new(Selector::Kind("Patch4"), Patch::new().width(Length::FILL).height(Length::FILL)),
        // **Every row needs its own height rule.** A derived container
        // is `Shrink` by default on its cross axis, so without these the
        // seven rows collapse onto the `Spacer`s inside them and the
        // wallpaper does not render at all — which is a black screen
        // with the icons still perfectly correct on top of it.
        Rule::new(Selector::Kind("WallRow0"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("WallRow1"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("WallRow2"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("WallRow3"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("WallRow4"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("WallRow5"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("WallRow6"), Patch::new().height(Length::FILL)),
        // The icons. **`radius` is what makes them app icons** — a
        // square of glass reads as a window, a superellipse-ish rounded
        // one reads as an icon.
        Rule::new(
            Selector::Id("icon"),
            Patch::new()
                .width(Length::Fixed(ICON))
                .height(Length::Fixed(ICON))
                .radius(58.0),
        ),
        Rule::new(
            Selector::Id("glyph"),
            Patch::new().color(INK).font_size(92.0).font_weight(600.0),
        ),
        // `align` is the container's, not the child's: `align_offset`
        // is applied by the parent to what it lays out.
        // Fixed on both axes and centred: see `Glyph` for why this is
        // a stack and why the size is pinned rather than filled.
        Rule::new(
            Selector::Kind("Glyph"),
            Patch::new()
                .width(Length::Fixed(ICON))
                .height(Length::Fixed(ICON))
                .align(Align::Center),
        ),
        Rule::new(Selector::Id("name"), Patch::new().color(DIM).font_size(30.0)),
        Rule::new(
            Selector::Kind("App"),
            Patch::new().width(Length::Fixed(ICON)).align(Align::Center),
        ),
        // The grid, centred between the two bars by its own padding and
        // two FILL spacers.
        Rule::new(
            Selector::Kind("Apps"),
            Patch::new().padding(Edges {
                left: MARGIN,
                right: MARGIN,
                top: 300.0,
                bottom: 400.0,
            }),
        ),
        Rule::new(Selector::Id("atop"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Id("abot"), Patch::new().height(Length::FILL)),
        // The status bar: a strip at the top of the shared rect.
        Rule::new(
            Selector::Id("status"),
            Patch::new()
                .height(Length::Fixed(190.0))
                .padding(Edges { left: 64.0, right: 64.0, top: 78.0, bottom: 34.0 }),
        ),
        Rule::new(
            Selector::Id("clock"),
            Patch::new().color(INK).font_size(38.0).font_weight(700.0),
        ),
        Rule::new(Selector::Id("sgap"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("meta"), Patch::new().color(DIM).font_size(30.0)),
        // The tab bar: a pill sized to its tabs, floated to the bottom
        // centre by three FILL spacers.
        Rule::new(Selector::Id("push"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Id("dockl"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("dockr"), Patch::new().width(Length::FILL)),
        // **Both of these, and the second is the one that catches
        // people.** A derived container defaults to FILL width, so
        // `Tabs` would expand to whatever box it was given and the
        // Shrink pill around it would measure *that* — a full-width bar
        // with Shrink written on it. A Shrink container needs Shrink
        // content.
        Rule::new(Selector::Kind("Tabs"), Patch::new().width(Length::Shrink)),
        Rule::new(
            Selector::Id("dock"),
            Patch::new()
                .width(Length::Shrink)
                .radius(74.0)
                .padding(Edges { left: 74.0, right: 74.0, top: 44.0, bottom: 44.0 }),
        ),
        Rule::new(
            Selector::Kind("Floating"),
            Patch::new().padding(Edges { left: 0.0, right: 0.0, top: 0.0, bottom: 90.0 }),
        ),
        // No background and no press dim: the *panel* deforms under a
        // press, which is the whole point, and a grey rectangle
        // appearing behind the glyph would hide it.
        Rule::new(
            Selector::Id("tab"),
            Patch::new()
                .color(INK)
                .font_size(52.0)
                .background(Color(0, 0, 0, 0))
                .pressed_dim(0.0),
        ),
    ];
    for (id, colour) in WALL_IDS.iter().zip(WALL.iter()) {
        rules.push(Rule::new(Selector::Id(id), Patch::new().background(*colour)));
    }
    rules
}

// ---- the apps --------------------------------------------------------

/// Twenty-four of them: a glyph and a name. Set here rather than by a
/// selector, because they are *content* — a stylesheet has no business
/// knowing what an app is called.
///
/// **Letters, not pictograms**, and that is a measurement rather than a
/// preference: the first draft used ☎ ✉ ♫ ⚙ and the rest, and the system
/// font had none of them. Twenty-two of the twenty-four icons rendered
/// as the missing-glyph box, which is a demonstration of the shaper's
/// fallback and not of glass. An example that ships has to draw on the
/// machine it ships to.
const APPS: [(&str, &str); 24] = [
    ("M", "Messages"), ("P", "Phone"), ("@", "Mail"), ("W", "Weather"),
    ("A", "Music"), ("V", "Video"), ("O", "Camera"), ("F", "Photos"),
    ("D", "Files"), ("N", "Notes"), ("R", "Reminders"), ("C", "Calendar"),
    ("T", "Clock"), ("S", "Settings"), ("$", "Wallet"), ("X", "Maps"),
    ("H", "Home"), ("+", "Health"), ("B", "Books"), ("G", "Podcasts"),
    ("K", "Stocks"), ("Z", "Voice"), ("Y", "Shortcuts"), ("E", "App Store"),
];

fn build() -> Home {
    let mut app = Home::default();
    {
        let a = &mut app.ui.apps;
        let rows = [&mut a.r0, &mut a.r1, &mut a.r2, &mut a.r3, &mut a.r4, &mut a.r5];
        let mut i = 0;
        for r in rows {
            for icon in [&mut r.a, &mut r.b, &mut r.c, &mut r.d] {
                icon.art.material = icon_glass();
                icon.art.content.ch.set(APPS[i].0.into());
                icon.name.set(APPS[i].1.into());
                i += 1;
            }
        }
    }
    app.ui.status.material = Material { blur: 26.0, tint: BAR_TINT, ..Material::FROSTED };
    app.ui.status.content.clock.set("9:41".into());
    app.ui.status.content.meta.set("twenty-six panels".into());

    app.ui.dock.row.dock.material = Material { tint: BAR_TINT, ..icon_glass() };
    for (t, g) in [
        (&mut app.ui.dock.row.dock.content.t0, "P"),
        (&mut app.ui.dock.row.dock.content.t1, "@"),
        (&mut app.ui.dock.row.dock.content.t2, "A"),
        (&mut app.ui.dock.row.dock.content.t3, "O"),
    ] {
        t.label = g.into();
    }
    app
}

// ---- headless: a frame, and what it costs ----------------------------

fn host(w: u32, h: u32) -> lumen_static_shell::Host<Home> {
    use lumen_static_shell::text::{FontMetrics, FontStack};
    let mut host = lumen_static_shell::Host::new(build(), w, h);
    host.set_stylesheet(&stylesheet());
    let mut metrics = FontStack::system().map(FontMetrics::new);
    // Twice: the first pump is what teaches the layout its sizes, and
    // the second is the one whose damage means anything.
    for _ in 0..2 {
        match metrics.as_mut() {
            Some(m) => host.pump_with(m),
            None => host.pump(),
        };
    }
    host
}

fn shot(path: &str, w: u32, h: u32) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::FontStack;

    let host = host(w, h);
    let mut canvas = match FontStack::system() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    canvas.clear(PAGE);
    let t = std::time::Instant::now();
    lumen_static::Renderer::render(&mut canvas, &host.list, w, h);
    let ms = t.elapsed().as_secs_f32() * 1000.0;

    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in canvas.pixels.chunks_exact(4) {
        out.extend_from_slice(&p[..3]);
    }
    std::fs::write(path, out).expect("write");
    println!("{path}: {w}x{h}, software render {ms:.1} ms");
}

/// What this scene costs, and what its damage degrades to.
fn bench(w: u32, h: u32) {
    use lumen_static::display::Cmd;
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::FontStack;

    let mut host = host(w, h);
    let panels = host.list.cmds.iter().filter(|c| matches!(c, Cmd::Glass { .. })).count();
    // A settled frame: nothing changed since the last pump, so anything
    // other than `Damage::None` is the dependency bound talking.
    let _ = host.take_damage_for_age(1);
    host.pump();
    println!("scene:  {panels} glass commands, {} commands total", host.list.cmds.len());
    println!(
        "damage: {:?} on a frame where nothing changed\n        \
         (MAX_DEPS is {}; past it every frame is a full redraw)",
        host.damage(),
        lumen_static::retain::MAX_DEPS,
    );

    let mut canvas = match FontStack::system() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    let mut best = f32::INFINITY;
    for _ in 0..8 {
        let t = std::time::Instant::now();
        canvas.clear(PAGE);
        lumen_static::Renderer::render(&mut canvas, &host.list, w, h);
        best = best.min(t.elapsed().as_secs_f32() * 1000.0);
    }
    println!("software: {best:.1} ms a frame at {w}x{h}");
    gpu_bench(&host, w, h);
}

#[cfg(not(feature = "gpu"))]
fn gpu_bench(_: &lumen_static_shell::Host<Home>, _: u32, _: u32) {
    println!("gpu:      not built — rebuild with `--features gpu` for the number that matters");
}

#[cfg(feature = "gpu")]
fn gpu_bench(host: &lumen_static_shell::Host<Home>, w: u32, h: u32) {
    use lumen_static::Renderer as _;
    let Some(mut g) = lumen_static_wgpu::Gpu::headless(w, h) else {
        println!("gpu:      no adapter");
        return;
    };
    if let Some(f) = lumen_static_shell::text::FontStack::system() {
        g.set_font(f);
    }
    // Warm: the first frame compiles pipelines and allocates.
    g.clear(PAGE);
    g.render(&host.list, w, h);
    g.wait();
    let mut best = f32::INFINITY;
    for _ in 0..20 {
        let t = std::time::Instant::now();
        g.clear(PAGE);
        g.render(&host.list, w, h);
        // **`wait`, or this times the queue submission.** `draw` returns
        // as soon as the work is queued, which is what a GPU is for.
        g.wait();
        best = best.min(t.elapsed().as_secs_f32() * 1000.0);
    }
    println!("gpu:      {best:.1} ms a frame on {}", g.adapter());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dim = |i: usize, d: u32| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    match args.first().map(String::as_str) {
        Some("--shot") => {
            let path = args.get(1).cloned().unwrap_or_else(|| "glasswall.ppm".into());
            shot(&path, dim(2, W), dim(3, H));
        }
        Some("--bench") => bench(dim(1, W), dim(2, H)),
        _ => run(
            build(),
            Config {
                title: "glasswall",
                width: W,
                height: H,
                page: PAGE,
                stylesheet: stylesheet(),
                ..Config::default()
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumen_static::display::Cmd;
    use lumen_static::event::Event;

    /// Every panel reaches the display list.
    ///
    /// Cheap, and it is what the others rest on: a scene that silently
    /// dropped its glass would still lay out, still paint, and still
    /// pass anything measured on what it did emit.
    #[test]
    fn the_scene_emits_every_panel() {
        let host = host(W, H);
        let n = host.list.cmds.iter().filter(|c| matches!(c, Cmd::Glass { .. })).count();
        println!("{n} glass commands");
        assert_eq!(n, 26, "twenty-four icons, the status bar and the dock");
    }

    /// The icon nodes, and the glyph label inside each — found by
    /// geometry rather than by a hand-counted index that a new field
    /// would silently shift.
    fn icons(host: &lumen_static_shell::Host<Home>) -> Vec<(usize, usize)> {
        let is_icon = |i: usize| {
            let s = &host.states[i];
            s.size.w == ICON && s.size.h == ICON
        };
        // A `Glass` and the stack inside it are both ICON × ICON, so
        // they arrive as adjacent pairs; the label is the node after.
        (0..host.states.len() - 2)
            .filter(|&i| is_icon(i) && is_icon(i + 1) && !is_icon(i + 2))
            .map(|i| (i, i + 2))
            .collect()
    }

    /// **The glyph is centred in its icon.**
    ///
    /// Two separate layout traps sit behind this one assertion, and both
    /// were live in this file before it existed: `FILL` spacers in a
    /// fixed-size column split the whole *row's* height rather than the
    /// icon's, and a `FILL`-width container takes the row's remaining
    /// width — which put every glyph a column to the right of its own
    /// icon. Neither is a paint bug, so nothing about the pixels of a
    /// single icon would have caught it.
    #[test]
    fn every_glyph_is_centred_in_its_icon() {
        let host = host(W, H);
        let found = icons(&host);
        assert_eq!(found.len(), 24, "the grid is not twenty-four icons");
        let mut worst = 0.0f32;
        for (icon, glyph) in found {
            let (a, b) = (&host.states[icon], &host.states[glyph]);
            let dx = (a.pos.x + a.size.w / 2.0) - (b.pos.x + b.size.w / 2.0);
            let dy = (a.pos.y + a.size.h / 2.0) - (b.pos.y + b.size.h / 2.0);
            worst = worst.max(dx.abs().max(dy.abs()));
        }
        println!("worst glyph offset from its icon's centre: {worst:.1} px");
        assert!(worst < 3.0, "a glyph is {worst:.1} px off its icon's centre");
    }

    /// **The icons are clear**, which is the whole look.
    ///
    /// Both directions, because each alone accepts a renderer the other
    /// rejects: the wallpaper must come through the middle of an icon
    /// almost unchanged, *and* the icon must still be visible — a tint
    /// of zero over a flat material would pass the first half and leave
    /// twenty-four invisible squares.
    #[test]
    fn the_icons_are_clear_and_still_visible() {
        use lumen_static_shell::software::Canvas;
        let host = host(W, H);
        let mut c = Canvas::new(W, H);
        c.clear(PAGE);
        lumen_static::Renderer::render(&mut c, &host.list, W, H);

        // The same wallpaper with no glass on it at all.
        let mut bare_host = host2();
        let mut bare = Canvas::new(W, H);
        bare.clear(PAGE);
        lumen_static::Renderer::render(&mut bare, &bare_host.list, W, H);
        let _ = &mut bare_host;

        let at = |cv: &Canvas, x: usize, y: usize| {
            let i = (y * W as usize + x) * 4;
            [cv.pixels[i] as f32, cv.pixels[i + 1] as f32, cv.pixels[i + 2] as f32]
        };
        let found = icons(&host);
        let (mut middle, mut edge) = (0.0f32, 0.0f32);
        for (icon, _) in &found {
            let s = &host.states[*icon];
            let (cx, cy) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
            // Off the glyph, which is ink and not glass.
            let m = (cx as usize + 80, cy as usize);
            let e = (s.pos.x as usize + 4, cy as usize);
            let d = |p: (usize, usize)| {
                let (a, b) = (at(&c, p.0, p.1), at(&bare, p.0, p.1));
                (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max)
            };
            middle = middle.max(d(m));
            edge = edge.max(d(e));
        }
        println!("worst change at an icon's middle {middle:.0}, at its rim {edge:.0}");
        assert!(middle < 40.0, "the icons are not clear — the middle moved {middle:.0} levels");
        assert!(edge > 18.0, "the icons are invisible — even the rim moved only {edge:.0}");
    }

    /// The same tree with every material made fully transparent, so the
    /// wallpaper can be measured on its own.
    fn host2() -> lumen_static_shell::Host<Home> {
        use lumen_static_shell::text::{FontMetrics, FontStack};
        let mut app = build();
        let clear = Material { blur: 0.0, rim: 0.0, tint: Color(0, 0, 0, 0), ..Material::FROSTED };
        {
            let a = &mut app.ui.apps;
            for r in [&mut a.r0, &mut a.r1, &mut a.r2, &mut a.r3, &mut a.r4, &mut a.r5] {
                for icon in [&mut r.a, &mut r.b, &mut r.c, &mut r.d] {
                    icon.art.material = clear;
                }
            }
        }
        app.ui.status.material = clear;
        app.ui.dock.row.dock.material = clear;
        let mut host = lumen_static_shell::Host::new(app, W, H);
        host.set_stylesheet(&stylesheet());
        let mut metrics = FontStack::system().map(FontMetrics::new);
        for _ in 0..2 {
            match metrics.as_mut() {
                Some(m) => host.pump_with(m),
                None => host.pump(),
            };
        }
        host
    }

    /// **Every icon is advanced, not just the first one moving.**
    ///
    /// `advance` folds rather than short-circuits, and this is the case
    /// that tells the difference: one icon is already animating when a
    /// second is pressed. With `any` or `||` the first returns `true`,
    /// the iteration stops, and the second never moves — on screen, an
    /// icon that ignores you while another is settling.
    #[test]
    fn a_second_press_animates_while_the_first_is_still_settling() {
        let mut host = host(W, H);
        let rest = icon_glass().rim;
        let found = icons(&host);
        assert_eq!(found.len(), 24, "the grid is not twenty-four icons");
        let centre = |h: &lumen_static_shell::Host<Home>, i: usize| {
            let s = &h.states[i];
            (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
        };
        let (p0, p1) = (centre(&host, found[0].0), centre(&host, found[23].0));

        host.post(Event::down(p0.0, p0.1));
        host.pump();
        for _ in 0..3 {
            host.tick(30.0);
            host.pump();
        }
        host.post(Event::up(p0.0, p0.1));
        host.pump();
        assert!(host.tick(0.0), "the first icon settled too early; the test proves nothing");

        host.post(Event::down(p1.0, p1.1));
        host.pump();
        host.tick(30.0);
        host.pump();

        let moved = host.app.ui.apps.r5.d.art.material_now().rim;
        println!("last icon's rim {moved:.1} against {rest:.1} at rest");
        assert!(
            moved > rest + 0.5,
            "the second icon never advanced ({moved:.1} vs {rest:.1}) — `advance` short-circuited"
        );
    }
}
