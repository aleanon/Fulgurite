//! **Two overlays, stacked** — M44/T44.0 item 12, closing R18-F7
//! (`docs/plan-input-semantics.md` §1.6).
//!
//! Every overlay test before this one built a single overlay, so two
//! mutants survived the whole suite: Escape closing the *bottom* overlay
//! instead of the top, and the overlay hit loop running bottom-first.
//! `escape_closes_the_topmost` could not tell "topmost" from "the only
//! one". Here two popups overlap (the overlap rect is printed and asserted
//! non-empty, or a press "in the overlap" proves nothing), are opened A
//! then B, and each question has two possible answers.
//!
//! Green on arrival, because the code is right; its evidence is the
//! injections in its commit. It lands before T44.9, which edits the code
//! it protects.

use lumen_static::event::InputState;
use lumen_static::overlay::OverlaySink;
use lumen_static::pass::Consumed;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{pass, Constraints, DisplayList, Event, EventCtx, Key, NodeState, Overlays, StaticTree, Widget};
use lumen_static_widgets::ops::Button;
use lumen_static_widgets::{Anchor, Popup};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct PanelA {
    #[widget_id("a_btn")]
    btn: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct PanelB {
    #[widget_id("b_btn")]
    btn: Button<App>,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("a")]
    a: Popup<PanelA>,
    #[widget_id("b")]
    b: Popup<PanelB>,
}

#[derive(lumen_static::App, Default)]
struct App {
    #[tree]
    ui: Ui,
    a_hits: u32,
    b_hits: u32,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 300.0, min_h: 0.0, max_h: 200.0 };

struct F {
    app: App,
    sty: Vec<lumen_static::Style>,
    st: [NodeState; Ui::NODE_COUNT],
    ov: Overlays<4>,
    input: InputState,
}

fn idx(wid: &str) -> usize {
    lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid)
}

impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.ui.a.open = true;
        app.ui.b.open = true;
        app.ui.a.anchor = Anchor::Over;
        app.ui.b.anchor = Anchor::Over;
        app.ui.a.content.btn.label = "A";
        app.ui.b.content.btn.label = "B";
        app.ui.a.content.btn.on_click = |s: &mut App, _: &EventCtx| s.a_hits += 1;
        app.ui.b.content.btn.on_click = |s: &mut App, _: &EventCtx| s.b_hits += 1;
        let sheet = [
            Rule::new(Selector::Id("a_btn"), Patch::new().width(Length::Fixed(120.0)).height(Length::Fixed(40.0))),
            Rule::new(Selector::Id("b_btn"), Patch::new().width(Length::Fixed(80.0)).height(Length::Fixed(30.0))),
        ];
        let mut sty = vec![lumen_static::Style::default(); Ui::NODE_COUNT];
        lumen_static::style::cascade_deep(&mut app.ui, &sheet, &mut sty);
        let mut f = F { app, sty, st: [NodeState::default(); Ui::NODE_COUNT], ov: Overlays::default(), input: InputState::default() };
        f.frame();
        f
    }

    fn frame(&mut self) {
        pass::layout(&mut self.app.ui, &self.sty, &mut self.st, VIEW);
        let mut list = DisplayList::new();
        self.ov = Overlays::default();
        pass::paint_overlaid(&self.app.ui, &self.st, &mut list, &mut self.ov);
    }

    fn open(&self) -> Vec<&'static str> {
        (0..self.ov.len())
            .map(|k| match self.ov.at(k) as usize {
                i if i == idx("a") => "A",
                i if i == idx("b") => "B",
                _ => "?",
            })
            .collect()
    }

    fn send(&mut self, ev: Event, consumed: &mut Consumed) -> bool {
        pass::dispatch_overlaid_into(&mut self.app, &self.sty, &mut self.st, &mut self.input, &self.ov, &ev, consumed)
    }
}

/// The rect both buttons cover, which must not be empty.
fn overlap(f: &F) -> [f32; 4] {
    let (a, b) = (f.st[idx("a_btn")], f.st[idx("b_btn")]);
    [
        a.pos.x.max(b.pos.x),
        a.pos.y.max(b.pos.y),
        (a.pos.x + a.size.w).min(b.pos.x + b.size.w),
        (a.pos.y + a.size.h).min(b.pos.y + b.size.h),
    ]
}

#[test]
fn escape_closes_the_top_overlay_only() {
    let mut f = F::new();
    let before = f.open();
    assert_eq!(before, ["A", "B"], "setup: both open, A painted first");
    let mut consumed = Consumed::default();
    let handled = f.send(Event::Key { key: Key::Escape, down: true }, &mut consumed);
    f.frame();
    let after = f.open();
    println!("open {before:?}; after Escape {after:?}; consumed {:?} (B is {})", consumed.nodes(), idx("b"));
    assert!(handled, "Escape was not handled with two overlays open");
    assert!(f.app.ui.a.open && !f.app.ui.b.open, "Escape closed {:?} instead of B only", after);
    assert_eq!(after, ["A"]);
    assert_eq!(consumed.nodes(), [idx("b") as u32], "the consumed set is not {{B}}");
}

#[test]
fn a_press_in_the_overlap_reaches_the_top_overlay() {
    let mut f = F::new();
    assert_eq!(f.open(), ["A", "B"], "setup: both open, A painted first");
    let o = overlap(&f);
    println!("overlap {o:?}");
    assert!(o[2] > o[0] && o[3] > o[1], "setup: the two overlays do not overlap, so 'topmost' is untested");
    let (x, y) = ((o[0] + o[2]) / 2.0, (o[1] + o[3]) / 2.0);

    let hit = pass::hit_test_overlaid(&f.app.ui, &f.st, &f.ov, x, y).expect("something at the overlap");
    println!("hit_test_overlaid at the overlap -> {} (a_btn {}, b_btn {})", hit.node, idx("a_btn"), idx("b_btn"));
    assert_eq!(hit.node as usize, idx("b_btn"), "the overlay hit test returned the bottom overlay");

    let mut consumed = Consumed::default();
    f.send(Event::down(x, y), &mut consumed);
    let handled = f.send(Event::up(x, y), &mut consumed);
    println!("overlap press -> {} (A: {}, B: {})", if f.app.b_hits == 1 { "B" } else { "A" }, f.app.a_hits, f.app.b_hits);
    assert!(handled, "the release in the overlap was not handled");
    assert_eq!((f.app.a_hits, f.app.b_hits), (0, 1), "the press in the overlap reached A, which is underneath");
}
