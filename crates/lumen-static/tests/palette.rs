//! M25/T25.3(a): the palette reaches `layout`.
//!
//! Nothing in the tree reads a palette yet, so the probe is a leaf
//! written here that records what its `layout` saw — and asserts that
//! `layout` ran at all, because a probe that was never laid out agrees
//! with every palette.

use std::cell::Cell;

use lumen_static::cx::BlockMetrics;
use lumen_static::desc::{NodeDesc, NodeState, NO_PARENT};
use lumen_static::display::{Color, Painter};
use lumen_static::ops::{record_painted, resolve_self};
use lumen_static::style::{Palette, Seed, Style};
use lumen_static::widget::{Node, Widget};
use lumen_static::{leaf_common, pass, Constraints, LayoutCx, Size};

/// A leaf that remembers the text colour of the palette it was laid out
/// against, and how many times it was.
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

const VIEW: Constraints = Constraints { min_w: 0.0, max_w: 100.0, min_h: 0.0, max_h: 100.0 };

fn laid_out(cx: impl FnOnce(&mut BlockMetrics) -> Option<Palette>) -> (Probe, Option<Color>) {
    let mut probe = Probe::default();
    let sty = pass::default_styles(&probe);
    let mut st = [NodeState::default(); 1];
    let mut m = BlockMetrics;
    let chosen = cx(&mut m);
    let mut ctx = LayoutCx::new(&mut m);
    let palette;
    if let Some(p) = chosen {
        palette = p;
        ctx = ctx.with_palette(&palette);
    }
    pass::layout_with(&mut probe, &mut ctx, &sty, &mut st, VIEW);
    let seen = probe.seen.get();
    (probe, seen)
}

#[test]
fn a_context_with_no_palette_resolves_against_light() {
    let (probe, seen) = laid_out(|_| None);
    assert_eq!(probe.layouts.get(), 1, "setup: the probe was laid out");
    assert_eq!(seen, Some(Palette::LIGHT.text));
}

#[test]
fn a_palette_given_to_the_context_is_the_one_layout_sees() {
    assert_ne!(Palette::DARK.text, Palette::LIGHT.text, "setup: the palettes differ where probed");
    let (probe, seen) = laid_out(|_| Some(Palette::DARK));
    assert_eq!(probe.layouts.get(), 1, "setup: the probe was laid out");
    println!("seen {seen:?}, dark {:?}", Palette::DARK.text);
    assert_eq!(seen, Some(Palette::DARK.text), "layout saw a palette other than the one given");
}
