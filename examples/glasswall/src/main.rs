//! **A 4K screen filled with glass** — every material M22 built, at once.
//!
//! `just example glasswall` (add `--features gpu`; see below). Fourteen
//! backdrop filters over one structured page: a full-width frosted bar,
//! a 4 × 3 grid of liquid panels each turning one part of the material
//! on or off, and a floating dock that squashes when pressed.
//!
//! # Build it with `--features gpu`
//!
//! This example is deliberately past the point the software filter is
//! for. One full-screen panel at 4K costs **750 ms** on the software
//! canvas and **2.8 ms** on the GPU (`docs/results-glass.md` §0h), and
//! there are thirteen panels here. `just glasswall-bench` prints what
//! this scene costs on whichever backends are available, so the claim is
//! measured rather than asserted.
//!
//! # It is also past the point *damage* is for, on purpose
//!
//! `MAX_DEPS` is 4: a frame with more backdrop filters than that reports
//! `Damage::All` and redraws everything, every frame. That is not a bug
//! being demonstrated — `retain.rs` states the reasoning, that a glass
//! panel serialises the rasteriser and "a frame with more than a handful
//! is a design problem rather than a case to optimise". This example is
//! that design problem, built so the cliff has a number next to it
//! rather than only a comment. `--bench` prints the damage the scene
//! actually reports.
//!
//! **So read this as a stress test, not as a layout to copy.** A real
//! interface puts glass on one or two surfaces — `just example glass` is
//! that one.

use lumen_static::display::{Color, Material};
use lumen_static::prelude::*;
use lumen_static::Glass;
use lumen_static_winit::{run, Config};

// ---- palette ---------------------------------------------------------

const PAGE: Color = Color(0x07, 0x09, 0x0e, 0xff);
const INK: Color = Color(0xf2, 0xf6, 0xfc, 0xff);
const DIM: Color = Color(0xd3, 0xdb, 0xea, 0xff);
const TILE_INK: Color = Color(0x0d, 0x10, 0x16, 0xff);

/// The panels' tint: the page colour, not white.
///
/// A legibility decision rather than a stylistic one, and the same one
/// `examples/glass` makes: the tiles behind are saturated and bright, a
/// white tint over them gives pastel, and white label text on pastel is
/// unreadable. Tinting *toward the page* keeps the backdrop visible and
/// the panel's own text at contrast.
const TINT: Color = Color(0x07, 0x09, 0x0e, 0x4d);

/// **`LIQUID`, scaled for 4K.**
///
/// `Material`'s lengths are in *physical* pixels. `Scaled` multiplies
/// them by the DPI factor, but a 4K window at scale 1 is not a scaled
/// window — it is a large one, and `LIQUID`'s 13 px rim on an 820 px
/// panel is a hairline. Everything here is the preset times 2.3, which
/// is the ratio of this design's panel to the one the preset was tuned
/// against. The unitless fields — specular, saturation, dispersion,
/// adapt — are *not* multiplied, because a fraction has no length in it.
fn liquid_4k() -> Material {
    Material { rim: 30.0, refraction: 26.0, blur: 5.0, tint: TINT, ..Material::LIQUID }
}

// ---- the page being blurred ------------------------------------------

/// What the glass reads. A blur over a flat background is a flat
/// background, so this is a field of saturated tiles with real glyphs on
/// them — structure at two scales, because a wide blur eats the small
/// one and a bevel only displaces the large one.
const TILES: [(&str, Color); 24] = [
    ("Aurora", Color(0xff, 0x6b, 0x6b, 0xff)),
    ("Basalt", Color(0xfe, 0xca, 0x57, 0xff)),
    ("Cinder", Color(0x1d, 0xd1, 0xa1, 0xff)),
    ("Drift", Color(0x54, 0xa0, 0xff, 0xff)),
    ("Ember", Color(0xff, 0x9f, 0x43, 0xff)),
    ("Fathom", Color(0xa2, 0x9b, 0xfe, 0xff)),
    ("Glacier", Color(0x48, 0xdb, 0xfb, 0xff)),
    ("Harbour", Color(0xff, 0x9f, 0xf3, 0xff)),
    ("Ingot", Color(0xba, 0xd7, 0x00, 0xff)),
    ("Lumen", Color(0x00, 0xd2, 0xd3, 0xff)),
    ("Marrow", Color(0xee, 0x52, 0x53, 0xff)),
    ("Nadir", Color(0x0a, 0xbd, 0xe3, 0xff)),
    ("Onyx", Color(0xf7, 0xd7, 0x94, 0xff)),
    ("Pallor", Color(0xc7, 0xec, 0xee, 0xff)),
    ("Quarry", Color(0xff, 0x79, 0x3f, 0xff)),
    ("Relic", Color(0x8e, 0xf6, 0xe4, 0xff)),
    ("Solder", Color(0xd6, 0xa2, 0xe8, 0xff)),
    ("Tundra", Color(0x7e, 0xd6, 0xdf, 0xff)),
    ("Umber", Color(0xf3, 0xa6, 0x83, 0xff)),
    ("Vellum", Color(0xcf, 0xf0, 0x9e, 0xff)),
    ("Wick", Color(0xff, 0xd3, 0x2a, 0xff)),
    ("Xenon", Color(0x9b, 0xf6, 0xff, 0xff)),
    ("Yarrow", Color(0xbd, 0xb2, 0xff, 0xff)),
    ("Zephyr", Color(0xff, 0xc6, 0xff, 0xff)),
];

/// One id per tile, all twenty-four of them.
const TILE_IDS: [&str; 24] = [
    "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k10", "k11", "k12", "k13",
    "k14", "k15", "k16", "k17", "k18", "k19", "k20", "k21", "k22", "k23",
];

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Tile {
    #[widget_id("tile_t")]
    name: Label<String>,
}

/// Six tiles across, **and four of these rather than one used four
/// times**. A `#[widget_id]` lives on a field of its parent, so one
/// shared row type gives all four rows the same six ids — a selector
/// matches every node carrying one, and the page came out as six
/// full-height stripes with eighteen of the twenty-four colours never
/// reaching the screen. A blur over vertical stripes has structure in
/// one axis, which is half a demonstration.
///
/// (`List<Tile>` cannot do it either: a list styles every item from
/// **one probe**, so all six would take the same background.)
#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct TileRow0 {
    #[widget_id("k0")]
    a: Tile,
    #[widget_id("k1")]
    b: Tile,
    #[widget_id("k2")]
    c: Tile,
    #[widget_id("k3")]
    d: Tile,
    #[widget_id("k4")]
    e: Tile,
    #[widget_id("k5")]
    f: Tile,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct TileRow1 {
    #[widget_id("k6")]
    a: Tile,
    #[widget_id("k7")]
    b: Tile,
    #[widget_id("k8")]
    c: Tile,
    #[widget_id("k9")]
    d: Tile,
    #[widget_id("k10")]
    e: Tile,
    #[widget_id("k11")]
    f: Tile,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct TileRow2 {
    #[widget_id("k12")]
    a: Tile,
    #[widget_id("k13")]
    b: Tile,
    #[widget_id("k14")]
    c: Tile,
    #[widget_id("k15")]
    d: Tile,
    #[widget_id("k16")]
    e: Tile,
    #[widget_id("k17")]
    f: Tile,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct TileRow3 {
    #[widget_id("k18")]
    a: Tile,
    #[widget_id("k19")]
    b: Tile,
    #[widget_id("k20")]
    c: Tile,
    #[widget_id("k21")]
    d: Tile,
    #[widget_id("k22")]
    e: Tile,
    #[widget_id("k23")]
    f: Tile,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Page {
    r0: TileRow0,
    r1: TileRow1,
    r2: TileRow2,
    r3: TileRow3,
}

// ---- the glass -------------------------------------------------------

/// One panel's contents: what it is, and what it is showing.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 10.0)]
struct Cell {
    #[widget_id("cap")]
    title: Label<String>,
    #[widget_id("sub")]
    sub: Label<String>,
}

/// Four panels across. **One id for all of them**, unlike the tiles: a
/// selector matches every node carrying it, and here that is what is
/// wanted — twelve panels of identical geometry, differing only in the
/// material each is given in `build`.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 48.0)]
struct PanelRow {
    #[widget_id("cellw")]
    a: Glass<Cell>,
    #[widget_id("cellw")]
    b: Glass<Cell>,
    #[widget_id("cellw")]
    c: Glass<Cell>,
    #[widget_id("cellw")]
    d: Glass<Cell>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 48.0)]
struct Grid {
    #[widget_id("gtop")]
    top: Spacer,
    r0: PanelRow,
    r1: PanelRow,
    r2: PanelRow,
    #[widget_id("gbot")]
    bottom: Spacer,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 28.0)]
struct Bar {
    #[widget_id("brand")]
    brand: Label<String>,
    #[widget_id("gap")]
    gap: Spacer,
    #[widget_id("note")]
    note: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(row, spacing = 30.0)]
struct Tabs {
    #[widget_id("t0")]
    t0: Button<Demo, String>,
    #[widget_id("t1")]
    t1: Button<Demo, String>,
    #[widget_id("t2")]
    t2: Button<Demo, String>,
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

/// **A stack, and the order in it is the whole feature.**
///
/// A `column` allocates non-overlapping rects, so a bar laid out above
/// the page would have nothing behind it to blur. The stack's children
/// share one rect and paint in field order, so each of these reads
/// everything declared before it: the grid reads the page, and the bar
/// and dock read the page *and* the grid.
#[derive(StaticTree, Default)]
#[widget(stack)]
struct Screen {
    #[widget_id("page")]
    page: Page,
    grid: Grid,
    #[widget_id("bar")]
    bar: Glass<Bar>,
    dock: Floating,
}

/// Advance what the tree animates — M22/T22.4.
///
/// There is no framework-walked per-node tick (`App::tick` records why),
/// so the application names what moves. Without this a panel retargets
/// on every press and then never travels.
///
/// **`|`, not `||`, and `fold` rather than `any`.** Both short-circuit,
/// and a short circuit here does not merely mis-report — it stops
/// calling `tick` on every panel after the first one that is moving, so
/// pressing two at once animates one of them.
fn advance(d: &mut Demo, dt_ms: f32) -> bool {
    let g = &mut d.ui.grid;
    let cells: [&mut Glass<Cell>; 12] = [
        &mut g.r0.a, &mut g.r0.b, &mut g.r0.c, &mut g.r0.d,
        &mut g.r1.a, &mut g.r1.b, &mut g.r1.c, &mut g.r1.d,
        &mut g.r2.a, &mut g.r2.b, &mut g.r2.c, &mut g.r2.d,
    ];
    let moved = cells.into_iter().fold(false, |acc, p| p.tick(dt_ms) | acc);
    moved | d.ui.bar.tick(dt_ms) | d.ui.dock.row.dock.tick(dt_ms)
}

#[derive(DeriveApp, Default)]
#[app(tick = advance)]
struct Demo {
    #[tree]
    ui: Screen,
}

// ---- layout ----------------------------------------------------------

/// Designed at 4K and laid out in fractions of it, so a smaller window
/// is the same picture rather than a cropped one.
const W: u32 = 3840;
const H: u32 = 2160;
const CELL_W: f32 = 820.0;
const CELL_H: f32 = 420.0;
/// `(3840 − 4·820 − 3·48) / 2`, so the grid is centred.
const MARGIN: f32 = 208.0;

fn cell_ink() -> Patch {
    Patch::new().color(INK).font_size(34.0).font_weight(700.0)
}

fn stylesheet() -> Vec<Rule> {
    let mut rules = vec![
        Rule::new(Selector::Kind("Screen"), Patch::new().background(PAGE)),
        // The page under everything: four rows of six tiles, each
        // filling its share.
        Rule::new(Selector::Kind("Tile"), Patch::new().width(Length::FILL).height(Length::FILL)),
        Rule::new(Selector::Kind("TileRow0"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("TileRow1"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("TileRow2"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Kind("TileRow3"), Patch::new().height(Length::FILL)),
        Rule::new(
            Selector::Id("tile_t"),
            Patch::new().color(TILE_INK).font_size(46.0).font_weight(700.0),
        ),
        // The bar: a strip at the top of the shared rect. `Glass`
        // defaults to FILL width and Shrink height, so this pins the
        // height and the padding and nothing else.
        Rule::new(
            Selector::Id("bar"),
            Patch::new()
                .height(Length::Fixed(150.0))
                .padding(Edges { left: MARGIN, right: MARGIN, top: 48.0, bottom: 48.0 }),
        ),
        Rule::new(
            Selector::Id("brand"),
            Patch::new().color(INK).font_size(44.0).font_weight(700.0),
        ),
        Rule::new(Selector::Id("gap"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("note"), Patch::new().color(DIM).font_size(26.0)),
        // The grid. Its own padding clears the bar at the top and the
        // dock at the bottom; the two FILL spacers centre what is left.
        Rule::new(
            Selector::Kind("Grid"),
            Patch::new().padding(Edges { left: MARGIN, right: MARGIN, top: 200.0, bottom: 220.0 }),
        ),
        Rule::new(Selector::Id("gtop"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Id("gbot"), Patch::new().height(Length::FILL)),
        // **One rule, twelve panels.** A selector matches every node
        // carrying the id, and all twelve are the same geometry.
        Rule::new(
            Selector::Id("cellw"),
            Patch::new()
                .width(Length::Fixed(CELL_W))
                .height(Length::Fixed(CELL_H))
                .radius(56.0)
                .padding(Edges { left: 44.0, right: 44.0, top: 40.0, bottom: 40.0 }),
        ),
        Rule::new(Selector::Id("cap"), cell_ink()),
        Rule::new(Selector::Id("sub"), Patch::new().color(DIM).font_size(25.0)),
        // The dock: a pill sized to its tabs, floated to the bottom
        // centre by three FILL spacers.
        Rule::new(Selector::Id("push"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Id("dockl"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("dockr"), Patch::new().width(Length::FILL)),
        // **Both of these, and the second is the one that catches
        // people.** A derived container defaults to FILL width, so
        // `Tabs` would expand to whatever box it was given and the
        // Shrink panel around it would measure *that* — a full-width
        // pill with Shrink written on it. A Shrink container needs
        // Shrink content.
        Rule::new(Selector::Kind("Tabs"), Patch::new().width(Length::Shrink)),
        Rule::new(
            Selector::Id("dock"),
            Patch::new()
                .width(Length::Shrink)
                .radius(66.0)
                .padding(Edges { left: 62.0, right: 62.0, top: 34.0, bottom: 34.0 }),
        ),
        Rule::new(
            Selector::Kind("Floating"),
            Patch::new().padding(Edges { left: 0.0, right: 0.0, top: 0.0, bottom: 70.0 }),
        ),
    ];
    for id in ["t0", "t1", "t2"] {
        rules.push(Rule::new(
            Selector::Id(id),
            Patch::new()
                .color(INK)
                .font_size(30.0)
                .font_weight(600.0)
                .background(Color(0, 0, 0, 0))
                .pressed_dim(0.0),
        ));
    }
    for (id, (_, colour)) in TILE_IDS.iter().zip(TILES.iter()) {
        rules.push(Rule::new(Selector::Id(id), Patch::new().background(*colour)));
    }
    rules
}

// ---- the thirteen materials ------------------------------------------

/// What each panel turns on, and what it is called.
///
/// Every one of these is `Material::LIQUID` with **one** thing changed,
/// so a screenshot is a controlled comparison rather than twelve
/// unrelated looks. The captions say which.
type Spec = (&'static str, &'static str, fn(Material) -> Material);

const SPECS: [Spec; 12] = [
    ("Frosted", "blur, no bevel — what M19 shipped", |_| Material {
        blur: 40.0,
        tint: TINT,
        ..Material::FROSTED
    }),
    ("Liquid", "the preset: bevel, Fresnel, shadow, dispersion", |m| m),
    ("No dispersion", "the same edge without the colour fringe", |m| Material {
        dispersion: 0,
        ..m
    }),
    ("Dispersion ×2.8", "blue deviates further than red — T22.2", |m| Material {
        dispersion: 255,
        ..m
    }),
    ("No adapt", "a fixed tint, whatever is behind it", |m| Material { adapt: 0, ..m }),
    ("Adapt full", "the tint follows the backdrop — T22.3", |m| Material { adapt: 255, ..m }),
    ("Desaturated", "chroma pulled out of what shows through", |m| Material {
        saturation: 40,
        ..m
    }),
    ("Deep shadow", "the unlit rim, darkened — T22.0", |m| Material {
        shadow: 220,
        specular: 0.0,
        ..m
    }),
    ("Thick rim", "a wider bevel bends more of the edge", |m| Material {
        rim: 96.0,
        refraction: 78.0,
        ..m
    }),
    ("Morph: merged", "one stretched blob — T22.5", |m| Material {
        morph_y: (260.0 * Material::MORPH_UNIT) as i16,
        ..m
    }),
    ("Morph: neck", "a separation inside κ/2, so it stays joined", |m| Material {
        morph_y: ((CELL_H + 10.0) * Material::MORPH_UNIT) as i16,
        ..m
    }),
    ("Morph: apart", "past κ/2 the neck pinches off", |m| Material {
        morph_y: ((CELL_H + 110.0) * Material::MORPH_UNIT) as i16,
        ..m
    }),
];

fn build() -> Demo {
    let mut app = Demo::default();

    // The page. Six tiles a row, four rows, twenty-four names.
    {
        let ui = &mut app.ui.page;
        let mut i = 0;
        let six = |t: [&mut Tile; 6], i: &mut usize| {
            for tile in t {
                tile.name.set(TILES[*i].0.to_string());
                *i += 1;
            }
        };
        six([&mut ui.r0.a, &mut ui.r0.b, &mut ui.r0.c, &mut ui.r0.d, &mut ui.r0.e, &mut ui.r0.f], &mut i);
        six([&mut ui.r1.a, &mut ui.r1.b, &mut ui.r1.c, &mut ui.r1.d, &mut ui.r1.e, &mut ui.r1.f], &mut i);
        six([&mut ui.r2.a, &mut ui.r2.b, &mut ui.r2.c, &mut ui.r2.d, &mut ui.r2.e, &mut ui.r2.f], &mut i);
        six([&mut ui.r3.a, &mut ui.r3.b, &mut ui.r3.c, &mut ui.r3.d, &mut ui.r3.e, &mut ui.r3.f], &mut i);
    }

    // The twelve panels.
    {
        let g = &mut app.ui.grid;
        let cells: [&mut Glass<Cell>; 12] = [
            &mut g.r0.a, &mut g.r0.b, &mut g.r0.c, &mut g.r0.d,
            &mut g.r1.a, &mut g.r1.b, &mut g.r1.c, &mut g.r1.d,
            &mut g.r2.a, &mut g.r2.b, &mut g.r2.c, &mut g.r2.d,
        ];
        let base = liquid_4k();
        for (cell, (title, sub, f)) in cells.into_iter().zip(SPECS.iter()) {
            cell.material = f(base);
            cell.content.title.set((*title).into());
            cell.content.sub.set((*sub).into());
        }
    }

    app.ui.bar.material = Material { blur: 36.0, tint: TINT, ..Material::FROSTED };
    app.ui.bar.content.brand.set("glasswall".into());
    app.ui.bar.content.note.set("fourteen backdrop filters, 3840 × 2160".into());

    app.ui.dock.row.dock.material = liquid_4k();
    app.ui.dock.row.dock.content.t0.label = "Press".into();
    app.ui.dock.row.dock.content.t1.label = "any".into();
    app.ui.dock.row.dock.content.t2.label = "panel".into();
    app
}

// ---- headless: a frame, and what it costs ----------------------------

fn host(w: u32, h: u32) -> lumen_static_shell::Host<Demo> {
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
fn gpu_bench(_: &lumen_static_shell::Host<Demo>, _: u32, _: u32) {
    println!("gpu:      not built — rebuild with `--features gpu` for the number that matters");
}

#[cfg(feature = "gpu")]
fn gpu_bench(host: &lumen_static_shell::Host<Demo>, w: u32, h: u32) {
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
    /// Cheap, and it is the assertion the other two rest on: a scene
    /// that silently dropped its glass would still lay out, still paint,
    /// and still pass anything measured on the pixels it did emit.
    #[test]
    fn the_scene_emits_every_panel() {
        let host = host(W, H);
        let n = host.list.cmds.iter().filter(|c| matches!(c, Cmd::Glass { .. })).count();
        println!("{n} glass commands");
        assert_eq!(n, 14, "twelve grid panels, the bar and the dock");
    }

    /// **A morphing panel inks the union of both blobs.**
    ///
    /// The command's rect is the ink, the clip test *and* the source
    /// (M22/T22.5), so a second blob outside it would be stale, holed
    /// and short of backdrop. Here that is visible as a command taller
    /// than the node it came from — and it is what makes the bottom row
    /// of this example show anything at all.
    #[test]
    fn the_morph_panels_ink_past_their_own_cells() {
        let host = host(W, H);
        let tall = host
            .list
            .cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::Glass { rect, material, .. } if material.morph_y != 0 => {
                    Some(rect[3] - rect[1])
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        println!("morphing panels are {tall:?} px tall against a {CELL_H} px cell");
        assert_eq!(tall.len(), 3, "three morph panels in the bottom row");
        for h in tall {
            assert!(h > CELL_H + 100.0, "a morphing panel inked only its own cell ({h} px)");
        }
    }

    /// **Every panel is advanced, not just the first one moving.**
    ///
    /// `advance` folds rather than short-circuits, and this is the case
    /// that tells the difference: one panel is already animating when a
    /// second is pressed. With `any` or `||` the first returns `true`,
    /// the iteration stops, and the second never moves — which on screen
    /// is a panel that ignores you while another is settling.
    #[test]
    fn a_second_press_animates_while_the_first_is_still_settling() {
        let mut host = host(W, H);
        let rest = liquid_4k().rim;

        // **The grid panels are the only nodes of this exact geometry**,
        // so they can be found by it rather than by a hand-counted index
        // that a new field would silently shift. Twelve of them, in tree
        // order, which the corner assertions below pin.
        let cells: Vec<usize> = host
            .states
            .iter()
            .enumerate()
            .filter(|(_, s)| s.size.w == CELL_W && s.size.h == CELL_H)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(cells.len(), 12, "the grid is not twelve panels of one size");
        let (tl, br) = (&host.states[cells[0]], &host.states[*cells.last().unwrap()]);
        assert!(
            tl.pos.x < br.pos.x && tl.pos.y < br.pos.y,
            "the first and last panels are not opposite corners; tree order moved"
        );
        let centre = |i: usize| {
            let s = &host.states[i];
            (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
        };
        let (p0, p1) = (centre(cells[0]), centre(*cells.last().unwrap()));

        // Press the first, deform it, then release — it is now animating
        // back to rest and keeps reporting `true`.
        host.post(Event::down(p0.0, p0.1));
        host.pump();
        for _ in 0..3 {
            host.tick(30.0);
            host.pump();
        }
        host.post(Event::up(p0.0, p0.1));
        host.pump();
        assert!(host.tick(0.0), "the first panel settled too early; the test proves nothing");

        // Now press the second and advance once.
        host.post(Event::down(p1.0, p1.1));
        host.pump();
        host.tick(30.0);
        host.pump();

        let moved = host.app.ui.grid.r2.d.material_now().rim;
        println!("second panel's rim {moved:.1} against {rest:.1} at rest");
        assert!(
            moved > rest + 1.0,
            "the second panel never advanced ({moved:.1} vs {rest:.1}) — `advance` short-circuited"
        );
    }
}
