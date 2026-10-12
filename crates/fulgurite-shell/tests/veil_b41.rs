//! **Veils: Shift+Tab walks back inside a modal, a sheet rounds only its
//! top corners, a drawer comes from either side** — M57/B41–B43
//! (`docs/plan-widget-fixes.md`, `docs/widget-audit.md` Modal/Sheet/Drawer).

use fulgurite::display::Color;
use fulgurite::event::{Key, Mods};
use fulgurite::style::{Patch, Rule, Selector};
use fulgurite::{App as DeriveApp, Event, StaticTree, Widget};
use fulgurite_shell::software::Canvas;
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Button;
use fulgurite_widgets::{Drawer, Edge, Modal, Sheet};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Three {
    #[widget_id("b0")]
    a: Button<App>,
    #[widget_id("b1")]
    b: Button<App>,
    #[widget_id("b2")]
    c: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column)]
struct Panel {
    #[widget_id("go")]
    go: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(stack)]
struct Ui {
    #[widget_id("modal")]
    modal: Modal<Three>,
    #[widget_id("sheet")]
    sheet: Sheet<Panel>,
    #[widget_id("drawer")]
    drawer: Drawer<Panel>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

const PANEL: Color = Color(255, 255, 255, 255);

fn host(which: &str) -> Host<App> {
    let mut app = App::default();
    match which {
        "modal" => { app.ui.modal.open(); }
        "sheet" => { app.ui.sheet.open(); }
        _ => {
            app.ui.drawer.edge = Edge::Right;
            app.ui.drawer.open();
        }
    }
    let mut h: Host<App> = Host::new(app, 400, 300);
    h.set_stylesheet(&[Rule::new(Selector::Kind("Panel"), Patch::new().background(PANEL).radius(12.0))]);
    h.pump();
    h
}

fn at(id: &str) -> usize {
    fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, id).unwrap()
}

#[test]
fn shift_tab_walks_back_inside_a_modal() {
    let mut h = host("modal");
    h.input.focus = Some(at("b0") as u32);
    h.pump();
    let tab = |h: &mut Host<App>, back: bool| {
        if back {
            h.post(Event::Modifiers(Mods::SHIFT));
        }
        h.post(Event::Key { key: Key::Tab, down: true });
        h.post(Event::Key { key: Key::Tab, down: false });
        h.post(Event::Modifiers(Mods::NONE));
        h.pump();
        h.input.focus
    };
    let back = tab(&mut h, true);
    let fwd = tab(&mut h, false);
    println!("from b0: Shift+Tab -> {back:?} (b2 is {}), then Tab -> {fwd:?}", at("b2"));
    assert_eq!(back, Some(at("b2") as u32), "Shift+Tab did not wrap back to the last control");
    assert_eq!(fwd, Some(at("b0") as u32), "Tab did not come forward again");
}

fn render(h: &Host<App>) -> Canvas {
    let mut c = Canvas::new(400, 300);
    c.clear(Color(0, 0, 0, 255));
    h.render(&mut c);
    c
}

fn px(c: &Canvas, x: u32, y: u32) -> [u8; 3] {
    let p = ((y * 400 + x) * 4) as usize;
    [c.pixels[p], c.pixels[p + 1], c.pixels[p + 2]]
}

#[test]
fn a_sheet_rounds_only_its_top_corners() {
    let h = host("sheet");
    let c = render(&h);
    let p = h.states[at("sheet") + 1];
    let top = p.pos.y as u32;
    let (bottom_corner, top_corner) = (px(&c, 1, 298), px(&c, 1, top + 1));
    println!("panel top {top}; bottom-left corner {bottom_corner:?}, top-left corner {top_corner:?}");
    assert_eq!(bottom_corner, [255, 255, 255], "the sheet's bottom corner is round (or missing)");
    assert_ne!(top_corner, [255, 255, 255], "the sheet's top corner is square");
}

#[test]
fn a_drawer_can_come_from_the_right() {
    let h = host("drawer");
    let c = render(&h);
    let p = h.states[at("drawer") + 1];
    println!("drawer panel {:?}+{:?}", p.pos, p.size);
    assert!(p.pos.x > 200.0, "a right-edge drawer is on the left");
    // Its outer (right) corners are square against the window; its inner
    // (left) ones round.
    assert_eq!(px(&c, 398, 1), [255, 255, 255], "the drawer's outer corner is round");
    assert_ne!(px(&c, p.pos.x as u32 + 1, 1), [255, 255, 255], "the drawer's inner corner is square");
}
