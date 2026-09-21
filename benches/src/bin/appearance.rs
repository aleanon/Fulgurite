//! M25/T25.1 and T25.12: the frame a widget's look is measured in.
//!
//! **Why this exists rather than `crossover`.** M25's pilot migrates
//! `Toggle`, and `crossover`'s trees hold labels, fixed boxes and
//! spacers — so a toggle's look could be arbitrarily expensive and that
//! frame would not move. `docs/plan-appearance.md` §5 records the review
//! finding: a kill switch measured on a tree that does not contain the
//! widget it judges cannot fire. This tree is toggles on the table, where
//! `Style` is per node, laid out and painted in full every iteration.
//!
//! **Three cases, because a cache is judged by its misses too** (T25.12).
//! Identical inputs every iteration would let a cached look hit every
//! time and never pay for a miss, so the cases differ only in what changes
//! between iterations: `still` (nothing), `hover` (two toggles swap their
//! hovered bit) and `palette` (the palette alternates). The same source
//! builds all three variants T25.12 compares: `main` with no features, the
//! pilot with `--features look`, the cached variant with
//! `--features cached`. With a look compiled in, an untimed pass counts
//! look calls per iteration first, prints them beside the count the case
//! is meant to produce, and — for the cached variant — refuses to time a
//! case whose count is wrong.
//!
//! It also prints the host sizes an arena row is made of, because a
//! `List` or `Repeat` shares one style row and its per-row cost is
//! `NodeState` plus the instance, not `NodeState` plus `Style`.
//!
//! Run: `cargo run --release -p benches --bin appearance [--features look|cached]`
//!
//! **Those two features build only at the commits T25.12 measured** —
//! `48a627c`, the pilot, and `0df160b`, the cached look — because the look
//! they install was reverted when the stop stood. Anywhere else, build it
//! without features: that is the B variant, and `docs/results-appearance.md`
//! T25.12 has all three.

use std::hint::black_box;
use std::mem::size_of;

use benches::bench;
use lumen_static::assemble::Assembled;
use lumen_static::cx::BlockMetrics;
use lumen_static_widgets::ops::{Button, Label};
use lumen_static::style::Palette;
use lumen_static::{Constraints, LayoutCx, NodeState, StaticTree, Style, Widget};
use lumen_static_widgets::{CheckBox, Radio, Switch, Toggle};

const ROOT_C: Constraints = Constraints { min_w: 0.0, max_w: 1920.0, min_h: 0.0, max_h: 100_000.0 };
const ROWS: usize = 100;
const TOGGLES: usize = ROWS * 10;

/// Ten toggles in a row, all three shapes: 11 nodes.
#[derive(StaticTree, Default)]
#[widget(row, spacing = 4.0)]
pub struct Toggles {
    a: CheckBox<()>,
    b: Radio<()>,
    c: Switch<()>,
    d: CheckBox<()>,
    e: Radio<()>,
    f: Switch<()>,
    g: CheckBox<()>,
    h: Radio<()>,
    i: Switch<()>,
    j: CheckBox<()>,
}

/// What changes between one iteration and the next.
#[derive(Clone, Copy, PartialEq)]
enum Case {
    Still,
    Hover,
    Palette,
}

impl Case {
    const ALL: [Case; 3] = [Case::Still, Case::Hover, Case::Palette];

    fn name(self) -> &'static str {
        match self {
            Case::Still => "still",
            Case::Hover => "hover",
            Case::Palette => "palette",
        }
    }

    /// Look calls per iteration a cache must produce: none when nothing
    /// changed, one per toggle whose hovered bit flipped, every toggle when
    /// the palette did.
    #[cfg_attr(not(feature = "look"), allow(dead_code))]
    fn cached_misses(self) -> u64 {
        match self {
            Case::Still => 0,
            Case::Hover => 2,
            Case::Palette => TOGGLES as u64,
        }
    }
}

/// The table indices of the first row's first two toggles: index 0 is the
/// slab's synthetic root, 1 the first row's container, 2 and 3 its `a`
/// and `b`.
const HOVER_A: usize = 2;
const HOVER_B: usize = 3;

/// One iteration: apply the case's change, lay everything out, paint it.
fn frame(slab: &mut Assembled<Toggles>, case: Case, it: u64) -> u64 {
    let odd = it % 2 == 1;
    if case == Case::Hover {
        slab.states[HOVER_A].hovered = odd;
        slab.states[HOVER_B].hovered = !odd;
    }
    let palette = if case == Case::Palette && odd { &Palette::DARK } else { &Palette::LIGHT };
    let mut metrics = BlockMetrics;
    let cx = LayoutCx::new(&mut metrics).with_palette(palette);
    // The cached variant keys on a generation, which the host bumps when
    // the palette changes; the bench stands in for the host.
    #[cfg(feature = "cached")]
    let cx = cx.with_palette_generation(if odd && case == Case::Palette { 2 } else { 1 });
    let mut cx = cx;
    slab.layout_with(&mut cx, ROOT_C);
    let mut out = lumen_static::DisplayList::new();
    slab.paint(&mut out);
    out.len() as u64
}

#[cfg(feature = "look")]
mod count {
    use std::sync::atomic::{AtomicU64, Ordering};

    use lumen_static::style::{Palette, Status};
    use lumen_static_widgets::toggle::{default_look, ToggleLook};

    pub static CALLS: AtomicU64 = AtomicU64::new(0);

    pub fn counting(p: &Palette, s: Status) -> ToggleLook {
        CALLS.fetch_add(1, Ordering::Relaxed);
        default_look(p, s)
    }

    pub fn take() -> u64 {
        CALLS.swap(0, Ordering::Relaxed)
    }
}

/// Give every toggle in the slab the look `f`.
#[cfg(feature = "look")]
fn install(
    slab: &mut Assembled<Toggles>,
    f: fn(&Palette, lumen_static::style::Status) -> lumen_static_widgets::toggle::ToggleLook,
) {
    for t in slab.instances.iter_mut() {
        t.a.set_look(f);
        t.b.set_look(f);
        t.c.set_look(f);
        t.d.set_look(f);
        t.e.set_look(f);
        t.f.set_look(f);
        t.g.set_look(f);
        t.h.set_look(f);
        t.i.set_look(f);
        t.j.set_look(f);
    }
}

/// Look calls per iteration in `case`, after two warm-up iterations so
/// the previous case's state is behind it. Even iteration counts, so both
/// halves of every alternation are counted.
#[cfg(feature = "look")]
fn calls_per_iteration(slab: &mut Assembled<Toggles>, case: Case) -> u64 {
    install(slab, count::counting);
    for it in 0..2 {
        black_box(frame(slab, case, it));
    }
    count::take();
    const N: u64 = 20;
    for it in 2..2 + N {
        black_box(frame(slab, case, it));
    }
    let per = count::take() / N;
    install(slab, lumen_static_widgets::toggle::default_look);
    per
}

fn main() {
    black_box(());
    let mut slab = Assembled::<Toggles>::new(ROWS);
    let variant = if cfg!(feature = "cached") {
        "cached"
    } else if cfg!(feature = "look") {
        "pilot"
    } else {
        "main"
    };
    println!(
        "appearance [{variant}]: full layout+paint per iteration; {} rows x {} nodes = {} table nodes, {} toggles\n",
        ROWS,
        Toggles::NODE_COUNT,
        ROWS * Toggles::NODE_COUNT,
        TOGGLES
    );

    // The setup the timings rest on, proven before any is taken.
    #[cfg(feature = "look")]
    for case in Case::ALL {
        let per = calls_per_iteration(&mut slab, case);
        let expected = if cfg!(feature = "cached") { case.cached_misses() } else { TOGGLES as u64 };
        println!("  look calls per iteration, {:<8} {per:>5}   expected {expected}", case.name());
        assert_eq!(per, expected, "the `{}` case does not produce the misses it is named for", case.name());
    }

    for case in Case::ALL {
        let mut it = 0u64;
        bench(&format!("1k-toggles/{}", case.name()), 20_000, || {
            it += 1;
            frame(&mut slab, case, it)
        });
    }

    println!("\nhost sizes, bytes (an arena row is NodeState + one instance):");
    for (name, s) in [
        ("NodeState", size_of::<NodeState>()),
        ("Style", size_of::<Style>()),
        ("Label<String>", size_of::<Label<String>>()),
        ("Button<(), String>", size_of::<Button<(), String>>()),
        ("Toggle<()>", size_of::<Toggle<()>>()),
    ] {
        println!("  {name:<20} {s:>4}");
    }
}
