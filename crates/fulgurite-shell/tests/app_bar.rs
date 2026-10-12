//! **An app bar cuts a long title with an ellipsis, and carries a
//! leading and an actions slot** — M57/B39 (`docs/plan-widget-fixes.md`).
//!
//! Its rustdoc said a long title "wraps or truncates"; it did neither and
//! drew past the bar, and the audit could not test the claim
//! (`docs/widget-audit.md`, AppBar). It had no slots.

use fulgurite::cx::TextMeasure;
use fulgurite::display::Cmd;
use fulgurite::{App as DeriveApp, Event, EventCtx, StaticTree, Widget};
use fulgurite_shell::Host;
use fulgurite_widgets::ops::Button;
use fulgurite_widgets::AppBar;

#[derive(StaticTree, Default)]
#[widget(column)]
struct Ui {
    #[widget_id("bar")]
    bar: AppBar<&'static str, Button<App>, Button<App>>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    backs: u32,
    saves: u32,
}

fn back(a: &mut App, _: &EventCtx) {
    a.backs += 1;
}
fn save(a: &mut App, _: &EventCtx) {
    a.saves += 1;
}

fn host(title: &'static str) -> Host<App> {
    let mut app = App::default();
    let mut b = Button::<App>::default();
    b.label = "Back";
    b.on_click = back;
    let mut s = Button::<App>::default();
    s.label = "Save";
    s.on_click = save;
    app.ui.bar = AppBar::with(title, b, s);
    let mut h = Host::new(app, 360, 120);
    h.pump();
    h
}

fn node(h: &Host<App>, k: usize) -> fulgurite::NodeState {
    let i = fulgurite::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "bar").unwrap();
    h.states[i + k]
}

#[test]
fn a_long_title_ends_in_an_ellipsis_inside_its_room() {
    let h = host("A title far longer than any app bar has room for on a phone");
    let (shown, cut) = h.app.ui.bar.shown_title();
    let (bar, lead, act) = (node(&h, 0), node(&h, 1), node(&h, 2));
    let room = (lead.pos.x + lead.size.w + 8.0, act.pos.x - 8.0);
    // The title's two runs, as painted: the prefix and the ellipsis.
    let runs: Vec<[f32; 4]> = h.list.cmds.iter().filter_map(|c| match c {
        Cmd::Text { rect, .. } if rect[0] >= room.0 - 0.5 && rect[0] < room.1 => Some(*rect),
        _ => None,
    }).collect();
    let mut m = fulgurite::cx::BlockMetrics;
    let end = runs.last().map(|r| r[0] + m.measure_weighted("…", 18.0, 600.0, f32::INFINITY).w).unwrap_or(0.0);
    println!("bar {}..{}, title room {:?}; shown {shown:?} cut {cut}; title ink ends at {end}", bar.pos.x, bar.pos.x + bar.size.w, room);
    assert!(cut, "a title far wider than the bar was not cut");
    assert!(!shown.ends_with(' '), "the cut leaves a space before the ellipsis");
    assert_eq!(runs.len(), 2, "the prefix and the ellipsis are not two runs in the title's room");
    assert!(end <= room.1 + 0.5, "the ellipsis ends at {end}, past the title's room at {}", room.1);
    // A cut beside a space must not leave "… …" with a gap: letters and
    // spaces alternate here so the fit lands next to one. The first title
    // above never cut at a space, so dropping the trim left it green.
    let spaced = host("a b c d e f g h i j k l m n o p q r s t u v w x y z a b c d e f g h");
    let (s2, _) = spaced.app.ui.bar.shown_title();
    let untrimmed = spaced.app.ui.bar.title.get(s2.len()..s2.len() + 1);
    println!("spaced title shown {s2:?}, next char {untrimmed:?}");
    assert!(!s2.ends_with(' '), "the cut leaves a space before the ellipsis");
    let short = host("Inbox");
    assert_eq!(short.app.ui.bar.shown_title(), ("Inbox", false), "a short title was cut");
}

#[test]
fn the_slots_sit_at_the_ends_and_take_their_clicks() {
    let mut h = host("Inbox");
    let (bar, lead, act) = (node(&h, 0), node(&h, 1), node(&h, 2));
    println!("bar {:?}+{:?}; leading at {}, actions end at {}", bar.pos, bar.size, lead.pos.x, act.pos.x + act.size.w);
    assert!((lead.pos.x - (bar.pos.x + 16.0)).abs() < 0.5, "the leading slot is not at the start");
    assert!((act.pos.x + act.size.w - (bar.pos.x + bar.size.w - 16.0)).abs() < 0.5, "the actions slot is not at the end");
    for (n, want) in [(lead, (1, 0)), (act, (1, 1))] {
        let (x, y) = (n.pos.x + n.size.w / 2.0, n.pos.y + n.size.h / 2.0);
        h.post(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
        h.pump();
        h.post(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
        h.pump();
        assert_eq!((h.app.backs, h.app.saves), want, "a slot's click did not reach its handler");
    }
}
