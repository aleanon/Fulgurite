//! M25/T25.3: `Host::set_palette` reaches `layout`, under the strategy
//! that can skip it.
//!
//! **Every gate here runs on `Boundaries<16>`.** Under the default
//! `Everything` every node is laid out every frame, so a `set_palette`
//! that marked nothing would still reach layout on the next pump and none
//! of these could fail — which is why the plan's first draft, which said
//! the setter "damages" the frame, would have passed with the mark
//! deleted (`docs/plan-appearance.md` §5).
//!
//! The probe is a leaf that records the palette its `layout` saw and
//! counts how many times it was laid out, so "the palette reached layout"
//! is asserted as a layout that ran, not inferred from a frame.

use std::cell::Cell;

use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::event::{EventCtx, Handlers, NodeSel};
use lumen_static::invalidate::Boundaries;
use lumen_static::ops::{record_painted, resolve_self, Label};
use lumen_static::style::{Palette, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{leaf_common, App as DeriveApp, Constraints, LayoutCx, Size, StaticTree};
use lumen_static_shell::Host;

#[derive(Default)]
struct Probe {
    seen: Cell<Option<Color>>,
    layouts: Cell<u32>,
}

impl Widget for Probe {
    const NODE_COUNT: usize = 1;
    const DEPTH: usize = 1;
    const IS_RIGID: bool = true;
    const SUB_TABLE: &'static [NodeDesc] = &[NodeDesc::new(0, NO_PARENT, 0, 0, 0)];
    const KIND: &'static str = "palette-probe";
    const SEEDS: &'static [Seed] = &[Seed::new(Self::KIND, Self::type_defaults)];
}

impl Node for Probe {
    fn layout(
        &mut self,
        cx: &mut LayoutCx,
        limits: Constraints,
        style: &[Style],
        st: &mut [NodeState],
        i: usize,
    ) -> Size {
        self.seen.set(Some(cx.palette.text));
        self.layouts.set(self.layouts.get() + 1);
        let size = resolve_self(limits, &style[i], Size { w: 10.0, h: 10.0 });
        record_painted(st, i, limits, size, &style[i])
    }

    fn paint(&self, _st: &[NodeState], _i: usize, _out: &mut dyn Painter) {}

    leaf_common!(Probe, 0, nohit);
}

impl<S> Handlers<S> for Probe {
    fn handler(&self, _sel: NodeSel) -> Option<fn(&mut S, &EventCtx)> {
        None
    }
}

/// The probe inside a container, so a skipped layout is the derive's
/// skip — the one `Boundaries` performs.
#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Panel {
    probe: Probe,
    note: Label,
}

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    panel: Panel,
}

#[derive(DeriveApp, Default)]
struct Themed {
    #[tree]
    ui: Ui,
}

type Bounded = Host<Themed, lumen_static::Incremental, Boundaries<16>>;

/// A host after two pumps, so the second was a still frame and the set is
/// empty — the state in which a frame can skip.
fn settled() -> Bounded {
    let mut h: Bounded = Host::new(Themed::default(), 200, 100);
    h.pump();
    h.pump();
    assert!(h.changed().can_skip(), "setup: a still frame leaves nothing marked");
    h
}

fn layouts(h: &Bounded) -> u32 {
    h.app.ui.panel.probe.layouts.get()
}

#[test]
fn a_still_frame_does_not_lay_the_probe_out() {
    // The premise every other test rests on: without a mark, the probe is
    // skipped. If this failed the other tests would pass for the wrong
    // reason — layout running every frame.
    let mut h = settled();
    let before = layouts(&h);
    h.pump();
    println!("layouts: {before} -> {}", layouts(&h));
    assert_eq!(layouts(&h), before, "a still frame laid the probe out: the skip never fires");
}

#[test]
fn a_new_palette_reaches_layout_on_the_next_frame() {
    let mut h = settled();
    assert_eq!(*h.palette(), Palette::LIGHT);
    assert_ne!(Palette::DARK.text, Palette::LIGHT.text, "setup: the palettes differ where probed");
    let before = layouts(&h);

    h.set_palette(Palette::DARK);
    h.pump();

    let seen = h.app.ui.panel.probe.seen.get();
    println!("layouts: {before} -> {}; seen {seen:?}, dark {:?}", layouts(&h), Palette::DARK.text);
    assert!(layouts(&h) > before, "set_palette did not cause a layout: the probe kept last frame");
    assert_eq!(
        seen,
        Some(Palette::DARK.text),
        "layout ran against a palette other than the host's"
    );
}

#[test]
fn an_equal_palette_marks_nothing() {
    let mut h = settled();
    let before = layouts(&h);
    h.set_palette(Palette::LIGHT);
    assert!(h.changed().can_skip(), "setting the palette the host already has marked the tree");
    h.pump();
    println!("layouts: {before} -> {}", layouts(&h));
    assert_eq!(layouts(&h), before, "an unchanged palette still laid the probe out");
}
