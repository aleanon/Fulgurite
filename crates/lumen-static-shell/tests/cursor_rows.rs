//! **The cursor over a repeater's row** — M44/T44.8, closing R15-F7
//! (`docs/plan-input-semantics.md` §1.5).
//!
//! `Host::cursor` walked only the main table's hovered bits, so a
//! `Button` inside a `Repeat` row — a menu's entries, a list's actions —
//! read as the repeater's own cursor, `Default`. That is the exact case
//! `docs/plan-desktop.md` T14.6 names. Three places to hover: a table
//! `Button`, a `Button` row of a repeater, and a `Label` row of another.
//! Each asserts the probe's hit first (printed, with `inner.row`), so a
//! wrong answer cannot come from a pointer that missed.

use lumen_static::prelude::*;
use lumen_static::repeat::Repeat;
use lumen_static::style::{Cursor, Length, Patch, Rule, Selector};
use lumen_static::Event;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

fn button_row(b: &mut Button<App>, _: usize) {
    b.label = "row";
}

fn label_row(l: &mut Label, _: usize) {
    l.set("text");
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("btn")]
    btn: Button<App>,
    #[widget_id("buttons")]
    buttons: Repeat<Button<App>>,
    #[widget_id("labels")]
    labels: Repeat<Label>,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
}

fn host() -> Host<App> {
    let mut app = App::default();
    app.ui.btn.label = "table";
    app.ui.buttons = Repeat::new(3, 30.0, button_row);
    app.ui.labels = Repeat::new(3, 30.0, label_row);
    let mut h: Host<App> = Host::new(app, 300, 300);
    h.set_stylesheet(&[
        Rule::new(Selector::Id("btn"), Patch::new().width(Length::Fixed(80.0)).height(Length::Fixed(30.0))),
        Rule::new(Selector::Id("buttons"), Patch::new().width(Length::Fixed(120.0)).height(Length::Fixed(90.0))),
        Rule::new(Selector::Id("labels"), Patch::new().width(Length::Fixed(120.0)).height(Length::Fixed(90.0))),
    ]);
    h.pump();
    h.pump();
    h
}

fn index(wid: &str) -> u32 {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid) as u32
}

/// Move the pointer to `(x, y)`, check what the probe hits, and read the
/// cursor.
fn hover(h: &mut Host<App>, what: &str, (x, y): (f32, f32), node: u32, row: Option<u64>) -> Cursor {
    let hit = h.probe(x, y).unwrap_or_else(|| panic!("setup: {what}: the probe hit nothing at ({x}, {y})"));
    println!("{what}: probe at ({x}, {y}) -> node {} inner {:?}", hit.node, hit.inner);
    assert_eq!(hit.node, node, "setup: {what}: the probe hit another node");
    assert_eq!(hit.inner.map(|i| i.row), row, "setup: {what}: the probe hit another row");
    h.post(Event::PointerMove { x, y });
    h.pump();
    h.cursor()
}

fn centre(s: NodeState) -> (f32, f32) {
    (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
}

#[test]
#[ignore = "red until T44.8: R15-F7 (Host::cursor reads only the main table, so a row's Button reads Default)"]
fn the_cursor_over_a_row_is_the_rows() {
    let mut h = host();
    let btn = centre(h.states[index("btn") as usize]);
    let row = centre(h.app.ui.buttons.row_state(1));
    let text = centre(h.app.ui.labels.row_state(1));

    let table = hover(&mut h, "table button", btn, index("btn"), None);
    let in_row = hover(&mut h, "button row 1", row, index("buttons"), Some(1));
    let label = hover(&mut h, "label row 1", text, index("labels"), Some(1));
    println!("table={table:?} row={in_row:?} label-row={label:?}  (expected table=Pointer row=Pointer label-row=Default)");
    assert_eq!(table, Cursor::Pointer, "the table button's cursor");
    assert_eq!(label, Cursor::Default, "a label row reads as a button");
    assert_eq!(in_row, Cursor::Pointer, "a button in a repeater row reads as its repeater, not as a button");
}

/// The accept direction, green today and after: a table button reads
/// `Pointer` before and after the pointer has been over a row.
#[test]
fn the_table_button_reads_pointer_before_and_after_a_row() {
    let mut h = host();
    let btn = centre(h.states[index("btn") as usize]);
    let text = centre(h.app.ui.labels.row_state(0));
    let before = hover(&mut h, "table button", btn, index("btn"), None);
    let _ = hover(&mut h, "label row 0", text, index("labels"), Some(0));
    let after = hover(&mut h, "table button again", btn, index("btn"), None);
    println!("table before={before:?} after={after:?} (expected Pointer, Pointer)");
    assert_eq!((before, after), (Cursor::Pointer, Cursor::Pointer));
}
