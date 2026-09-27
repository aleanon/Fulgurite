//! **The shadow oracle** — M34/T34.1 (`docs/plan-boundaries-default.md`),
//! redesigned by M41/T41.2 (`docs/plan-boundaries-repair.md` §2.2).
//!
//! After a bounded host paints, it lays out, places and paints again
//! under an all-dirty view into scratch state and a scratch list, and
//! compares `NodeState` and then the lists. **This file is the standing
//! test that the oracle can fire** (the re-triage's I-NEW-1): until
//! T41.2 its one test asserted `compared > 0` and `first.is_none()`, so
//! replacing the scratch layout with a clone of the states — an oracle
//! comparing the frame with itself — left it green.
//!
//! Four cases, each printing `divergence at frame F, node N ("wid"),
//! field X`:
//!
//! - a correct session is compared and quiet (the accept direction). A
//!   consumed press marks everything, so it also moves a **hover**,
//!   which only T25.0's precise mark covers; skipping that mark is
//!   T34.1's own injection, and it turns this case red.
//! - a write to state a widget keeps for itself, with no mark, is named
//!   `paint` — the shape of `TextInput::scroll` and a `Repeat`'s rows,
//!   which `NodeState` cannot show and which the first oracle repaired
//!   before it judged.
//! - a direct write to a label, with no mark, is named.
//! - a tick that writes on a zero `dt`, against `App::tick`'s contract,
//!   is named on that frame.
//!
//! Since M53/T53.3 a pump with nothing marked is skipped — no layout,
//! place or paint — and an unmarked write is exactly what such a pump
//! does not see. The oracle judges skipped frames too, and the last case
//! here asserts it fires on one: **if it could not, the fast path would
//! not ship** (`docs/plan-frame-cost.md` §5).

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
    // Two more with nothing happening: the idle fast path skips them
    // (M53/T53.3), and the oracle judges skipped frames too.
    h.pump();
    h.pump();
    let r = h.shadow_report();
    println!(
        "compared {} frames ({} skipped by the idle fast path), first divergence {:?}, presses {}",
        r.compared, r.skipped, r.first, h.app.tally.n
    );
    assert!(r.compared > 0, "the shadow oracle never ran — 'no divergence' would prove nothing");
    assert!(r.skipped > 0, "setup: no frame was skipped, so the fast path went unjudged");
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
    label: Label<String>,
    inner: Inner,
}

#[derive(DeriveApp, Default)]
#[app(tick = zero_dt_write)]
struct OffsetApp {
    #[tree]
    ui: OffsetUi,
    /// Set by a test: the next tick writes the label, on any `dt`.
    write_on_tick: bool,
}

/// Against `App::tick`'s contract: it writes the tree on a zero `dt`,
/// which `Host::tick` does not mark.
fn zero_dt_write(a: &mut OffsetApp, _dt: f32) -> bool {
    if core::mem::take(&mut a.write_on_tick) {
        a.ui.label.set("written by a zero-dt tick".into());
    }
    false
}

fn offset_host() -> Host<OffsetApp, Incremental, Boundaries<16>> {
    let mut a = OffsetApp::default();
    a.ui.label.set("label".into());
    let mut h: Host<OffsetApp, Incremental, Boundaries<16>> = Host::new(a, 200, 80);
    h.pump();
    h.pump();
    let r = h.shadow_report();
    assert!(r.compared > 0 && r.first.is_none(), "setup: a quiet start, got {r:?}");
    h
}

/// Print the divergence in the form every case shares, and return it.
fn named(h: &Host<OffsetApp, Incremental, Boundaries<16>>, what: &str) -> lumen_static_shell::Divergence {
    let r = h.shadow_report();
    assert!(r.compared > 0, "the shadow oracle never ran");
    let d = r.first.unwrap_or_else(|| panic!("{what} went unnamed (compared {} frames)", r.compared));
    let wid = <OffsetUi as Widget>::SUB_TABLE
        .get(d.node as usize)
        .map(|n| n.wid)
        .unwrap_or("?");
    println!("divergence at frame {}, node {} ({wid:?}), field {}", d.frame, d.node, d.field);
    d
}

#[test]
fn the_oracle_names_a_widget_internal_divergence() {
    let mut h = offset_host();
    let before = h.list.hash_from(0);
    // The canonical unreported write: nothing marks it.
    h.app.ui.inner.offset.input.set(20.0);
    h.pump();
    let leaf = lumen_static::style::index_of_wid(<OffsetUi as Widget>::SUB_TABLE, "offset").unwrap();
    // The bounded frame must be the stale one: the oracle judges it and
    // may not repair it. Before T41.2 the list moved here, because the
    // oracle's layout ran before paint.
    println!("leaf is node {leaf}, bounded list moved: {}", h.list.hash_from(0) != before);
    assert_eq!(h.list.hash_from(0), before, "the oracle repaired the frame it judged");
    let d = named(&h, "an unreported write to widget-internal state");
    assert_eq!((d.node, d.field), (leaf as u32, "paint"), "the oracle named the wrong node or field: {d:?}");
}

#[test]
fn a_wrong_mark_is_named() {
    let mut h = offset_host();
    let label = lumen_static::style::index_of_wid(<OffsetUi as Widget>::SUB_TABLE, "label").unwrap();
    // A direct write with no mark — the canonical unreported write.
    h.app.ui.label.set("a much longer label than before".into());
    h.pump();
    let d = named(&h, "a direct write to a label");
    assert_eq!(d.node, label as u32, "the oracle named the wrong node: {d:?}");
    assert!(matches!(d.field, "size" | "paint"), "unexpected field: {d:?}");
}

#[test]
fn a_zero_dt_tick_write_is_named() {
    let mut h = offset_host();
    let label = lumen_static::style::index_of_wid(<OffsetUi as Widget>::SUB_TABLE, "label").unwrap();
    h.app.write_on_tick = true;
    h.tick(0.0);
    assert_eq!(h.app.ui.label.as_str(), "written by a zero-dt tick", "setup: the tick did not run");
    let frame = h.shadow_report().compared;
    h.pump();
    let d = named(&h, "a tick's write on a zero dt");
    assert_eq!(d.node, label as u32, "the oracle named the wrong node: {d:?}");
    assert_eq!(d.frame, frame, "named on a later frame than the write's: {d:?}");
}

/// **The fast path's own gate** — M53/T53.3. A write with no mark leaves
/// every mark site quiet, so the idle fast path skips the frame: nothing
/// is laid out or painted, and the screen keeps last frame's list. The
/// oracle must still judge that frame, and name the write on it.
#[test]
fn the_oracle_fires_on_a_frame_the_fast_path_skipped() {
    let mut h = offset_host();
    let label = lumen_static::style::index_of_wid(<OffsetUi as Widget>::SUB_TABLE, "label").unwrap();
    let before = h.pass_counts();
    let frame = h.shadow_report().compared;
    h.app.ui.label.set("written with no mark".into());
    h.pump();
    let d_passes = h.pass_counts().since(before);
    println!(
        "the write's frame: skipped {}, place {}, paint {} (the fast path took it); oracle skipped-frames {}",
        d_passes.skipped,
        d_passes.places,
        d_passes.paints,
        h.shadow_report().skipped
    );
    assert_eq!((d_passes.skipped, d_passes.paints), (1, 0), "setup: the fast path did not skip the frame");
    let d = named(&h, "an unmarked write on a skipped frame");
    assert_eq!(d.node, label as u32, "the oracle named the wrong node: {d:?}");
    assert_eq!(d.frame, frame, "named on a later frame than the write's: {d:?}");
}

// ---------------------------------------------------------------------
// M54/T54.4 (`docs/plan-a11y-platform.md` §2.4): an assistive
// technology's action is a new kind of event, so the oracle must see a
// frame one drove. A consumed action marks the node that consumed it,
// like a consumed key. A `Collapsible` is the widget to drive: `Expand`
// changes its **size**, which a bounded layout skips if nothing marked it
// (a slider's thumb moves only in `paint`, which is not bounded, so a
// slider could not show a missing mark at all -- tried first).
// ---------------------------------------------------------------------

#[derive(StaticTree, Default)]
#[widget(column, spacing = 4.0)]
struct ActionUi {
    #[widget_id("more")]
    more: Collapsible<Label<String>, String>,
    #[widget_id("after")]
    after: Label<String>,
}

#[derive(Default)]
struct ActionState;

/// Split (`#[state]`), so a handled frame marks only the nodes that
/// consumed it (M29/T29.6): with the whole tree marked, an action that
/// forgot to report its node would be repaired before anyone could see.
#[derive(DeriveApp, Default)]
struct ActionApp {
    #[tree]
    ui: ActionUi,
    #[state]
    st: ActionState,
}

#[test]
fn an_action_frame_is_compared_and_quiet() {
    let mut a = ActionApp::default();
    a.ui.more = Collapsible::new("More".into(), Label::new("hidden body".into()));
    a.ui.after.set("below".into());
    let mut h: Host<ActionApp, Incremental, Boundaries<16>> = Host::new(a, 300, 200);
    h.pump();
    h.pump();
    let i = lumen_static::style::index_of_wid(<ActionUi as Widget>::SUB_TABLE, "more").unwrap();
    let before = h.states[i].size.h;
    h.action(i as u32, lumen_static::Action::Expand);
    h.pump();
    h.pump();
    let r = h.shadow_report();
    println!(
        "action frame: section {before} -> {} px tall; compared {} frames, first divergence {:?}",
        h.states[i].size.h,
        r.compared,
        r.first
    );
    assert!(r.compared > 0, "the shadow oracle never ran");
    assert!(r.first.is_none(), "an action frame diverged: {:?}", r.first);
    assert!(h.states[i].size.h > before, "setup: Expand did not open the section");
}
