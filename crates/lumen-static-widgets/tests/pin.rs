//! M85/T85.2 — `Pin<W>`: one child at an offset inside the pin's box,
//! clipped to it in paint and in hit alike.
//!
//! Three claims, each printed beside its bound:
//!
//! 1. the child sits at the pin's position plus the offset, exactly;
//! 2. a press at the child's centre lands on the child — the hit node is
//!    the child's index, not the pin's — and its handler runs;
//! 3. the pin's rect gates the child: a press inside the pin but off the
//!    child is not the child's, and a press on the part of a child pinned
//!    past the pin's edge is not the child's either.

use lumen_static::event::{Event, EventCtx};
use lumen_static::prelude::*;
use lumen_static::{pass, Constraints, NodeState, PointerButton};
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::Pin;

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 400.0 };

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    /// A 200 x 150 pin holding a 60 x 30 button at (30, 40).
    #[widget_id("pin")]
    pin: Pin<Button<App, &'static str>>,
    /// A 100 x 100 pin whose 60 x 30 button sits at (-30, 20), so its
    /// left 30 px are past the pin's edge (a negative offset is the one
    /// way a child leaves the box it is laid out in).
    #[widget_id("edge")]
    edge: Pin<Button<App, &'static str>>,
}

#[derive(Default)]
struct App {
    ui: Ui,
    clicks: u32,
}

impl lumen_static::event::App for App {
    type State = Self;
    fn state_mut(&mut self) -> &mut Self::State {
        self
    }
    // This fixture defers no work.
    type Msg = lumen_static::event::Never;
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
    input: lumen_static::event::InputState,
}

fn idx(wid: &str) -> usize {
    <Ui as Widget>::SUB_TABLE.iter().position(|d| d.wid == wid).expect("wid")
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.pin = Pin::new(Button::default(), 30.0, 40.0);
        app.ui.pin.content.label = "pinned";
        app.ui.pin.content.on_click = bump;
        app.ui.edge = Pin::new(Button::default(), -30.0, 20.0);
        app.ui.edge.content.label = "edge";
        app.ui.edge.content.on_click = bump;
        let mut styles = pass::default_styles(&app.ui);
        let (pin, edge) = (idx("pin"), idx("edge"));
        // Room on the left of both pins, so a press past the edge pin's
        // left edge is still inside the root.
        styles[0].padding = lumen_static::Edges::all(50.0);
        styles[pin].width = Length::Fixed(200.0);
        styles[pin].height = Length::Fixed(150.0);
        styles[edge].width = Length::Fixed(100.0);
        styles[edge].height = Length::Fixed(100.0);
        for b in [pin + 1, edge + 1] {
            styles[b].width = Length::Fixed(60.0);
            styles[b].height = Length::Fixed(30.0);
        }
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

    /// A press and release at `(x, y)`: whether each was handled, and the
    /// node the press hit.
    fn click(&mut self, x: f32, y: f32) -> (bool, bool, Option<u32>) {
        let hit = pass::hit_test(&self.app.ui, &self.st, x, y).map(|h| h.node);
        let down = pass::dispatch(
            &mut self.app,
            &self.styles,
            &mut self.st,
            &mut self.input,
            &Event::PointerDown { x, y, button: PointerButton::Left, clicks: 1 },
        );
        self.frame();
        let up = pass::dispatch(
            &mut self.app,
            &self.styles,
            &mut self.st,
            &mut self.input,
            &Event::PointerUp { x, y, button: PointerButton::Left },
        );
        self.frame();
        (down, up, hit)
    }
}

/// (1) The child is at the pin's position plus the offset, exactly.
#[test]
fn the_child_sits_at_the_pins_position_plus_its_offset() {
    let f = F::new();
    let pin = idx("pin");
    let (p, c) = (f.st[pin], f.st[pin + 1]);
    println!(
        "pin at ({}, {}) {}x{}; child at ({}, {}) {}x{}; offset (30, 40)",
        p.pos.x, p.pos.y, p.size.w, p.size.h, c.pos.x, c.pos.y, c.size.w, c.size.h
    );
    // Setup: the pin has its own 200 x 150 box, and the child its 60 x 30.
    assert_eq!((p.size.w, p.size.h), (200.0, 150.0), "setup: the pin's box");
    assert_eq!((c.size.w, c.size.h), (60.0, 30.0), "setup: the child's box");
    assert_eq!(
        (c.pos.x, c.pos.y),
        (p.pos.x + 30.0, p.pos.y + 40.0),
        "the child is not at the pin's position plus (30, 40)"
    );
    // The second pin is below the first, so its offset is not the root's.
    let e = idx("edge");
    println!("edge pin at ({}, {}); child at ({}, {})", f.st[e].pos.x, f.st[e].pos.y, f.st[e + 1].pos.x, f.st[e + 1].pos.y);
    assert_eq!(f.st[e].pos.y, p.pos.y + 150.0, "setup: the second pin is below the first");
    assert_eq!(
        (f.st[e + 1].pos.x, f.st[e + 1].pos.y),
        (f.st[e].pos.x - 30.0, f.st[e].pos.y + 20.0),
        "the edge pin's child is not at its position plus (-30, 20)"
    );
}

/// (2) A press at the child's centre is the child's.
#[test]
fn a_press_at_the_childs_centre_lands_on_the_child() {
    let mut f = F::new();
    let pin = idx("pin");
    // Where the child's centre *should* be — the pin's position plus the
    // offset plus half its 60 x 30 — not where the child reports it is,
    // so a pin that ignores its offset misses here.
    let p = f.st[pin];
    let (x, y) = (p.pos.x + 30.0 + 30.0, p.pos.y + 40.0 + 15.0);
    let (down, up, hit) = f.click(x, y);
    println!(
        "press at ({x}, {y}): down handled {down}, up handled {up}, hit {hit:?} \
         (child {}, pin {pin}); clicks {}",
        pin + 1,
        f.app.clicks
    );
    // A button takes the press on its release: that is the click.
    assert!(up, "the release at the child's centre was not handled");
    assert_eq!(hit, Some((pin + 1) as u32), "the hit node is not the child");
    assert_eq!(f.app.clicks, 1, "the child's handler did not run");
}

/// (3a) Inside the pin, off the child: not the child's.
#[test]
fn a_press_inside_the_pin_but_off_the_child_is_not_the_childs() {
    let mut f = F::new();
    let pin = idx("pin");
    let p = f.st[pin];
    // (150, 120) in the pin's own coordinates: inside its 200 x 150, well
    // clear of the child at (30, 40)-(90, 70).
    let (x, y) = (p.pos.x + 150.0, p.pos.y + 120.0);
    let (_, _, hit) = f.click(x, y);
    println!("press at ({x}, {y}) inside the pin: hit {hit:?} (child {}); clicks {}", pin + 1, f.app.clicks);
    assert!(lumen_static::hit::contains(&p, x, y), "setup: the press is inside the pin");
    assert_ne!(hit, Some((pin + 1) as u32), "a press off the child hit the child");
    assert_eq!(f.app.clicks, 0, "a press off the child ran its handler");
}

/// (3b) The pin's rect gates the child: the part of a child pinned past
/// the edge is not pressable, and the part inside is.
#[test]
fn a_child_past_the_pins_edge_is_not_pressable_there() {
    let mut f = F::new();
    let e = idx("edge");
    let (p, c) = (f.st[e], f.st[e + 1]);
    let y = c.pos.y + c.size.h / 2.0;
    // 15 px past the pin's left edge, on the child.
    let past = p.pos.x - 15.0;
    println!(
        "edge pin x {}..{}; child x {}..{}; press at x {past}",
        p.pos.x,
        p.pos.x + p.size.w,
        c.pos.x,
        c.pos.x + c.size.w
    );
    assert!(lumen_static::hit::contains(&c, past, y), "setup: the press is on the child");
    assert!(!lumen_static::hit::contains(&p, past, y), "setup: the press is outside the pin");
    let (_, _, hit) = f.click(past, y);
    println!("past the edge: hit {hit:?} (child {}); clicks {}", e + 1, f.app.clicks);
    assert_ne!(hit, Some((e + 1) as u32), "a press past the pin's edge reached its child");
    assert_eq!(f.app.clicks, 0, "a press past the pin's edge ran the child's handler");
    // Accept: the same child, inside the pin, takes the press.
    let inside = p.pos.x + 10.0;
    let (down, up, hit) = f.click(inside, y);
    println!("inside the edge at x {inside}: handled {down}/{up}, hit {hit:?}; clicks {}", f.app.clicks);
    assert_eq!(hit, Some((e + 1) as u32), "the child's part inside the pin was not pressable");
    assert_eq!(f.app.clicks, 1, "the child's part inside the pin did not run its handler");
}

/// The clip is in paint as well as in hit: the child's commands are
/// bracketed by a clip of the pin's own rect.
#[test]
fn the_pin_clips_its_child_to_its_rect_in_paint() {
    use lumen_static::display::{Cmd, DisplayList};
    let f = F::new();
    let e = idx("edge");
    let r = lumen_static::ops::rect_of(&f.st[e]);
    let mut l = DisplayList::new();
    lumen_static::widget::Node::paint(&f.app.ui, &f.st, 0, &mut l);
    let clips = l.cmds.iter().filter(|c| matches!(c, Cmd::PushClip { rect } if *rect == r)).count();
    println!("PushClip at the edge pin's rect {r:?}: {clips} (bound 1)");
    assert_eq!(clips, 1, "the pin did not clip its child to its own rect");
}
