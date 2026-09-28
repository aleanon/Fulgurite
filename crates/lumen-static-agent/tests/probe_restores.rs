//! **The audit leaves every widget as the frame laid it out** — M57/A3
//! (`docs/plan-widget-fixes.md`).
//!
//! The agent's per-frame audit asks each leaf for its natural size by
//! laying it out again inside million-pixel boxes. A widget that caches
//! what layout computed kept *those* results, and painted from them on
//! every frame that did not lay it out again — which, under
//! `Boundaries`, is every frame nothing marked it. So in a live window
//! with the agent attached, `RichText` drew its paragraph on one line,
//! a text field's click always put the caret at the end, and a tab strip
//! drew its labels into the wrong widths (`docs/widget-audit.md` X1).
//!
//! Headless, with no agent, everything was right, which is why nothing
//! caught it. So this compares exactly those two: the paint of a frame,
//! and the paint of the same frame after the audit has run over it.

use lumen_static::event::{Event, EventCtx};
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{
    Cluster, Constraints, DisplayList, GlyphSink, LayoutCx, Node, NodeState, Size, StaticTree, TextMeasure, Widget,
};
use lumen_static_agent::{Agent, TreeMut};
use lumen_static_widgets::{RichText, Tabs, TextInput};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 12.0)]
struct Page {
    #[widget_id("rich")]
    rich: RichText,
    #[widget_id("field")]
    field: TextInput,
    #[widget_id("tabs")]
    tabs: Tabs,
}

/// The host's face, standing in: one cluster per character, 8 px apart.
///
/// **Not `BlockMetrics`**, which reports no clusters at all — a field
/// laid out with it puts every click at the end whatever the audit does,
/// so the first draft of the caret test below could not fail. The live
/// window lays out with a real face, and a real face shapes.
struct Monospace;

const ADVANCE: f32 = 8.0;

impl TextMeasure for Monospace {
    fn measure(&mut self, text: &str, size: f32, max_w: f32) -> Size {
        let w = text.chars().count() as f32 * ADVANCE;
        // Wrap like a real face: whole lines of `max_w`.
        if w > max_w && max_w > 0.0 {
            let lines = (w / max_w).ceil();
            return Size { w: max_w, h: size * 1.2 * lines };
        }
        Size { w, h: size * 1.2 }
    }
    fn shape(&mut self, text: &str, size: f32, _weight: f32, max_w: f32, sink: &mut dyn GlyphSink) -> Size {
        for (i, (b, _)) in text.char_indices().enumerate() {
            sink.cluster(Cluster { byte: b as u32, x: i as f32 * ADVANCE, line: 0 });
        }
        sink.cluster(Cluster { byte: text.len() as u32, x: text.chars().count() as f32 * ADVANCE, line: 0 });
        self.measure(text, size, max_w)
    }
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

struct F {
    page: Page,
    styles: Vec<lumen_static::Style>,
    st: Vec<NodeState>,
}

fn fixture() -> F {
    let mut page = Page {
        rich: RichText::new(
            "A span here, a bold word there, and enough words after them that the paragraph \
             has to wrap across three lines inside its box.",
        ),
        field: TextInput::new("type here"),
        tabs: Tabs::new(&["Overview", "Activity", "Settings"]),
    };
    let mut styles = vec![lumen_static::Style::default(); Page::NODE_COUNT];
    let sheet = [
        Rule::new(Selector::Id("rich"), Patch::new().width(Length::Fixed(300.0))),
        Rule::new(Selector::Id("field"), Patch::new().width(Length::Fixed(240.0))),
        Rule::new(Selector::Id("tabs"), Patch::new().width(Length::Fixed(360.0))),
    ];
    lumen_static::style::cascade_deep(&mut page, &sheet, &mut styles);
    let mut st = vec![NodeState::default(); Page::NODE_COUNT];
    let mut m = Monospace;
    let mut cx = LayoutCx::new(&mut m);
    lumen_static::pass::layout_with(&mut page, &mut cx, &styles, &mut st, VIEW);
    F { page, styles, st }
}

fn paint(f: &F) -> DisplayList {
    let mut list = DisplayList::new();
    lumen_static::pass::paint(&f.page, &f.st, &mut list);
    list
}

fn index(wid: &str) -> usize {
    lumen_static::style::index_of_wid(Page::SUB_TABLE, wid).unwrap()
}

#[test]
fn the_audit_changes_nothing_the_next_paint_draws() {
    let mut f = fixture();
    let before = paint(&f);
    let mut agent = Agent::new();
    {
        let mut tree = TreeMut::of_root(&mut f.page, &f.styles, &f.st);
        agent.frame_done_measured(&mut tree, lumen_static_agent::FrameTiming::default(), &mut Monospace, 1.0);
    }
    // No layout here: this is the frame `Boundaries` skips.
    let after = paint(&f);
    let (a, b) = (before.hash_from(0), after.hash_from(0));
    println!("paint before the audit: {} cmds, after: {} cmds", before.len(), after.len());
    assert_eq!(
        (before.len(), a),
        (after.len(), b),
        "the audit changed what the next frame paints (commands {} -> {})",
        before.len(),
        after.len()
    );
}

/// The symptom a user met, asked directly: a press inside the text puts
/// the caret there, not at the end.
#[test]
fn a_press_inside_the_text_places_the_caret_after_the_audit() {
    let mut f = fixture();
    let mut agent = Agent::new();
    {
        let mut tree = TreeMut::of_root(&mut f.page, &f.styles, &f.st);
        agent.frame_done_measured(&mut tree, lumen_static_agent::FrameTiming::default(), &mut Monospace, 1.0);
    }
    let i = index("field");
    let s = f.st[i];
    // A third of the way in: past "ty", well before the end.
    let (x, y) = (s.pos.x + s.pad.0 + 24.0, s.pos.y + s.size.h / 2.0);
    let ev = Event::PointerDown { x, y, button: lumen_static::PointerButton::Left, clicks: 1 };
    let mut ctx = EventCtx::for_event(i as u32, ev);
    ctx.pos = lumen_static::Point { x, y };
    ctx.local = lumen_static::Point { x: x - s.pos.x, y: y - s.pos.y };
    assert!(f.page.field.on_event(&ev, &ctx), "setup: the press did not reach the field");
    let len = "type here".len();
    println!("caret after a press 24 px in: {} of {len}", f.page.field.caret());
    assert!(f.page.field.caret() < len, "the caret went to the end: the field's clusters were the audit's");
}
