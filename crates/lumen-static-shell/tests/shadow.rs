//! **The shadow oracle** — M34/T34.1 (`docs/plan-boundaries-default.md`).
//!
//! A bounded host re-lays out every frame under an all-dirty view into
//! scratch state and compares. This checks the checker: it must count
//! the frames it compared (a zero means it never ran, and "no divergence"
//! then proves nothing), stay quiet on a correct session, and name the
//! node when a mark is wrong — on a frame that *can* show it. A consumed
//! press marks everything, so the injection targets a **hover**, which
//! only T25.0's precise mark covers.

use lumen_static::invalidate::Boundaries;
use lumen_static::prelude::*;
use lumen_static::retain::Incremental;
use lumen_static::style::{Patch, Rule, Selector};
use lumen_static::Event;
use lumen_static_shell::Host;
use lumen_static_widgets::prelude::*;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct Ui {
    #[widget_id("title")]
    title: Label<String>,
    #[widget_id("go")]
    go: Button<Tally>,
}

#[derive(Default)]
struct Tally {
    n: u32,
}

#[derive(DeriveApp, Default)]
struct App {
    #[tree]
    ui: Ui,
    #[state]
    #[bind(ui.title, n, "{} presses")]
    tally: Tally,
}

fn host() -> Host<App, Incremental, Boundaries<16>> {
    let mut a = App::default();
    a.ui.go.label = "go";
    a.ui.go.on_click = |t: &mut Tally, _: &lumen_static::EventCtx| t.n += 1;
    let mut h: Host<App, Incremental, Boundaries<16>> = Host::new(a, 300, 120);
    // A hover shade that differs from rest, or a stale hover is invisible.
    h.set_stylesheet(&[Rule::new(Selector::Kind("button"), Patch::new().hover_dim(0.3))]);
    h.pump();
    h.pump();
    h
}

#[test]
fn a_correct_session_is_compared_and_quiet() {
    let mut h = host();
    let i = lumen_static::style::index_of_wid(<Ui as Widget>::SUB_TABLE, "go").unwrap();
    let s = h.states[i];
    let (x, y) = (s.pos.x + s.size.w / 2.0, s.pos.y + s.size.h / 2.0);
    h.post(Event::PointerMove { x, y });
    h.pump();
    assert!(h.states[i].hovered, "setup: the move did not hover the button");
    h.click(x, y);
    h.pump();
    h.post(Event::PointerMove { x: 290.0, y: 110.0 });
    h.pump();
    let r = h.shadow_report();
    println!("compared {} frames, first divergence {:?}, presses {}", r.compared, r.first, h.app.tally.n);
    assert!(r.compared > 0, "the shadow oracle never ran — 'no divergence' would prove nothing");
    assert_eq!(h.app.tally.n, 1, "setup: the click did not land");
    assert!(r.first.is_none(), "a correct session diverged: {:?}", r.first);
}

// ---------------------------------------------------------------------
// M41/T41.0 (`docs/plan-boundaries-repair.md` §1.3, R1-F6): the oracle
// must be able to name a divergence that lives in a widget's own state.
// ---------------------------------------------------------------------

/// A leaf whose `layout` copies `input` into an offset that only `paint`
/// reads — the shape of `TextInput::scroll` and a `Repeat`'s bound rows.
/// Its box never changes, so its `NodeState` cannot show the difference.
#[derive(Default)]
struct Offset {
    /// Written by the test between pumps, with no mark.
    input: std::cell::Cell<f32>,
    /// What the last layout saw; paint draws from this.
    off: f32,
}

impl lumen_static::Widget for Offset {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [lumen_static::NodeDesc] =
        &[lumen_static::NodeDesc::new(0, lumen_static::NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "offset-probe";
    const SEEDS: &'static [lumen_static::style::Seed] =
        &[lumen_static::style::Seed::unlinted(Self::KIND, Self::type_defaults)];
}

impl lumen_static::widget::Node for Offset {
    fn layout(
        &mut self,
        _cx: &mut lumen_static::LayoutCx,
        limits: lumen_static::Constraints,
        style: &[lumen_static::Style],
        st: &mut [lumen_static::NodeState],
        i: usize,
    ) -> lumen_static::Size {
        self.off = self.input.get();
        let size = lumen_static::ops::resolve_self(limits, &style[i], lumen_static::Size { w: 40.0, h: 10.0 });
        lumen_static::ops::record_painted(st, i, limits, size, &style[i])
    }

    fn paint(&self, st: &[lumen_static::NodeState], i: usize, out: &mut dyn lumen_static::display::Painter) {
        let p = st[i].pos;
        out.rect([p.x + self.off, p.y, p.x + self.off + 4.0, p.y + 10.0], lumen_static::Color(0, 0, 0, 255));
    }

    lumen_static::leaf_common!(Offset, 0, nohit);
}

impl<S> lumen_static::event::Handlers<S> for Offset {
    fn handler(&self, _sel: lumen_static::event::NodeSel) -> Option<fn(&mut S, &lumen_static::EventCtx)> {
        None
    }
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Inner {
    #[widget_id("offset")]
    offset: Offset,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct OffsetUi {
    #[widget_id("label")]
    label: Label,
    inner: Inner,
}

#[derive(DeriveApp, Default)]
struct OffsetApp {
    #[tree]
    ui: OffsetUi,
}

#[test]
#[ignore = "red until T41.2: R1-F6, the oracle compares NodeState only and repairs the frame it judges"]
fn the_oracle_names_a_widget_internal_divergence() {
    let mut h: Host<OffsetApp, Incremental, Boundaries<16>> = Host::new(OffsetApp::default(), 200, 80);
    h.pump();
    h.pump();
    let before = h.list.hash_from(0);
    // The canonical unreported write: nothing marks it.
    h.app.ui.inner.offset.input.set(20.0);
    h.pump();
    let r = h.shadow_report();
    let leaf = lumen_static::style::index_of_wid(<OffsetUi as Widget>::SUB_TABLE, "offset").unwrap();
    println!(
        "compared {} frames, first divergence {:?}, leaf is node {leaf}, list moved: {}",
        r.compared,
        r.first,
        h.list.hash_from(0) != before
    );
    assert!(r.compared > 0, "the shadow oracle never ran");
    let d = r.first.expect("an unreported write to widget-internal state went unnamed");
    assert_eq!(d.node, leaf as u32, "the oracle named the wrong node: {d:?}");
}
