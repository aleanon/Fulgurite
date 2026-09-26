//! **Rows a keyboard can choose, in the widgets that own their rows** —
//! M44/T44.10 step 4 (`docs/plan-input-semantics.md`), closing the rest
//! of R3-F6.
//!
//! `Select` is a `Repeat` of the application's buttons and goes through
//! the pass's active row (`crates/lumen-static/tests/select.rs`). The
//! choosers, pickers, `Tree` and `DataGrid` draw and hit their rows
//! themselves, so each answers the keyboard itself. Every case runs
//! through `pass::dispatch_overlaid` with the overlays the frame painted,
//! so focus, bubbling and the overlay rules are the real ones; each prints
//! what it chose beside what it should have.

use lumen_static::event::InputState;
use lumen_static::overlay::Overlays;
use lumen_static::widget::Widget;
use lumen_static::{pass, Constraints, DisplayList, Event, Key, NodeState, StaticTree};
use lumen_static_widgets::{Combobox, Menu, PickList};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 500.0, min_h: 0.0, max_h: 500.0 };
const OPTS: [&str; 4] = ["Apple", "Banana", "Cherry", "apricot"];

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Ui {
    #[widget_id("pl")]
    pl: PickList,
    #[widget_id("cb")]
    cb: Combobox,
    #[widget_id("mn")]
    mn: Menu,
}

#[derive(lumen_static::App, Default)]
struct App {
    #[tree]
    ui: Ui,
}

struct F {
    app: App,
    sty: Vec<lumen_static::Style>,
    st: Vec<NodeState>,
    input: InputState,
    ov: Overlays<8>,
}

fn wid(w: &str) -> u32 {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, w).expect(w) as u32
}

impl F {
    fn new() -> F {
        let mut app = App::default();
        app.ui.pl = PickList::new(&OPTS);
        app.ui.cb = Combobox::new(&OPTS);
        app.ui.mn = Menu::new(&OPTS);
        let sty = pass::default_styles(&app.ui);
        let mut f = F { app, sty, st: vec![NodeState::default(); Ui::NODE_COUNT], input: InputState::default(), ov: Overlays::default() };
        f.frame();
        f
    }
    fn frame(&mut self) {
        pass::layout(&mut self.app.ui, &self.sty, &mut self.st, VIEW);
        let mut list = DisplayList::new();
        self.ov = Overlays::default();
        pass::paint_overlaid(&self.app.ui, &self.st, &mut list, &mut self.ov);
    }
    fn send(&mut self, ev: Event) -> bool {
        let h = pass::dispatch_overlaid(&mut self.app, &self.sty, &mut self.st, &mut self.input, &self.ov, &ev);
        self.frame();
        h
    }
    fn key(&mut self, k: Key) -> bool {
        self.send(Event::Key { key: k, down: true })
    }
    fn click(&mut self, i: u32) {
        let s = self.st[i as usize];
        let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
        self.send(Event::down(x, y));
        self.send(Event::up(x, y));
    }
}

/// A picklist's trigger holds focus; Down and Enter bubble to the list.
#[test]
fn a_picklist_is_chosen_from_the_keyboard() {
    let mut f = F::new();
    let trigger = wid("pl") + 1;
    f.click(trigger);
    assert!(f.app.ui.pl.is_open(), "setup: focusing the trigger did not open the list");
    assert_eq!(f.input.focus, Some(trigger), "setup: the trigger does not hold focus");
    let handled = [f.key(Key::Down), f.key(Key::Down), f.key(Key::Down), f.key(Key::Enter)];
    println!(
        "picklist: Down x3, Enter handled {handled:?}; selected {:?} (expected Some(2), Cherry), trigger {:?}, open {}",
        f.app.ui.pl.selected(),
        f.app.ui.pl.text(),
        f.app.ui.pl.is_open()
    );
    assert_eq!(f.app.ui.pl.selected(), Some(2), "the keyboard did not choose the third row");
    assert!(!f.app.ui.pl.is_open(), "choosing did not close the list");
}

/// A filtered combobox walks the rows you can see, not the raw indices.
#[test]
fn a_filtered_combobox_walks_the_visible_rows() {
    let mut f = F::new();
    let trigger = wid("cb") + 1;
    f.click(trigger);
    // Replace the selection with a query: "ap" shows Apple and apricot.
    for _ in 0..8 {
        f.key(Key::Backspace);
    }
    for ch in "ap".chars() {
        f.send(Event::Text { ch });
    }
    assert!(f.app.ui.cb.is_query(), "setup: the trigger does not hold a query");
    f.key(Key::Down);
    f.key(Key::Down);
    f.key(Key::Enter);
    println!(
        "combobox 'ap', Down x2, Enter: selected {:?} (expected Some(3), apricot)",
        f.app.ui.cb.selected()
    );
    assert_eq!(f.app.ui.cb.selected(), Some(3), "the second visible row of 'ap' is apricot, index 3");
}

/// A menu has no trigger: its list takes focus when it opens, and the
/// keys land on the list itself.
#[test]
fn an_open_menu_takes_focus_and_the_keyboard_chooses() {
    let mut f = F::new();
    f.app.ui.mn.open();
    f.frame();
    // The first event after it appears is when the pass records it and
    // moves focus in (T44.9, T44.10).
    f.send(Event::Modifiers(lumen_static::Mods::NONE));
    let list = wid("mn") + 3;
    println!("open menu: focus {:?} (the list is {list})", f.input.focus);
    assert_eq!(f.input.focus, Some(list), "the open menu did not take focus");
    let handled = [f.key(Key::Down), f.key(Key::Down), f.key(Key::Enter)];
    println!("menu: Down x2, Enter handled {handled:?}; selected {:?} (expected Some(1))", f.app.ui.mn.selected());
    assert_eq!(f.app.ui.mn.selected(), Some(1), "the keyboard did not choose the second command");
}

// ---- pickers, trees and grids ------------------------------------------

use lumen_static::event::EventCtx;
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::{Branch, ColorPicker, DataGrid, DatePicker, Tree};

const TREE: [Branch; 5] = [
    Branch::new("src", 0),
    Branch::new("main.rs", 1),
    Branch::new("lib.rs", 1),
    Branch::new("tests", 0),
    Branch::new("it.rs", 1),
];

fn grid_row(b: &mut Button<Other>, i: usize) {
    b.label = "row";
    let _ = i;
    b.on_click = |o: &mut Other, cx: &EventCtx| o.chosen = cx.row;
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct OtherUi {
    #[widget_id("color")]
    color: ColorPicker,
    #[widget_id("date")]
    date: DatePicker,
    #[widget_id("tree")]
    tree: Tree,
    #[widget_id("grid")]
    grid: DataGrid<Button<Other>>,
}

#[derive(lumen_static::App, Default)]
struct Other {
    #[tree]
    ui: OtherUi,
    chosen: Option<u64>,
}

struct G {
    app: Other,
    sty: Vec<lumen_static::Style>,
    st: Vec<NodeState>,
    input: InputState,
    ov: Overlays<8>,
}

fn owid(w: &str) -> u32 {
    lumen_static::style::index_of_wid(<OtherUi as Widget>::SUB_TABLE, w).expect(w) as u32
}

impl G {
    fn new() -> G {
        let mut app = Other::default();
        app.ui.tree = Tree::new(&TREE);
        app.ui.grid = DataGrid::new(&["Name"], 40, 24.0, grid_row);
        let sheet = [lumen_static::style::Rule::new(
            lumen_static::style::Selector::Id("grid"),
            lumen_static::style::Patch::new().height(lumen_static::style::Length::Fixed(128.0)),
        )];
        let mut sty = pass::default_styles(&app.ui);
        lumen_static::style::cascade_deep(&mut app.ui, &sheet, &mut sty);
        let mut g = G { app, sty, st: vec![NodeState::default(); OtherUi::NODE_COUNT], input: InputState::default(), ov: Overlays::default() };
        g.frame();
        g
    }
    fn frame(&mut self) {
        let big = Constraints { min_w: 0.0, max_w: 600.0, min_h: 0.0, max_h: 1400.0 };
        pass::layout(&mut self.app.ui, &self.sty, &mut self.st, big);
        let mut list = DisplayList::new();
        self.ov = Overlays::default();
        pass::paint_overlaid(&self.app.ui, &self.st, &mut list, &mut self.ov);
    }
    fn key(&mut self, k: Key) -> bool {
        let h = pass::dispatch_overlaid(&mut self.app, &self.sty, &mut self.st, &mut self.input, &self.ov, &Event::Key { key: k, down: true });
        self.frame();
        h
    }
}

/// A colour picker: Enter opens, the first arrow lights the current
/// swatch, the next moves, Enter chooses and closes.
#[test]
fn a_colour_picker_is_chosen_from_the_keyboard() {
    let mut g = G::new();
    g.input.focus = Some(owid("color") + 1);
    g.key(Key::Enter);
    assert!(g.app.ui.color.is_open(), "setup: Enter did not open the picker");
    let handled = [g.key(Key::Right), g.key(Key::Right), g.key(Key::Down), g.key(Key::Enter)];
    let cols = g.app.ui.color.cells().cols();
    println!(
        "colour: Right Right Down Enter handled {handled:?}; colour {} (expected 1 + {cols} cols = {}), open {}",
        g.app.ui.color.cells().color,
        1 + cols,
        g.app.ui.color.is_open()
    );
    assert_eq!(g.app.ui.color.cells().color, 1 + cols, "the keyboard did not choose the swatch it walked to");
    assert!(!g.app.ui.color.is_open(), "choosing did not close the picker");
}

/// A date picker: Down is a week.
#[test]
fn a_date_picker_moves_by_weeks() {
    let mut g = G::new();
    g.input.focus = Some(owid("date") + 1);
    let day0 = g.app.ui.date.cells().day;
    g.key(Key::Down); // closed: Down opens
    assert!(g.app.ui.date.is_open(), "setup: Down did not open the picker");
    g.key(Key::Down); // lights the current day
    g.key(Key::Down); // a week on
    g.key(Key::Enter);
    println!("date: day {day0} -> {} (expected {})", g.app.ui.date.cells().day, day0 + 7);
    assert_eq!(g.app.ui.date.cells().day, day0 + 7, "Down did not move by a week");
}

/// A tree: Down walks the visible rows, Left folds a parent, and the
/// rows under it drop out of the walk.
#[test]
fn a_tree_is_walked_and_folded_from_the_keyboard() {
    let mut g = G::new();
    g.input.focus = Some(owid("tree"));
    assert!(g.key(Key::Down), "the first Down lit nothing");
    assert_eq!(g.app.ui.tree.hot(), Some(0));
    g.key(Key::Left); // fold "src"
    let folded = !g.app.ui.tree.is_open(0);
    g.key(Key::Down); // the next visible row is now "tests"
    println!(
        "tree: folded src {folded}; visible {} of 5 (expected 3); lit {:?} (expected Some(3), tests)",
        g.app.ui.tree.visible(),
        g.app.ui.tree.hot()
    );
    assert!(folded, "Left did not fold the open parent");
    assert_eq!(g.app.ui.tree.hot(), Some(3), "Down walked into a folded subtree");
    g.key(Key::Right); // unfold "tests"? it is open: step to its child
    assert_eq!(g.app.ui.tree.hot(), Some(4), "Right on an open parent did not step to its child");
    g.key(Key::Left); // on a leaf: to the parent
    assert_eq!(g.app.ui.tree.hot(), Some(3), "Left on a leaf did not step to its parent");
}

/// A data grid: its rows are a Tab stop; End goes to the last row and
/// scrolls it into view; Enter runs that row's handler with its index.
#[test]
fn a_data_grid_row_is_chosen_from_the_keyboard() {
    let mut g = G::new();
    let rows = owid("grid") + 1;
    // **Reached by Tab**, not by setting focus: a focus set by hand
    // proves the keys work on a node nothing can reach, which is what the
    // grid's rows were until they became a Tab stop — and the first
    // version of this test set it by hand and stayed green with the flag
    // removed.
    g.input.focus = Some(owid("tree"));
    g.key(Key::Tab);
    println!("grid: Tab from the tree -> focus {:?} (the rows are {rows})", g.input.focus);
    assert_eq!(g.input.focus, Some(rows), "Tab did not reach the grid's rows");
    g.key(Key::End);
    let scroll = g.app.ui.grid.rows.scroll;
    g.key(Key::Enter);
    println!(
        "grid: End then Enter -> chosen {:?} (expected Some(39)); scroll {scroll} (the last row in view)",
        g.app.chosen
    );
    assert_eq!(g.app.chosen, Some(39), "Enter did not run the last row's handler");
    assert!(scroll > 0.0, "End did not scroll the last row into view");
}
