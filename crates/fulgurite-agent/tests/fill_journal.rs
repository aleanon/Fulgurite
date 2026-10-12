//! **The journal sees a fill change** — M82/T82.3
//! (`docs/plan-paint-vocabulary.md`).
//!
//! `ui.lastDamage` is the journal's answer, and the journal paints into a
//! list of its own, not the host's: a leaf's fill reaches its signature
//! only if the journal's list carries the frame's table (`TreeMut::fills`,
//! which the window's session sets from `Host::fills`), and a container
//! is signed from its resolved state, so its signature must fold the
//! fill's index *and* the resolved entry. A palette swap moves neither the
//! index nor any geometry — only a role-named stop's colour — so it is the
//! change that exercises both halves.
//!
//! Two nodes, each with a transparent background so the fill is all they
//! paint: a derive container and a `Button`. After the swap each must
//! report damage at 1.00x its own box.

use fulgurite::display::{Color, GradShape, Stops};
use fulgurite::style::{Fill, Length, Palette, Patch, Role, Rule, Selector};
use fulgurite::{pass, Constraints, NodeState, StaticTree, Widget};
use fulgurite_agent::{Agent, TreeMut, TreeRef};
use fulgurite_widgets::ops::{Button, Label};
use serde_json::{json, Value};

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    note: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 8.0)]
struct Root {
    #[widget_id("panel")]
    panel: Panel,
    #[widget_id("go")]
    go: Button<()>,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 400.0, min_h: 0.0, max_h: 300.0 };

/// One entry, its first stop named by role: a palette swap recolours it.
const TABLE: [Fill; 1] = [Fill::new(
    Stops { at: [0, 255, 0, 0], color: [Color(0, 0, 0, 255), Color(20, 120, 20, 255), Color(0, 0, 0, 0), Color(0, 0, 0, 0)], len: 2 },
    GradShape::Radial { center: [0.5, 0.5], radius: 1.0 },
)
.role(0, Role::Accent)];

struct Fixture {
    app: Root,
    states: [NodeState; Root::NODE_COUNT],
    styles: Vec<fulgurite::Style>,
    fills: Vec<Fill>,
    agent: Agent,
}

impl Fixture {
    fn new() -> Self {
        let filled = Patch::new()
            .background(Color(0, 0, 0, 0))
            .width(Length::Fixed(160.0))
            .height(Length::Fixed(40.0))
            .background_fill(1);
        let sheet = [Rule::new(Selector::Id("panel"), filled), Rule::new(Selector::Id("go"), filled)];
        let mut app = Root::default();
        let mut styles = vec![fulgurite::Style::default(); Root::NODE_COUNT];
        fulgurite::style::cascade_deep(&mut app, &sheet, &mut styles);
        let mut f = Fixture {
            app,
            states: [NodeState::default(); Root::NODE_COUNT],
            styles,
            fills: TABLE.iter().map(|e| e.resolve(&Palette::LIGHT)).collect(),
            agent: Agent::new(),
        };
        f.frame();
        f
    }

    fn frame(&mut self) {
        pass::layout(&mut self.app, &self.styles, &mut self.states, VIEW);
        let mut tree = TreeMut::of_root(&mut self.app, &self.styles, &self.states).with_fills(&self.fills);
        self.agent.frame_done(&mut tree, fulgurite_agent::FrameTiming::passes(1000, 500));
    }

    fn regions(&self) -> Vec<[f32; 4]> {
        let tree = TreeRef::of_root(&self.app, &self.styles, &self.states).with_fills(&self.fills);
        let resp = self
            .agent
            .dispatch(&tree, &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastDamage", "params": {} }));
        assert!(resp.get("error").is_none(), "unexpected error: {resp}");
        let r: Value = resp["result"]["regions"].clone();
        r.as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let a = r.as_array().unwrap();
                [0, 1, 2, 3].map(|i| a[i].as_f64().unwrap() as f32)
            })
            .collect()
    }
}

fn area(r: [f32; 4]) -> f32 {
    (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0)
}

fn idx(wid: &str) -> usize {
    fulgurite::style::index_of_wid(<Root as Widget>::SUB_TABLE, wid).expect("the id is in the table")
}

#[test]
fn a_palette_swap_damages_each_filled_node_at_one_times_its_box() {
    let mut f = Fixture::new();
    let mut failures = Vec::new();
    for wid in ["panel", "go"] {
        let s = f.states[idx(wid)];
        assert_eq!(s.fill, 1, "setup: #{wid} carries the fill");
        assert_eq!(s.bg.3, 0, "setup: #{wid}'s background is transparent, so the fill is all it paints");
    }
    // The swap: what `Host::set_palette` does to the table.
    f.fills = TABLE.iter().map(|e| e.resolve(&Palette::DARK)).collect();
    assert_ne!(f.fills[0].stops.color[0], Palette::LIGHT.accent, "setup: the resolved stop moved");
    f.frame();
    let regions = f.regions();
    for wid in ["panel", "go"] {
        let s = f.states[idx(wid)];
        let own = [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h];
        let hit: f32 = regions
            .iter()
            .map(|r| area([r[0].max(own[0]), r[1].max(own[1]), r[2].min(own[2]), r[3].min(own[3])]))
            .sum();
        let ratio = hit / area(own);
        println!("#{wid}: damage over its box {hit} px2 of {} px2 = {ratio:.2}x (want 1.00x)", area(own));
        if (ratio - 1.0).abs() > 1e-3 {
            failures.push(format!("#{wid}: {ratio:.2}x its box"));
        }
    }
    let total: f32 = regions.iter().copied().map(area).sum();
    let boxes: f32 = ["panel", "go"].iter().map(|w| {
        let s = f.states[idx(w)];
        s.size.w * s.size.h
    }).sum();
    println!("total damage {total} px2 against the two boxes' {boxes} px2");
    assert!(failures.is_empty(), "the journal did not see the fill change: {failures:?}");
    assert_eq!(total, boxes, "the swap damaged something other than the two filled nodes");
}
