//! M27/T27.3 — what a change actually damages.
//!
//! Four defects, all found by the **first runs** of `just widget-drive`
//! and none by any test, because nothing had ever asked a per-widget
//! damage question. A click on a 24x24 checkbox reported 691 200 px² —
//! the whole window, 1 200x the widget — and the showcase looked fine.
//!
//! 1. `ui.lastDamage` reported the **bounding box** of every changed
//!    node, which is the union M18 exists to avoid: its own doc says "a
//!    scrollbar thumb moving down the right edge, unioned with a toolbar
//!    across the top, is the whole window". M18 replaced the union with
//!    a bounded set in the renderer and this surface was never updated.
//! 2. The journal folded a container's signature from its **children's**,
//!    so one leaf's repaint marked every ancestor changed — and an
//!    ancestor's rect is bigger at every level, with the root's being the
//!    window.
//! 3. That same fold meant a container's **own** paint was never signed:
//!    a container whose background, radius, shadow or border changed
//!    reported nothing at all.
//! 4. `Cmd::hash_into` folded a text run's **offset into the shared text
//!    buffer**, so a node whose text changed length shifted every node
//!    painted after it and reported all of them as changed.

use lumen_static::display::Color;
use lumen_static_widgets::ops::Label;
use lumen_static::style::{Length, Patch, Rule, Selector};
use lumen_static::{pass, Constraints, NodeState, StaticTree, Widget};
use lumen_static_agent::{Agent, TreeMut, TreeRef};
use serde_json::{json, Value};

/// A deep tree: the leaf that changes is far from the root, and every
/// level between them is a container with a rect of its own. Depth is
/// the point — amplification grows with it.
/// **A column, and that is load-bearing.** The sibling is painted after
/// the leaf, so a shift in the shared text buffer lands on it — but it
/// must not *move*, or the defect hides behind an honest geometry
/// change. In a row a longer run pushes the sibling right and the
/// journal classifies it `geometry` (because `content` is
/// `repainted && !moved`), which is exactly what the first version of
/// this test saw: the injection went green.
///
/// A single-line run's *height* does not change with its length, so in a
/// column the sibling stays put and any report about it is a repaint.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Inner {
    #[widget_id("leaf")]
    leaf: Label,
    /// **A `Label`, not a `Fixed`.** The text offset only enters a
    /// node's signature if that node paints text; with a `Fixed` here
    /// the injection went green, because a rect's hash cannot notice a
    /// shift in the text buffer. That is a finding about the test, not a
    /// reason to soften the injection.
    #[widget_id("sibling")]
    sibling: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Mid {
    #[widget_id("inner")]
    inner: Inner,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Root {
    #[widget_id("mid")]
    mid: Mid,
}

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 800.0, min_h: 0.0, max_h: 600.0 };

struct Fixture {
    app: Root,
    states: [NodeState; Root::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    agent: Agent,
}

impl Fixture {
    fn new(sheet: &[Rule]) -> Self {
        let mut app = Root::default();
        app.mid.inner.leaf.set("hello");
        app.mid.inner.sibling.set("after");
        let mut styles = vec![lumen_static::Style::default(); Root::NODE_COUNT];
        lumen_static::style::cascade_deep(&mut app, sheet, &mut styles);
        let mut f = Fixture {
            app,
            states: [NodeState::default(); Root::NODE_COUNT],
            styles,
            agent: Agent::new(),
        };
        f.frame();
        f
    }

    fn frame(&mut self) {
        pass::layout(&mut self.app, &self.styles, &mut self.states, VIEW);
        let mut tree = TreeMut::of_root(&mut self.app, &self.styles, &self.states);
        self.agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(1000, 500));
    }

    fn result(&self, method: &str, params: Value) -> Value {
        let tree = TreeRef::of_root(&self.app, &self.styles, &self.states);
        let resp = self.agent.dispatch(
            &tree,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
        );
        assert!(resp.get("error").is_none(), "unexpected error: {resp}");
        resp["result"].clone()
    }

    /// The change kinds by wid: "geometry", "content" or "both".
    ///
    /// The distinction matters and a test that ignores it is wrong in a
    /// way that looks right: a sibling *does* legitimately move when the
    /// run beside it changes width, and reporting that is correct. What
    /// must not happen is a sibling reporting a **repaint** it did not
    /// make.
    fn changes(&self) -> Vec<(String, String)> {
        self.result("ui.lastChange", json!({}))["changed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    c["wid"].as_str().map(str::to_owned)
                        .unwrap_or_else(|| format!("#{}", c["index"])),
                    c["change"].as_str().unwrap_or("?").to_owned(),
                )
            })
            .collect()
    }

    fn changed_wids(&self) -> Vec<String> {
        self.result("ui.lastChange", json!({}))["changed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                c["wid"].as_str().map(str::to_owned).unwrap_or_else(|| format!("#{}", c["index"]))
            })
            .collect()
    }

    fn regions(&self) -> Vec<[f32; 4]> {
        self.result("ui.lastDamage", json!({}))["regions"]
            .as_array()
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

/// (1) and (2): one leaf changes; its ancestors do not.
#[test]
fn a_leaf_change_does_not_damage_its_ancestors() {
    let mut f = Fixture::new(&[]);
    // **Index 3, and the index is asserted.** The tree is
    // Root(0) > Mid(1) > Inner(2) > leaf(3), sibling(4); the first
    // version of this test read `states[2]` and compared the damage
    // against *Inner's* area, which is the whole row — a bound computed
    // from the wrong quantity, in the test written to catch exactly that.
    let leaf = f.states[3];
    assert!(
        leaf.size.h > 0.0 && leaf.size.h < f.states[2].size.h,
        "states[3] is not the leaf: {:?} against its parent {:?}",
        leaf.size, f.states[2].size
    );
    f.app.mid.inner.leaf.set("hello there, a longer run");
    f.frame();

    let wids = f.changed_wids();
    for a in ["mid", "inner", "#0"] {
        assert!(
            !wids.contains(&a.to_string()),
            "ancestor {a:?} reported changed for a leaf's repaint; changed = {wids:?}"
        );
    }
    assert!(wids.contains(&"leaf".to_string()), "the leaf itself was not reported: {wids:?}");

    // The number beside the bound. Bounded by the **leaf's own box**,
    // not by a fraction of the root's: the root here is one short row, so
    // "less than a quarter of the root" would be satisfied by damage
    // four times the leaf's size, which is the amplification this test
    // exists to refuse.
    let regions = f.regions();
    let damaged: f32 = regions.iter().copied().map(area).sum();
    let after = f.states[3];
    let leaf_area = (leaf.size.w * leaf.size.h).max(after.size.w * after.size.h);
    let root = area([0.0, 0.0, f.states[0].size.w, f.states[0].size.h]);
    assert!(damaged > 0.0, "nothing was damaged at all, so this test proves nothing");
    assert!(
        damaged <= leaf_area * 2.5,
        "damage {damaged:.0} px² against the leaf's own {leaf_area:.0} px² \
         (bound {:.0}); the root is {root:.0} px², and reporting every ancestor \
         would read about that",
        leaf_area * 2.5
    );
}

/// (3) A container's own paint is signed. Nothing checked this, because
/// the fold made every container look busy whenever anything under it
/// moved — so the gap was invisible *because of* the other defect.
#[test]
fn a_containers_own_background_change_is_reported() {
    let sheet = [Rule::new(
        Selector::Id("mid"),
        Patch::new().background(Color(10, 20, 30, 255)).width(Length::FILL),
    )];
    let mut f = Fixture::new(&sheet);
    // Re-cascade with a different fill: nothing moves, nothing else
    // repaints, only this container's own background differs.
    let sheet2 = [Rule::new(
        Selector::Id("mid"),
        Patch::new().background(Color(200, 40, 40, 255)).width(Length::FILL),
    )];
    lumen_static::style::cascade_deep(&mut f.app, &sheet2, &mut f.styles);
    f.frame();

    let wids = f.changed_wids();
    assert!(
        wids.contains(&"mid".to_string()),
        "a container's own background changed and it reported nothing: {wids:?}"
    );
}

/// The same, for the border M27/T27.1 added — the field most likely to
/// be forgotten, because it is the newest.
#[test]
fn a_containers_own_border_change_is_reported() {
    let base = Patch::new().background(Color(10, 20, 30, 255)).width(Length::FILL);
    let mut f = Fixture::new(&[Rule::new(Selector::Id("mid"), base)]);
    let sheet2 =
        [Rule::new(Selector::Id("mid"), base.border_width(2).border_color(Color(9, 9, 9, 255)))];
    lumen_static::style::cascade_deep(&mut f.app, &sheet2, &mut f.styles);
    f.frame();
    assert!(
        f.changed_wids().contains(&"mid".to_string()),
        "a border appeared on a container and it reported nothing: {:?}",
        f.changed_wids()
    );
}

/// (4) A text run that changes *length* must not report every node
/// painted after it.
///
/// The showcase's readout going from "checked: false" to "checked: true"
/// — one byte shorter — reported a 954 px note at the far end of the
/// window as repainted.
#[test]
fn a_text_length_change_does_not_report_later_nodes() {
    let mut f = Fixture::new(&[]);
    // The leaf is painted *before* its sibling, so a shift in the shared
    // text buffer lands on the sibling. Shorter, not longer: the offset
    // moves either way and only one direction was ever seen by accident.
    f.app.mid.inner.leaf.set("hi");
    f.frame();
    let changes = f.changes();
    // The sibling may legitimately **move** — a shorter run beside it in
    // a row shifts it left, and reporting that is correct. What it must
    // not report is a *repaint*, which is what a shifted text offset
    // produced.
    // In a column the sibling cannot have moved, so *any* report about
    // it is a repaint it did not make.
    assert!(
        !changes.iter().any(|(w, _)| w == "sibling"),
        "the sibling reported a change it did not make — its text offset in the \
         shared buffer shifted under it: {changes:?}"
    );
    assert!(
        changes.iter().any(|(w, _)| w == "leaf"),
        "the leaf that did change was not reported: {changes:?}"
    );
}

/// (1), directly: two changes far apart must stay **two regions**.
///
/// The tests above cannot see this — one change is one region either
/// way, so an injection that replaced the set with its bounding box left
/// them green. This is the case M18 was built for, and the sentence its
/// doc uses: "a scrollbar thumb moving down the right edge, unioned with
/// a toolbar across the top, is the whole window."
///
/// **The first version of this test was vacuous** and the same injection
/// proved it: it asserted `summed <= hull`, which a bounding box
/// satisfies exactly (it *is* one region, so the sum equals the hull).
/// A bound that the wrong answer meets is the second shape AGENT.md
/// names. What separates a set from its hull is the **count**, and how
/// much smaller the set is.
#[test]
fn two_distant_changes_stay_two_regions() {
    let sheet = [
        Rule::new(Selector::Id("leaf"), Patch::new().height(Length::Fixed(20.0))),
        Rule::new(Selector::Id("sibling"), Patch::new().height(Length::Fixed(20.0))),
        // A tall gap between them, so their hull is far larger than
        // either — which is the whole point and is asserted below.
        Rule::new(Selector::Id("inner"), Patch::new().height(Length::Fixed(440.0)).spacing(400.0)),
    ];
    let mut f = Fixture::new(&sheet);
    // Both ends change in the SAME frame, and nothing between them does.
    f.app.mid.inner.leaf.set("top changed");
    f.app.mid.inner.sibling.set("bottom changed");
    f.frame();

    let regions = f.regions();
    let summed: f32 = regions.iter().copied().map(area).sum();
    let hull = regions.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, r| {
        [a[0].min(r[0]), a[1].min(r[1]), a[2].max(r[2]), a[3].max(r[3])]
    });
    println!(
        "regions={} summed={summed:.0} px² hull={:.0} px² ({:.0}% of the hull)",
        regions.len(),
        area(hull),
        100.0 * summed / area(hull).max(1.0)
    );
    assert_eq!(
        regions.len(),
        2,
        "two changes 400 px apart came back as {} region(s) — a union, not a set: {regions:?}",
        regions.len()
    );
    assert!(
        summed < area(hull) * 0.5,
        "the set is {summed:.0} px² against a hull of {:.0} px²; a union would be \
         100% of it and this bound exists to tell them apart",
        area(hull)
    );
}

// ---- M80/T80.3: a cut label's two runs -----------------------------------

/// Two labels in a column, each cut to one line — the `…` run is a second
/// `Cmd::Text` per label, so the shared text buffer holds four runs and a
/// cut that moves shifts the offset of every run after it (S2,
/// `docs/plan-iced-roadmap.md` §2.9).
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Cuts {
    #[widget_id("first")]
    first: Label<String>,
    #[widget_id("second")]
    second: Label<String>,
}

struct CutFixture {
    app: Cuts,
    states: [NodeState; Cuts::NODE_COUNT],
    styles: Vec<lumen_static::Style>,
    agent: Agent,
}

impl CutFixture {
    /// Both labels 80 px wide under `overflow`, with texts far longer.
    fn new(overflow: lumen_static::TextOverflow) -> Self {
        let mut app = Cuts::default();
        app.first.set("a first label far too long for its box".into());
        app.second.set("a second label far too long for its box".into());
        let patch = Patch::new().width(Length::Fixed(80.0)).overflow(overflow);
        let sheet = [Rule::new(Selector::Id("first"), patch), Rule::new(Selector::Id("second"), patch)];
        let mut styles = vec![lumen_static::Style::default(); Cuts::NODE_COUNT];
        lumen_static::style::cascade_deep(&mut app, &sheet, &mut styles);
        let mut f = CutFixture { app, states: [NodeState::default(); Cuts::NODE_COUNT], styles, agent: Agent::new() };
        f.frame();
        f
    }

    fn frame(&mut self) {
        pass::layout(&mut self.app, &self.styles, &mut self.states, VIEW);
        let mut tree = TreeMut::of_root(&mut self.app, &self.styles, &self.states);
        self.agent.frame_done(&mut tree, lumen_static_agent::FrameTiming::passes(1000, 500));
    }

    fn result(&self, method: &str) -> Value {
        let tree = TreeRef::of_root(&self.app, &self.styles, &self.states);
        let resp = self
            .agent
            .dispatch(&tree, &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": {} }));
        assert!(resp.get("error").is_none(), "unexpected error: {resp}");
        resp["result"].clone()
    }

    fn changed_wids(&self) -> Vec<String> {
        self.result("ui.lastChange")["changed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["wid"].as_str().map(str::to_owned).unwrap_or_else(|| format!("#{}", c["index"])))
            .collect()
    }
}

/// **S2 with a second run in play** — M80/T80.3. The `…` is an ordinary
/// `Cmd::Text`, whose hash folds the run's length and never its offset in
/// the shared buffer (M27/T27.3). Changing the first label's text so its
/// cut moves reports the first label and **not** the second, whose prefix
/// and mark both sit after it in the buffer.
#[test]
fn a_moved_cut_does_not_report_the_next_cut_label() {
    let mut f = CutFixture::new(lumen_static::TextOverflow::Ellipsis);
    let before = f.app.first.shown().0.to_owned();
    assert!(f.app.first.shown().1 && f.app.second.shown().1, "setup: both labels are cut");
    // A space where the old cut ended, so the new cut drops it: shorter.
    f.app.first.set("abcdef label far too long for its box".into());
    f.frame();
    let after = f.app.first.shown().0.to_owned();
    let wids = f.changed_wids();
    println!("first cut {before:?} -> {after:?}; second {:?}; changed {wids:?} (bound: second absent)", f.app.second.shown());
    assert_ne!(before.len(), after.len(), "setup: the first label's cut did not move");
    assert!(wids.contains(&"first".to_string()), "setup: the first label's change was not reported: {wids:?}");
    assert!(
        !wids.contains(&"second".to_string()),
        "the second cut label reported a change it did not make -- a run's offset entered the hash: {wids:?}"
    );
}

// ---- M83/T83.6: `ShaderView`'s uniforms ----------------------------------

/// **A uniform write damages its own shader and nothing else** — M83/T83.6
/// (`docs/plan-gpu-extension.md`). Two `ShaderView`s in a container one
/// level down (a bound node hung off the root is laid out whenever the
/// root is, `bind_marks.rs`'s trap), each 32 B of uniforms, through a real
/// `Host` under the `Boundaries` default and the agent's journal, as the
/// window runs them. Only with the agent's `shader` feature (its manifest
/// says why it is not on the dev-dependency).
#[cfg(feature = "shader")]
mod shader {
    use lumen_static::display::ShaderId;
    use lumen_static::invalidate::{Boundaries, Invalidation};
    use lumen_static::prelude::*;
    use lumen_static::retain::{Damage, Incremental};
    use lumen_static::{Cmd, DisplayList, Widget};
    use lumen_static_agent::{Agent, FrameTiming, TreeMut, TreeRef};
    use lumen_static_shell::Host;
    use lumen_static_widgets::ShaderView;
    use serde_json::{json, Value};

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 8.0)]
    struct Pair {
        #[widget_id("first")]
        first: ShaderView<32>,
        #[widget_id("second")]
        second: ShaderView<32>,
    }

    #[derive(StaticTree, Default)]
    #[widget(column, spacing = 0.0)]
    struct Ui {
        pair: Pair,
    }

    #[derive(Default)]
    struct Bytes {
        a: [u8; 32],
        b: [u8; 32],
    }

    /// The value form of `#[bind]`: each view's bytes are state, written
    /// through `ShaderView`'s `BindValue` impl, whose answer marks it.
    #[derive(DeriveApp, Default)]
    struct Shaded {
        #[tree]
        ui: Ui,
        #[state]
        #[bind(ui.pair.first, a)]
        #[bind(ui.pair.second, b)]
        bytes: Bytes,
    }

    type H = Host<Shaded, Incremental, Boundaries<256>>;

    fn colour(r: u8, g: u8, b: u8) -> [u8; 32] {
        let mut u = [0; 32];
        u[..4].copy_from_slice(&[r, g, b, 255]);
        u
    }

    fn app() -> Shaded {
        let mut s = Shaded::default();
        s.ui.pair.first = ShaderView::new(ShaderId(0), ShaderView::<32>::solid).sized(120.0, 40.0);
        s.ui.pair.second = ShaderView::new(ShaderId(0), ShaderView::<32>::solid).sized(120.0, 40.0);
        s.bytes.a = colour(200, 30, 40);
        s.bytes.b = colour(30, 60, 220);
        s
    }

    fn index(wid: &str) -> usize {
        lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, wid).expect(wid)
    }

    fn rect<I: Invalidation>(h: &Host<Shaded, Incremental, I>, wid: &str) -> [f32; 4] {
        let s = h.states[index(wid)];
        [s.pos.x, s.pos.y, s.pos.x + s.size.w, s.pos.y + s.size.h]
    }

    fn observe<I: Invalidation>(h: &mut Host<Shaded, Incremental, I>, agent: &mut Agent) {
        let mut t = TreeMut::of_root(h.app.tree_mut(), &h.styles, &h.states);
        agent.frame_done(&mut t, FrameTiming::passes(1, 1));
    }

    fn changed<I: Invalidation>(h: &Host<Shaded, Incremental, I>, agent: &Agent) -> Vec<String> {
        let tree = TreeRef::of_root(&h.app.ui, &h.styles, &h.states);
        let r: Value = agent.dispatch(&tree, &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastChange", "params": {} }));
        r["result"]["changed"]
            .as_array()
            .expect("changed")
            .iter()
            .map(|c| c["wid"].as_str().map(str::to_owned).unwrap_or_else(|| format!("#{}", c["index"])))
            .collect()
    }

    /// Each bracket in paint order: its rect, its uniform length, and the
    /// first four bytes (the colour the fallback fills with).
    fn brackets(list: &DisplayList) -> Vec<([f32; 4], u32, [u8; 4])> {
        list.cmds
            .iter()
            .filter_map(|c| match c {
                Cmd::Shader { rect, uniforms, .. } => {
                    let u = list.uniforms_of(*uniforms);
                    Some((*rect, uniforms.1, [u[0], u[1], u[2], u[3]]))
                }
                _ => None,
            })
            .collect()
    }

    fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
        a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
    }

    fn settled() -> (H, Agent) {
        let mut h: H = Host::new(app(), 320, 200);
        let mut agent = Agent::new();
        for _ in 0..2 {
            h.pump();
            observe(&mut h, &mut agent);
        }
        (h, agent)
    }

    #[test]
    fn a_uniform_change_damages_only_its_shader() {
        let (mut h, mut agent) = settled();
        let (first, second) = (rect(&h, "first"), rect(&h, "second"));
        assert!(!overlaps(first, second) && first[2] > first[0], "setup: the views overlap or are empty: {first:?} {second:?}");
        assert_eq!(brackets(&h.list).len(), 2, "setup: two brackets in the host's list");

        h.app.bytes.a = colour(20, 160, 60);
        h.pump();
        observe(&mut h, &mut agent);

        let wids = changed(&h, &agent);
        let painted = brackets(&h.list);
        let regions: Vec<[f32; 4]> = match h.damage() {
            Damage::Regions(r) => r.as_slice().to_vec(),
            Damage::All => vec![[0.0, 0.0, 320.0, 200.0]],
            Damage::None => vec![],
        };
        let marks = h.last_marks();
        println!(
            "first {first:?} second {second:?}; marks {marks:?}; damage {regions:?}; \
             brackets {painted:?}; changed {wids:?} (bound: first reported, second not)"
        );
        assert_eq!(
            painted[0].2,
            [20, 160, 60, 255],
            "the render is stale: the host's list still carries the first view's old bytes -- \
             the write did not mark its node, so the frame was skipped"
        );
        assert!(regions.iter().any(|r| overlaps(*r, first)), "setup: the first view's write was not damaged: {regions:?}");
        assert!(!regions.iter().any(|r| overlaps(*r, second)), "the second view was damaged by the first's uniforms: {regions:?}");
        assert!(wids.contains(&"first".to_string()), "setup: the first view's change was not reported: {wids:?}");
        assert!(!wids.contains(&"second".to_string()), "the second view reported a change it did not make: {wids:?}");
    }

    /// **What a `ShaderView` costs, and what it does not** — M83/T83.6. Its
    /// id and bytes live in the widget, so `Style` and `NodeState` keep
    /// the parent commit's sizes in a build with `shader` on (the core's
    /// `tests/cursor.rs` and `tests/focus.rs` hold them in the default
    /// build). The widget's own size is printed for
    /// `docs/results-shader.md`; this build has `a11y` (the agent's), so
    /// it includes the label.
    #[test]
    fn a_shader_view_grows_no_per_node_table() {
        let (view, style, state) = (
            core::mem::size_of::<ShaderView<16>>(),
            core::mem::size_of::<lumen_static::Style>(),
            core::mem::size_of::<lumen_static::NodeState>(),
        );
        println!("size_of: ShaderView<16> {view} B (a11y on); Style {style} B (bound 84); NodeState {state} B (bound 84)");
        assert_eq!(style, 84, "Style changed size in a shader build");
        assert_eq!(state, 84, "NodeState changed size in a shader build");
    }

    /// Two views with no bindings, written directly: the first's uniform
    /// block grows from 16 to 32 bytes, as switching to a shader with a
    /// 32 B table size does, so every bracket painted after it moves in
    /// the list's shared uniform buffer. T83.3's shift, end to end through
    /// the journal: the second view's bytes did not change, so it must not
    /// report.
    #[derive(DeriveApp, Default)]
    struct Plain {
        #[tree]
        ui: Ui,
    }

    #[test]
    fn a_uniform_shift_does_not_report_later_shaders() {
        let mut p = Plain::default();
        p.ui.pair.first = ShaderView::new(ShaderId(0), ShaderView::<32>::solid).sized(120.0, 40.0);
        p.ui.pair.first.set_uniforms(&colour(200, 30, 40)[..16]);
        p.ui.pair.second = ShaderView::new(ShaderId(0), ShaderView::<32>::solid).sized(120.0, 40.0);
        p.ui.pair.second.set_uniforms(&colour(30, 60, 220));
        let mut h: Host<Plain, Incremental, Boundaries<256>> = Host::new(p, 320, 200);
        let mut agent = Agent::new();
        let observe = |h: &mut Host<Plain, Incremental, Boundaries<256>>, agent: &mut Agent| {
            let mut t = TreeMut::of_root(h.app.tree_mut(), &h.styles, &h.states);
            agent.frame_done(&mut t, FrameTiming::passes(1, 1));
        };
        for _ in 0..2 {
            h.pump();
            observe(&mut h, &mut agent);
        }
        let offset = |list: &DisplayList| -> Vec<u32> {
            list.cmds.iter().filter_map(|c| if let Cmd::Shader { uniforms, .. } = c { Some(uniforms.0) } else { None }).collect()
        };
        let before = offset(&h.list);

        assert!(h.app.ui.pair.first.set_shader(ShaderId(1), 32), "setup: the switch changed nothing");
        h.invalidate_all();
        h.pump();
        observe(&mut h, &mut agent);

        let after = offset(&h.list);
        let tree = TreeRef::of_root(&h.app.ui, &h.styles, &h.states);
        let r: Value = agent.dispatch(&tree, &json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.lastChange", "params": {} }));
        let wids: Vec<String> = r["result"]["changed"]
            .as_array()
            .expect("changed")
            .iter()
            .map(|c| c["wid"].as_str().map(str::to_owned).unwrap_or_else(|| format!("#{}", c["index"])))
            .collect();
        println!(
            "uniform offsets before {before:?} after {after:?}; first len {}; changed {wids:?} (bound: second absent)",
            h.app.ui.pair.first.len()
        );
        assert_ne!(before[1], after[1], "setup: the second bracket's offset did not move, so this case cannot fail");
        assert!(wids.contains(&"first".to_string()), "setup: the first view's switch was not reported: {wids:?}");
        assert!(
            !wids.contains(&"second".to_string()),
            "the second view reported a change it did not make -- a bracket's uniform offset entered the hash: {wids:?}"
        );
    }
}
