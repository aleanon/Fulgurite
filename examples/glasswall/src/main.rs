//! **A phone home screen made of glass** — what the material is *for*.
//!
//! `just example glasswall` (add `--features gpu`; see below). Twenty-six
//! backdrop filters laid out the way a mobile screen actually is: a
//! status bar across the top, a 4 × 6 grid of clear app icons, and a
//! floating tab bar at the bottom.
//!
//! # It is designed at 1290 × 2796 and opens at 460 × 997
//!
//! Two different questions. The *design* is a phone at 3×, which is
//! where a 24 pt icon lands as 224 physical pixels and a bevel has room
//! to be seen; `--shot` and `--bench` render there. The *window* opens
//! at a size a desktop will actually grant, and everything is a fraction
//! of [`k`], the ratio between the two.
//!
//! **Hard-coded sizes are not merely wrong at the other size, they are
//! invisible.** `resolve_self` ends in `limits.clamp(...)`, so a
//! `Fixed(224)` height inside a container that has run out of room
//! resolves to **zero** rather than overflowing, and a zero-height node
//! paints nothing. Asking for a 2796 px window, getting 1008, and
//! laying out at 1× produces a black screen with nothing on it but the
//! bar and the dock — which the `FILL` spacers still place.
//!
//! # Clear icons over a picture, and the two have to go together
//!
//! The icons carry a tint of 20/255. Nearly everything you see in one is
//! the wallpaper behind it, refracted through a bevelled edge and lit
//! from one side.
//!
//! **That only works over a picture.** The first version of this file
//! drew the wallpaper as a grid of flat colour tiles, and it showed: a
//! clear icon over a *uniform* patch of colour has nothing to refract,
//! because bending a flat colour gives a flat colour. What you saw was
//! coloured squares with faint outlines on them. The wallpaper is now
//! generated — a soft ramp with wide radial blooms screened over it —
//! for one reason, which is that it has a gradient everywhere.
//!
//! This is also why the icons sit *over* the wallpaper in a
//! `#[widget(stack)]` and not above it in a column: a column allocates
//! non-overlapping rects, so an icon laid out that way would have the
//! page behind it and nothing to read.
//!
//! `tests::the_wallpaper_is_a_picture` pins both halves — smooth
//! everywhere, and not flat — because each is a way to end up with no
//! picture and neither is visible in a test of the glass itself.
//!
//! # Build it with `--features gpu`
//!
//! Twenty-six panels is far past what the software filter is for.
//! Measured, `just glasswall-bench`: **150 ms** a frame on the software
//! canvas against **2.1 ms** on a GPU — seven frames a second against
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
use lumen_static_widgets::prelude::*;
use lumen_static_widgets::Glass;
use lumen_static_winit::{application, Program, Run};

// ---- the device ------------------------------------------------------

/// The design's own size: a phone at 3×, which is where a 24 pt icon
/// lands as 224 physical pixels and a bevel has room to be seen.
const DESIGN_W: f32 = 1290.0;
const DESIGN_H: f32 = 2796.0;

/// What the window actually opens at.
///
/// **Not the design size**, and that is the whole lesson of this
/// constant. Asking for a 2796 px tall window gets you whatever the
/// window manager is willing to give — 1008 px on an ordinary desktop —
/// and a layout with hard-coded sizes then does not fit. In this
/// framework it does not overflow or clip when that happens: it
/// *vanishes*. `resolve_self` ends in `limits.clamp(...)`, so a
/// `Fixed(224)` height inside a container that has run out of room
/// resolves to **zero**, and a zero-height node paints nothing. The
/// screen goes black except for whatever the `FILL` spacers still
/// place, which is exactly the bar and the dock.
///
/// So everything below is a fraction of [`k`], and the window opens at
/// a size a desktop will actually grant.
const W: u32 = 460;
const H: u32 = 997;

/// The scale between the design and the window it got.
///
/// `min` of the two ratios, so the layout fits on both axes rather than
/// fitting one and overflowing the other — an overflow here is a
/// collapse, not a scrollbar.
fn k(w: u32, h: u32) -> f32 {
    (w as f32 / DESIGN_W).min(h as f32 / DESIGN_H)
}

const ICON: f32 = 224.0;
/// `(1290 − 4·224 − 3·104) / 2`, where 104 is the icon gap.
const MARGIN: f32 = 41.0;
const ICON_GAP: f32 = 104.0;

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
///
/// **Light enough to still be glass.** At 0x59 the status bar read as a
/// black stripe across the top rather than as a frosted one, which is
/// the failure mode of every tint chosen for legibility alone: past
/// about a third opacity a backdrop filter is just a rectangle, and the
/// blur behind it is work with no output.
const BAR_TINT: Color = Color(0x08, 0x0a, 0x12, 0x33);
/// The dock carries a little more than the status bar: it sits over the
/// brightest part of the wallpaper and its glyphs are tap targets.
const DOCK_TINT: Color = Color(0x08, 0x0a, 0x12, 0x40);

/// **`LIQUID`, scaled to the icon it is drawn on.**
///
/// `Material`'s lengths are in *physical* pixels, and `Scaled`
/// multiplies them by the DPI factor — which is 1 here, because a large
/// window is not a scaled one. So they are scaled by hand, by the same
/// `k` every other length in this file uses: a 24 px rim is right on a
/// 224 px icon and absurd on an 80 px one.
///
/// The unitless fields — specular, saturation, dispersion, adapt — are
/// **not** multiplied. A fraction has no length in it, and scaling one
/// is how a material stops meaning the same thing at two sizes.
fn icon_glass(k: f32) -> Material {
    Material {
        rim: 24.0 * k,
        refraction: 20.0 * k,
        blur: 2.0 * k,
        tint: ICON_TINT,
        // Clear glass shows its edge or it shows nothing, so the rim
        // works harder here than on a tinted panel.
        specular: 1.0,
        shadow: 110,
        ..Material::LIQUID
    }
}

// ---- the wallpaper ---------------------------------------------------

/// **A picture, not a grid of coloured squares.**
///
/// The first version of this file laid the wallpaper out as twenty-eight
/// flat tiles, and it looked like twenty-eight flat tiles: hard edges
/// everywhere, and a clear icon sitting over one of them had a *uniform*
/// backdrop to refract, which is to say nothing to refract. Bending a
/// flat colour produces a flat colour.
///
/// So this is generated: a soft vertical ramp with a handful of wide
/// radial blooms screened over it, which is what a phone wallpaper
/// actually looks like and — more to the point — has a gradient
/// everywhere. The icon rims have something to bend.
///
/// **Both dimensions are under 1024, and that is a hard requirement
/// rather than a preference.** The GPU backend atlases images into a
/// 1024 px texture and *refuses* anything larger — `image_slot` returns
/// `Ok(None)`, which `ImageSource`'s contract says draws nothing. The
/// software canvas has no atlas and draws any size. So a wallpaper of
/// 540 × 1170 renders perfectly headless, renders perfectly in a
/// software window, and is **missing entirely on the GPU** — which is
/// what a black screen with the glass still on it looks like.
/// The `const` assertion below refuses it at compile time.
const WALL_W: u32 = 470;
const WALL_H: u32 = 1018;

/// `lumen-static-wgpu`'s `IMAGE_ATLAS`, restated because it is private —
/// with the consequence beside it, which is what makes a restated
/// constant survive being restated.
const GPU_IMAGE_ATLAS: u32 = 1024;
const _: () = assert!(
    WALL_W <= GPU_IMAGE_ATLAS && WALL_H <= GPU_IMAGE_ATLAS,
    "the wallpaper is larger than the GPU's image atlas: it will draw on \
     the software canvas, draw headless, and vanish on the GPU"
);
const WALL_ID: lumen_static::display::ImageId = lumen_static::display::ImageId(1);

/// Where `Cmd::Image` gets its pixels. One image, generated once.
struct Wallpaper {
    px: Vec<u8>,
}

impl lumen_static::ImageSource for Wallpaper {
    fn image(&self, id: lumen_static::display::ImageId) -> Option<(u32, u32, &[u8])> {
        (id == WALL_ID).then_some((WALL_W, WALL_H, &self.px))
    }
}

impl Wallpaper {
    /// Rendered at a fraction of the screen and scaled up by `Fit::Cover`.
    ///
    /// **Deliberately low resolution.** Every gradient here is smooth
    /// over hundreds of pixels, so a bilinear upscale loses nothing a
    /// viewer can see, and generating 3.6 M pixels to blur and refract
    /// a picture that has no detail in it would be a cost with no
    /// output.
    fn new() -> Self {
        // (x, y, radius, colour) in fractions of the image.
        const BLOOMS: [(f32, f32, f32, [f32; 3]); 6] = [
            (0.18, 0.10, 0.55, [0.15, 0.55, 1.00]),
            (0.88, 0.20, 0.50, [0.60, 0.25, 0.95]),
            (0.10, 0.48, 0.45, [0.00, 0.75, 0.80]),
            (0.95, 0.58, 0.52, [1.00, 0.30, 0.55]),
            (0.30, 0.85, 0.55, [1.00, 0.55, 0.20]),
            (0.80, 0.96, 0.48, [0.95, 0.20, 0.40]),
        ];
        let mut px = vec![0u8; (WALL_W * WALL_H * 4) as usize];
        for y in 0..WALL_H {
            let v = y as f32 / (WALL_H - 1) as f32;
            for x in 0..WALL_W {
                let u = x as f32 / (WALL_W - 1) as f32;
                // The ground: night at the top, ember at the bottom.
                let mut c = [
                    0.05 + 0.22 * v,
                    0.03 + 0.07 * v,
                    0.18 + 0.10 * (1.0 - v),
                ];
                for (bx, by, r, bc) in BLOOMS {
                    // Aspect-corrected, or a blob on a 1:2.2 canvas is an
                    // ellipse and the seams between them go vertical.
                    let dx = (u - bx) * (WALL_W as f32 / WALL_H as f32) * 2.2;
                    let dy = v - by;
                    let d = ((dx * dx + dy * dy).sqrt() / r).min(1.0);
                    // Smooth falloff: `1 - smoothstep`, so a bloom has
                    // no edge of its own to show up as a ring.
                    let w = 1.0 - d * d * (3.0 - 2.0 * d);
                    let w = w * w;
                    // Screen, not add: it saturates toward white instead
                    // of clipping each channel at a different point,
                    // which is the same argument the inner shadow makes
                    // for multiplying.
                    for k in 0..3 {
                        c[k] = 1.0 - (1.0 - c[k]) * (1.0 - bc[k] * w * 0.85);
                    }
                }
                let i = ((y * WALL_W + x) * 4) as usize;
                for k in 0..3 {
                    px[i + k] = (c[k].clamp(0.0, 1.0) * 255.0) as u8;
                }
                px[i + 3] = 255;
            }
        }
        Wallpaper { px }
    }
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
    wall: lumen_static_widgets::Image,
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

/// Every length here is `design × k`. The `#[widget(..., spacing = N)]`
/// attributes take a literal, so the spacings are re-stated as rules —
/// a `Patch` overrides the attribute, and a scaled layout cannot leave
/// its gaps at 1×.
fn stylesheet(k: f32) -> Vec<Rule> {
    let s = |v: f32| v * k;
    let pad = |l: f32, r: f32, t: f32, b: f32| {
        Edges { left: s(l), right: s(r), top: s(t), bottom: s(b) }
    };
    vec![
        Rule::new(Selector::Kind("Screen"), Patch::new().background(PAGE)),
        // The spacings, re-stated because an attribute takes a literal.
        Rule::new(Selector::Kind("AppRow"), Patch::new().spacing(s(ICON_GAP))),
        Rule::new(Selector::Kind("Apps"), Patch::new().spacing(s(58.0))),
        Rule::new(Selector::Kind("App"), Patch::new().spacing(s(14.0))),
        Rule::new(Selector::Kind("Tabs"), Patch::new().spacing(s(74.0))),
        Rule::new(Selector::Kind("Status"), Patch::new().spacing(s(20.0))),
        // The wallpaper: one image, cropped to cover.
        Rule::new(
            Selector::Id("wall"),
            Patch::new().width(Length::FILL).height(Length::FILL),
        ),
        // The icons. **`radius` is what makes them app icons** — a
        // square of glass reads as a window, a superellipse-ish rounded
        // one reads as an icon.
        Rule::new(
            Selector::Id("icon"),
            Patch::new()
                .width(Length::Fixed(s(ICON)))
                .height(Length::Fixed(s(ICON)))
                .radius(s(58.0)),
        ),
        Rule::new(
            Selector::Id("glyph"),
            Patch::new().color(INK).font_size(s(92.0)).font_weight(600.0),
        ),
        // `align` is the container's, not the child's: `align_offset`
        // is applied by the parent to what it lays out.
        // Fixed on both axes and centred: see `Glyph` for why this is
        // a stack and why the size is pinned rather than filled.
        Rule::new(
            Selector::Kind("Glyph"),
            Patch::new()
                .width(Length::Fixed(s(ICON)))
                .height(Length::Fixed(s(ICON)))
                .align(Align::Center),
        ),
        Rule::new(Selector::Id("name"), Patch::new().color(DIM).font_size(s(30.0))),
        Rule::new(
            Selector::Kind("App"),
            Patch::new().width(Length::Fixed(s(ICON))).align(Align::Center),
        ),
        // The grid, centred between the two bars by its own padding and
        // two FILL spacers.
        Rule::new(
            Selector::Kind("Apps"),
            Patch::new().padding(pad(MARGIN, MARGIN, 300.0, 400.0)),
        ),
        Rule::new(Selector::Id("atop"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Id("abot"), Patch::new().height(Length::FILL)),
        // The status bar: a strip at the top of the shared rect.
        Rule::new(
            Selector::Id("status"),
            Patch::new()
                .height(Length::Fixed(s(158.0)))
                .padding(pad(64.0, 64.0, 62.0, 30.0)),
        ),
        Rule::new(
            Selector::Id("clock"),
            Patch::new().color(INK).font_size(s(38.0)).font_weight(700.0),
        ),
        Rule::new(Selector::Id("sgap"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("meta"), Patch::new().color(DIM).font_size(s(30.0))),
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
                .radius(s(74.0))
                .padding(pad(74.0, 74.0, 44.0, 44.0)),
        ),
        Rule::new(
            Selector::Kind("Floating"),
            Patch::new().padding(pad(0.0, 0.0, 0.0, 90.0)),
        ),
        // No background and no press dim: the *panel* deforms under a
        // press, which is the whole point, and a grey rectangle
        // appearing behind the glyph would hide it.
        Rule::new(
            Selector::Id("tab"),
            Patch::new()
                .color(INK)
                .font_size(s(52.0))
                .background(Color(0, 0, 0, 0))
                .pressed_dim(0.0),
        ),
    ]
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

fn build(k: f32) -> Home {
    let mut app = Home::default();
    app.ui.wall = lumen_static_widgets::Image::new(
        WALL_ID,
        lumen_static::Size { w: WALL_W as f32, h: WALL_H as f32 },
    );
    // **`Cover`, not `Contain`.** A wallpaper fills the screen and is
    // cropped; letterboxing one would put the page colour down the
    // sides, and the icons over that edge would have nothing to refract.
    app.ui.wall.fit = lumen_static_widgets::image::Fit::Cover;
    {
        let a = &mut app.ui.apps;
        let rows = [&mut a.r0, &mut a.r1, &mut a.r2, &mut a.r3, &mut a.r4, &mut a.r5];
        let mut i = 0;
        for r in rows {
            for icon in [&mut r.a, &mut r.b, &mut r.c, &mut r.d] {
                icon.art.material = icon_glass(k);
                icon.art.content.ch.set(APPS[i].0.into());
                icon.name.set(APPS[i].1.into());
                i += 1;
            }
        }
    }
    app.ui.status.material = Material { blur: 26.0 * k, tint: BAR_TINT, ..Material::FROSTED };
    app.ui.status.content.clock.set("9:41".into());
    app.ui.status.content.meta.set("twenty-six panels".into());

    app.ui.dock.row.dock.material = Material { tint: DOCK_TINT, ..icon_glass(k) };
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
    let mut host: lumen_static_shell::Host<Home> = program(w, h).host();
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
    canvas.images = Some(Box::new(Wallpaper::new()));
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
    canvas.images = Some(Box::new(Wallpaper::new()));
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
    g.set_images(Box::new(Wallpaper::new()));
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
    // **The headless modes default to the *design* size, the window to
    // what a desktop will grant.** They are different questions: a shot
    // wants the phone's real 1290 × 2796, and a window that asks for it
    // gets whatever the window manager feels like.
    let dim = |i: usize, d: u32| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (dw, dh) = (DESIGN_W as u32, DESIGN_H as u32);
    match args.first().map(String::as_str) {
        Some("--shot") => {
            let path = args.get(1).cloned().unwrap_or_else(|| "glasswall.ppm".into());
            shot(&path, dim(2, dw), dim(3, dh));
        }
        Some("--bench") => bench(dim(1, dw), dim(2, dh)),
        _ => {
            let mut p = program(W, H).title("glasswall").images(Wallpaper::new());
            // Inert unless built with `--features agent` (M47/T47.7).
            if let Ok(addr) = std::env::var("LUMEN_AGENT") {
                p = p.agent(addr);
            }
            p.run()
        }
    }
}

/// **The program, described once** — M32 (`docs/plan-program.md`). The
/// window, `--shot`, `--bench` and the tests are built from it. The
/// wallpaper is added by the window only: a headless canvas takes its
/// own, because `ImageSource` is taken, not shared.
fn program(w: u32, h: u32) -> Program<Home> {
    let scale = k(w, h);
    application(move || build(scale)).size(w, h).page(PAGE).style(move |_: &Home| stylesheet(scale))
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
    /// M25/T25.2: no rule in the sheet is a no-op.
    #[test]
    fn every_stylesheet_rule_changes_something() {
        let host = host(W, H);
        let sheet = host.stylesheet();
        let found: Vec<String> = host
            .sheet_findings()
            .iter()
            .map(|f| format!("rule {} {:?}: {:?}", f.rule, sheet[f.rule].selector, f.issue))
            .collect();
        println!("{} rules, {} findings", sheet.len(), found.len());
        assert!(!sheet.is_empty(), "setup: the sheet was applied");
        assert!(found.is_empty(), "stylesheet rules that change nothing:\n{}", found.join("\n"));
    }

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
        // **The scaled size, not `ICON`.** Every length in this file is
        // `design × k`, so a helper that looked for 224 px nodes found
        // none at any window size but the design's.
        let want = ICON * k(W, H);
        let is_icon = |i: usize| {
            let s = &host.states[i];
            (s.size.w - want).abs() < 0.5 && (s.size.h - want).abs() < 0.5
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
        // **Both canvases need the image source.** Without it neither
        // draws the wallpaper, and the test compares glass over the page
        // colour with page colour — which is a real measurement of
        // something nobody will ever see.
        let shot = |h: &lumen_static_shell::Host<Home>| {
            let mut c = Canvas::new(W, H);
            c.images = Some(Box::new(Wallpaper::new()));
            c.clear(PAGE);
            lumen_static::Renderer::render(&mut c, &h.list, W, H);
            c
        };
        let host = host(W, H);
        let c = shot(&host);
        // The same wallpaper with no glass on it at all.
        let bare = shot(&host2());

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
            let d = |x: usize, y: usize| {
                let (a, b) = (at(&c, x, y), at(&bare, x, y));
                (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max)
            };
            // Off the glyph, which is ink and not glass.
            middle = middle.max(d(cx as usize + (s.size.w * 0.36) as usize, cy as usize));
            // **A band across the rim, not one pixel.** The rim is
            // `24 × k` px wide, so a fixed 4 px offset lands mid-bevel at
            // the design size and past it at any smaller one.
            let rim = (24.0 * k(W, H)).max(2.0) as usize;
            for off in 1..=rim {
                edge = edge.max(d(s.pos.x as usize + off, cy as usize));
            }
        }
        println!("worst change at an icon's middle {middle:.0}, at its rim {edge:.0}");
        assert!(middle < 40.0, "the icons are not clear — the middle moved {middle:.0} levels");
        assert!(edge > 18.0, "the icons are invisible — even the rim moved only {edge:.0}");
    }

    /// **The wallpaper is a picture: smooth everywhere, and not flat.**
    ///
    /// Both halves are the assertion. The first version of this example
    /// drew the wallpaper as a grid of flat colour tiles, which fails
    /// *smooth* — hard edges every few hundred pixels — and a clear icon
    /// over one of them had a uniform backdrop to refract, which is to
    /// say nothing to refract. Dropping the image source entirely fails
    /// *not flat*, and that is the other way to end up with no picture.
    ///
    /// Measured on a row well clear of the icons, so what is scanned is
    /// the wallpaper and not a rim.
    #[test]
    fn the_wallpaper_is_a_picture() {
        use lumen_static_shell::software::Canvas;
        let host = host2();
        let mut c = Canvas::new(W, H);
        c.images = Some(Box::new(Wallpaper::new()));
        c.clear(PAGE);
        lumen_static::Renderer::render(&mut c, &host.list, W, H);

        let lum = |x: usize, y: usize| {
            let i = (y * W as usize + x) * 4;
            0.299 * c.pixels[i] as f32
                + 0.587 * c.pixels[i + 1] as f32
                + 0.114 * c.pixels[i + 2] as f32
        };
        // Two rows clear of every glyph and label: one under the status
        // bar and above the first icon row, one below the dock.
        //
        // **The jump assertion verifies its own fixture.** A row that
        // strayed into text would produce exactly the hard edge it
        // forbids, so a badly chosen row fails loudly instead of passing
        // quietly — which is the opposite polarity to the usual "the
        // setup silently did not happen".
        let (mut jump, mut lo, mut hi) = (0.0f32, f32::MAX, f32::MIN);
        // **Fractions of the window, not pixels.** These were 260 and
        // 2740 — correct for the design's 2796 px height and off the end
        // of the 997 px one the window actually opens at.
        let rows = [
            (H as f32 * 260.0 / DESIGN_H) as usize,
            (H as f32 * 2740.0 / DESIGN_H) as usize,
        ];
        for y in rows {
            for x in 1..W as usize {
                let v = lum(x, y);
                jump = jump.max((v - lum(x - 1, y)).abs());
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        println!("wallpaper: largest adjacent jump {jump:.1}, range {:.0}", hi - lo);
        assert!(jump < 4.0, "the wallpaper has a hard edge in it ({jump:.1} levels in one pixel)");
        assert!(hi - lo > 40.0, "the wallpaper is flat ({:.0} levels of range)", hi - lo);
    }

    /// The same tree with every material made fully transparent, so the
    /// wallpaper can be measured on its own.
    fn host2() -> lumen_static_shell::Host<Home> {
        use lumen_static_shell::text::{FontMetrics, FontStack};
        let mut host: lumen_static_shell::Host<Home> = program(W, H).host();
        let app = &mut host.app;
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
        let rest = icon_glass(k(W, H)).rim;
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
        // **Relative.** The rim is `24 × k` px, so an absolute threshold
        // that held at the design size failed at the window's — the
        // press deforms by a *fraction*, and a fraction of a smaller rim
        // is a smaller number.
        println!("last icon's rim {moved:.2} against {rest:.2} at rest (×{:.3})", moved / rest);
        assert!(
            moved > rest * 1.01,
            "the second icon never advanced ({moved:.2} vs {rest:.2}) — `advance` short-circuited"
        );
    }
}

