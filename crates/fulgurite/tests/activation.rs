//! **A click is a press and a release on the same item** — M44/T44.2
//! (`docs/plan-input-semantics.md` §2.2), closing R3-F3.
//!
//! Activation runs on `PointerUp` at the release's hit, and nothing
//! compared that hit with the press. So dragging off one button and
//! releasing on another activated the second; pressing one row and
//! releasing on another picked the second; and when the list changed
//! shape between press and release, the handler received the index now
//! under the pointer, which is a different item.
//!
//! Every case asserts that the press hit the node it meant to, and prints
//! the press's `(node, row, generation)` beside the release's — the
//! generation being what the hit node answers to
//! `Node::row_generation` — for the list, its `generation` plus a bump
//! per change of `count` — which T44.2's latch compares.

use fulgurite::event::App as _;
use fulgurite::widget::Node as _;
use fulgurite::repeat::Repeat;
use fulgurite::{pass, Constraints, Event, EventCtx, NodeState, StaticTree, Widget};
use fulgurite_widgets::ops::{Button, Fixed};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 200.0, min_h: 0.0, max_h: 300.0 };
const ROW_H: f32 = 20.0;

#[derive(StaticTree, Default)]
#[widget(row, spacing = 0.0)]
struct RowT {
    icon: Fixed,
    pick: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("save")]
    save: Button<App>,
    #[widget_id("other")]
    other: Button<App>,
    #[widget_id("list")]
    list: Repeat<RowT>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    /// The application's data: one item per row, bound by index.
    items: Vec<u64>,
    saved: u32,
    othered: u32,
    /// The item each pick landed on, looked up from the row it reported.
    picked: Vec<u64>,
}

impl fulgurite::event::App for App {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    type Msg = fulgurite::event::Never;
    fn apply(_: &mut Self, m: Self::Msg) -> bool {
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

fn on_pick(app: &mut App, ctx: &EventCtx) {
    let row = ctx.row.expect("a row index arrives from hit testing") as usize;
    app.picked.push(app.items[row]);
}

fn bind_row(r: &mut RowT, _row: usize) {
    r.icon.w = 10.0;
    r.icon.h = 10.0;
    r.pick.label = "p";
    r.pick.on_click = on_pick;
}

const SAVE: usize = 1;
const OTHER: usize = 2;
const LIST: usize = 3;

struct F {
    app: App,
    st: [NodeState; Ui::NODE_COUNT],
    styles: Vec<fulgurite::Style>,
    input: fulgurite::event::InputState,
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.save.label = "save";
        app.ui.save.on_click = |a: &mut App, _: &EventCtx| a.saved += 1;
        app.ui.other.label = "other";
        app.ui.other.on_click = |a: &mut App, _: &EventCtx| a.othered += 1;
        app.items = (100..120).collect();
        app.ui.list = Repeat::new(app.items.len(), ROW_H, bind_row);
        let styles = pass::default_styles(&app.ui);
        let mut f = F { app, st: [NodeState::default(); Ui::NODE_COUNT], styles, input: Default::default() };
        f.layout();
        f
    }

    fn layout(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.st, VIEW);
    }

    fn send(&mut self, ev: Event) -> bool {
        let h = pass::dispatch(&mut self.app, &self.styles, &mut self.st, &mut self.input, &ev);
        self.layout();
        h
    }

    fn centre(&self, i: usize) -> (f32, f32) {
        let s = &self.st[i];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    }

    /// A point on row `k`'s pick button, found by asking the hit test
    /// rather than by arithmetic, and asserted.
    fn row_point(&self, k: u64) -> (f32, f32) {
        let l = &self.st[LIST];
        let y = l.pos.y + k as f32 * ROW_H + ROW_H / 2.0;
        // Right of the 10 px icon, on the button.
        let x = l.pos.x + 14.0;
        let h = pass::hit_test(&self.app.ui, &self.st, x, y).expect("setup: nothing at the row's point");
        assert_eq!(h.node as usize, LIST, "setup: the row's point did not hit the list");
        assert_eq!(h.inner.map(|i| i.row), Some(k), "setup: the row's point hit another row");
        (x, y)
    }

    /// What T44.2's latch holds for a hit at `(x, y)`: the table node,
    /// its row, and the list's generation.
    fn latch_at(&self, x: f32, y: f32) -> (u32, Option<u64>, u64) {
        let h = pass::hit_test(&self.app.ui, &self.st, x, y).expect("a hit");
        let generation = self.app.tree().node(h.node as usize).row_generation();
        (h.node, h.inner.map(|i| i.row), generation)
    }
}

#[test]
fn press_one_button_release_on_another_fires_nothing() {
    let mut f = F::new();
    let (px, py) = f.centre(SAVE);
    let (rx, ry) = f.centre(OTHER);
    let press = f.latch_at(px, py);
    assert_eq!(press.0 as usize, SAVE, "setup: the press did not hit #save");
    f.send(Event::down(px, py));
    assert_eq!(f.input.pressed, Some(SAVE as u32), "setup: #save is not pressed");
    let release = f.latch_at(rx, ry);
    f.send(Event::up(rx, ry));
    println!(
        "press #save, release #other: press {press:?} release {release:?}; saved {} othered {} (expected 0 0)",
        f.app.saved, f.app.othered
    );
    assert_eq!((f.app.saved, f.app.othered), (0, 0), "a drag from one button to another activated one");
    assert!(f.st.iter().all(|n| !n.pressed), "the release left a press shade");
}

#[test]
fn press_one_row_release_on_another_picks_nothing() {
    let mut f = F::new();
    let (px, py) = f.row_point(1);
    let (rx, ry) = f.row_point(7);
    let press = f.latch_at(px, py);
    f.send(Event::down(px, py));
    let release = f.latch_at(rx, ry);
    f.send(Event::up(rx, ry));
    println!("press row 1, release row 7: press {press:?} release {release:?}; picked {:?} (expected [])", f.app.picked);
    assert!(f.app.picked.is_empty(), "a drag from row 1 to row 7 picked an item");
}

#[test]
fn a_row_removed_between_press_and_release_picks_nothing() {
    let mut f = F::new();
    let (x, y) = f.row_point(5);
    let pressed_item = f.app.items[5];
    let press = f.latch_at(x, y);
    f.send(Event::down(x, y));

    // Remove row 4: every row below it moves up one, so the pointer,
    // which has not moved, is now over the item that was row 6.
    f.app.items.remove(4);
    f.app.ui.list.count = f.app.items.len();
    f.layout();
    let release = f.latch_at(x, y);
    assert_eq!(release.1, Some(5), "setup: the release is not over row 5");
    assert_ne!(f.app.items[5], pressed_item, "setup: the item under the pointer did not change");

    f.send(Event::up(x, y));
    println!(
        "press row 5 (item {pressed_item}), remove row 4, release in place: press {press:?} release {release:?}; \
         picked {:?} (expected [])",
        f.app.picked
    );
    assert!(f.app.picked.is_empty(), "the release picked the item that moved under the pointer");
}

/// **Accepts good input**: one press and release on one item activates
/// it exactly once — a button and a row.
#[test]
fn a_press_and_release_on_one_item_activates_it_once() {
    let mut f = F::new();
    let (x, y) = f.centre(SAVE);
    assert_eq!(f.latch_at(x, y).0 as usize, SAVE, "setup: the press did not hit #save");
    f.send(Event::down(x, y));
    let handled = f.send(Event::up(x, y));
    println!("click #save: handled {handled}, saved {} (expected 1)", f.app.saved);
    assert!(handled, "the click was not handled");
    assert_eq!(f.app.saved, 1, "a click on #save did not activate it exactly once");

    let (x, y) = f.row_point(3);
    f.send(Event::down(x, y));
    f.send(Event::up(x, y));
    println!("click row 3: picked {:?} (expected [{}])", f.app.picked, f.app.items[3]);
    assert_eq!(f.app.picked, vec![f.app.items[3]], "a click on row 3 did not pick it exactly once");
}
