//! M27/T27.11: a stylesheet reaches a **repeated row**.
//!
//! It never had. A sheet is applied once, before the first frame; a
//! `Repeat`'s row template lives in an arena, and an arena belongs to a
//! widget the application may only build later — or, in the case that
//! found this, to a widget that exists from the start but whose arena
//! `set_stylesheet` had no way to reach because the cascade walked the
//! *table*. Every rule naming a node inside a repeated row was dead,
//! invisibly: the main table carries every other node, an enum's
//! inactive variants included, and the sheet lint runs after a pump and
//! therefore sees an arena the cascade never did.
//!
//! Found by rendering a `DataGrid` whose values did not line up with its
//! own header (`docs/plan-widget-parity.md` T27.11's review, finding 4).

use lumen_static::event::App;
use lumen_static_widgets::ops::Label;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{repeat::Repeat, StaticTree};
use lumen_static_shell::Host;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct Row {
    #[widget_id("row-cell")]
    cell: Label<String>,
}

fn bind(r: &mut Row, i: usize) {
    r.cell.set(alloc_string(i));
}

fn alloc_string(i: usize) -> String {
    format!("row {i}")
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("list")]
    list: Repeat<Row>,
}

struct Demo {
    ui: Ui,
}

impl App for Demo {
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
    type Msg = lumen_static::event::Never;
    fn apply(&mut self, m: Self::Msg) -> bool {
        match m {}
    }
    type Tree = Ui;
    fn tree(&self) -> &Ui {
        &self.ui
    }
    fn tree_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}

fn sheet() -> [Rule; 1] {
    [Rule::new(Selector::Id("row-cell"), Patch::new().width(Length::Fixed(120.0)))]
}

/// The path that already worked: the repeater exists when the sheet is
/// set, so `set_stylesheet`'s own deep cascade reaches its template.
///
/// Named for that and not for the bug: an injection that removes the
/// per-frame pass leaves this green, because nothing here is built
/// after the sheet. It is the control, and the test below is the claim.
#[test]
fn a_rule_reaches_an_arena_that_exists_when_the_sheet_is_set() {
    let app = Demo { ui: Ui { list: Repeat::new(20, 24.0, bind) } };
    let mut h: Host<Demo> = Host::new(app, 300, 200);
    h.set_stylesheet(&sheet());
    h.pump();

    // Node 1 of `Row` is `#row-cell`.
    let cell = h.app.ui.list.instance_state(0, 1);
    // A `Label` is `Shrink`, so without the rule the cell is as wide as
    // "row 0" and no wider — which is what every repeated row got until
    // this existed.
    assert!(
        (cell.size.w - 120.0).abs() < 0.5,
        "the row's cell is {:.1} px wide, not the 120 the sheet gives it — \
         the rule never reached the arena",
        cell.size.w
    );
}

/// And a rule that arrives **after** the widget does.
///
/// `set_stylesheet` cascades arenas itself; this is the other order — a
/// sheet already applied, and a change to the application that could
/// have built a new subtree. The flag the host keeps is what makes the
/// second case work, and an injection that only ever cascaded at
/// `set_stylesheet` leaves the first test green.
#[test]
fn a_row_built_after_the_sheet_is_still_reached() {
    let app = Demo { ui: Ui { list: Repeat::new(0, 24.0, bind) } };
    let mut h: Host<Demo> = Host::new(app, 300, 200);
    h.set_stylesheet(&sheet());
    h.pump();
    assert_eq!(h.app.ui.list.window_len(), 0, "setup: the list starts empty");

    // The application grows the list — the shape of a demo being swapped
    // in, or a fetch arriving.
    h.app.ui.list = Repeat::new(20, 24.0, bind);
    h.invalidate_all();
    h.pump();
    h.pump();

    let cell = h.app.ui.list.instance_state(0, 1);
    assert!(
        (cell.size.w - 120.0).abs() < 0.5,
        "a row built after the sheet was applied is {:.1} px wide, not 120",
        cell.size.w
    );
}
