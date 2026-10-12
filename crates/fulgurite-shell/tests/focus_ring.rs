//! **The focus ring, at the pixel, on the controls it used to swallow** —
//! M57/A6 (`docs/plan-widget-fixes.md`).
//!
//! The ring was a filled rect underneath each widget, in `s.fg`, relying
//! on the widget to paint an opaque body over its middle. The audit
//! (`docs/widget-audit.md` X4) found four controls where that failed:
//! a `Tabs` strip (no body: a solid block), a `Slider` (a thin track: a
//! solid bar, knob gone), a primary button (`fg` white: white on white),
//! a checked box (its own blue, no gap).
//!
//! For each, under both palettes, keyboard-focused against unfocused:
//! - the **interior** is unchanged — the ring fills nothing;
//! - the **gap** just outside the control is unchanged — the ring does
//!   not touch it, so a ring the body's colour still reads as a ring;
//! - the **ring** differs from what was there by ≥ 3:1 (WCAG 1.4.11).

use fulgurite::display::Color;
use fulgurite::event::Key;
use fulgurite::style::{Palette, Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Event, StaticTree};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::Host;
use fulgurite_widgets::{Button, CheckBox, Slider, Tabs};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 24.0)]
struct Controls {
    #[widget_id("primary")]
    primary: Button<App>,
    #[widget_id("slider")]
    slider: Slider,
    #[widget_id("check")]
    check: CheckBox<App>,
    #[widget_id("tabs")]
    tabs: Tabs,
}

/// Padded, so every control has page around it for the ring to land on
/// — the first run sampled x = −1.5, which is column 0 inside the button.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("pad")]
    c: Controls,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const W: u32 = 360;
const H: u32 = 320;

fn sheet() -> Vec<Rule> {
    vec![
        Rule::new(Selector::Id("pad"), Patch::new().padding(fulgurite::style::Edges::all(40.0))),
        // A primary button: accent body, white label — the case where
        // `fg` was white and the ring was white on a white page.
        Rule::new(
            Selector::Id("primary"),
            Patch::new().background(Color(47, 107, 255, 255)).color(Color(255, 255, 255, 255)),
        ),
        Rule::new(Selector::Id("slider"), Patch::new().width(fulgurite::style::Length::Fixed(200.0))),
        Rule::new(Selector::Id("tabs"), Patch::new().width(fulgurite::style::Length::Fixed(260.0))),
    ]
}

fn host(palette: Palette) -> Host<App> {
    let mut app = App::default();
    app.ui.c.primary.label = "Primary";
    app.ui.c.check.on = true;
    app.ui.c.tabs = Tabs::new(&["One", "Two", "Three"]);
    let mut h = Host::new(app, W, H);
    h.set_stylesheet(&sheet());
    h.set_palette(palette);
    h.pump();
    h
}

fn render(h: &Host<App>, page: Color) -> Canvas {
    let mut c = Canvas::new(W, H);
    c.clear(page);
    h.render(&mut c);
    c
}

fn px(c: &Canvas, x: f32, y: f32) -> Color {
    let i = ((y as u32 * c.width + x as u32) * 4) as usize;
    Color(c.pixels[i], c.pixels[i + 1], c.pixels[i + 2], 255)
}

fn contrast(a: Color, b: Color) -> f32 {
    fn lum(c: Color) -> f32 {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
    }
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn index(wid: &str) -> usize {
    fulgurite::style::index_of_wid(<Ui as fulgurite::Widget>::SUB_TABLE, wid).unwrap()
}

#[test]
fn the_ring_marks_every_control_without_covering_it() {
    for (name, palette) in [("light", Palette::LIGHT), ("dark", Palette::DARK)] {
        let mut h = host(palette);
        let before = render(&h, palette.page);
        let mut failures = Vec::new();
        for wid in ["primary", "slider", "check", "tabs"] {
            let i = index(wid);
            // Tab until focus reaches it: keyboard focus, so the ring shows.
            for _ in 0..8 {
                if h.input.focus == Some(i as u32) {
                    break;
                }
                h.post(Event::Key { key: Key::Tab, down: true });
                h.pump();
            }
            assert_eq!(h.input.focus, Some(i as u32), "setup: Tab never reached {wid}");
            let after = render(&h, palette.page);
            let s = h.states[i];
            let w = s.focus_ring as f32;
            let gap = fulgurite::ops::FOCUS_GAP;
            let cy = s.pos.y + s.size.h / 2.0;
            // Sample on the left edge, at the middle of the box's height,
            // well clear of the rounded corners.
            let inside = (s.pos.x + s.size.w / 2.0, cy);
            let gap_x = s.pos.x - gap / 2.0 - 0.5;
            let ring_x = s.pos.x - gap - w / 2.0 - 0.5;
            let (bi, ai) = (px(&before, inside.0, inside.1), px(&after, inside.0, inside.1));
            let (bg, ag) = (px(&before, gap_x, cy), px(&after, gap_x, cy));
            let (br, ar) = (px(&before, ring_x, cy), px(&after, ring_x, cy));
            let ratio = contrast(br, ar);
            println!(
                "{name} {wid}: interior {bi:?}->{ai:?}, gap {bg:?}->{ag:?}, ring {br:?}->{ar:?} = {ratio:.2}:1 (bound 3.00)"
            );
            if bi != ai {
                failures.push(format!("{name} {wid}: the ring changed the control's interior {bi:?} -> {ai:?}"));
            }
            // Not exact: a control on a half pixel antialiases the ring's
            // inner edge a level or two into the gap (the slider measured
            // 255 -> 253). A ring *touching* the control is a full colour.
            if contrast(bg, ag) > 1.1 {
                failures.push(format!("{name} {wid}: the ring touches the control (gap {bg:?} -> {ag:?})"));
            }
            if ratio < 3.0 {
                failures.push(format!("{name} {wid}: ring {ar:?} over {br:?} is {ratio:.2}:1, under 3:1"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

/// The other direction: a pointer press focuses without a ring.
#[test]
fn a_pointer_press_shows_no_ring() {
    let mut h = host(Palette::LIGHT);
    let before = render(&h, Palette::LIGHT.page);
    let s = h.states[index("primary")];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    h.post(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
    h.pump();
    assert_eq!(h.input.focus, Some(index("primary") as u32), "setup: the press did not focus");
    let after = render(&h, Palette::LIGHT.page);
    let ring_x = s.pos.x - fulgurite::ops::FOCUS_GAP - 1.5;
    assert_eq!(px(&before, ring_x, y), px(&after, ring_x, y), "a pointer press drew a ring");
}
