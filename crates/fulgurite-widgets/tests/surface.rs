//! M27/T27.4 — `Card`, `Toast`, `Tooltip`.
//!
//! Four claims, and the first is the one this repository keeps paying
//! for when it is skipped:
//!
//! 1. **A handler inside a wrapper runs.** `Dyn` shipped without this
//!    and every control inside an island was dead — hit-testable,
//!    hoverable, pressable, and wired to nothing. Nothing that stopped
//!    at "it paints" could see it.
//! 2. **A card looks like a card with no stylesheet.** A widget whose
//!    `defaults` leave it transparent is a container with a different
//!    name, and a transparent control has nothing for `pressed_dim` to
//!    scale — a third of the showcase's "clicking does nothing".
//! 3. **A tooltip opens on a hover of its *content*.** `dispatch` marks
//!    the deepest node it hits, so a tip keyed on the wrapper's own
//!    `hovered` never appears.
//! 4. **A tooltip is not a hit target.** One that swallowed the pointer
//!    would close the instant it opened, because the content underneath
//!    would stop being hovered.

use fulgurite::display::{Cmd, DisplayList};
use fulgurite::event::{Event, EventCtx, Handlers, NodeSel};
use fulgurite_widgets::ops::Button;
use fulgurite::prelude::*;
use fulgurite::widget::Node;
use fulgurite::{pass, Constraints, NodeState, Point};
use fulgurite_widgets::{Card, Toast, Tooltip};

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("card")]
    card: Card<Button<App, &'static str>>,
    #[widget_id("tip")]
    tip: Tooltip<Button<App, &'static str>>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    clicks: u32,
}

impl fulgurite::event::App for App {
    // M33: this app keeps handlers on the whole of itself.
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // M28/T28.2: this fixture defers no work. `Never` is
    // uninhabited, so nothing can be sent and `apply` cannot
    // be called.
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

fn bump(a: &mut App, _: &EventCtx) {
    a.clicks += 1;
}

struct F {
    app: App,
    styles: Vec<Style>,
    st: Vec<NodeState>,
    input: fulgurite::event::InputState,
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.card.content.label = "on the card";
        app.ui.card.content.on_click = bump;
        app.ui.tip.content.label = "hover me";
        app.ui.tip.content.on_click = bump;
        app.ui.tip.tip.content = Toast::new("the tip");
        let styles = pass::default_styles(&app.ui);
        let mut f = F {
            app,
            styles,
            st: vec![NodeState::default(); <Ui as Widget>::NODE_COUNT],
            input: Default::default(),
        };
        f.frame();
        f
    }

    fn frame(&mut self) {
        pass::layout(&mut self.app.ui, &self.styles, &mut self.st, VIEW);
    }

    fn send(&mut self, ev: Event) {
        pass::dispatch(&mut self.app, &self.styles, &mut self.st, &mut self.input, &ev);
        self.frame();
    }

    /// Hover intent, as the host sends it once the pointer has rested
    /// (M57/B35): the tip waits for it.
    fn rest(&mut self) {
        if let Some(n) = self.input.hovered {
            let mut c = pass::Consumed::default();
            pass::deliver(&mut self.app, n, Event::Linger, fulgurite::event::Mods::NONE, &mut c);
        }
        self.frame();
    }

    fn idx(&self, wid: &str) -> usize {
        <Ui as Widget>::SUB_TABLE.iter().position(|d| d.wid == wid).expect("wid")
    }

    fn centre(&self, i: usize) -> (f32, f32) {
        let s = &self.st[i];
        (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0)
    }

    fn paint(&self) -> DisplayList {
        let mut l = DisplayList::new();
        Node::paint(&self.app.ui, &self.st, 0, &mut l);
        l
    }
}

/// (1) The claim `Dyn` failed.
#[test]
fn a_button_on_a_card_reaches_the_application() {
    let mut f = F::new();
    let card = f.idx("card");
    // The button is the card's only child.
    let (x, y) = f.centre(card + 1);
    f.send(Event::PointerDown { x, y, button: fulgurite::PointerButton::Left, clicks: 1 });
    f.send(Event::PointerUp { x, y, button: fulgurite::PointerButton::Left });
    assert_eq!(f.app.clicks, 1, "the handler under the card never ran");
}

/// And directly, because a click can land for the wrong reason: the
/// wrapper must *route*, and node 0 must not answer for its content.
#[test]
fn the_wrapper_routes_and_does_not_answer_for_itself() {
    let f = F::new();
    assert!(
        Handlers::<App>::handler(&f.app.ui.card, NodeSel { i: 0, inner: None }).is_none(),
        "the card answered a handler for its own node"
    );
    assert!(
        Handlers::<App>::handler(&f.app.ui.card, NodeSel { i: 1, inner: None }).is_some(),
        "the card did not route to the button on it"
    );
}

/// (2) A card is visible without a stylesheet.
#[test]
fn a_card_paints_a_surface_from_its_defaults() {
    let f = F::new();
    let card = f.idx("card");
    let r = fulgurite::ops::rect_of(&f.st[card]);
    let list = f.paint();
    let fills: Vec<&Cmd> = list
        .cmds
        .iter()
        .filter(|c| matches!(c, Cmd::RoundedRect { rect, color, .. }
                             if *rect == r && color.3 > 0))
        .collect();
    assert!(
        !fills.is_empty(),
        "the card painted no opaque fill at its own rect — it is a transparent \
         container with a different name; commands were {:?}",
        list.cmds.len()
    );
    let shadows = list.cmds.iter().filter(|c| matches!(c, Cmd::Shadow { .. })).count();
    assert!(shadows > 0, "a card with no shadow is a rectangle");
    // The padding is real, not decorative: the button must be inset.
    let inner = fulgurite::ops::rect_of(&f.st[card + 1]);
    assert!(
        inner[0] > r[0] && inner[1] > r[1],
        "the card's content is not inset by its padding: content {inner:?} in {r:?}"
    );
}

/// (3) The tip opens on a hover of the *content*.
#[test]
fn a_tooltip_opens_when_its_content_is_hovered() {
    let mut f = F::new();
    assert!(!f.app.ui.tip.is_open(), "the tip is open before anything was hovered");
    let tip = f.idx("tip");
    // The wrapper's own node is never marked hovered; the button is.
    let (x, y) = f.centre(tip + 1);
    f.send(Event::PointerMove { x, y });
    f.rest();
    assert!(
        f.app.ui.tip.is_open(),
        "hovering the content did not open the tip — the wrapper's own `hovered` \
         is never set, because dispatch marks the deepest node it hits"
    );
    // And it closes when the pointer leaves.
    f.send(Event::PointerMove { x: 0.0, y: 0.0 });
    assert!(!f.app.ui.tip.is_open(), "the tip stayed open after the pointer left");
}

/// (4) The tip is not a hit target.
#[test]
fn the_tip_does_not_take_the_pointer() {
    let mut f = F::new();
    let tip = f.idx("tip");
    let (x, y) = f.centre(tip + 1);
    f.send(Event::PointerMove { x, y });
    f.rest();
    assert!(f.app.ui.tip.is_open(), "setup: the tip is not open, so this proves nothing");
    // With the tip open and overlapping, the same point must still find
    // the content — a tip that took the hit would close itself.
    let hit = Node::hit(&f.app.ui, &f.st, 0, x, y).expect("something under the pointer");
    assert_eq!(
        hit.node as usize,
        tip + 1,
        "the pointer landed on node {} instead of the tooltip's content ({})",
        hit.node,
        tip + 1
    );
}

/// A `Toast` draws its text on its own ground, with no sheet.
#[test]
fn a_toast_paints_text_on_a_filled_pill() {
    let mut t: Toast = Toast::new("saved");
    let styles = vec![<Toast as Widget>::type_defaults()];
    let mut st = vec![NodeState::default()];
    let mut m = fulgurite::cx::BlockMetrics;
    let mut cx = fulgurite::LayoutCx::new(&mut m);
    Node::layout(&mut t, &mut cx, VIEW, &styles, &mut st, 0);
    Node::place(&mut t, Point { x: 0.0, y: 0.0 }, &styles, &mut st, 0);
    let mut l = DisplayList::new();
    Node::paint(&t, &st, 0, &mut l);
    let fills = l.cmds.iter().filter(|c| matches!(c, Cmd::RoundedRect { color, .. } if color.3 > 0));
    assert_eq!(fills.count(), 1, "a toast with no ground is text on the page");
    let texts: Vec<&str> = l
        .cmds
        .iter()
        .filter_map(|c| match c {
            Cmd::Text { text, .. } => Some(l.str_of(*text)),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["saved"], "the toast's text is not what it was given");
    // Fits its text plus its padding, rather than filling the window.
    assert!(
        st[0].size.w < VIEW.max_w,
        "the toast is {}px wide in a {}px window — it is not shrinking to its text",
        st[0].size.w, VIEW.max_w
    );
}
