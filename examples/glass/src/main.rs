//! **A glass toolbar over a scrolling list** — what `Cmd::Glass` is for.
//!
//! `just example glass`. Scroll the list with the wheel. The toolbar stays put and the blur
//! under it changes continuously, because a backdrop filter is a
//! function of the frame beneath it rather than of its own inputs. That
//! is the whole feature, and it is also the whole difficulty: see
//! `docs/plan-glass.md` §5 for the damage rule that makes it correct on
//! a partial redraw.
//!
//! `+` and `-` in the toolbar change σ, so the material is adjustable
//! rather than a fixed look. `just example glass --shot frame.ppm 120 30`
//! renders one frame headlessly instead, scrolled 120 px at σ = 30. Watch the cost while you do: a small blur
//! is the *expensive* one, because it cannot downsample
//! (`docs/results-glass.md` §1).
//!
//! # The three things this example is showing
//!
//! 1. **A `#[widget(stack)]`, not a column.** A column allocates
//!    non-overlapping rects, so a toolbar laid out above a list has the
//!    page behind it and nothing to blur. The stack's children share
//!    one rect; the toolbar is declared *after* the list, so it paints
//!    after it and reads it.
//! 2. **A translucent tint.** At full alpha the panel is a flat
//!    rectangle and none of this is visible — which is why the test
//!    suite forbids an opaque tint too.
//! 3. **Content with structure in it.** A blur over a flat background
//!    is a flat background. The rows carry colour swatches and real
//!    glyphs so there is something to smear.

use lumen_static::display::{Color, Material};
use lumen_static::prelude::*;
use lumen_static::{Glass, ScrollView};
use lumen_static_winit::{run, Config};

// ---- palette ---------------------------------------------------------

const PAGE: Color = Color(0x0b, 0x0e, 0x14, 0xff);
const INK: Color = Color(0xf1, 0xf5, 0xfb, 0xff);
const GHOST: Color = Color(0x23, 0x2c, 0x3e, 0xff);
/// On the cards, which are bright: near-black, not the page's ink.
const CARD_INK: Color = Color(0x14, 0x18, 0x20, 0xff);
const CARD_SUB: Color = Color(0x3a, 0x42, 0x52, 0xff);
/// The panel's own tint: the page colour at 45%. See `build` for why it
/// darkens rather than lightens.
const TINT: Color = Color(0x0b, 0x0e, 0x14, 0x73);

// ---- the tree --------------------------------------------------------

/// One card. Its *background* carries the colour, because a stylesheet
/// selector is `#id`, `.class` or kind — there are no descendant
/// selectors, so the only way to colour twelve cards differently is
/// twelve ids, and an id lives on a field of the parent.
///
/// (`List<Row>` was the first shape here and could not do it: a list
/// styles every item from **one probe**, so all twelve would have been
/// the same colour and the blur would have had one hue to smear.)
#[derive(StaticTree, Default)]
#[widget(column, spacing = 2.0)]
struct Card {
    #[widget_id("title")]
    title: Label<String>,
    #[widget_id("sub")]
    sub: Label<String>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Cards {
    #[widget_id("c0")]
    c0: Card,
    #[widget_id("c1")]
    c1: Card,
    #[widget_id("c2")]
    c2: Card,
    #[widget_id("c3")]
    c3: Card,
    #[widget_id("c4")]
    c4: Card,
    #[widget_id("c5")]
    c5: Card,
    #[widget_id("c6")]
    c6: Card,
    #[widget_id("c7")]
    c7: Card,
    #[widget_id("c8")]
    c8: Card,
    #[widget_id("c9")]
    c9: Card,
}

/// The toolbar's contents. It sits *inside* the `Glass`, so it paints on
/// top of the frost rather than under it.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 10.0)]
struct Toolbar {
    #[widget_id("heading")]
    heading: Label,
    #[widget_id("gap")]
    gap: Spacer,
    #[widget_id("sigma")]
    sigma: Label<String>,
    #[widget_id("less")]
    less: Button<Demo, String>,
    #[widget_id("more")]
    more: Button<Demo, String>,
}

/// The floating dock: a liquid pill pushed to the bottom centre.
///
/// Two `Spacer`s at `FILL` do the pushing, because a stack aligns all
/// of its children the same way and this one has to sit somewhere the
/// bar does not. It is also why the pill is a *pill*: a bevel is only
/// visible where there is an edge on screen, and a full-bleed bar has
/// three of its four rims off the window.
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

#[derive(StaticTree, Default)]
#[widget(row, spacing = 18.0)]
struct Tabs {
    #[widget_id("t0")]
    t0: Label,
    #[widget_id("t1")]
    t1: Label,
    #[widget_id("t2")]
    t2: Label,
}

/// **The composition the whole feature needs.**
///
/// A stack, so the two children share one rect: the scroller fills it,
/// the bar is a strip at the top, and the bar is declared *second* so it
/// paints — and therefore reads — after the content.
///
/// A `column` here would put the bar *above* the list instead of over
/// it, and a panel with nothing behind it is a tinted rectangle.
#[derive(StaticTree, Default)]
#[widget(stack)]
struct Screen {
    #[widget_id("content")]
    content: ScrollView<Cards>,
    /// **Frosted**: a blur, no bevel. What M19 shipped.
    #[widget_id("bar")]
    bar: Glass<Toolbar>,
    /// **Liquid**: barely any blur, a bevelled edge that bends what is
    /// behind it and catches the light. Side by side with the bar so the
    /// difference is one screenshot rather than two.
    dock: Floating,
}

#[derive(DeriveApp, Default)]
struct Demo {
    #[tree]
    ui: Screen,
    #[bind(ui.bar.content.sigma, "blur {} px")]
    sigma: i64,
}

// ---- style -----------------------------------------------------------

const CARDS: [(&str, &str, Color); 10] = [
    ("Aurora", "Polar Sessions", Color(0xff, 0x6b, 0x6b, 0xff)),
    ("Basalt", "Field Recordings", Color(0xfe, 0xca, 0x57, 0xff)),
    ("Cinder", "Late Transmissions", Color(0x1d, 0xd1, 0xa1, 0xff)),
    ("Drift", "Long Exposure", Color(0x54, 0xa0, 0xff, 0xff)),
    ("Ember", "Night Shift", Color(0xff, 0x9f, 0x43, 0xff)),
    ("Fathom", "Deep Water", Color(0xa2, 0x9b, 0xfe, 0xff)),
    ("Glacier", "Slow Melt", Color(0x48, 0xdb, 0xfb, 0xff)),
    ("Harbour", "Low Tide", Color(0xff, 0x9f, 0xf3, 0xff)),
    ("Ingot", "Cold Forge", Color(0xba, 0xd7, 0x00, 0xff)),
    ("Lumen", "Static Tree", Color(0x00, 0xd2, 0xd3, 0xff)),
];

const IDS: [&str; 10] = ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9"];

fn stylesheet() -> Vec<Rule> {
    let mut rules = vec![
        Rule::new(Selector::Kind("Screen"), Patch::new().background(PAGE)),
        // The bar: a strip at the top of the shared rect. `Glass`'s own
        // default is FILL width and Shrink height, so this pins the
        // height and the padding and nothing else.
        Rule::new(
            Selector::Id("bar"),
            Patch::new()
                .height(Length::Fixed(64.0))
                .padding(Edges { left: 20.0, right: 14.0, top: 16.0, bottom: 16.0 }),
        ),
        Rule::new(
            Selector::Id("heading"),
            Patch::new().color(INK).font_size(19.0).font_weight(700.0),
        ),
        Rule::new(Selector::Id("gap"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("sigma"), Patch::new().color(INK).font_size(13.0)),
        // The list starts **below** the bar, so the first card is not
        // born under the frost. Everything after it scrolls up into it.
        Rule::new(
            Selector::Id("content"),
            Patch::new().padding(Edges { left: 20.0, right: 20.0, top: 78.0, bottom: 20.0 }),
        ),
        // Dark ink: these cards are bright.
        Rule::new(Selector::Id("title"), Patch::new().color(CARD_INK).font_size(17.0).font_weight(700.0)),
        Rule::new(Selector::Id("sub"), Patch::new().color(CARD_SUB).font_size(12.0)),
        Rule::new(Selector::Id("less"), btn()),
        Rule::new(Selector::Id("more"), btn()),
        // The dock: a rounded pill, sized to its tabs, floated to the
        // bottom centre by three FILL spacers.
        Rule::new(Selector::Id("push"), Patch::new().height(Length::FILL)),
        Rule::new(Selector::Id("dockl"), Patch::new().width(Length::FILL)),
        Rule::new(Selector::Id("dockr"), Patch::new().width(Length::FILL)),
        // **Both of these, and the second is the one that catches
        // people.** A derived container defaults to `FILL` width, so
        // `Tabs` would expand to whatever box it was given and the
        // `Shrink` panel around it would then measure *that* — a
        // full-width pill with `Shrink` written on it. A `Shrink`
        // container needs `Shrink` content.
        Rule::new(Selector::Kind("Tabs"), Patch::new().width(Length::Shrink)),
        Rule::new(
            Selector::Id("dock"),
            Patch::new()
                .width(Length::Shrink)
                .radius(26.0)
                .padding(Edges { left: 26.0, right: 26.0, top: 15.0, bottom: 15.0 }),
        ),
        Rule::new(
            Selector::Kind("Floating"),
            Patch::new().padding(Edges { left: 0.0, right: 0.0, top: 0.0, bottom: 26.0 }),
        ),
        Rule::new(Selector::Id("t0"), Patch::new().color(INK).font_size(14.0).font_weight(600.0)),
        Rule::new(Selector::Id("t1"), Patch::new().color(INK).font_size(14.0).font_weight(600.0)),
        Rule::new(Selector::Id("t2"), Patch::new().color(INK).font_size(14.0).font_weight(600.0)),
    ];
    for (id, (_, _, colour)) in IDS.iter().zip(CARDS.iter()) {
        rules.push(Rule::new(
            Selector::Id(id),
            Patch::new()
                .background(*colour)
                .radius(14.0)
                .width(Length::FILL)
                .padding(Edges { left: 16.0, right: 16.0, top: 14.0, bottom: 14.0 }),
        ));
    }
    rules
}

fn btn() -> Patch {
    Patch::new()
        .background(GHOST)
        .color(INK)
        .radius(8.0)
        .font_size(15.0)
        .padding(Edges { left: 11.0, right: 11.0, top: 4.0, bottom: 4.0 })
}

/// `--shot <file.ppm> [scroll]` — one frame, headless.
///
/// Not a feature of the demo so much as a condition of shipping it: an
/// example nobody has looked at is a screenshot nobody has taken. The
/// `scroll` argument moves the list under the panel, which is the state
/// the whole example exists to show.
fn shot(path: &str, scroll: f32, sigma: f32) {
    use lumen_static_shell::software::Canvas;
    use lumen_static_shell::text::{FontMetrics, FontStack};
    use lumen_static_shell::Host;

    let (w, h) = (460u32, 620u32);
    let mut host: Host<Demo> = Host::new(build(sigma), w, h);
    host.set_stylesheet(&stylesheet());
    host.app.ui.content.scroll = scroll;
    let mut canvas = match FontStack::system() {
        Some(f) => Canvas::with_font(w, h, f),
        None => Canvas::new(w, h),
    };
    let mut metrics = FontStack::system().map(FontMetrics::new);
    match metrics.as_mut() {
        Some(m) => {
            host.pump_with(m);
        }
        None => {
            host.pump();
        }
    }
    // Twice: `ScrollView` clamps its offset during layout, so the first
    // pump is what teaches it how far the content actually goes.
    match metrics.as_mut() {
        Some(m) => {
            host.pump_with(m);
        }
        None => {
            host.pump();
        }
    }
    canvas.clear(PAGE);
    host.render(&mut canvas);
    std::fs::write(path, canvas.to_ppm()).expect("write frame");
}

/// The application, built once and shared by the window and `--shot`.
fn build(sigma: f32) -> Demo {
    let mut app = Demo { sigma: sigma as i64, ..Demo::default() };
    let cards = &mut app.ui.content.content;
    let each: [&mut Card; 10] = [
        &mut cards.c0, &mut cards.c1, &mut cards.c2, &mut cards.c3, &mut cards.c4,
        &mut cards.c5, &mut cards.c6, &mut cards.c7, &mut cards.c8, &mut cards.c9,
    ];
    for (card, (title, sub, _)) in each.into_iter().zip(CARDS.iter()) {
        card.title.set(title.to_string());
        card.sub.set(sub.to_string());
    }
    app.ui.bar.content.heading.set("Library");
    app.ui.bar.content.less.label = "-".into();
    app.ui.bar.content.more.label = "+".into();
    app.ui.bar.content.less.on_click = |d: &mut Demo, _: &EventCtx| {
        d.sigma = (d.sigma - 4).max(0);
        d.ui.bar.material.blur = d.sigma as f32;
    };
    app.ui.bar.content.more.on_click = |d: &mut Demo, _: &EventCtx| {
        d.sigma = (d.sigma + 4).min(40);
        d.ui.bar.material.blur = d.sigma as f32;
    };

    // The material.
    //
    // Dark rather than white, and that is a legibility decision rather
    // than a stylistic one. The cards behind are saturated and bright;
    // a white tint over them gives pastel, and white text on pastel is
    // unreadable. Tinting toward the page colour keeps the backdrop
    // visible *and* keeps the bar's own text at contrast — which is the
    // trade every real glass surface makes.
    // Frosted: a blur and a tint, no bevel.
    app.ui.bar.material.blur = sigma;
    app.ui.bar.material.tint = TINT;

    // Liquid: **barely any blur**. The glassiness is the bevel -- an
    // edge that displaces what is behind it and catches the light --
    // which is why this one is nearly see-through and still reads as
    // glass. Everything here is `Material::LIQUID` except the tint,
    // which is darkened for the same legibility reason the bar's is.
    app.ui.dock.row.dock = Glass::with(
        Tabs::default(),
        Material { tint: Color(0x0b, 0x0e, 0x14, 0x3d), ..Material::LIQUID },
    );
    app.ui.dock.row.dock.content.t0.set("Home");
    app.ui.dock.row.dock.content.t1.set("Search");
    app.ui.dock.row.dock.content.t2.set("Library");
    app
}

fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag == "--shot" {
            let path = args.next().unwrap_or_else(|| "glass.ppm".into());
            let scroll = args.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let sigma = args.next().and_then(|s| s.parse().ok()).unwrap_or(14.0);
            shot(&path, scroll, sigma);
            return;
        }
    }
    run(
        build(14.0),
        Config {
            title: "glass",
            width: 460,
            height: 620,
            page: PAGE,
            stylesheet: stylesheet(),
            ..Config::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumen_static::event::{Event, PointerButton};
    use lumen_static_shell::text::{FontMetrics, FontStack};
    use lumen_static_shell::Host;

    const W: u32 = 460;
    const H: u32 = 620;

    fn driven() -> Host<Demo> {
        let mut host: Host<Demo> = Host::new(build(14.0), W, H);
        host.set_stylesheet(&stylesheet());
        let mut metrics = FontStack::system().map(FontMetrics::new);
        for _ in 0..2 {
            match metrics.as_mut() {
                Some(m) => {
                    host.pump_with(m);
                }
                None => {
                    host.pump();
                }
            }
        }
        host
    }

    fn wid_at(host: &Host<Demo>, x: f32, y: f32) -> &'static str {
        lumen_static::pass::hit_test(&host.app.ui, &host.states, x, y)
            .map(|h| Screen::SUB_TABLE[h.node as usize].wid)
            .unwrap_or("<none>")
    }

    /// **The regression this file exists for.**
    ///
    /// The dock is a full-height layer over everything, and a `stack`
    /// hits its children in reverse — so before `Spacer` stopped
    /// claiming hits and containers stopped claiming their transparent
    /// gaps, node `push` (an invisible `FILL` spacer) answered *every*
    /// point in the window. The buttons did nothing and the list would
    /// not scroll.
    #[test]
    fn the_dock_layer_does_not_swallow_the_window() {
        let host = driven();
        // **Positive assertions.** `!= "push"` was the first version and
        // it was too weak: with only half the fix the hit landed on the
        // layer's *container* instead of its spacer, which is a
        // different wrong answer that still swallows the frame.
        let on_list = wid_at(&host, 230.0, 300.0);
        assert!(
            on_list.starts_with('c') || on_list == "title" || on_list == "sub",
            "a point over the list hit {on_list:?}, not a card"
        );
        assert_eq!(wid_at(&host, 430.0, 30.0), "more", "a point over the + button hit something else");
        // And the dock itself is still hittable, or this fixed nothing.
        let dock = wid_at(&host, 230.0, 568.0);
        assert!(
            matches!(dock, "t0" | "t1" | "t2" | "dock"),
            "the dock is no longer reachable: {dock}"
        );
    }

    /// The wheel reaches the scroller, which is what the whole example
    /// is for: the blur only changes because the backdrop moves.
    #[test]
    fn the_wheel_scrolls_the_list() {
        let mut host = driven();
        assert_eq!(host.app.ui.content.scroll, 0.0);
        host.post(Event::Scroll { x: 230.0, y: 300.0, dy: -60.0 });
        host.pump();
        let after = host.app.ui.content.scroll;
        println!("scroll after one wheel notch: {after}");
        assert!(after > 0.0, "the wheel did not reach the scroller");
    }

    /// And the toolbar buttons run their handlers.
    #[test]
    fn the_blur_buttons_work() {
        let mut host = driven();
        let before = host.app.sigma;
        let (bx, by) = {
            let i = Screen::SUB_TABLE.iter().position(|d| d.wid == "more").expect("the + button");
            let st = &host.states[i];
            (st.pos.x + st.size.w / 2.0, st.pos.y + st.size.h / 2.0)
        };
        host.post(Event::PointerDown { x: bx, y: by, button: PointerButton::Left, clicks: 1 });
        host.post(Event::PointerUp { x: bx, y: by, button: PointerButton::Left });
        host.pump();
        println!("sigma {before} -> {}", host.app.sigma);
        assert!(host.app.sigma > before, "the + button did nothing");
        assert_eq!(host.app.ui.bar.material.blur, host.app.sigma as f32, "and the panel followed it");
    }
}
